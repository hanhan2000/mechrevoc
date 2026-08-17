//! 界面渲染:侧边栏导航 + 各模块面板。

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;

/// 整体布局:左侧模块导航,右侧内容区
pub fn draw(frame: &mut Frame, app: &App) {
    let chunks =
        Layout::horizontal([Constraint::Length(22), Constraint::Min(0)]).split(frame.area());
    render_sidebar(frame, chunks[0], app);
    render_content(frame, chunks[1], app);
}

/// 左侧模块导航栏
fn render_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .view_names()
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let style = if index == app.selected {
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new()
            };
            ListItem::new(*name).style(style)
        })
        .collect();

    frame.render_widget(
        List::new(items).block(Block::bordered().title("Modules")),
        area,
    );
}

/// 右侧内容区:按选中的模块分发渲染
fn render_content(frame: &mut Frame, area: Rect, app: &App) {
    match app.selected {
        0 => render_touchpad_view(frame, area, app),
        _ => render_system_view(frame, area, app),
    }
}

/// 触摸板控制面板
fn render_touchpad_view(frame: &mut Frame, area: Rect, app: &App) {
    let status_style = if app.touchpad_enabled {
        Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("状态: "),
            Span::styled(
                if app.touchpad_enabled { "ON" } else { "OFF" },
                status_style,
            ),
            Span::raw("  ("),
            Span::raw(app.touchpad_desc.as_str()),
            Span::raw(")"),
        ]),
        Line::from(""),
        Line::from("  角落双击 或 [Enter/Space] 切换开关"),
        Line::from("  [q] 退出"),
    ];

    let paragraph = Paragraph::new(lines).block(Block::bordered().title("Touchpad"));
    frame.render_widget(paragraph, area);
}

/// 系统信息面板:系统级操作(删除 daemon 服务)
fn render_system_view(frame: &mut Frame, area: Rect, app: &App) {
    let confirm_style = if app.confirming_delete {
        Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::new()
    };

    let lines = vec![
        Line::from(""),
        Line::styled(
            if app.confirming_delete {
                "  [Enter] 确认删除 daemon 服务?  [Esc] 取消"
            } else {
                "  [Enter] 删除 daemon 服务(卸载)"
            },
            confirm_style,
        ),
        Line::from(""),
        Line::from("  删除后:守护进程停止、开机自启移除,"),
        Line::from("  触摸板角落双击自动切换将失效(需重新安装恢复)。"),
        Line::from("  (需要系统授权)"),
    ];

    let paragraph = Paragraph::new(lines).block(Block::bordered().title("System"));
    frame.render_widget(paragraph, area);
}
