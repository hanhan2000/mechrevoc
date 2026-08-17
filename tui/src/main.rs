//! mechrevoc TUI 控制台(客户端)。
//!
//! 通过 UNIX socket 连接 mechrevoc-daemon 守护进程,查询 / 切换触摸板状态,
//! 本身不接触任何硬件。启动时若守护进程未运行,进入引导流程。

mod app;
mod boot;
mod ui;

use app::{Action, App};
use boot::BootResult;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = mechrevoc_core::default_socket_path();

    // 先尝试直接连接;失败则进入引导流程
    let mut client = match mechrevoc_core::Client::connect(&socket_path) {
        Ok(client) => client,
        Err(_) => match boot::run(&socket_path)? {
            BootResult::Connected(client) => client,
            BootResult::Quit => return Ok(()),
        },
    };
    client.subscribe()?;

    let mut terminal = ratatui::init();
    let mut app = App::new();
    app.sync_from_client(&client);

    let result = run(&mut terminal, &mut app, &mut client);

    ratatui::restore();
    result
}

/// 主事件循环:处理按键 + 接收守护进程推送的状态
fn run(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    client: &mut mechrevoc_core::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.handle_key(&key) {
                        Action::Quit => return Ok(()),
                        Action::ToggleTouchpad => client.toggle()?,
                        Action::RefreshState => {
                            client.get_state()?;
                            app.sync_from_client(client);
                        }
                        Action::DeleteDaemon => {
                            // 退出全屏 → 卸载(pkexec 授权)→ 提示并退出
                            ratatui::restore();
                            let removed = boot::run_uninstall();
                            if removed {
                                println!("mechrevoc-daemon 服务已删除。");
                            } else {
                                println!("删除失败或已取消(需要系统授权)。");
                            }
                            return Ok(());
                        }
                        Action::None => {}
                    }
                }
            }
        }

        // 接收守护进程推送的状态变化(角落双击、其他客户端操作等)
        if client.poll()? {
            app.sync_from_client(client);
        }
    }
}
