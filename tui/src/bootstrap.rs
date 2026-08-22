//! 启动引导:检测 mechrevoc-daemon 守护进程状态,未运行时提示用户处理。
//!
//! 主界面启动前先尝试连接守护进程;失败则进入本模块的全屏引导界面,
//! 展示诊断结果并提供「启动服务 / 安装指引 / 退出」操作。
//! 部署动作本身见 [`service`](crate::service)。

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::service::{self, DaemonState};

/// 引导流程的退出结果
pub enum BootResult {
    /// 引导后成功连接守护进程,携带客户端
    Connected(mechrevoc_core::Client),
    /// 用户选择退出
    Quit,
}

/// 运行引导流程;成功连接后返回 [`BootResult::Connected`]
pub fn run(socket_path: &str) -> Result<BootResult, String> {
    let mut terminal = ratatui::init();
    let mut state = service::detect_daemon();
    let mut notice: Option<String> = None;

    let result = loop {
        terminal
            .draw(|frame| draw(frame, state, notice.as_deref()))
            .map_err(|e| e.to_string())?;

        if event::poll(Duration::from_millis(50)).map_err(|e| e.to_string())? {
            if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break BootResult::Quit,
                    KeyCode::Char('1') => {
                        // 未安装 → 安装服务;已安装 → 启动服务
                        ratatui::restore();
                        let (ok, fail_msg) = match state {
                            DaemonState::NotInstalled => (
                                service::run_install(),
                                "安装失败:请手动运行 ./deploy/install.sh".into(),
                            ),
                            DaemonState::InstalledStopped => (
                                service::try_start_daemon(),
                                "启动失败:请手动运行 `sudo systemctl start mechrevoc-daemon`"
                                    .into(),
                            ),
                        };
                        terminal = ratatui::init();

                        if ok {
                            if let Ok(client) = mechrevoc_core::Client::connect(socket_path) {
                                break BootResult::Connected(client);
                            }
                            notice = Some("操作成功,但连接失败,请稍后重试。".into());
                        } else {
                            notice = Some(fail_msg);
                        }
                        state = service::detect_daemon();
                    }
                    KeyCode::Char('2') => notice = Some(install_help(&state)),
                    _ => {}
                }
            }
        }
    };

    ratatui::restore();
    Ok(result)
}

/// 安装指引文本(未安装时展示)
fn install_help(state: &DaemonState) -> String {
    match state {
        DaemonState::NotInstalled => {
            "未检测到 mechrevoc-daemon 服务。\n安装方法:\n  cd <mechrevoc 项目目录>\n  ./deploy/install.sh\n安装完成后重新启动本程序。".into()
        }
        DaemonState::InstalledStopped => {
            "服务已安装但未运行。按 1 启动;或手动执行:\n  sudo systemctl start mechrevoc-daemon".into()
        }
    }
}

/// 渲染引导界面
fn draw(frame: &mut Frame, state: DaemonState, notice: Option<&str>) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(frame.area());
    frame.render_widget(header(), chunks[0]);
    frame.render_widget(body(state, notice), chunks[1]);
}

fn header() -> Paragraph<'static> {
    Paragraph::new(Line::from(Span::styled(
        " mechrevoc — 启动引导 ",
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )))
    .block(Block::default().borders(Borders::ALL))
}

fn body(state: DaemonState, notice: Option<&str>) -> Paragraph<'static> {
    let status = match state {
        DaemonState::NotInstalled => Span::styled("未安装", Style::new().fg(Color::Yellow)),
        DaemonState::InstalledStopped => {
            Span::styled("已安装,未运行", Style::new().fg(Color::Yellow))
        }
    };

    let action1 = match state {
        DaemonState::NotInstalled => "  [1] 安装服务",
        DaemonState::InstalledStopped => "  [1] 启动服务(需要系统授权)",
    };

    let mut lines: Vec<Line> = vec![
        Line::from(vec![Span::raw("守护进程状态: "), status]),
        Line::from(""),
        Line::from(action1),
        Line::from("  [2] 查看说明"),
        Line::from("  [q] 退出"),
    ];

    if let Some(text) = notice {
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        for part in text.lines() {
            lines.push(Line::from(Span::styled(
                part.to_string(),
                Style::new().fg(Color::DarkGray),
            )));
        }
    }

    Paragraph::new(lines)
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL))
}
