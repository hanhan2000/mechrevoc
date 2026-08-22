//! System 面板:系统信息(温度/电池)+ 温度波形 + 系统操作(删除 daemon)。

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::Line,
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph},
    Frame,
};

use crate::components::{ComponentKind, SelectableRegion, CONTENT_BASE};
use crate::state::State;
use crate::view::{inner, Pane};

/// System 面板垂直分区(渲染与组件区域构建共用)
const SYSTEM_LAYOUT: [Constraint; 3] = [
    Constraint::Length(4), // 系统信息
    Constraint::Min(3),    // 温度波形
    Constraint::Length(7), // 系统操作
];

/// System 面板
pub struct SystemPane;

impl Pane for SystemPane {
    fn draw(&self, frame: &mut Frame, area: Rect, app: &State) {
        let chunks = Layout::vertical(SYSTEM_LAYOUT).split(area);
        render_sysinfo_panel(frame, chunks[0], app);
        render_temp_chart(frame, chunks[1], app);
        render_sysops_panel(frame, chunks[2], app);
    }

    fn regions(&self, app: &State, area: Rect) -> Vec<SelectableRegion> {
        let chunks = Layout::vertical(SYSTEM_LAYOUT).split(area);
        let info = inner(chunks[0]);
        vec![
            // 系统信息行(Text):首行为空行,内容行位于第二行,字符级拖动选中
            SelectableRegion {
                id: CONTENT_BASE + 2,
                kind: ComponentKind::Text,
                rect: Rect::new(info.x, info.y + 1, info.width, 1),
                lines: vec![sysinfo_line(app)],
            },
            // 删除操作(Button):点击触发二次确认
            SelectableRegion {
                id: CONTENT_BASE + 3,
                kind: ComponentKind::Button,
                rect: inner(chunks[2]),
                lines: Vec::new(),
            },
            // 波形区(chunks[1])默认无交互
        ]
    }
}

/// 系统信息面板:CPU 温度与电池当前值
fn render_sysinfo_panel(frame: &mut Frame, area: Rect, app: &State) {
    let lines = vec![Line::from(""), Line::from(sysinfo_line(app))];

    let paragraph = Paragraph::new(lines).block(Block::bordered().title("系统信息"));
    frame.render_widget(paragraph, area);
}

/// CPU 温度波形:Braille 点阵曲线,y 轴动态范围跟随数据
fn render_temp_chart(frame: &mut Frame, area: Rect, app: &State) {
    let block = Block::bordered().title("CPU 温度波形");

    // 采样不足 2 点时无法成线,显示等待提示
    if app.temp_history.len() < 2 {
        frame.render_widget(
            Paragraph::new(Line::from("  采样中,等待温度数据..."))
                .block(block)
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let data: Vec<(f64, f64)> = app
        .temp_history
        .iter()
        .enumerate()
        .map(|(i, t)| (i as f64, *t as f64))
        .collect();

    // y 轴动态范围:数据极值 ±2°C,并保证最小跨度避免退化
    let min_temp = data.iter().map(|d| d.1).fold(f64::INFINITY, f64::min);
    let max_temp = data.iter().map(|d| d.1).fold(f64::NEG_INFINITY, f64::max);
    let (mut y_min, mut y_max) = (min_temp - 2.0, max_temp + 2.0);
    if y_max - y_min < 4.0 {
        let mid = (y_min + y_max) / 2.0;
        (y_min, y_max) = (mid - 2.0, mid + 2.0);
    }

    let datasets = vec![Dataset::default()
        .name("CPU")
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::new().fg(Color::Cyan))
        .data(&data)];

    let x_axis = Axis::default()
        .bounds([0.0, app.temp_history.len() as f64])
        .labels(vec![
            Line::from("0"),
            Line::from(format!("{}", app.temp_history.len())),
        ]);
    let y_axis = Axis::default().bounds([y_min, y_max]).labels(vec![
        Line::from(format!("{y_max:.0}°")),
        Line::from(format!("{y_min:.0}°")),
    ]);

    let chart = Chart::new(datasets)
        .block(block)
        .x_axis(x_axis)
        .y_axis(y_axis);
    frame.render_widget(chart, area);
}

/// 系统操作面板:删除 daemon 服务(二次确认)
fn render_sysops_panel(frame: &mut Frame, area: Rect, app: &State) {
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
        Line::from("  删除后:守护进程停止、开机自启移除,"),
        Line::from("  触摸板角落双击自动切换将失效(需重新安装恢复)。"),
        Line::from("  (需要系统授权)"),
    ];

    let paragraph = Paragraph::new(lines).block(Block::bordered().title("系统操作"));
    frame.render_widget(paragraph, area);
}

/// 系统信息行纯文本(渲染与 Text 组件共用,保证字符偏移一致)
fn sysinfo_line(app: &State) -> String {
    let temp_text = app
        .system_info
        .cpu_temp_c
        .map(|c| format!("{c:.1}°C"))
        .unwrap_or_else(|| "N/A".into());
    let battery_text = app
        .system_info
        .battery
        .as_ref()
        .map(|b| format!("{}% ({})", b.capacity, b.status))
        .unwrap_or_else(|| "N/A".into());
    format!("  CPU 温度: {temp_text}    电池: {battery_text}")
}
