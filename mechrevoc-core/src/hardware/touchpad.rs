//! 触摸板 hidraw 后端实现:通过 hidraw feature report 控制开关与 LED,
//! 并监听固件注入的「Super+Ctrl+Paste」组合键实现角落双击切换。
//!
//! 原理:此触摸板固件在检测到「左上角双击」时会通过键盘控制器注入
//! Super+Ctrl+Paste 组合键(Windows 驱动正是注册这个全局热键来实现
//! 开关)。固件注入与触摸板开关状态无关,因此任何状态下都能触发,
//! 形成完整的「角落双击开/关」闭环。
//!
//! 本模块是 [`TouchpadBackend`] 的具体实现;调用方只依赖 trait,
//! 无硬件环境下可用 mock 后端替换(见 `backend::tests`)。

use std::collections::BTreeSet;

use crate::backend::TouchpadBackend;
use crate::hardware::discovery;
use crate::hardware::evdev::{EvdevReader, InputEvents, READ_BUFFER_BYTES};
use crate::hardware::ffi::{self, PollFd, POLLIN};
use crate::hardware::hidraw::{describe_state, state_is_on, TouchpadSwitcher, STATE_OFF, STATE_ON};

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

/// hidraw 触摸板后端:封装硬件开关与组合键监听,实现 [`TouchpadBackend`]。
///
/// 设计为非阻塞:调用方在自身事件循环中周期性调用 [`TouchpadBackend::poll`],
/// 即可在等待自身事件的同时处理固件注入的组合键。
///
/// 交互模型:每次检测到完整组合键即切换,不引入去抖等防护——
/// 系统状态以硬件(最后一次交互)为准,daemon 重启后重新读取硬件状态。
pub struct HidrawTouchpad {
    switcher: TouchpadSwitcher,
    readers: Vec<EvdevReader>,
    /// 系统中存在但打不开的键盘设备(权限/被独占),不参与健康判定
    skipped: BTreeSet<String>,
    detector: ComboDetector,
    enabled: bool,
}

impl TouchpadBackend for HidrawTouchpad {
    /// 打开触摸板 hidraw 并监听所有键盘类设备
    fn open() -> Result<Self, String> {
        let hid = discovery::find_hidraw().ok_or("touchpad hidraw not found")?;
        let switcher = TouchpadSwitcher::open(&hid)?;
        let enabled = state_is_on(switcher.state()?);
        let (readers, skipped) = open_all_evdev()?;
        Ok(HidrawTouchpad {
            switcher,
            readers,
            skipped,
            detector: ComboDetector::new(),
            enabled,
        })
    }

    /// 触摸板是否开启
    fn enabled(&self) -> bool {
        self.enabled
    }

    /// 当前状态的可读描述(如 "ON, LED off, clicks on")
    fn state_description(&self) -> &'static str {
        describe_state(self.switcher_state_value())
    }

    /// 非阻塞处理一批键盘事件;返回是否有组合键触发了切换
    fn poll(&mut self) -> Result<bool, String> {
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
            // 失效句柄不读取,循环结束后统一清理
            if pfd.revents & ffi::POLL_DEVICE_GONE != 0 {
                continue;
            }
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
        // 丢弃本轮判定为失效的句柄(设备移除或重枚举后旧节点消失)。
        // 注意:触摸板关闭后其 input 节点消失是预期行为,不视为故障;
        // 是否需要补建新设备由 is_healthy 与 reattach 决定。
        self.readers.retain(|reader| {
            !pfds
                .iter()
                .any(|pfd| pfd.fd == reader.fd() && pfd.revents & ffi::POLL_DEVICE_GONE != 0)
        });
        Ok(toggled)
    }

    /// 手动翻转开关状态。
    ///
    /// 以硬件实际状态为准:先读取当前状态再写入相反值,不依赖内存缓存——
    /// 即使缓存与硬件不同步(异常/外部修改),也能按"最后一次交互"正确翻转。
    fn toggle(&mut self) -> Result<(), String> {
        let enabled = !state_is_on(self.switcher.state()?);
        self.switcher
            .set_state(if enabled { STATE_ON } else { STATE_OFF })?;
        self.enabled = enabled;
        Ok(())
    }

    fn refresh_state(&mut self) -> Result<bool, String> {
        let actual = state_is_on(self.switcher.state()?);
        let changed = actual != self.enabled;
        self.enabled = actual;
        Ok(changed)
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
        // 以硬件实际状态判断是否需要写入,避免无谓的 feature report
        if state_is_on(self.switcher.state()?) == enabled {
            self.enabled = enabled;
            return Ok(());
        }
        self.switcher
            .set_state(if enabled { STATE_ON } else { STATE_OFF })?;
        self.enabled = enabled;
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        // hidraw 失效时 ioctl 直接报错(挂起恢复后节点可能重编号)
        if self.switcher.state().is_err() {
            return false;
        }
        let Ok(devices) = discovery::key_devices() else {
            return false;
        };
        // 期望集合 = 系统当前的键盘设备,排除已知打不开的
        let want: BTreeSet<String> = devices
            .into_iter()
            .filter(|path| !self.skipped.contains(path))
            .collect();
        let have: BTreeSet<String> = self
            .readers
            .iter()
            .map(|reader| reader.path().to_string())
            .collect();
        have == want
    }

    fn reattach(&mut self) -> Result<(), String> {
        // 挂起恢复后设备可能重编号,必须整组重建而非复用旧节点路径
        let hid = discovery::find_hidraw().ok_or("touchpad hidraw not found on reattach")?;
        let switcher = TouchpadSwitcher::open(&hid)?;
        let enabled = state_is_on(switcher.state()?);
        let (readers, skipped) = open_all_evdev()?;
        // 旧句柄由 Drop 关闭;detector 重置以丢弃挂起前残留的半按状态
        self.switcher = switcher;
        self.readers = readers;
        self.skipped = skipped;
        self.detector = ComboDetector::new();
        self.enabled = enabled;
        Ok(())
    }
}

impl HidrawTouchpad {
    fn switcher_state_value(&self) -> u8 {
        if self.enabled {
            STATE_ON
        } else {
            STATE_OFF
        }
    }
}

/// 打开系统上所有「支持按键」的 evdev 设备。
///
/// 返回 (已打开的 reader, 打不开的设备路径)。后者通常是权限不足或被独占,
/// 记录后不参与健康判定,避免被误判为「漏开的设备」而反复重建。
fn open_all_evdev() -> Result<(Vec<EvdevReader>, BTreeSet<String>), String> {
    let mut readers = Vec::new();
    let mut skipped = BTreeSet::new();
    for path in discovery::key_devices()? {
        match EvdevReader::open(&path) {
            Ok(reader) => readers.push(reader),
            Err(_) => {
                skipped.insert(path);
            }
        }
    }
    if readers.is_empty() {
        return Err("no keyboard evdev devices could be opened (need root or input group)".into());
    }
    Ok((readers, skipped))
}
