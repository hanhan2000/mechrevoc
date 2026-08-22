//! 自研交互组件层:在 ratatui 渲染之上构建的组件系统。
//!
//! ratatui 只提供渲染能力(widget → buffer),不处理任何输入。
//! 本模块实现组件交互模型:
//! - [`ComponentKind`] - 组件类型:Button(点击)/ Text(拖动选中)
//! - [`ComponentLayer`] - 组件区域管理 / 命中检测 / 选中高亮
//!
//! 组件层不依赖具体面板布局:ui 层每次 draw 时通过
//! [`ComponentLayer::set_regions`] 注入组件区域。

mod kind;
mod layer;

pub use kind::ComponentKind;
pub use layer::{ComponentLayer, SelectableRegion};

/// 内容组件 id 基址(模块项 id 为 0..N-1,内容组件从本值起)
pub const CONTENT_BASE: u16 = 100;
