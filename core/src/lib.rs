//! mechrevoc-core:机械革命笔记本的硬件控制库。
//!
//! 分层结构(自底向上):
//! - [`backend`] - 接口层:触摸板后端抽象 trait(调用方依赖此接口,实现可替换)
//! - [`hardware`] - 硬件实现层:libc FFI / sysfs 发现 / hidraw 开关 / evdev 事件 / 后端实现
//! - [`protocol`] - 通信协议层:与 daemon 的 UNIX socket 客户端(供 TUI 等使用)
//! - [`system`] - 系统信息层:只读 sysfs 的 CPU 温度 / 电池状态

pub mod backend;
pub mod hardware;
pub mod protocol;
pub mod system;

// 对外 API:通过重导出保持扁平路径,daemon/tui 无需感知内部分层
pub use backend::TouchpadBackend;
pub use hardware::discovery::{find_evdev, find_hidraw};
pub use hardware::hidraw::{describe_state, state_is_on, TouchpadSwitcher, STATE_OFF, STATE_ON};
pub use hardware::touchpad::{ComboDetector, HidrawTouchpad};
pub use protocol::client::{default_socket_path, Client};
pub use protocol::line::{
    format_state_line, parse_state_line, CMD_GET, CMD_SUB, CMD_TOGGLE, ERR_PREFIX, STATE_PREFIX,
};
pub use system::{read_system_info, BatteryInfo, SystemInfo};
