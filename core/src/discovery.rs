//! 通过 sysfs 发现触摸板对应的 hidraw 与 evdev 设备节点。
//!
//! 设备厂商标识为 `UNIW0001`(i2c-hid 多点触控触摸板),
//! 内核同时导出 hidraw 与 evdev 两个接口。

use std::fs;
use std::path::Path;

const HIDRAW_SYS_DIR: &str = "/sys/class/hidraw";
const INPUT_SYS_DIR: &str = "/sys/class/input";

/// /dev 下的设备节点目录:hidraw 在 /dev 根下,evdev 在 /dev/input 下
const DEV_HIDRAW_DIR: &str = "/dev";
const DEV_INPUT_DIR: &str = "/dev/input";

/// hidraw 的 uevent 中用于识别本触摸板的特征串
const HIDRAW_UDEV_MARKER: [&str; 2] = ["UNIW0001", "093A:0255"];
/// input 设备的名称中用于识别本触摸板的特征串
const INPUT_NAME_MARKERS: [&str; 2] = ["UNIW0001", "Touchpad"];

/// 在 /sys/class/hidraw 下寻找触摸板的 hidraw 节点(如 /dev/hidraw7)
pub fn find_hidraw() -> Option<String> {
    let entries = fs::read_dir(HIDRAW_SYS_DIR).ok()?;
    for entry in entries.flatten() {
        let uevent = fs::read_to_string(entry.path().join("device/uevent")).ok()?;
        if is_touchpad_uevent(&uevent) {
            return Some(dev_node_path(DEV_HIDRAW_DIR, entry.file_name()));
        }
    }
    None
}

/// 在 /sys/class/input 下寻找触摸板的 evdev 节点(如 /dev/input/event23)
pub fn find_evdev() -> Option<String> {
    let entries = fs::read_dir(INPUT_SYS_DIR).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("event") {
            continue;
        }
        let devname = fs::read_to_string(entry.path().join("device/name")).ok()?;
        if is_touchpad_input_name(&devname) {
            return Some(dev_node_path(DEV_INPUT_DIR, entry.file_name()));
        }
    }
    None
}

/// 枚举所有 evdev 设备,返回 (设备路径, 设备名称)
pub fn all_evdev_devices() -> Result<Vec<(String, String)>, String> {
    let entries = fs::read_dir(INPUT_SYS_DIR).map_err(|e| format!("read {INPUT_SYS_DIR}: {e}"))?;
    let mut devices = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("event") {
            continue;
        }
        let devname = fs::read_to_string(entry.path().join("device/name")).unwrap_or_default();
        devices.push((
            dev_node_path(DEV_INPUT_DIR, entry.file_name()),
            devname.trim().to_string(),
        ));
    }
    Ok(devices)
}

/// hidraw 的 uevent 是否描述的是本触摸板
fn is_touchpad_uevent(uevent: &str) -> bool {
    uevent.contains(HIDRAW_UDEV_MARKER[0])
        && (uevent.contains("i2c") || uevent.contains(HIDRAW_UDEV_MARKER[1]))
}

/// input 设备名称是否描述的是本触摸板
fn is_touchpad_input_name(name: &str) -> bool {
    name.contains(INPUT_NAME_MARKERS[0]) && name.contains(INPUT_NAME_MARKERS[1])
}

/// 由 sysfs 目录名组装 /dev 下的设备节点路径
fn dev_node_path(dir: &str, name: impl AsRef<Path>) -> String {
    format!("{dir}/{}", name.as_ref().to_string_lossy())
}
