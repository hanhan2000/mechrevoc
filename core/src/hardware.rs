//! 硬件实现层:底层 FFI、设备发现与具体后端实现。
//!
//! 与 `backend`(接口层)分层:本层包含 libc FFI、sysfs 设备发现、
//! HID 开关封装、evdev 事件读取,以及 [`HidrawTouchpad`] 后端实现。
//! 调用方应依赖 `backend` 中的 trait,而非本层具体类型。
//!
//! 对外 API 统一由 crate 根(`lib.rs`)重导出。

pub mod discovery;
pub mod evdev;
pub mod ffi;
pub mod hidraw;
pub mod touchpad;
