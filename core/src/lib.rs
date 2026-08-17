//! mechrevoc-core:机械革命笔记本的硬件控制库。
//!
//! 各模块职责:
//! - [`backend`] - 触摸板后端抽象 trait(调用方依赖此接口,实现可替换)
//! - [`discovery`] - 通过 sysfs 发现触摸板的 hidraw / evdev 设备节点
//! - [`hidraw`] - 通过 HID feature report 控制触摸板开关与 LED
//! - [`evdev`] - 读取输入设备事件(组合键检测的数据源)
//! - [`touchpad`] - hidraw 触摸板后端实现(组合键监听 + 手动切换)
//! - [`client`] - 守护进程的 UNIX socket 客户端(供 TUI 等使用)
//! - [`ffi`] - 零依赖的 libc FFI 与 ioctl 常量

pub mod backend;
pub mod client;
pub mod discovery;
pub mod evdev;
pub mod ffi;
pub mod hidraw;
pub mod touchpad;

pub use backend::TouchpadBackend;
pub use client::{default_socket_path, Client};
pub use discovery::{find_evdev, find_hidraw};
pub use hidraw::{describe_state, state_is_on, TouchpadSwitcher, STATE_OFF, STATE_ON};
pub use touchpad::{ComboDetector, HidrawTouchpad};
