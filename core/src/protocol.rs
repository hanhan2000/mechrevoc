//! 通信协议层:与 mechrevoc-daemon 的 UNIX socket 客户端。
//!
//! 与硬件层无关,供 TUI 等客户端复用:连接 socket、查询状态、
//! 发送切换命令、接收状态推送。

pub mod client;

pub use client::{default_socket_path, Client};
