//! UNIX domain socket 服务:供 TUI 客户端查询 / 切换触摸板状态。
//!
//! 协议为换行分隔的文本行:
//! - 客户端 → 守护进程:`GET` / `TOGGLE` / `SUB`
//! - 守护进程 → 客户端:`STATE <0|1> <描述>` / `ERR <信息>`
//!
//! `SUB` 后守护进程会在状态变化时主动推送 `STATE` 行。

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// 客户端命令
pub enum Cmd {
    Toggle,
}

/// 在独立线程中启动 socket 服务并监听客户端连接
pub fn start(
    path: &str,
    cmd_tx: Sender<Cmd>,
    state: Arc<Mutex<bool>>,
    subscribers: Arc<Mutex<Vec<Sender<String>>>>,
) -> std::io::Result<()> {
    let _ = std::fs::remove_file(path); // 清理上次异常退出残留的 socket 文件
    let listener = UnixListener::bind(path)?;
    // root 创建的 socket 默认仅 owner 可连;放开写权限让普通用户客户端连接
    std::fs::set_permissions(
        path,
        std::fs::Permissions::from_mode(crate::config::SOCKET_PERMISSIONS),
    )
    .map_err(|e| std::io::Error::other(format!("set socket permissions: {e}")))?;

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                continue;
            };
            std::thread::spawn({
                let (cmd_tx, state, subscribers) =
                    (cmd_tx.clone(), state.clone(), subscribers.clone());
                move || handle_client(stream, cmd_tx, state, subscribers)
            });
        }
    });
    Ok(())
}

/// 将当前状态格式化为协议行
pub fn format_state(enabled: bool) -> String {
    let (value, desc) = if enabled {
        (
            "1",
            mechrevoc_core::describe_state(mechrevoc_core::STATE_ON),
        )
    } else {
        (
            "0",
            mechrevoc_core::describe_state(mechrevoc_core::STATE_OFF),
        )
    };
    format!("STATE {value} {desc}")
}

/// 处理单个客户端连接,直到断开
fn handle_client(
    stream: UnixStream,
    cmd_tx: Sender<Cmd>,
    state: Arc<Mutex<bool>>,
    subscribers: Arc<Mutex<Vec<Sender<String>>>>,
) {
    let (sub_tx, sub_rx) = std::sync::mpsc::channel::<String>();
    // 写入端与读端分开:推送线程专门消费订阅消息,
    // 主线程可以一直阻塞等待客户端命令,二者互不阻塞。
    let writer = Arc::new(Mutex::new(stream));
    let mut reader = BufReader::new(
        writer
            .lock()
            .expect("writer lock")
            .try_clone()
            .expect("clone stream"),
    );

    let push_writer = writer.clone();
    std::thread::spawn(move || {
        while let Ok(line) = sub_rx.recv() {
            let mut writer = push_writer.lock().expect("writer lock");
            if writeln!(writer, "{line}").is_err() || writer.flush().is_err() {
                break; // 客户端已断开
            }
        }
    });

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => match line.trim() {
                "GET" => {
                    let enabled = *state.lock().expect("state lock");
                    let _ = sub_tx.send(format_state(enabled));
                }
                "TOGGLE" => {
                    // 实际切换由守护进程主线程执行(monitor 独占)
                    let _ = cmd_tx.send(Cmd::Toggle);
                }
                "SUB" => {
                    subscribers
                        .lock()
                        .expect("subscribers lock")
                        .push(sub_tx.clone());
                    // 订阅后立即推送当前状态
                    let enabled = *state.lock().expect("state lock");
                    let _ = sub_tx.send(format_state(enabled));
                }
                other => {
                    let _ = sub_tx.send(format!("ERR unknown command: {other}"));
                }
            },
        }
    }
    // 断开时无需显式移除订阅:推送失败(send 返回 Err)会被主线程 retain 清理
}
