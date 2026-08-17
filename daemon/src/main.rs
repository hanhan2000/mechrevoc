//! mechrevoc-daemon:机械革命笔记本硬件控制守护进程。
//!
//! 常驻后台,唯一接触硬件的进程:
//! - 监听触摸板角落双击(固件注入的 Super+Ctrl+Paste)自动切换开关
//! - 通过 UNIX socket 为 TUI 客户端提供查询与切换接口

mod config;
mod server;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use mechrevoc_core::{default_socket_path, TouchpadMonitor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut monitor = TouchpadMonitor::open()?;
    let socket_path = default_socket_path();

    let state: Arc<Mutex<bool>> = Arc::new(Mutex::new(monitor.enabled()));
    let subscribers: Arc<Mutex<Vec<mpsc::Sender<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let (cmd_tx, cmd_rx) = mpsc::channel::<server::Cmd>();
    let cmd_rx = Arc::new(Mutex::new(cmd_rx));

    server::start(&socket_path, cmd_tx, state.clone(), subscribers.clone())?;
    println!("mechrevoc-daemon: listening on {socket_path}");
    println!(
        "mechrevoc-daemon: touchpad is currently {}",
        monitor.state_description()
    );

    loop {
        // 处理客户端命令与组合键;任一来源切换了状态则广播
        let mut changed = false;

        if let Ok(server::Cmd::Toggle) = cmd_rx.lock().expect("cmd rx lock").try_recv() {
            monitor.toggle()?;
            *state.lock().expect("state lock") = monitor.enabled();
            changed = true;
        }
        if monitor.poll()? {
            *state.lock().expect("state lock") = monitor.enabled();
            changed = true;
        }
        if changed {
            broadcast(&subscribers, &state);
        }

        std::thread::sleep(config::POLL_INTERVAL);
    }
}

/// 向所有订阅客户端推送当前状态(失败的连接会被自动清理)
fn broadcast(subscribers: &Arc<Mutex<Vec<mpsc::Sender<String>>>>, state: &Arc<Mutex<bool>>) {
    let enabled = *state.lock().expect("state lock");
    let line = server::format_state(enabled);
    subscribers
        .lock()
        .expect("subscribers lock")
        .retain(|tx| tx.send(line.clone()).is_ok());
}
