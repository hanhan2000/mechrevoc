//! 应用状态(Elm 风格:状态 + 事件处理)。

use crossterm::event::{KeyCode, KeyEvent};

/// 应用界面中的模块列表
const VIEW_NAMES: [&str; 2] = ["Touchpad", "System"];

/// 按键处理产生的动作
pub enum Action {
    None,
    Quit,
    ToggleTouchpad,
    /// 手动刷新状态(发送 GET 重新拉取)
    RefreshState,
    /// 确认删除 daemon 服务(需要系统授权)
    DeleteDaemon,
}

/// 应用全局状态
pub struct App {
    /// 侧边栏选中的模块下标
    pub selected: usize,
    /// 触摸板开关状态
    pub touchpad_enabled: bool,
    /// 触摸板状态描述(如 "ON, LED off, clicks on")
    pub touchpad_desc: String,
    /// System 模块:删除 daemon 的二次确认状态
    pub confirming_delete: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            selected: 0,
            touchpad_enabled: true,
            touchpad_desc: String::new(),
            confirming_delete: false,
        }
    }

    /// 从守护进程客户端同步触摸板状态到界面
    pub fn sync_from_client(&mut self, client: &mechrevoc_core::Client) {
        self.touchpad_enabled = client.enabled;
        self.touchpad_desc = client.desc.clone();
    }

    /// 模块名称列表(供侧边栏渲染)
    pub fn view_names(&self) -> &[&'static str] {
        &VIEW_NAMES
    }

    /// 处理按键,返回对应的动作
    pub fn handle_key(&mut self, key: &KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.confirming_delete {
                    self.confirming_delete = false;
                    Action::None
                } else {
                    Action::Quit
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.confirming_delete = false;
                self.selected = (self.selected + 1).min(VIEW_NAMES.len() - 1);
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.confirming_delete = false;
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.activate_selected(),
            KeyCode::Char('r') => Action::RefreshState,
            _ => Action::None,
        }
    }

    /// 激活当前选中的模块(回车/空格)
    fn activate_selected(&mut self) -> Action {
        match self.selected {
            0 => Action::ToggleTouchpad,
            // System 模块:第一次回车进入确认,第二次回车执行删除
            1 => {
                if self.confirming_delete {
                    self.confirming_delete = false;
                    Action::DeleteDaemon
                } else {
                    self.confirming_delete = true;
                    Action::None
                }
            }
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn quit_with_q_or_esc() {
        let mut app = App::new();
        assert!(matches!(
            app.handle_key(&key(KeyCode::Char('q'))),
            Action::Quit
        ));
        let mut app = App::new();
        assert!(matches!(app.handle_key(&key(KeyCode::Esc)), Action::Quit));
    }

    #[test]
    fn navigate_with_jk_and_arrows() {
        let mut app = App::new();
        app.handle_key(&key(KeyCode::Down));
        assert_eq!(app.selected, 1);
        // 到达边界后不再越界
        app.handle_key(&key(KeyCode::Char('j')));
        assert_eq!(app.selected, 1);
        app.handle_key(&key(KeyCode::Up));
        assert_eq!(app.selected, 0);
        // saturating_sub:不越界到负值
        app.handle_key(&key(KeyCode::Char('k')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn touchpad_enter_toggles() {
        let mut app = App::new();
        assert!(matches!(
            app.handle_key(&key(KeyCode::Enter)),
            Action::ToggleTouchpad
        ));
        assert!(matches!(
            app.handle_key(&key(KeyCode::Char(' '))),
            Action::ToggleTouchpad
        ));
    }

    #[test]
    fn system_requires_double_confirm() {
        let mut app = App::new();
        app.selected = 1;
        // 第一次回车进入确认
        assert!(matches!(app.handle_key(&key(KeyCode::Enter)), Action::None));
        assert!(app.confirming_delete);
        // 第二次回车执行删除
        assert!(matches!(
            app.handle_key(&key(KeyCode::Enter)),
            Action::DeleteDaemon
        ));
        assert!(!app.confirming_delete);
    }

    #[test]
    fn esc_cancels_delete_confirmation() {
        let mut app = App::new();
        app.selected = 1;
        app.handle_key(&key(KeyCode::Enter));
        assert!(app.confirming_delete);
        // 确认状态下 Esc 只取消确认,不退出
        assert!(matches!(app.handle_key(&key(KeyCode::Esc)), Action::None));
        assert!(!app.confirming_delete);
        // 再次 Esc 则退出
        assert!(matches!(app.handle_key(&key(KeyCode::Esc)), Action::Quit));
    }

    #[test]
    fn navigating_clears_delete_confirmation() {
        let mut app = App::new();
        app.selected = 1;
        app.handle_key(&key(KeyCode::Enter));
        assert!(app.confirming_delete);
        app.handle_key(&key(KeyCode::Up));
        assert!(!app.confirming_delete);
    }

    #[test]
    fn refresh_state_on_r() {
        let mut app = App::new();
        assert!(matches!(
            app.handle_key(&key(KeyCode::Char('r'))),
            Action::RefreshState
        ));
    }
}
