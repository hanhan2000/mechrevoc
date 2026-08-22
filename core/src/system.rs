//! 系统信息读取:CPU 温度与电池状态(只读 sysfs,零依赖)。
//!
//! sysfs 节点对普通用户可读,客户端(TUI)可直接调用,无需 daemon 代理。
//! 单项读取失败不影响其他项(容错),平台不支持时对应字段为 `None`。

use std::fs;
use std::path::{Path, PathBuf};

/// 系统信息快照
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SystemInfo {
    /// CPU 温度(摄氏度);读取失败或平台不支持时为 None
    pub cpu_temp_c: Option<f32>,
    /// 电池信息;无电池设备为 None
    pub battery: Option<BatteryInfo>,
}

/// 电池状态信息
#[derive(Debug, Clone, PartialEq)]
pub struct BatteryInfo {
    /// 剩余电量百分比(0-100)
    pub capacity: u8,
    /// 充电状态(Charging / Discharging / Full / Unknown)
    pub status: String,
}

/// 读取当前系统信息;单项失败不影响其他项
pub fn read_system_info() -> SystemInfo {
    SystemInfo {
        cpu_temp_c: read_cpu_temp(),
        battery: read_battery(),
    }
}

/// 读取 CPU 温度:优先 coretemp / x86_pkg_temp 热区,否则取第一个可读热区
fn read_cpu_temp() -> Option<f32> {
    let zones = thermal_zones()?;
    for zone in &zones {
        if let Ok(typ) = fs::read_to_string(zone.join("type")) {
            let typ = typ.trim();
            if typ == "x86_pkg_temp" || typ == "coretemp" {
                return read_zone_temp(zone);
            }
        }
    }
    zones.iter().find_map(|zone| read_zone_temp(zone))
}

/// 枚举 sysfs 中的 thermal zone 目录
fn thermal_zones() -> Option<Vec<PathBuf>> {
    let entries = fs::read_dir("/sys/class/thermal").ok()?;
    let mut zones = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("thermal_zone") {
            zones.push(entry.path());
        }
    }
    (!zones.is_empty()).then_some(zones)
}

/// 读取热区温度文件并换算为摄氏度
fn read_zone_temp(zone: &Path) -> Option<f32> {
    let raw = fs::read_to_string(zone.join("temp")).ok()?;
    parse_temp_milli(raw.trim())
}

/// 解析毫摄氏度文本(如 "52300")为摄氏度
fn parse_temp_milli(raw: &str) -> Option<f32> {
    let milli: i32 = raw.parse().ok()?;
    Some(milli as f32 / 1000.0)
}

/// 读取电池信息:定位首个 BAT* 目录,读取 capacity 与 status
fn read_battery() -> Option<BatteryInfo> {
    let dir = find_battery_dir()?;
    let capacity: u8 = fs::read_to_string(dir.join("capacity"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let status = fs::read_to_string(dir.join("status")).unwrap_or_default();
    Some(BatteryInfo {
        capacity,
        status: status.trim().to_string(),
    })
}

/// 定位电池 sysfs 目录(如 /sys/class/power_supply/BAT0)
fn find_battery_dir() -> Option<PathBuf> {
    let entries = fs::read_dir("/sys/class/power_supply").ok()?;
    entries.flatten().map(|entry| entry.path()).find(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().starts_with("BAT"))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_temp_milli_converts_to_celsius() {
        assert_eq!(parse_temp_milli("52300"), Some(52.3));
        assert_eq!(parse_temp_milli("0"), Some(0.0));
    }

    #[test]
    fn parse_temp_milli_rejects_garbage() {
        assert_eq!(parse_temp_milli("abc"), None);
        assert_eq!(parse_temp_milli(""), None);
    }

    #[test]
    fn parse_temp_milli_handles_negative() {
        // 探头未初始化时某些平台可能短暂报告负值
        assert_eq!(parse_temp_milli("-500"), Some(-0.5));
    }
}
