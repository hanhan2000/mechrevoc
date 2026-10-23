//! 主事件循环:事件分发 + 守护进程推送接收 + 系统信息定期刷新。

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;
use std::time::Duration;

use crate::service;
use crate::state::{Action, State};
use crate::terminal;
use crate::view;

/// 系统信息(温度/电池)的刷新间隔
const SYSINFO_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// 主事件循环:处理按键/鼠标、接收守护进程推送、定期刷新系统信息
pub fn run(
    terminal: &mut DefaultTerminal,
    state: &mut State,
    client: &mut mechrevoc_core::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_sysinfo_refresh = std::time::Instant::now();

    loop {
        terminal.draw(|frame| view::draw(frame, state))?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        match state.handle_key(&key) {
                            Action::Quit => return Ok(()),
                            Action::ToggleTouchpad => client.toggle()?,
                            Action::RefreshState => {
                                client.get_state()?;
                                state.sync_from_client(client);
                                state.system_info = mechrevoc_core::read_system_info();
                            }
                            Action::DeleteDaemon => {
                                uninstall_daemon()?;
                                return Ok(());
                            }
                            Action::None => {}
                        }
                    }
                }
                Event::Mouse(mouse) => match state.handle_mouse(&mouse) {
                    Action::ToggleTouchpad => client.toggle()?,
                    Action::DeleteDaemon => {
                        uninstall_daemon()?;
                        return Ok(());
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        // 接收守护进程推送的状态变化(角落双击、其他客户端操作等)
        if client.poll()? {
            state.sync_from_client(client);
        }

        // 定期刷新系统信息(读取 sysfs 成本极低)并记录温度采样
        if last_sysinfo_refresh.elapsed() >= SYSINFO_REFRESH_INTERVAL {
            state.system_info = mechrevoc_core::read_system_info();
            state.record_temp();
            last_sysinfo_refresh = std::time::Instant::now();
        }
    }
}

/// 卸载 daemon 服务:退出全屏 → pkexec 卸载 → 打印结果
fn uninstall_daemon() -> Result<(), Box<dyn std::error::Error>> {
    terminal::restore();
    let removed = service::run_uninstall();
    if removed {
        println!("mechrevoc-daemon 服务已删除。");
    } else {
        println!("删除失败或已取消(需要系统授权)。");
    }
    Ok(())
}
