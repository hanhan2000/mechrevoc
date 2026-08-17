//! 触摸板开关的 hidraw 通道。
//!
//! 通过 HID feature report 0x06("surface button switch")控制触摸板开关与
//! 左上角 LED。协议来源:微软 Windows Precision Touchpad 官方规范
//! (Touchpad configuration collection),TUXEDO 官方工具
//! tuxedo-touchpad-switch 亦使用同一协议。

use std::ffi::CString;
use std::fs;
use std::os::unix::io::RawFd;

use crate::hardware::ffi::{self, c_int, PollFd, O_NONBLOCK, O_RDONLY, O_RDWR, POLLIN};

/// 关闭状态:LED 亮 + 触摸板关 + 点击关
pub const STATE_OFF: u8 = 0x00;
/// 开启状态:LED 灭 + 触摸板开 + 点击开
pub const STATE_ON: u8 = 0x03;

/// feature report 的数据长度(报告 ID + 1 字节状态值)
const REPORT_SIZE: usize = 2;

/// 触摸板开关句柄:持有已打开的 hidraw 描述符与定位好的报告 ID
pub struct TouchpadSwitcher {
    fd: RawFd,
    pub report_id: u8,
}

impl TouchpadSwitcher {
    /// 打开 hidraw 并自动定位开关报告 ID
    pub fn open(dev: &str) -> Result<Self, String> {
        let fd = open_hidraw(dev)?;
        let report_id = find_switch_report_id(dev)?;
        Ok(TouchpadSwitcher { fd, report_id })
    }

    /// 读取当前状态值
    pub fn state(&self) -> Result<u8, String> {
        let mut buf = [self.report_id, 0x00];
        let rc = unsafe {
            ffi::ioctl(
                self.fd,
                ffi::hid_iocgfeature(REPORT_SIZE as u32),
                buf.as_mut_ptr(),
            )
        };
        if rc < 0 {
            return Err(ioctl_error("HIDIOCGFEATURE"));
        }
        Ok(buf[1])
    }

    /// 写入状态值(通常传 [`STATE_OFF`] 或 [`STATE_ON`])
    pub fn set_state(&self, value: u8) -> Result<(), String> {
        let buf = [self.report_id, value];
        let rc = unsafe {
            ffi::ioctl(
                self.fd,
                ffi::hid_iocsfeature(REPORT_SIZE as u32),
                buf.as_ptr(),
            )
        };
        if rc < 0 {
            return Err(ioctl_error("HIDIOCSFEATURE"));
        }
        Ok(())
    }
}

impl Drop for TouchpadSwitcher {
    fn drop(&mut self) {
        unsafe { ffi::close(self.fd) };
    }
}

/// 状态值中的"开启"位:bit1 为 1 表示触摸板开启
pub fn state_is_on(value: u8) -> bool {
    value & 0x02 != 0
}

/// 将状态值描述为可读文本
pub fn describe_state(value: u8) -> &'static str {
    match value & 0x03 {
        0x00 => "OFF, LED on, clicks off",
        0x01 => "OFF, LED on, clicks on",
        0x02 => "ON, LED off, clicks off",
        _ => "ON, LED off, clicks on",
    }
}

/// 以只读方式打开 hidraw(用于监听输入报告)
pub fn open_readonly(dev: &str) -> Result<RawFd, String> {
    let cpath = CString::new(dev).map_err(|_| "path contains NUL".to_string())?;
    let fd = unsafe { ffi::open(cpath.as_ptr(), O_RDONLY | O_NONBLOCK) };
    if fd < 0 {
        return Err(format!("open({dev}): {}", std::io::Error::last_os_error()));
    }
    Ok(fd)
}

/// 等待并读取一批 hidraw 输入报告字节;超时返回空切片
pub fn poll_read(fd: RawFd, buf: &mut [u8], timeout_ms: c_int) -> Result<&[u8], String> {
    let mut pfd = PollFd {
        fd,
        events: POLLIN,
        revents: 0,
    };
    unsafe {
        ffi::poll(&mut pfd, 1, timeout_ms);
    }
    if pfd.revents & POLLIN == 0 {
        return Ok(&buf[..0]);
    }
    let n = unsafe { ffi::read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
    if n <= 0 {
        return Ok(&buf[..0]);
    }
    Ok(&buf[..n as usize])
}

/// 从 sysfs 的报告描述符中定位 "surface button switch" 报告 ID。
///
/// 选择 sysfs 而非 HIDIOCGRDESC 的原因:该文件世界可读,无需 root,
/// 且不依赖 hidraw ioctl。实现与 TUXEDO touchpad-control.cpp 一致。
pub fn find_switch_report_id(hidraw_dev: &str) -> Result<u8, String> {
    let node = hidraw_dev.rsplit('/').next().ok_or("invalid hidraw path")?;
    let descriptor_path = format!("/sys/class/hidraw/{node}/device/report_descriptor");
    let descriptor =
        fs::read(&descriptor_path).map_err(|e| format!("read {descriptor_path}: {e}"))?;

    // "surface button switch" 集合的特征字节序列
    const SWITCH_COLLECTION: [u8; 10] =
        [0x05, 0x0d, 0x09, 0x22, 0xa1, 0x00, 0x09, 0x57, 0x09, 0x58];
    /// Report ID 标记字节(0x85),其后一个字节即报告 ID
    const REPORT_ID_MARKER: u8 = 0x85;

    let last_start = descriptor.len().saturating_sub(SWITCH_COLLECTION.len());
    for start in 0..last_start {
        if descriptor[start..start + SWITCH_COLLECTION.len()] != SWITCH_COLLECTION {
            continue;
        }
        // 集合内第一个 0x85 后面的字节就是报告 ID
        for idx in start..descriptor.len() - 1 {
            if descriptor[idx] == REPORT_ID_MARKER {
                return Ok(descriptor[idx + 1]);
            }
        }
    }
    Err("surface button switch report not found in HID descriptor".into())
}

/// 以读写方式打开 hidraw 设备
fn open_hidraw(dev: &str) -> Result<RawFd, String> {
    let cpath = CString::new(dev).map_err(|_| "path contains NUL".to_string())?;
    let fd = unsafe { ffi::open(cpath.as_ptr(), O_RDWR | O_NONBLOCK) };
    if fd < 0 {
        return Err(format!("open({dev}): {}", std::io::Error::last_os_error()));
    }
    Ok(fd)
}

/// 将最后一次 ioctl 失败转换为错误信息
fn ioctl_error(name: &str) -> String {
    format!("{name}: {}", std::io::Error::last_os_error())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_is_on_detects_bit1() {
        assert!(!state_is_on(0x00));
        assert!(!state_is_on(0x01));
        assert!(state_is_on(0x02));
        assert!(state_is_on(0x03));
        // 高位无关:按低两位判断
        assert!(state_is_on(0x83));
        assert!(!state_is_on(0x80));
    }

    #[test]
    fn describe_state_maps_all_states() {
        assert_eq!(describe_state(0x00), "OFF, LED on, clicks off");
        assert_eq!(describe_state(0x01), "OFF, LED on, clicks on");
        assert_eq!(describe_state(0x02), "ON, LED off, clicks off");
        assert_eq!(describe_state(0x03), "ON, LED off, clicks on");
    }

    #[test]
    fn describe_state_masks_unknown_values() {
        // 未知值按低两位掩码映射,不 panic
        assert_eq!(describe_state(0x7f), describe_state(0x03));
        assert_eq!(describe_state(0xf1), describe_state(0x01));
    }
}
