//! Touchpad 面板:开关状态展示 + 切换操作。

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

use crate::components::{ComponentKind, SelectableRegion, CONTENT_BASE};
use crate::state::State;
use crate::view::{inner, Pane};

/// Touchpad 面板
pub struct TouchpadPane;

impl Pane for TouchpadPane {
    fn draw(&self, frame: &mut Frame, area: Rect, app: &State) {
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
            Line::from("  [r] 刷新  [Esc] 返回"),
        ];

        let paragraph = Paragraph::new(lines).block(Block::bordered().title("Touchpad"));
        frame.render_widget(paragraph, area);
    }

    fn regions(&self, app: &State, area: Rect) -> Vec<SelectableRegion> {
        let content = inner(area);
        vec![
            // 状态行(Text):首行为空行,状态行位于第二行,字符级拖动选中
            SelectableRegion {
                id: CONTENT_BASE,
                kind: ComponentKind::Text,
                rect: Rect::new(content.x, content.y + 1, content.width, 1),
                lines: vec![status_text(app)],
            },
            // 切换开关(Button):提示行到底部,点击切换
            SelectableRegion {
                id: CONTENT_BASE + 1,
                kind: ComponentKind::Button,
                rect: Rect::new(
                    content.x,
                    content.y + 3,
                    content.width,
                    content.height.saturating_sub(3),
                ),
                lines: Vec::new(),
            },
        ]
    }
}

/// 状态行纯文本(渲染与 Text 组件共用,保证字符偏移一致)
fn status_text(app: &State) -> String {
    format!(
        "状态: {}  ({})",
        if app.touchpad_enabled { "ON" } else { "OFF" },
        app.touchpad_desc
    )
}
