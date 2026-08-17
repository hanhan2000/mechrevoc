//! 触摸板开关功能:通过 hidraw feature report 控制开关与 LED,
//! 并监听固件注入的「Super+Ctrl+Paste」组合键实现角落双击切换。
//!
//! 原理:此触摸板固件在检测到「左上角双击」时会通过键盘控制器注入
//! Super+Ctrl+Paste 组合键(Windows 驱动正是注册这个全局热键来实现
//! 开关)。固件注入与触摸板开关状态无关,因此任何状态下都能触发,
//! 形成完整的「角落双击开/关」闭环。

use std::fs;

use crate::discovery;
use crate::evdev::{EvdevReader, InputEvents, READ_BUFFER_BYTES};
use crate::ffi::{self, PollFd, POLLIN};
use crate::hidraw::{describe_state, state_is_on, TouchpadSwitcher, STATE_OFF, STATE_ON};

/// 固件注入的键盘组合:Super+Ctrl+Paste
const KEY_LEFTMETA: u16 = 0x7d; // 125
const KEY_LEFTCTRL: u16 = 0x1d; // 29
const KEY_PASTE: u16 = 0xc2; // 194

/// 组合键状态机:仅维护 Super/Ctrl 的按下状态,Paste 按下时判定触发
#[derive(Default)]
pub struct ComboDetector {
    super_down: bool,
    ctrl_down: bool,
}

impl ComboDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// 处理一个按键事件;命中「Super+Ctrl+Paste」时返回 true
    pub fn feed(&mut self, event: &ffi::InputEvent) -> bool {
        match event.code {
            KEY_LEFTMETA => {
                self.super_down = event.value != 0;
                false
            }
            KEY_LEFTCTRL => {
                self.ctrl_down = event.value != 0;
                false
            }
            KEY_PASTE => self.super_down && self.ctrl_down && event.value == 1,
            _ => false,
        }
    }
}

/// 触摸板开关监视器:封装硬件开关与组合键监听,供守护进程或 TUI 集成。
///
/// 设计为非阻塞:调用方在自身事件循环中周期性调用 [`TouchpadMonitor::poll`],
/// 即可在等待自身事件的同时处理固件注入的组合键。
pub struct TouchpadMonitor {
    switcher: TouchpadSwitcher,
    readers: Vec<EvdevReader>,
    detector: ComboDetector,
    enabled: bool,
}

impl TouchpadMonitor {
    /// 打开触摸板 hidraw 并监听所有键盘类设备
    pub fn open() -> Result<Self, String> {
        let hid = discovery::find_hidraw().ok_or("touchpad hidraw not found")?;
        let switcher = TouchpadSwitcher::open(&hid)?;
        let enabled = state_is_on(switcher.state()?);
        let readers = open_all_evdev()?;
        Ok(TouchpadMonitor {
            switcher,
            readers,
            detector: ComboDetector::new(),
            enabled,
        })
    }

    /// 触摸板是否开启
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 当前状态的可读描述(如 "ON, LED off, clicks on")
    pub fn state_description(&self) -> &'static str {
        describe_state(self.switcher_state_value())
    }

    fn switcher_state_value(&self) -> u8 {
        if self.enabled {
            STATE_ON
        } else {
            STATE_OFF
        }
    }

    /// 非阻塞处理一批键盘事件;返回是否有组合键触发了切换
    pub fn poll(&mut self) -> Result<bool, String> {
        let mut pfds: Vec<PollFd> = self
            .readers
            .iter()
            .map(|r| PollFd {
                fd: r.fd(),
                events: POLLIN,
                revents: 0,
            })
            .collect();
        // timeout=0:仅查询,不阻塞
        unsafe {
            ffi::poll(pfds.as_mut_ptr(), pfds.len() as u64, 0);
        }
        let mut toggled = false;
        let mut buf = vec![0u8; READ_BUFFER_BYTES];
        for (idx, pfd) in pfds.iter().enumerate() {
            if pfd.revents & POLLIN == 0 {
                continue;
            }
            let data = self.readers[idx].read_available(&mut buf)?;
            for event in InputEvents::new(data) {
                if event.type_ == ffi::EV_KEY && self.detector.feed(&event) {
                    self.toggle()?;
                    toggled = true;
                }
            }
        }
        Ok(toggled)
    }

    /// 手动翻转开关状态
    pub fn toggle(&mut self) -> Result<(), String> {
        self.enabled = !self.enabled;
        self.switcher.set_state(self.switcher_state_value())
    }

    /// 常驻监听循环(守护进程模式,阻塞运行)
    pub fn run_forever(mut self) -> Result<(), String> {
        println!(
            "touchpad monitor started, current state: {}",
            self.state_description()
        );
        loop {
            if self.poll()? {
                println!(">> Super+Ctrl+Paste -> {}", self.state_description());
            }
            // 短暂休眠以降低 CPU 占用(组合键事件通常间隔远大于此)
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

/// 打开系统上所有「支持按键」的 evdev 设备。
///
/// 组合键由固件注入,来源设备因机型而异,因此监听所有键盘类设备;
/// 但排除 HDA 音频、PC Speaker 等不可能产生按键的设备,避免浪费。
fn open_all_evdev() -> Result<Vec<EvdevReader>, String> {
    let devices = discovery::all_evdev_devices()?;
    let mut readers = Vec::new();
    for (path, _name) in &devices {
        if !supports_keys(path) {
            continue;
        }
        if let Ok(reader) = EvdevReader::open(path) {
            readers.push(reader);
        }
    }
    if readers.is_empty() {
        return Err("no keyboard evdev devices could be opened (need root or input group)".into());
    }
    Ok(readers)
}

/// 通过 sysfs 的按键能力位图判断设备是否会产生按键事件
fn supports_keys(dev_path: &str) -> bool {
    let Some(node) = dev_path.rsplit('/').next() else {
        return false;
    };
    let key_caps = format!("/sys/class/input/{node}/device/capabilities/key");
    let Ok(content) = fs::read_to_string(key_caps) else {
        return false;
    };
    // sysfs 位图以空白分隔的十六进制词表示(每词 32 位),
    // 任一非零即表示该设备支持按键
    content
        .split_whitespace()
        .any(|word| u64::from_str_radix(word, 16).unwrap_or(0) != 0)
}
