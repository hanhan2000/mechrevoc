//! 终端生命周期:初始化 / 恢复 + 鼠标捕获。

use ratatui::DefaultTerminal;

/// 进入 TUI:raw mode + alternate screen + 显式启用鼠标捕获。
///
/// 注意:`ratatui::init()` 默认不启用鼠标捕获(源码仅 raw mode + alternate screen),
/// 若不手动执行 `EnableMouseCapture`,程序收不到任何鼠标事件。
pub fn init() -> Result<DefaultTerminal, Box<dyn std::error::Error>> {
    let terminal = ratatui::init();
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
    Ok(terminal)
}

/// 退出 TUI:恢复终端并禁用鼠标捕获(ratatui::restore 不负责鼠标)
pub fn restore() {
    ratatui::restore();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
}
