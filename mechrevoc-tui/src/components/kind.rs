//! 组件交互类型。

/// 组件交互类型:区分"可点击按钮"与"可拖动选中文本"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentKind {
    /// 按钮:点击触发动作,不支持拖动选中
    Button,
    /// 文本:可拖动选中高亮,点击不触发动作
    Text,
}
