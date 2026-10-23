//! 渲染层:布局框架 + 面板注册与分发。
//!
//! 每个面板([`Pane`])独立成模块(touchpad / system),通过 [`PANES`] 注册,
//! [`draw`] 按当前选中的模块分发渲染。面板各自负责自己的渲染与组件区域。

pub mod system;
pub mod touchpad;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph},
    Frame,
};

use crate::components::{ComponentKind, SelectableRegion};
use crate::state::{Mode, State};

/// 面板视图 trait:渲染 + 组件区域声明
pub trait Pane {
    /// 渲染面板内容
    fn draw(&self, frame: &mut Frame, area: Rect, app: &State);

    /// 面板内的可交互组件区域(注入组件层,用于鼠标交互)
    fn regions(&self, app: &State, area: Rect) -> Vec<SelectableRegion>;
}

/// 已注册面板(顺序与侧边栏模块一一对应)
pub const PANES: [&dyn Pane; 2] = [&touchpad::TouchpadPane, &system::SystemPane];

/// 侧边栏宽度
const SIDEBAR_WIDTH: u16 = 22;

/// 选区高亮色:真彩色 RGB 蓝。
///
/// 不使用 `Color::Blue`(输出 ANSI 256 色索引,会被终端主题重新映射成粉/紫色),
/// 真彩色不受主题调色板影响,现代终端均支持。
const SELECTION_BG: Color = Color::Rgb(0x3B, 0x82, 0xF6);

/// 整体布局:侧边栏模块导航 + 当前面板 + 底部状态栏
pub fn draw(frame: &mut Frame, app: &mut State) {
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
    let body = Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)])
        .split(chunks[0]);
    let pane = PANES[app.selected];

    // 组件区域 = 侧边栏模块项(Button)+ 当前面板组件,注入组件层
    let mut regions = sidebar_regions(app, body[0]);
    regions.extend(pane.regions(app, body[1]));
    app.layer.set_regions(regions);

    render_sidebar(frame, body[0], app);
    pane.draw(frame, body[1], app);
    render_statusbar(frame, chunks[1], app);
    app.layer.apply_highlight(frame.buffer_mut(), SELECTION_BG);
}

/// 侧边栏模块项区域(Button):block 边框占首行,列表项从 y+1 起,每项一行
fn sidebar_regions(app: &State, area: Rect) -> Vec<SelectableRegion> {
    app.view_names()
        .iter()
        .enumerate()
        .map(|(index, _name)| SelectableRegion {
            id: index as u16,
            kind: ComponentKind::Button,
            rect: Rect::new(
                area.x + 1,
                area.y + 1 + index as u16,
                area.width.saturating_sub(2),
                1,
            ),
            lines: Vec::new(),
        })
        .collect()
}

/// 边框内区域(去掉 1 格边框);供各面板计算内容区
pub(crate) fn inner(area: Rect) -> Rect {
    Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

/// 左侧模块导航栏:选中项高亮(无额外 active 指示)
fn render_sidebar(frame: &mut Frame, area: Rect, app: &State) {
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

/// 底部状态栏:显示当前交互模式与按键/鼠标提示
fn render_statusbar(frame: &mut Frame, area: Rect, app: &State) {
    let (hint, style) = match app.mode {
        Mode::Nav => (
            " [导航] j/k 选择  Enter 进入  q/Esc 退出 | 鼠标:点击模块进入 / 拖动选中元素",
            Style::new().fg(Color::DarkGray),
        ),
        Mode::Pane => (
            " [面板] Esc 返回  r 刷新 | 鼠标:拖动选中元素",
            Style::new().fg(Color::Cyan),
        ),
    };
    frame.render_widget(Paragraph::new(Line::from(Span::styled(hint, style))), area);
}
