//! 触摸板开关状态持久化。
//!
//! 触摸板固件/EC 不保存开关状态,重启后恢复默认开启。
//! 本模块将状态写入本地文件,daemon 启动时恢复,实现"重启保持关闭"。

use std::fs;
use std::path::Path;

/// 读取保存的触摸板开关状态;无保存记录或格式异常返回 `None`
pub fn read_touchpad_state() -> Option<bool> {
    read_from(crate::config::STATE_FILE)
}

/// 保存触摸板开关状态(写入失败静默,不影响主功能)
pub fn write_touchpad_state(enabled: bool) {
    write_to(crate::config::STATE_FILE, enabled);
}

/// 从指定路径读取状态(纯函数,便于测试)
fn read_from(path: &str) -> Option<bool> {
    let content = fs::read_to_string(path).ok()?;
    match content.trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

/// 写入指定路径(自动创建父目录)
fn write_to(path: &str, enabled: bool) {
    if let Some(parent) = Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, if enabled { "1" } else { "0" });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_path() -> String {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "mechrevoc_state_test_{}_{id}.txt",
                std::process::id()
            ))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn write_then_read_roundtrips() {
        let path = temp_path();
        write_to(&path, false);
        assert_eq!(read_from(&path), Some(false));
        write_to(&path, true);
        assert_eq!(read_from(&path), Some(true));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_missing_returns_none() {
        assert_eq!(read_from("/nonexistent/mechrevoc_state"), None);
    }

    #[test]
    fn read_garbage_returns_none() {
        let path = temp_path();
        fs::write(&path, "maybe").expect("write");
        assert_eq!(read_from(&path), None);
        let _ = fs::remove_file(path);
    }
}
