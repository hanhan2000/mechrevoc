//! 应用状态(Elm 风格:状态 + 事件处理)。

use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};

use crate::components::{ComponentLayer, CONTENT_BASE};

/// 应用界面中的模块列表
const VIEW_NAMES: [&str; 2] = ["Touchpad", "System"];

/// 温度波形采样点上限(60 点 × 2s 刷新 ≈ 近 2 分钟窗口)
const TEMP_HISTORY_LEN: usize = 60;

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

/// 交互模式:模块导航 ↔ 面板交互
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// 模块导航:j/k 切换模块,Enter 进入面板,q/Esc 退出程序
    Nav,
    /// 面板交互:按键作用于当前面板,Esc 返回模块导航
    Pane,
}

/// 应用全局状态
pub struct State {
    /// 当前交互模式
    pub mode: Mode,
    /// 侧边栏选中的模块下标
    pub selected: usize,
    /// 触摸板开关状态
    pub touchpad_enabled: bool,
    /// 触摸板状态描述(如 "ON, LED off, clicks on")
    pub touchpad_desc: String,
    /// System 模块:系统信息快照(CPU 温度 / 电池)
    pub system_info: mechrevoc_core::SystemInfo,
    /// System 模块:CPU 温度波形历史(环形,最多 [`TEMP_HISTORY_LEN`] 个采样点)
    pub temp_history: VecDeque<f32>,
    /// System 模块:删除 daemon 的二次确认状态
    pub confirming_delete: bool,
    /// 交互组件层:组件区域 / 鼠标手势 / 选中高亮
    pub layer: ComponentLayer,
}

impl State {
    pub fn new() -> Self {
        State {
            mode: Mode::Nav,
            selected: 0,
            touchpad_enabled: true,
            touchpad_desc: String::new(),
            system_info: mechrevoc_core::SystemInfo::default(),
            temp_history: VecDeque::new(),
            confirming_delete: false,
            layer: ComponentLayer::new(),
        }
    }

    /// 记录一次温度采样;无温度数据(如平台不支持)时跳过,避免波形断点
    pub fn record_temp(&mut self) {
        let Some(temp) = self.system_info.cpu_temp_c else {
            return;
        };
        self.temp_history.push_back(temp);
        if self.temp_history.len() > TEMP_HISTORY_LEN {
            self.temp_history.pop_front();
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

    /// 处理按键,按当前交互模式分发
    pub fn handle_key(&mut self, key: &KeyEvent) -> Action {
        match self.mode {
            Mode::Nav => self.handle_nav_key(key),
            Mode::Pane => self.handle_pane_key(key),
        }
    }

    /// 模块导航模式:j/k 选择模块,Enter 进入面板,q/Esc 退出程序
    fn handle_nav_key(&mut self, key: &KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
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
            KeyCode::Enter => {
                self.mode = Mode::Pane;
                Action::None
            }
            _ => Action::None,
        }
    }

    /// 面板交互模式:Esc 返回导航,其余按键分发给当前面板
    fn handle_pane_key(&mut self, key: &KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Nav;
                self.confirming_delete = false;
                Action::None
            }
            _ => match self.selected {
                0 => self.handle_touchpad_key(key),
                _ => self.handle_system_key(key),
            },
        }
    }

    /// Touchpad 面板:Enter/Space 切换开关,r 刷新
    fn handle_touchpad_key(&mut self, key: &KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => Action::ToggleTouchpad,
            KeyCode::Char('r') => Action::RefreshState,
            _ => Action::None,
        }
    }

    /// System 面板:Enter/Space 二次确认删除,r 刷新
    fn handle_system_key(&mut self, key: &KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.confirming_delete {
                    self.confirming_delete = false;
                    Action::DeleteDaemon
                } else {
                    self.confirming_delete = true;
                    Action::None
                }
            }
            KeyCode::Char('r') => Action::RefreshState,
            _ => Action::None,
        }
    }

    /// 处理鼠标事件:组件交互委托给组件层,应用只处理导航滚轮与激活动作。
    pub fn handle_mouse(&mut self, event: &MouseEvent) -> Action {
        match event.kind {
            MouseEventKind::Down(_) => {
                self.layer.begin(event.column, event.row);
                Action::None
            }
            MouseEventKind::Drag(_) => {
                self.layer.drag(event.column, event.row);
                Action::None
            }
            MouseEventKind::Up(_) => {
                // 未拖动且命中 Button 时由组件层返回组件 id
                match self.layer.end(event.column, event.row) {
                    Some(id) => self.activate_component(id),
                    None => Action::None,
                }
            }
            MouseEventKind::ScrollDown => {
                if self.mode == Mode::Nav {
                    self.selected = (self.selected + 1).min(VIEW_NAMES.len() - 1);
                }
                Action::None
            }
            MouseEventKind::ScrollUp => {
                if self.mode == Mode::Nav {
                    self.selected = self.selected.saturating_sub(1);
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    /// 激活 Button 组件:模块项进入面板,内容按钮触发对应动作。
    /// 点击不保留激活高亮(仅拖动选中保留)。
    fn activate_component(&mut self, id: u16) -> Action {
        self.layer.clear_selection();
        // 模块项:切换选中并进入面板
        if (id as usize) < VIEW_NAMES.len() {
            self.selected = id as usize;
            self.mode = Mode::Pane;
            return Action::None;
        }
        match id {
            // Touchpad 切换开关
            id if id == CONTENT_BASE + 1 => Action::ToggleTouchpad,
            // System 删除 daemon(与键盘一致的二次确认)
            id if id == CONTENT_BASE + 3 => {
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
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers, MouseButton};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn quit_with_q_or_esc_in_nav_mode() {
        let mut app = State::new();
        assert!(matches!(
            app.handle_key(&key(KeyCode::Char('q'))),
            Action::Quit
        ));
        let mut app = State::new();
        assert!(matches!(app.handle_key(&key(KeyCode::Esc)), Action::Quit));
    }

    #[test]
    fn navigate_with_jk_and_arrows() {
        let mut app = State::new();
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
    fn enter_enters_pane_mode() {
        let mut app = State::new();
        assert_eq!(app.mode, Mode::Nav);
        app.handle_key(&key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Pane);
    }

    #[test]
    fn nav_mode_enter_does_not_toggle() {
        // 导航模式下 Enter 只进入面板,不触发面板动作
        let mut app = State::new();
        assert!(matches!(app.handle_key(&key(KeyCode::Enter)), Action::None));
        assert_eq!(app.mode, Mode::Pane);
    }

    #[test]
    fn touchpad_pane_enter_toggles() {
        let mut app = State::new();
        app.handle_key(&key(KeyCode::Enter)); // 进入 Touchpad 面板
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
    fn system_pane_requires_double_confirm() {
        let mut app = State::new();
        app.selected = 1;
        app.handle_key(&key(KeyCode::Enter)); // 进入 System 面板
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
    fn esc_returns_to_nav_and_clears_confirmation() {
        let mut app = State::new();
        app.selected = 1;
        app.handle_key(&key(KeyCode::Enter)); // 进入 System 面板
        app.handle_key(&key(KeyCode::Enter)); // 进入确认
        assert!(app.confirming_delete);
        // 面板模式 Esc:返回导航并清除确认
        assert!(matches!(app.handle_key(&key(KeyCode::Esc)), Action::None));
        assert_eq!(app.mode, Mode::Nav);
        assert!(!app.confirming_delete);
    }

    #[test]
    fn pane_mode_esc_does_not_quit() {
        let mut app = State::new();
        app.handle_key(&key(KeyCode::Enter)); // 进入面板
        assert!(matches!(app.handle_key(&key(KeyCode::Esc)), Action::None));
        assert_eq!(app.mode, Mode::Nav);
    }

    #[test]
    fn navigating_clears_delete_confirmation() {
        let mut app = State::new();
        app.selected = 1;
        app.handle_key(&key(KeyCode::Enter)); // 进入 System 面板
        app.handle_key(&key(KeyCode::Enter)); // 进入确认
        assert!(app.confirming_delete);
        app.handle_key(&key(KeyCode::Esc)); // 返回导航
        assert!(!app.confirming_delete);
        app.handle_key(&key(KeyCode::Up));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn refresh_state_on_r_in_pane() {
        let mut app = State::new();
        app.handle_key(&key(KeyCode::Enter)); // 进入面板
        assert!(matches!(
            app.handle_key(&key(KeyCode::Char('r'))),
            Action::RefreshState
        ));
    }

    #[test]
    fn record_temp_appends_and_caps_history() {
        let mut app = State::new();
        app.system_info.cpu_temp_c = Some(52.3);
        for _ in 0..(TEMP_HISTORY_LEN + 5) {
            app.record_temp();
        }
        // 环形上限:超出后裁剪,保留最近 TEMP_HISTORY_LEN 个点
        assert_eq!(app.temp_history.len(), TEMP_HISTORY_LEN);
        assert_eq!(*app.temp_history.front().unwrap(), 52.3);
        assert_eq!(*app.temp_history.back().unwrap(), 52.3);
    }

    #[test]
    fn record_temp_skips_missing_data() {
        let mut app = State::new();
        // cpu_temp_c 为 None(平台不支持)时不记录,避免波形断点
        app.record_temp();
        assert!(app.temp_history.is_empty());
    }

    /// 构造带组件区域的 State:2 个模块项(Button)+ Touchpad 状态行(Text)
    /// + Touchpad 切换(Button)+ System 信息(Text)+ System 删除(Button)
    fn app_with_regions() -> State {
        use crate::components::{ComponentKind, SelectableRegion};
        use ratatui::layout::Rect;

        let mut app = State::new();
        app.layer.set_regions(vec![
            SelectableRegion {
                id: 0,
                kind: ComponentKind::Button,
                rect: Rect::new(1, 1, 20, 1),
                lines: Vec::new(),
            },
            SelectableRegion {
                id: 1,
                kind: ComponentKind::Button,
                rect: Rect::new(1, 2, 20, 1),
                lines: Vec::new(),
            },
            SelectableRegion {
                id: CONTENT_BASE,
                kind: ComponentKind::Text,
                rect: Rect::new(23, 1, 50, 1),
                lines: vec!["状态: ON  (desc)".into()],
            },
            SelectableRegion {
                id: CONTENT_BASE + 1,
                kind: ComponentKind::Button,
                rect: Rect::new(23, 4, 50, 5),
                lines: Vec::new(),
            },
            SelectableRegion {
                id: CONTENT_BASE + 2,
                kind: ComponentKind::Text,
                rect: Rect::new(23, 21, 50, 1),
                lines: vec!["  CPU 温度: 57.0°C".into()],
            },
            SelectableRegion {
                id: CONTENT_BASE + 3,
                kind: ComponentKind::Button,
                rect: Rect::new(23, 23, 50, 3),
                lines: Vec::new(),
            },
        ]);
        app
    }

    #[test]
    fn click_module_item_enters_pane() {
        let mut app = app_with_regions();
        // 点击 System 模块项(Button,row 2),未拖动
        app.handle_mouse(&mouse_down(3, 2));
        app.handle_mouse(&mouse_up(3, 2));
        assert_eq!(app.selected, 1);
        assert_eq!(app.mode, Mode::Pane);
    }

    #[test]
    fn click_touchpad_button_toggles() {
        let mut app = app_with_regions();
        // 点击 Touchpad 切换(Button,row 4-8),未拖动 → 返回切换动作
        app.handle_mouse(&mouse_down(30, 5));
        let action = app.handle_mouse(&mouse_up(30, 5));
        assert!(matches!(action, Action::ToggleTouchpad));
        // 点击不保留激活高亮
        assert!(!app.layer.has_char_selection());
    }

    #[test]
    fn click_text_does_not_activate() {
        let mut app = app_with_regions();
        // 点击 Text(状态行),未拖动 → 无动作
        app.handle_mouse(&mouse_down(30, 1));
        let action = app.handle_mouse(&mouse_up(30, 1));
        assert!(matches!(action, Action::None));
        assert_eq!(app.mode, Mode::Nav);
        // 单点点击不产生字符选区
        assert!(!app.layer.has_char_selection());
    }

    #[test]
    fn click_system_delete_button_double_confirm() {
        let mut app = app_with_regions();
        // System 删除(Button,row 23-25)第一次点击进入确认
        app.handle_mouse(&mouse_down(30, 24));
        let first = app.handle_mouse(&mouse_up(30, 24));
        assert!(matches!(first, Action::None));
        assert!(app.confirming_delete);
        // 第二次点击执行删除
        app.handle_mouse(&mouse_down(30, 24));
        let second = app.handle_mouse(&mouse_up(30, 24));
        assert!(matches!(second, Action::DeleteDaemon));
        assert!(!app.confirming_delete);
    }

    #[test]
    fn drag_over_module_buttons_does_not_select() {
        let mut app = app_with_regions();
        // Button 不支持拖动选中:从 Touchpad(row 1)拖到 System(row 2)
        app.handle_mouse(&mouse_down(3, 1));
        app.handle_mouse(&mouse_drag(3, 2));
        app.handle_mouse(&mouse_up(3, 2));
        assert!(!app.layer.has_char_selection());
        assert_eq!(app.mode, Mode::Nav);
    }

    #[test]
    fn drag_in_text_creates_char_selection() {
        let mut app = app_with_regions();
        // 状态行(Text)内拖动 → 建立字符选区
        app.handle_mouse(&mouse_down(24, 1));
        app.handle_mouse(&mouse_drag(28, 1));
        assert!(app.layer.has_char_selection());
    }

    #[test]
    fn drag_starting_on_button_does_not_select_text() {
        let mut app = app_with_regions();
        // 从 Button 开始拖动:不启动 Text 字符选择
        app.handle_mouse(&mouse_down(3, 1));
        app.handle_mouse(&mouse_drag(30, 1));
        assert!(!app.layer.has_char_selection());
    }

    #[test]
    fn scroll_changes_module_in_nav_only() {
        let mut app = State::new();
        app.handle_mouse(&scroll(MouseEventKind::ScrollDown));
        assert_eq!(app.selected, 1);
        app.handle_mouse(&scroll(MouseEventKind::ScrollUp));
        assert_eq!(app.selected, 0);
        // Pane 模式滚轮不切模块
        app.handle_key(&key(KeyCode::Enter));
        app.handle_mouse(&scroll(MouseEventKind::ScrollDown));
        assert_eq!(app.selected, 0);
    }

    fn mouse_down(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    fn mouse_drag(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    fn mouse_up(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    fn scroll(kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }
}
