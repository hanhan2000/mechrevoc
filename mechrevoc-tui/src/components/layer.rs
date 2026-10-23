//! 交互组件层:管理组件区域、鼠标手势与选中高亮。
//!
//! 组件层只处理"鼠标落在哪个组件上、选中了哪些内容",
//! 不依赖具体应用布局——布局由 ui 层声明组件区域后注入。
//!
//! 交互模型:
//! - [`ComponentKind::Button`] - 点击触发动作(由应用层处理),不支持选中
//! - [`ComponentKind::Text`] - 字符级流式拖动选中(类似文本编辑器)

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

use super::kind::ComponentKind;

/// 组件区域:声明一个可交互组件的位置、类型与文本内容
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectableRegion {
    /// 组件 id(模块项 = 0..N-1,内容组件 = [`CONTENT_BASE`](crate::components::CONTENT_BASE)+)
    pub id: u16,
    /// 组件交互类型
    pub kind: ComponentKind,
    /// 组件内容矩形(不含边框)
    pub rect: Rect,
    /// Text 组件的每行文本(字符级选中需要);Button 为空
    pub lines: Vec<String>,
}

/// 鼠标手势状态:区分"点击"与"拖动"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    /// 按下位置 (列, 行)
    pub anchor: Option<(u16, u16)>,
    /// 是否发生过拖动
    pub moved: bool,
}

/// 交互组件层:持有组件区域与选择状态,提供命中/选择/高亮
#[derive(Debug, Default)]
pub struct ComponentLayer {
    /// 当前布局的组件区域(每次 draw 刷新)
    regions: Vec<SelectableRegion>,
    /// Text 字符选区:(所属组件 id, 起始字符偏移, 结束字符偏移)
    char_selection: Option<(u16, usize, usize)>,
    /// 鼠标手势状态
    pub gesture: Selection,
}

impl ComponentLayer {
    pub fn new() -> Self {
        ComponentLayer::default()
    }

    /// 更新组件区域(ui 层每次 draw 时注入)
    pub fn set_regions(&mut self, regions: Vec<SelectableRegion>) {
        self.regions = regions;
    }

    /// 组件区域列表(供渲染层构建高亮)
    pub fn regions(&self) -> &[SelectableRegion] {
        &self.regions
    }

    /// 字符选区是否存在(零长度选区 = 单点点击,不算选择)
    ///
    /// anchor 为按下时的固定锚点,current 随拖动更新;两者不等才算有选区。
    pub fn has_char_selection(&self) -> bool {
        self.char_selection
            .map(|(_, anchor, current)| anchor != current)
            .unwrap_or(false)
    }

    /// 清除全部选区
    pub fn clear_selection(&mut self) {
        self.char_selection = None;
    }

    /// 鼠标按下:记录手势起点;命中 Text 时建立字符选区锚点
    pub fn begin(&mut self, col: u16, row: u16) {
        self.gesture = Selection {
            anchor: Some((col, row)),
            moved: false,
        };
        self.char_selection = self
            .hit_region(col, row)
            .filter(|region| region.kind == ComponentKind::Text)
            .map(|region| {
                let offset = char_offset(region, col, row);
                (region.id, offset, offset)
            });
    }

    /// 鼠标拖动:更新字符选区——**anchor 保持按下时固定不变**,
    /// 只更新 current 到当前位置,规范化(min/max)在消费时进行。
    ///
    /// 这样正向/反向拖动都会从锚点持续扩展选区,不会收缩。
    pub fn drag(&mut self, col: u16, row: u16) {
        self.gesture.moved = true;
        let Some((id, anchor, _)) = self.char_selection else {
            return;
        };
        let Some(region) = self.regions.iter().find(|r| r.id == id) else {
            return;
        };
        let current = char_offset(region, col, row);
        self.char_selection = Some((id, anchor, current));
    }

    /// 鼠标释放:未拖动且命中 Button 时返回该组件 id(由应用层激活)
    pub fn end(&mut self, col: u16, row: u16) -> Option<u16> {
        if self.gesture.moved {
            return None;
        }
        let region = self.hit_region(col, row)?;
        (region.kind == ComponentKind::Button).then_some(region.id)
    }

    /// 命中检测:返回坐标所在的组件区域
    fn hit_region(&self, col: u16, row: u16) -> Option<&SelectableRegion> {
        self.regions.iter().find(|region| {
            let r = region.rect;
            col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
        })
    }

    /// 将 Text 字符选区渲染为高亮:只点亮被选区覆盖的字符段。
    ///
    /// 偏移以"显示列"为单位(全角字符占 2 列),与屏幕 cell 坐标直接对应;
    /// anchor/current 在此处规范化(start ≤ end),支持任意方向拖动。
    pub fn apply_highlight(&self, buf: &mut Buffer, bg: ratatui::style::Color) {
        if !self.has_char_selection() {
            return;
        }
        let Some((id, anchor, current)) = self.char_selection else {
            return;
        };
        let (start, end) = (anchor.min(current), anchor.max(current));
        let Some(region) = self.regions().iter().find(|r| r.id == id) else {
            return;
        };
        let mut offset = 0; // 行累计列偏移
        for (index, line) in region.lines.iter().enumerate() {
            let line_cols = UnicodeWidthStr::width(line.as_str());
            let (line_start, line_end) = (offset, offset + line_cols);
            let (sel_start, sel_end) = (line_start.max(start), line_end.min(end));
            if sel_start < sel_end {
                let col_start = region.rect.x + (sel_start - line_start) as u16;
                let col_end = region.rect.x + (sel_end - line_start) as u16;
                let row = region.rect.y + index as u16;
                for x in col_start..col_end {
                    if let Some(cell) = buf.cell_mut((x, row)) {
                        cell.set_bg(bg);
                    }
                }
            }
            offset = line_end;
        }
    }
}

/// 将鼠标坐标映射为组件文本内的全局"列偏移"(跨行累计)。
///
/// 列偏移 = 字符显示宽度之和(全角字符占 2 列),与屏幕 cell 坐标一致;
/// 行内列超出该行显示宽度时截断到行尾;坐标低于组件区域时按 0 处理。
fn char_offset(region: &SelectableRegion, col: u16, row: u16) -> usize {
    let dy = row.saturating_sub(region.rect.y) as usize;
    let dx = col.saturating_sub(region.rect.x) as usize;
    let mut offset = 0;
    for (index, line) in region.lines.iter().enumerate() {
        let line_cols = UnicodeWidthStr::width(line.as_str());
        if index == dy {
            return offset + dx.min(line_cols);
        }
        offset += line_cols;
    }
    // 拖动到组件下方:视为选中到文本末尾
    offset
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::CONTENT_BASE;

    fn button(id: u16, x: u16, y: u16) -> SelectableRegion {
        SelectableRegion {
            id,
            kind: ComponentKind::Button,
            rect: Rect::new(x, y, 20, 1),
            lines: Vec::new(),
        }
    }

    fn text(id: u16, x: u16, y: u16, lines: &[&str]) -> SelectableRegion {
        SelectableRegion {
            id,
            kind: ComponentKind::Text,
            rect: Rect::new(x, y, 40, lines.len() as u16),
            lines: lines.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// 构造测试组件层:2 个 Button 模块项 + 1 个两行 Text
    fn layer_with_regions() -> ComponentLayer {
        let mut layer = ComponentLayer::new();
        layer.set_regions(vec![
            button(0, 1, 1),
            button(1, 1, 2),
            text(CONTENT_BASE, 23, 1, &["hello", "world"]),
        ]);
        layer
    }

    #[test]
    fn hit_detects_region() {
        let layer = layer_with_regions();
        assert_eq!(layer.hit_region(3, 1).map(|r| r.id), Some(0));
        assert_eq!(layer.hit_region(30, 1).map(|r| r.id), Some(CONTENT_BASE));
        assert_eq!(layer.hit_region(0, 0).map(|r| r.id), None);
    }

    #[test]
    fn char_offset_maps_across_lines() {
        let region = text(CONTENT_BASE, 23, 1, &["hello", "world"]);
        // 第一行第 3 个字符
        assert_eq!(char_offset(&region, 26, 1), 3);
        // 第二行第 2 个字符(偏移 = 5 + 2)
        assert_eq!(char_offset(&region, 25, 2), 7);
        // 行内超出文本长度 → 截断到行尾
        assert_eq!(char_offset(&region, 60, 1), 5);
        // 拖到组件下方 → 文本末尾
        assert_eq!(char_offset(&region, 25, 5), 10);
    }

    #[test]
    fn char_offset_counts_fullwidth_as_two_columns() {
        let region = text(CONTENT_BASE, 23, 1, &["状态: ON"]);
        // 全角字符占 2 列:'状'=0-1,'态'=2-3,': '=4-5,'O'=6,'N'=7
        assert_eq!(char_offset(&region, 23, 1), 0);
        assert_eq!(char_offset(&region, 25, 1), 2); // '态'起始
        assert_eq!(char_offset(&region, 26, 1), 3); // '态'后半格
        assert_eq!(char_offset(&region, 29, 1), 6); // 'O'
        assert_eq!(char_offset(&region, 31, 1), 8); // 行尾
                                                    // 超出行宽 → 截断到行尾
        assert_eq!(char_offset(&region, 40, 1), 8);
    }

    #[test]
    fn drag_selects_char_range_single_line() {
        let mut layer = layer_with_regions();
        layer.begin(24, 1); // "hello" 第 1 个字符
        layer.drag(28, 1); // 第 5 个字符
        assert_eq!(layer.char_selection, Some((CONTENT_BASE, 1, 5)));
    }

    #[test]
    fn drag_selects_char_range_across_lines() {
        let mut layer = layer_with_regions();
        layer.begin(24, 1); // 第一行第 1 个字符(全局偏移 1)
        layer.drag(25, 2); // 第二行第 2 个字符(全局偏移 7)
        assert_eq!(layer.char_selection, Some((CONTENT_BASE, 1, 7)));
    }

    #[test]
    fn drag_backwards_keeps_anchor_and_expands() {
        let mut layer = layer_with_regions();
        layer.begin(28, 1); // anchor offset 5(行尾)
        layer.drag(24, 1); // 向左拖到 offset 1 → 选区覆盖 [1,5]
        assert_eq!(layer.char_selection, Some((CONTENT_BASE, 5, 1)));
        // 继续向左:anchor 保持 5,选区扩展到 [0,5]
        layer.drag(22, 1); // offset 0
        assert_eq!(layer.char_selection, Some((CONTENT_BASE, 5, 0)));
        assert!(layer.has_char_selection());
    }

    #[test]
    fn drag_then_back_to_anchor_clears_selection() {
        let mut layer = layer_with_regions();
        layer.begin(24, 1); // anchor offset 1
        layer.drag(28, 1); // 向右到 offset 5
        assert!(layer.has_char_selection());
        layer.drag(24, 1); // 拖回锚点 offset 1 → 零长度选区
        assert!(!layer.has_char_selection());
    }

    #[test]
    fn begin_on_button_has_no_char_selection() {
        let mut layer = layer_with_regions();
        layer.begin(3, 1); // Button 上按下
        assert!(!layer.has_char_selection());
        layer.drag(3, 2);
        assert!(!layer.has_char_selection());
    }

    #[test]
    fn click_on_button_returns_id() {
        let mut layer = layer_with_regions();
        layer.begin(3, 1);
        assert_eq!(layer.end(3, 1), Some(0));
    }

    #[test]
    fn click_on_text_returns_none_and_no_selection() {
        let mut layer = layer_with_regions();
        layer.begin(25, 1);
        assert_eq!(layer.end(25, 1), None);
        // 单点点击不产生字符选区
        assert!(!layer.has_char_selection());
    }

    #[test]
    fn drag_then_up_returns_none() {
        let mut layer = layer_with_regions();
        layer.begin(3, 1);
        layer.drag(3, 2);
        assert_eq!(layer.end(3, 2), None); // 拖动后不再视为点击
    }
}
