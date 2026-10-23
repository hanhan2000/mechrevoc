//! mechrevoc-daemon:机械革命笔记本硬件控制守护进程。
//!
//! 常驻后台,唯一接触硬件的进程:
//! - 监听触摸板角落双击(固件注入的 Super+Ctrl+Paste)自动切换开关
//! - 通过 UNIX socket 为 TUI 客户端提供查询与切换接口
//! - 持久化触摸板开关状态,重启与挂起恢复后按用户意图校正硬件

// 守护进程不允许任何 unsafe 代码(硬件 FFI 全部收敛于 core)
#![forbid(unsafe_code)]

mod config;
mod lifecycle;
mod server;
mod state_store;

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use mechrevoc_core::{default_socket_path, HidrawTouchpad, TouchpadBackend};

use lifecycle::{Action, Lifecycle};

/// 状态推送订阅者列表(发送端交给每个订阅连接)
type Subscribers = Arc<Mutex<Vec<mpsc::Sender<String>>>>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 面向 TouchpadBackend trait 编程:换硬件后端 / 注入测试 mock 均无需改动此处
    let mut monitor: Box<dyn TouchpadBackend> = Box::new(HidrawTouchpad::open()?);

    let socket_path = default_socket_path();
    let state: Arc<Mutex<bool>> = Arc::new(Mutex::new(monitor.enabled()));
    let subscribers: Subscribers = Arc::new(Mutex::new(Vec::new()));
    let (cmd_tx, cmd_rx) = mpsc::channel::<server::Cmd>();
    let cmd_rx = Arc::new(Mutex::new(cmd_rx));

    // 先按用户意图校正硬件再开放 socket:客户端连上时看到的即正确状态
    reconcile(&mut monitor, &state, &subscribers);
    server::start(&socket_path, cmd_tx, state.clone(), subscribers.clone())?;

    println!("mechrevoc-daemon: listening on {socket_path}");
    println!(
        "mechrevoc-daemon: touchpad is currently {}",
        monitor.state_description()
    );

    run_loop(&mut monitor, &state, &subscribers, cmd_rx);
    Ok(())
}

/// 主循环:客户端命令 + 组合键轮询 + 意图持久化 + 唤醒/对账调度
fn run_loop(
    monitor: &mut Box<dyn TouchpadBackend>,
    state: &Arc<Mutex<bool>>,
    subscribers: &Subscribers,
    cmd_rx: Arc<Mutex<mpsc::Receiver<server::Cmd>>>,
) {
    let mut lifecycle = Lifecycle::new(Instant::now());

    loop {
        let mut changed = false;

        // 客户端切换请求:失败不退出,交由下一轮对账判定句柄是否失效
        if let Ok(server::Cmd::Toggle) = cmd_rx.lock().expect("cmd rx lock").try_recv() {
            if let Err(e) = monitor.toggle() {
                eprintln!("mechrevoc-daemon: toggle failed: {e}");
            } else {
                *state.lock().expect("state lock") = monitor.enabled();
                changed = true;
            }
        }

        // 组合键轮询(poll 内部会清理失效的设备句柄)
        match monitor.poll() {
            Ok(true) => {
                *state.lock().expect("state lock") = monitor.enabled();
                changed = true;
            }
            Ok(false) => {}
            Err(e) => eprintln!("mechrevoc-daemon: poll failed: {e}"),
        }

        if changed {
            // 状态变化即持久化用户意图,重启与唤醒后按此恢复
            state_store::write_touchpad_state(monitor.enabled());
            broadcast(subscribers, state);
        }

        std::thread::sleep(config::POLL_INTERVAL);

        match lifecycle.tick(Instant::now()) {
            Action::Idle => {}
            Action::Resume => {
                println!("mechrevoc-daemon: resume detected, rebuilding device handles");
                rebuild(monitor);
                reconcile(monitor, state, subscribers);
            }
            Action::Reconcile => {
                if !monitor.is_healthy() {
                    println!("mechrevoc-daemon: device handles stale, rebuilding");
                    rebuild(monitor);
                }
                reassert_on_drift(monitor, state, subscribers);
            }
        }
    }
}

/// 重建全部设备句柄(挂起恢复或句柄失效时)
fn rebuild(monitor: &mut Box<dyn TouchpadBackend>) {
    if let Err(e) = monitor.reattach() {
        // 设备可能仍在重枚举:记录后交给下一轮对账重试
        eprintln!("mechrevoc-daemon: reattach failed: {e}");
    }
}

/// 硬件状态被外部改动时(如固件复位)按用户意图写回
fn reassert_on_drift(
    monitor: &mut Box<dyn TouchpadBackend>,
    state: &Arc<Mutex<bool>>,
    subscribers: &Subscribers,
) {
    match monitor.refresh_state() {
        Ok(true) => {
            println!("mechrevoc-daemon: hardware state drifted, re-asserting saved state");
            reconcile(monitor, state, subscribers);
        }
        Ok(false) => {}
        Err(e) => eprintln!("mechrevoc-daemon: refresh state failed: {e}"),
    }
}

/// 按保存的用户意图校正硬件状态,并同步给订阅客户端。
///
/// 用户意图(状态文件)是唯一权威:挂起恢复后固件复位、外部工具改动,
/// 都以保存的意图为准写回硬件。无保存记录(首次运行)时以当前硬件状态建档。
fn reconcile(
    monitor: &mut Box<dyn TouchpadBackend>,
    state: &Arc<Mutex<bool>>,
    subscribers: &Subscribers,
) {
    let Some(desired) = state_store::read_touchpad_state() else {
        state_store::write_touchpad_state(monitor.enabled());
        *state.lock().expect("state lock") = monitor.enabled();
        return;
    };

    let before = monitor.enabled();
    if let Err(e) = monitor.set_enabled(desired) {
        eprintln!("mechrevoc-daemon: restore touchpad state failed: {e}");
        return;
    }

    let after = monitor.enabled();
    if before != after {
        println!(
            "mechrevoc-daemon: touchpad restored to {}",
            if after { "ON" } else { "OFF" }
        );
    }
    *state.lock().expect("state lock") = after;
    broadcast(subscribers, state);
}

/// 向所有订阅客户端推送当前状态(失败的连接会被自动清理)
fn broadcast(subscribers: &Subscribers, state: &Arc<Mutex<bool>>) {
    let enabled = *state.lock().expect("state lock");
    let line = server::format_state(enabled);
    subscribers
        .lock()
        .expect("subscribers lock")
        .retain(|tx| tx.send(line.clone()).is_ok());
}
