//! mechrevoc-daemon:机械革命笔记本硬件控制守护进程。
//!
//! 常驻后台,唯一接触硬件的进程:
//! - 监听触摸板角落双击(固件注入的 Super+Ctrl+Paste)自动切换开关
//! - 通过 UNIX socket 为 TUI 客户端提供查询与切换接口
//! - 持久化触摸板开关状态,重启后恢复(触摸板固件不保存状态)

// 守护进程不允许任何 unsafe 代码(硬件 FFI 全部收敛于 core)
#![forbid(unsafe_code)]

mod config;
mod server;
mod state_store;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use mechrevoc_core::{default_socket_path, HidrawTouchpad, TouchpadBackend};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 面向 TouchpadBackend trait 编程:换硬件后端 / 注入测试 mock 均无需改动此处
    let mut monitor: Box<dyn TouchpadBackend> = Box::new(HidrawTouchpad::open()?);
    restore_saved_state(&mut monitor);

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
        // 处理客户端命令与组合键;任一来源切换了状态则广播。
        // 不引入防护:系统状态以硬件(最后一次交互)为准,
        // 异常退出由 systemd 重启,重启后重新读取硬件状态。
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
            // 状态变化即持久化,重启后按此恢复
            state_store::write_touchpad_state(monitor.enabled());
            broadcast(&subscribers, &state);
        }

        std::thread::sleep(config::POLL_INTERVAL);
    }
}

/// 启动时恢复上次保存的触摸板状态(如上次关闭则保持关闭)。
///
/// 触摸板固件/EC 重启后恢复默认开启,本函数在 daemon 启动时
/// 将硬件状态校正为上次保存的状态。
fn restore_saved_state(monitor: &mut Box<dyn TouchpadBackend>) {
    let Some(saved) = state_store::read_touchpad_state() else {
        return; // 无保存记录(首次运行):保持硬件默认状态
    };
    if monitor.enabled() != saved {
        match monitor.toggle() {
            Ok(()) => println!("mechrevoc-daemon: restored touchpad to {}", saved),
            Err(e) => eprintln!("mechrevoc-daemon: restore touchpad state failed: {e}"),
        }
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
