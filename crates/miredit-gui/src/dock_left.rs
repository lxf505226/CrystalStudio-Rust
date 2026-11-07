//! 左停靠区（宽 156）：工具 / 工具选项 / 图层 / 单元格检查器 四个面板。
//!
//! 全部尺寸来自 [`crate::metrics`]，全部颜色来自 [`crate::theme`]，
//! **全部文案来自 [`crate::ui_text`]**（本文件不再出现界面字面量）。
//!
//! # 工具选项面板：胶囊与滑块都是**真值**（2026-11-04 审计 P0-5 收口）
//!
//! - 胶囊文字 = 当前工具 + 当前笔刷尺寸（`7×7` / `填充`，官方 `MainWindow.axaml.cs:932` 的三分支），
//!   由 [`crate::screen_text::pill_text`] 装配，**不是写死的 `3×3`**；
//! - 金色滑块的位置 = 当前尺寸在 3..=31 里的比例（[`thumb_rect`]），尺寸 3 时正好落在
//!   参考截图那个位置，所以视觉基准不变；
//! - 拖拽改尺寸**尚未接线**（尺寸走 `[` / `]` 与 `--brush-size`）；滑块停在哪必须是真的，
//!   否则就是一块会骗人的装饰。
//!
//! ⚠ 两处**与任务书不符、以截图为准**（已在汇报 §六 显式列明）：
//! 1. 工具网格实为 **4 列 × 3 行（末行 2 个）**，不是 156 宽下放不下的 5 列；
//! 2. 图层列表有 **5 个图层行 + 1 个 `单元格高亮` 子组（2 行复选框）**，
//!    而不是「6 行」——截图里前 5 行是图层，第 6 行起是子组标题。
//!
//! # 接线情况（**五行全部接线**，2026-11-04 审计 P0-9 收口）
//!
//! | 行 | 名称 | 可见性开关 | **当前编辑层** |
//! |---|---|---|---|
//! | 0 | 地表层 | `LayerMask::ground` ✅ | ✅ 可选中（写 `bk_img` + `btTiles`）|
//! | 1 | 中间层 | `LayerMask::middle` ✅ | ✅ 可选中（写 `mid_img` + `btsmTiles`）|
//! | 2 | 物体 | `LayerMask::front` ✅ | ✅ 可选中（写 `fr_img` + `btArea`）|
//! | 3 | 低位阻挡 | `CollisionOverlay::low` ✅ | ✅ 可选中（**只改** `bk_img` 的 `bit15`）|
//! | 4 | 高位阻挡 | `CollisionOverlay::high` ✅ | ✅ 可选中（**只改** `fr_img` 的 `bit15`）|
//!
//! 后两行的可见性**不走 `LayerMask`**：`miredit_render::LayerMask` 只有
//! `ground/middle/front` 三个开关（那是"三层精灵"），阻挡标记是**独立通道**
//! [`miredit_render::CollisionOverlay`]（`low` / `high` 各自与判定相与）。
//! 两套开关在界面上长得一样（都是"眼睛"），所以这里用一组函数把两者**统一成"第 i 行"**
//! （[`layer_visible`] / [`toggle_visibility`]）—— 调用方永远只跟行号打交道，
//! 不必知道哪一行归哪套结构管。
//!
//! 上一版这两行是**置灰 + 眼睛永远带斜杠 + 点了没反应**（因为当时渲染库没有阻挡通道）。
//! 现在通道有了（`miredit-render` 的 `try_render_region_with`），置灰必须取消 ——
//! 永远置灰的行会让人以为"这两行天生不可用"，而不是"现在没开"。
//!
//! # 两个"选中"是**互不影响**的两件事
//!
//! - **眼睛（勾选框）**：管该层**画不画**（`LayerMask` / `CollisionOverlay`），
//!   点眼睛不改变当前编辑层；
//! - **行（金框 + 左侧金条）**：管**写到哪一层**（[`crate::edit::EditLayer`]），
//!   点行不改变可见性。选中的行用 `ACCENT_DARK` 金框 + `ACCENT_SOFT` 浅金底（复用
//!   `theme` 现有色），再加一根 [`m::LAYER_EDIT_BAR_W`] 宽的金条 —— 1px 金框在 1500×950
//!   截图里与眼睛图标糊在一起，金条是"当前编辑层"的可辨标识。

use egui::{pos2, vec2, Align2, Color32, FontId, Painter, Rect};
use miredit_render::{CollisionOverlay, LayerMask};

use crate::edit::EditLayer;
use crate::layout::Layout;
use crate::metrics as m;
use crate::paint as pt;
use crate::panel;
use crate::screen_text::DocText;
use crate::text_fit::{DynLabel, Label, LabelStyle};
use crate::theme as t;
use crate::ui_text as tx;

/// 图层行总数（五行全部接线）。
pub const LAYER_ROWS: usize = 5;

/// 图层行里第 `i` 行是否已经接线（**五行都接了**；越界为 `false`）。
///
/// 保留这个函数而不是删掉：界面绘制与验收都要"按行问一句"，而"接线"这件事
/// 将来还可能退化（例如某一行被临时摘掉）—— 有一个唯一判据比散落的 `i < 3` 好。
pub const fn layer_wired(i: usize) -> bool {
    i < LAYER_ROWS
}

/// 第 `i` 行的**可见性**取值；越界返回 `None`。
///
/// 前三行读 [`LayerMask`]（三层精灵），后两行读 [`CollisionOverlay`]（阻挡标记通道）。
pub fn layer_visible(mask: &LayerMask, overlay: &CollisionOverlay, i: usize) -> Option<bool> {
    match i {
        0 => Some(mask.ground),
        1 => Some(mask.middle),
        2 => Some(mask.front),
        3 => Some(overlay.low),
        4 => Some(overlay.high),
        _ => None,
    }
}

/// 切换第 `i` 行的可见性；越界返回 `false`（什么都没做）。
///
/// 前三行动 `LayerMask`（画布重渲靠它），后两行动 `CollisionOverlay`
/// （重渲靠缓存键里的 overlay —— 见 [`crate::canvas_view::CanvasView`]）。
pub fn toggle_visibility(
    mask: &mut LayerMask,
    overlay: &mut CollisionOverlay,
    i: usize,
) -> bool {
    match i {
        0 => {
            mask.ground = !mask.ground;
            true
        }
        1 => {
            mask.middle = !mask.middle;
            true
        }
        2 => {
            mask.front = !mask.front;
            true
        }
        3 => {
            overlay.low = !overlay.low;
            true
        }
        4 => {
            overlay.high = !overlay.high;
            true
        }
        _ => false,
    }
}

/// 点一下图层行该做什么（**纯函数**：点在眼睛上由调用方判定后传进来）。
///
/// 两个动作互相独立（见模块文档）：眼睛管"画不画"，行管"写到哪一层"。
/// 点眼睛**不会**顺手改当前编辑层；点行也**不会**改可见性 —— 这条在界面上很难用截图证明，
/// 所以在类型上分开（[`LayerClick`]），并由单元测试逐条钉死。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerClick {
    /// 点的是眼睛 ⇒ 只切该行可见性。
    ToggleVisibility(usize),
    /// 点的是行内其它位置 ⇒ 把该行设为**当前编辑层**。
    SetEditLayer(EditLayer),
    /// 越界行（`row >= 5`）⇒ 什么都不做。
    Ignore,
}

/// 图层行点击分派：`on_eye` = 点中的是不是眼睛按钮。
pub fn dispatch_click(row: usize, on_eye: bool) -> LayerClick {
    if row >= LAYER_ROWS {
        return LayerClick::Ignore;
    }
    if on_eye {
        return LayerClick::ToggleVisibility(row);
    }
    match EditLayer::from_row(row) {
        Some(layer) => LayerClick::SetEditLayer(layer),
        None => LayerClick::Ignore,
    }
}

/// 第 `i` 行「眼睛」图标的矩形（16×16，命中测试与绘制共用）。
pub fn eye_rect(l: &Layout, i: usize) -> Rect {
    let row = l.layer_rows[i];
    Rect::from_center_size(
        pos2(
            row.left() + m::LAYER_EYE_INSET + m::LAYER_EYE_BTN_W * 0.5,
            row.center().y,
        ),
        vec2(m::ICON_EYE, m::ICON_EYE),
    )
}

/// 第 `i` 行「当前编辑层」左侧金条的矩形（绘制与验收共用）。
///
/// 压在 1px 金框内沿：`LAYER_EDIT_BAR_INSET` 起、宽 `LAYER_EDIT_BAR_W`、
/// 高 = 行高 - 上下各一个内沿，保证**整块落在行内**（不会被行边界裁掉一半）。
pub fn edit_bar_rect(l: &Layout, i: usize) -> Rect {
    let row = l.layer_rows[i];
    Rect::from_min_size(
        pos2(
            row.left() + m::LAYER_EDIT_BAR_INSET,
            row.top() + m::LAYER_EDIT_BAR_INSET,
        ),
        vec2(
            m::LAYER_EDIT_BAR_W,
            row.height() - 2.0 * m::LAYER_EDIT_BAR_INSET,
        ),
    )
}

/// 画整个左停靠区。
///
/// `vis` = 7 个面板的显隐（视图菜单的开关）。**隐藏的面板既不画也不响应点击**
/// （点击的闸门在 [`crate::app`] 的 `interact` 里，两边用同一个判据 `PanelVis::get`）。
// 8 个参数是"画一整块停靠区"所需的最小输入集（painter + 布局 + 工具 + 编辑层 +
// 两个可见性通道 + 文案 + 面板显隐）；打成一个结构体只是把参数搬个家，不减少耦合，
// 所以这里直接 `allow`。
// ⚠ 别再拿 `text_fit::DynLabel::new` 当同类先例 —— 它 2026-11-05 已改成 4 参
// （排版五项收进 `text_fit::LabelStyle`），不再需要 `allow`。
#[allow(clippy::too_many_arguments)]
pub fn draw(
    p: &Painter,
    l: &Layout,
    selected_tool: usize,
    edit_layer: EditLayer,
    mask: &LayerMask,
    overlay: &CollisionOverlay,
    d: &DocText,
    vis: &crate::menu::panels::PanelVis,
    // 裁剪按钮这一帧是否可用（`有图 + 有选区`；判据由 `app` 给，`dock_left` 只负责画）。
    crop_enabled: bool,
) {
    use crate::menu::panels::PanelId;

    // 本帧的静态标签只算一次，各子面板按 id 前缀取用
    let labs = labels(p, l);

    if vis.get(PanelId::Tools) {
        panel::draw_panel(p, &l.p_tools, tx::TITLE_TOOLS, "panel.tools", t::ui().panel_bg);
        draw_tools(p, l, selected_tool, crop_enabled);
    }

    if vis.get(PanelId::ToolOptions) {
        panel::draw_panel(
            p,
            &l.p_tool_options,
            tx::TITLE_TOOL_OPTIONS,
            "panel.tool_options",
            t::ui().panel_bg,
        );
        draw_tool_options(p, l, d);
        draw_dyn_sel(p, l, d, "tool_options.");
    }

    if vis.get(PanelId::Layers) {
        panel::draw_panel(p, &l.p_layers, tx::TITLE_LAYERS, "panel.layers", t::ui().panel_bg);
        draw_layers(p, l, edit_layer, mask, overlay, &labs);
        // 图层行的运行期标记（地表「不可落笔」）—— 与静态层名分开画，静态表一个字都不改
        draw_dyn_sel(p, l, d, "layers.");
    }

    if vis.get(PanelId::CellInspector) {
        panel::draw_panel(
            p,
            &l.p_cell_inspector,
            tx::TITLE_CELL_INSPECTOR,
            "panel.cell_inspector",
            t::ui().panel_bg,
        );
        draw_inspector(p, l, d, &labs);
    }
}

/// 按 id 前缀取标签并绘制。
fn draw_sel(p: &Painter, labs: &[Label], prefix: &str) {
    for lab in labs.iter().filter(|x| x.name.starts_with(prefix)) {
        lab.draw(p);
    }
}

/// 按 id 前缀取**动态**标签（实测截断后）并绘制。
fn draw_dyn_sel(p: &Painter, l: &Layout, d: &DocText, prefix: &str) {
    for mut lab in dyn_labels(p, l, d)
        .into_iter()
        .filter(|x| x.name.starts_with(prefix))
    {
        lab.truncate_to_fit(p);
        lab.draw(p);
    }
}

// ───────────────────────── 标签表 ─────────────────────────

/// 画笔胶囊的**外框尺寸 = 上游算式**（A17，2026-11-08；推导见 [`m::PILL_MIN_W`]）。
///
/// 上游 `CrystalStudio/MainWindow.axaml:6`：
/// `<Border CornerRadius="4" Padding="5,2"><TextBlock x:Name="BrushSizeLabel"
///   Text="3×3" MinWidth="35" TextAlignment="Center" FontWeight="SemiBold" /></Border>`
/// ⇒ 宽 = `max(MinWidth 35, 文字宽) + Padding 5×2`；高 = `文字高 + Padding 2×2`。
///
/// 为什么拆成一个**不吃 `Painter` 的纯函数**：这样「算式对不对」可以用任意输入直接断言，
/// 与「本仓字体到底多宽」（[`pill_text_size`] 的 `layout_no_wrap` 实测）分开验 ——
/// 两条都不许靠估算。
fn pill_size(文字: egui::Vec2) -> egui::Vec2 {
    vec2(
        // `MinWidth` 在内层 TextBlock 上 ⇒ 先夹文字宽，再加外层 Border 的左右 padding。
        文字.x.max(m::PILL_MIN_W) + m::PILL_PAD_X * 2.0,
        文字.y + m::PILL_PAD_Y * 2.0,
    )
}

/// 胶囊里文字的**上屏实测尺寸**（宽 = advance、高 = 行高），用本仓实际上屏字体量。
///
/// 与 [`crate::paint::text_w`] 同一口径（`layout_no_wrap`，只排版不进绘制队列），
/// 只是这里连**高**一起要（上游算式的高度项要用它）。
/// 上游 Avalonia + Segoe UI 的字体度量本仓拿不到 ⇒ 按任务书口径取本仓实测值。
fn pill_text_size(p: &Painter, text: &str) -> egui::Vec2 {
    p.layout_no_wrap(
        text.to_owned(),
        FontId::proportional(m::BODY_FONT),
        Color32::WHITE,
    )
    .size()
}

/// 画笔胶囊的矩形（绘制与验收共用）。
///
/// **尺寸按上游算式现算**（宽 `max(35, 文字宽)+10`、高 `文字高+4`），
/// 不再用固定 45×22 —— 原值「截图实测 `x92..136` 宽 44~45、高 22」保留作历史引用，
/// 2026-11-08 按上游 `MainWindow.axaml:6` 改为动态。
/// `PILL_X`（左偏移 84）**不参与算式**，仍是布局偏移。
fn pill_rect(p: &Painter, l: &Layout, d: &DocText) -> Rect {
    let c = l.p_tool_options.content;
    let 尺寸 = pill_size(pill_text_size(p, &d.pill));
    Rect::from_min_size(
        pos2(c.left() + m::PILL_X, c.center().y - 尺寸.y * 0.5),
        vec2(尺寸.x, 尺寸.y),
    )
}

/// 滑杆滑块（金色块）的矩形：横向位置 = 当前笔刷尺寸在 3..=31 里的比例。
///
/// 轨道 `x = SLIDER_X .. SLIDER_X + SLIDER_W`（`metrics` 里量自截图），
/// 滑块宽 `SWATCH_W`，可走行程 = `SLIDER_W - SWATCH_W`；尺寸 3 落在最左（= 参考截图那个位置），
/// 31 落在最右。**纯几何，不做交互**：本版拖拽未接线（尺寸走 `[` / `]` 与 `--brush-size`），
/// 但"滑块停在哪"必须是真的 —— 否则就是一块会骗人的装饰。
pub fn thumb_rect(l: &Layout, d: &DocText) -> Rect {
    let c = l.p_tool_options.content;
    let span = (m::BRUSH_MAX - m::BRUSH_MIN).max(1) as f32;
    let t = ((d.brush_size.clamp(m::BRUSH_MIN, m::BRUSH_MAX) - m::BRUSH_MIN) as f32 / span)
        .clamp(0.0, 1.0);
    let travel = (m::SLIDER_W - m::SWATCH_W).max(0.0);
    Rect::from_min_size(
        pos2(
            c.left() + m::SLIDER_X + t * travel,
            c.center().y - m::SWATCH_H * 0.5,
        ),
        vec2(m::SWATCH_W, m::SWATCH_H),
    )
}

/// 图层行文字的左边（碰撞两行还要再右移 `LAYER_MARK_TEXT_OFF`）。
fn layer_text_x(l: &Layout, i: usize) -> f32 {
    let row = l.layer_rows[i];
    let base = row.left() + m::LAYER_EYE_INSET + m::LAYER_EYE_BTN_W + m::LAYER_TEXT_GAP;
    if i >= 3 {
        base + m::LAYER_MARK_TEXT_OFF
    } else {
        base
    }
}

/// 键列裁剪盒（截图里英文 `Tile Animati` 就是被这条线硬裁的；中文放得下就不用裁）。
///
/// 纵向按行切：每条键只占自己那一行，避免相邻行的字混进同一条验收带。
fn inspector_key_clip(l: &Layout, y: f32) -> Rect {
    let c = l.p_cell_inspector.content;
    Rect::from_min_max(
        pos2(c.left() + m::INSPECTOR_KEY_X, y),
        pos2(
            c.left() + m::INSPECTOR_VAL_X - m::INSPECTOR_KEY_CLIP_PAD,
            y + m::INSPECTOR_LINE_H,
        ),
    )
}

/// 单元格检查器面板里键列的起点与行高（动态值的排版与静态键共用这一份计算）。
fn inspector_layout(l: &Layout) -> (f32, f32, f32, Rect) {
    let c = l.p_cell_inspector.content;
    (
        c.left() + m::INSPECTOR_KEY_X, // 键列 / 标题的 x
        c.left() + m::INSPECTOR_VAL_X, // 值列的 x
        c.top() + m::INSPECTOR_HEAD_OFF,
        c,
    )
}

/// 左停靠区全部**静态**文字标签（验收用，绘制也用）。
pub fn labels(_p: &Painter, l: &Layout) -> Vec<Label> {
    let mut out = Vec::with_capacity(32);

    // ① 四个面板标题
    out.push(panel::title_label(&l.p_tools, tx::TITLE_TOOLS, "panel.tools"));
    out.push(panel::title_label(
        &l.p_tool_options,
        tx::TITLE_TOOL_OPTIONS,
        "panel.tool_options",
    ));
    out.push(panel::title_label(&l.p_layers, tx::TITLE_LAYERS, "panel.layers"));
    out.push(panel::title_label(
        &l.p_cell_inspector,
        tx::TITLE_CELL_INSPECTOR,
        "panel.cell_inspector",
    ));

    // ② 工具选项胶囊 —— **不在静态表里**：它的文字是运行期数据（尺寸 / 工具），
    //    见 [`dyn_labels`] 的 `tool_options.pill`（上一版这里写死 `3×3`，改尺寸胶囊不动）。

    // ③ 五个图层行（五行都接线 ⇒ 都是正文色；"没开"由眼睛的斜杠表达，不靠置灰）
    let row_right = l.layer_rows[0].right();
    for (i, row) in l.layer_rows.iter().enumerate() {
        let x = layer_text_x(l, i);
        out.push(Label {
            name: ["layer.0", "layer.1", "layer.2", "layer.3", "layer.4"][i],
            text: tx::LAYER_NAMES[i],
            font: m::BODY_FONT,
            pos: pos2(x, row.center().y),
            anchor: Align2::LEFT_CENTER,
            clip: Rect::from_min_max(pos2(x, row.top()), pos2(row_right, row.bottom())),
            // ⚠ 不再按 `layer_wired` 置灰：五行都接了，永远灰着会让人以为"这两行不可用"
            color: t::ui().text,
        });
    }

    // ④ 子组标题
    let label_y = l.layer_group_sep.bottom() + m::LAYER_GROUP_TITLE_OFF;
    out.push(Label {
        name: "layers.group_title",
        text: tx::GROUP_CELL_HIGHLIGHTS,
        font: m::LAYER_GROUP_TITLE_FONT,
        pos: pos2(l.layer_rows[0].left(), label_y),
        anchor: Align2::LEFT_TOP,
        clip: Rect::from_min_max(
            pos2(l.layer_rows[0].left(), label_y),
            pos2(
                row_right,
                label_y + m::LAYER_GROUP_LABEL_H - m::LAYER_GROUP_TITLE_OFF,
            ),
        ),
        color: t::ui().text,
    });

    // ⑤ 两个复选框标签
    for (i, row) in l.layer_sub_rows.iter().enumerate() {
        let x = row.left() + m::LAYER_CB_TEXT_X;
        out.push(Label {
            name: ["layers.light_cells", "layers.door_cells"][i],
            text: tx::HIGHLIGHT_NAMES[i],
            font: m::BODY_FONT,
            pos: pos2(x, row.center().y),
            anchor: Align2::LEFT_CENTER,
            clip: Rect::from_min_max(pos2(x, row.top()), pos2(row_right, row.bottom())),
            color: t::ui().text,
        });
    }

    // ⑥ 单元格检查器的键列与值列都在 dyn_labels 里画 ——
    //    原因：键的纵向位置由「该行值有几行」决定（见 value_row_spans），
    //    而"值有几行"是运行期数据；若把键留在静态表里，换一张图就可能键值错行。
    out
}

/// 左停靠区全部**动态**文字标签（单元格检查器面板：标题行 + 六个键 + 六行值）。
///
/// 为什么键也算"动态"：键的纵向位置由「该行值有几行」决定（[`value_row_spans`]），
/// 而"值有几行"是运行期数据。键的**文字**是静态常量（`tmpl` 传自身即可，仍在
/// `ui_text::ALL` 里，字形与来源检查一样覆盖得到），但它的**位置**不是静态的。
pub fn dyn_labels(p: &Painter, l: &Layout, d: &DocText) -> Vec<DynLabel> {
    let (key_x, val_x, head_y, c) = inspector_layout(l);
    let mut out = Vec::with_capacity(18);

    // ⓪a **地表图层行的「不可落笔」标记**（2026-11-05 九格式收口；九种里只有 Type5 会挂）。
    //
    //     为什么另挂一条动态文字，而不是改 `tx::LAYER_NAMES[0]`：那张表是**官方文案**
    //     （`GroundTiles_3931E191`），五行共用，改它会连带改掉状态栏的「当前层：地表图层」，
    //     而且别的格式根本没有这个限制 —— 一个会随文档变的标记不属于静态表。
    //
    //     位置：行右端右对齐（行内左侧是眼睛 + 层名，右侧是空的），
    //     颜色用告警深褐（与黄条同族），字号比层名小一档，不抢主文案。
    if d.ground_locked {
        let row = l.layer_rows[0];
        out.push(DynLabel::new(
            "layers.ground_locked",
            tx::GROUND_LOCKED_MARK,
            tx::GROUND_LOCKED_MARK.to_owned(),
            LabelStyle::new(
                m::LAYER_GROUP_TITLE_FONT,
                pos2(row.right() - 2.0, row.center().y),
                Align2::RIGHT_CENTER,
                Rect::from_min_max(pos2(row.center().x, row.top()), pos2(row.right(), row.bottom())),
                t::ui().warn_banner_text,
            ),
        ));
    }

    // ⓪ 工具选项胶囊：`7×7` / `填充`（**运行期数据**，见 [`crate::screen_text::pill_text`]）
    //
    //    胶囊外框**按上游算式现算**（[`pill_rect`]）：宽 `max(35, 文字宽)+10`、高 `文字高+4`
    //    —— 上游 `MainWindow.axaml:6` 里没有固定宽高，45×22 是截图里「内容撑出来」的结果。
    let pill = pill_rect(p, l, d);
    out.push(DynLabel::new(
        "tool_options.pill",
        tx::PILL_SIZE_TMPL,
        d.pill.clone(),
        LabelStyle::new(
            m::BODY_FONT,
            pill.center(),
            Align2::CENTER_CENTER,
            pill,
            t::ui().text,
        ),
    ));

    // ① 标题行：`单元格 x,y · N 个物体`（没悬停时是 `—`）
    out.push(DynLabel::new(
        "inspector.head",
        tx::INSPECTOR_HEAD_TMPL,
        d.inspector_head.clone(),
        LabelStyle::new(
            m::BODY_FONT,
            pos2(key_x, head_y),
            Align2::LEFT_TOP,
            Rect::from_min_max(
            pos2(key_x, head_y),
            pos2(c.right(), head_y + m::INSPECTOR_LINE_H + 3.0),
        ),
            t::ui().text,
        ),
    ));

    // ② 六个键 + 六条值：逐行铺，行 y 全部来自 value_row_spans（键值同源，不会错行）
    let key_names = [
        "inspector.key.collision",
        "inspector.key.ground",
        "inspector.key.middle",
        "inspector.key.objects",
        "inspector.key.metadata",
        "inspector.key.tile_animation",
    ];
    let val_names = [
        "inspector.val.collision",
        "inspector.val.ground",
        "inspector.val.middle",
        "inspector.val.objects",
        "inspector.val.metadata",
        "inspector.val.tile_animation",
    ];
    let tmpls = [
        tx::VAL_COLLISION_TMPL,
        tx::VAL_GROUND_TMPL,
        tx::VAL_MIDDLE_TMPL,
        tx::VAL_OBJECTS_TMPL,
        tx::VAL_METADATA_TMPL,
        tx::VAL_TILE_ANIMATION_TMPL,
    ];
    for (i, (y, _n)) in value_row_spans(l, d).into_iter().enumerate() {
        // 键（只占第一行的位置）
        let key = tx::INSPECTOR_KEYS[i];
        out.push(DynLabel::new(
            key_names[i],
            key,
            key.to_string(),
            LabelStyle::new(
                m::INSPECTOR_FONT,
                pos2(key_x, y),
                Align2::LEFT_TOP,
                inspector_key_clip(l, y),
                t::ui().text,
            ),
        ));
        // 值（可能多行）
        for (j, line) in d.inspector_values[i].split('\n').enumerate() {
            let ly = y + j as f32 * m::INSPECTOR_LINE_H;
            out.push(DynLabel::new(
                val_names[i],
                tmpls[i],
                line.to_string(),
                LabelStyle::new(
                    m::INSPECTOR_FONT,
                    pos2(val_x, ly),
                    Align2::LEFT_TOP,
                    Rect::from_min_max(pos2(val_x, ly), pos2(c.right(), ly + m::INSPECTOR_LINE_H)),
                    t::ui().text,
                ),
            ));
        }
    }

    out
}

/// 六行值每行占的纵向区间：`(起始 y, 行数)`。
///
/// 键列与值列**必须共用**这套推进，否则一旦某条值从 2 行变 3 行（比如换成 Ext36 图），
/// 键就会与值错行 —— 这是上一版静态值靠"人工数行数"埋下的隐患，本单收口成一处。
pub fn value_row_spans(l: &Layout, d: &DocText) -> Vec<(f32, usize)> {
    let (_, _, head_y, _) = inspector_layout(l);
    let mut out = Vec::with_capacity(6);
    let mut y = head_y + m::INSPECTOR_LINE_H + m::INSPECTOR_ROW_GAP;
    for v in &d.inspector_values {
        let n = v.split('\n').count().max(1);
        out.push((y, n));
        y += n as f32 * m::INSPECTOR_LINE_H + m::INSPECTOR_ROW_GAP;
    }
    out
}

// ───────────────────────── Tools ─────────────────────────

fn draw_tools(p: &Painter, l: &Layout, selected: usize, crop_enabled: bool) {
    for (i, btn) in l.tool_buttons.iter().enumerate() {
        let on = i == selected;
        if on {
            // 选中态：1px 金色描边（截图 (15,97)-(46,128) 的金色框）
            // 用「先填圆角外圈、再填内圈」实现圆角描边，避免 rect_stroke 的版本差异
            pt::fill_round(p, *btn, 5.0, t::ui().accent_dark);
            pt::fill_round(p, btn.shrink(1.0), 4.0, t::ui().accent_soft);
        }
        // 图标显示区：以按钮中心为中心，边长取 JSON 的 icon.box（16 或 18）
        let side = crate::icons::box_size_of(i);
        let icon_rect = Rect::from_center_size(btn.center(), vec2(side, side));
        let color = if on { t::ui().accent_dark } else { t::ui().text };
        crate::icons::paint_tool_icon(p, i, icon_rect, color);
    }

    // **裁剪按钮**（A-015 / R54）：**不是工具槽 ⇒ 没有选中态**；可用 = 强调色 + 浅底，
    // 禁用 = 灰色图标（悬停由 `app::interact` 用 `on_disabled_hover_text` 说明原因）。
    let crop = l.crop_button;
    let pal = t::ui();
    let 图标 = Rect::from_center_size(crop.center(), vec2(16.0, 16.0));
    if crop_enabled {
        pt::fill_round(p, crop, 5.0, pal.accent_soft);
        crate::icons::paint_crop_icon(p, 图标, pal.accent_dark);
    } else {
        crate::icons::paint_crop_icon(p, 图标, pal.text_muted);
    }
}

// ───────────────────────── Tool Options ─────────────────────────

fn draw_tool_options(p: &Painter, l: &Layout, d: &DocText) {
    let c = l.p_tool_options.content;
    let cy = c.center().y;

    // ① 滑杆滑块（金色，12×16）。截图 (14,271)-(25,288) 的 `#AD7F22` 像素簇。
    // 参考截图里这一块同时充当 slider 的 thumb（JSON `slider_thumb_width=12` /
    // `slider_thumb_height=18` 与它 12×16 吻合），本单按任务书 §三 称它「色块」，
    // 不再另画一个灰 thumb（早期版本多画一块灰 thumb，实测会与色块粘成一条双色条）。
    //
    // **本版起它的横向位置是真实数据**：按当前笔刷尺寸在 3..=31 里的位置摆放
    // （官方滑块 `Minimum=3 Maximum=31`，`MainWindow.axaml:6`）。
    // 尺寸 = 3（缺省）时它正好落在参考截图那个位置（左端），所以视觉基准不变。
    let swatch = thumb_rect(l, d);
    pt::fill_round(p, swatch, 2.0, t::ui().color_swatch);

    // ② 滑杆轨道 64×3（截图 y=280 行 `x26..90` 为 `#E3E6EA`）
    let track = Rect::from_min_size(
        pos2(c.left() + m::SLIDER_X, cy - m::SLIDER_TRACK_H * 0.5),
        vec2(m::SLIDER_W, m::SLIDER_TRACK_H),
    );
    pt::fill(p, track, t::ui().panel_header);

    // ③ 胶囊底（上游 `MainWindow.axaml:6`：`CornerRadius="4" Padding="5,2"` + `MinWidth="35"`）；
    //    尺寸**按算式现算**（[`pill_rect`]），原「45×22（截图 y=280 行 `x92..136` 为 `#ECEEF0`）」
    //    保留作历史引用 —— 2026-11-08 按上游算式改为动态。
    //    胶囊里的字是动态的，由 `draw_dyn_sel(p, l, d, "tool_options.")` 画。
    let pill = pill_rect(p, l, d);
    pt::fill_round(p, pill, m::PILL_RADIUS, t::ui().input_bg);
}

// ───────────────────────── Layers ─────────────────────────

fn draw_layers(
    p: &Painter,
    l: &Layout,
    edit_layer: EditLayer,
    mask: &LayerMask,
    overlay: &CollisionOverlay,
    labs: &[Label],
) {
    for (i, row) in l.layer_rows.iter().enumerate() {
        // **当前编辑层**的选中态：五行都能当编辑层（含低位/高位阻挡）
        let on = EditLayer::from_row(i) == Some(edit_layer);
        if on {
            // 金框（1px）+ 浅金底：与左侧工具按钮选中态同一套颜色（`theme` 现有色）
            pt::fill_round(p, *row, 4.0, t::ui().accent_dark);
            pt::fill_round(p, row.shrink(1.0), 3.0, t::ui().accent_soft);
            // 左侧金条：3px，压在金框内沿 —— "写哪一层"的可辨标识
            pt::fill(p, edit_bar_rect(l, i), t::ui().accent_dark);
        }
        // 眼睛 = 该行可见性：开着画干净的眼睛，关着加一道斜杠。
        // 五行都接了线 ⇒ 颜色一律正文色（"没开"是状态，不是"不可用"）。
        let visible = layer_visible(mask, overlay, i).unwrap_or(false);
        draw_eye(p, eye_rect(l, i).center(), !visible, t::ui().text);

        // 碰撞标记色块（低=粉、高=蓝），块 7×7
        if i == 3 || i == 4 {
            let x = layer_text_x(l, i) - m::LAYER_MARK_TEXT_OFF;
            pt::fill(
                p,
                Rect::from_center_size(
                    pos2(x + m::LAYER_MARK_W * 0.5, row.center().y),
                    vec2(m::LAYER_MARK_W, m::LAYER_MARK_W),
                ),
                if i == 3 {
                    t::ui().collision_low_mark
                } else {
                    t::ui().collision_high_mark
                },
            );
        }
    }

    // 子组分隔线（截图 y=478 的 `#EAECEF`）
    pt::fill(p, l.layer_group_sep, t::ui().separator);

    // 复选框底 + 边框（文字与标签统一在 labels() 里画）
    for row in l.layer_sub_rows.iter() {
        let cb = Rect::from_min_size(
            pos2(row.left() + m::LAYER_CB_INSET, row.center().y - m::CHECKBOX * 0.5),
            vec2(m::CHECKBOX, m::CHECKBOX),
        );
        pt::fill(p, cb, t::ui().input_search_bg);
        pt::border(p, cb, t::ui().checkbox_border);
    }

    // 图层名 / 子组标题 / 复选框标签
    draw_sel(p, labs, "layer.");
    draw_sel(p, labs, "layers.");
}

/// 画一个"眼睛"图标（椭圆轮廓 + 瞳孔；`hidden` 时加一道斜杠）。
fn draw_eye(p: &Painter, c: egui::Pos2, hidden: bool, color: egui::Color32) {
    const RX: f32 = 7.0;
    const RY: f32 = 4.0;
    const SEG: usize = 20;
    let mut pts = Vec::with_capacity(SEG + 1);
    for i in 0..=SEG {
        let a = std::f32::consts::TAU * i as f32 / SEG as f32;
        pts.push(pos2(c.x + RX * a.cos(), c.y + RY * a.sin()));
    }
    pt::polyline(p, &pts, egui::Stroke::new(1.2_f32, color));
    p.circle_filled(c, 1.9, color);
    if hidden {
        p.line_segment(
            [pos2(c.x - RX, c.y + RY), pos2(c.x + RX, c.y - RY)],
            egui::Stroke::new(1.4_f32, color),
        );
    }
}

// ───────────────────────── Cell Inspector ─────────────────────────

fn draw_inspector(p: &Painter, l: &Layout, d: &DocText, labs: &[Label]) {
    draw_sel(p, labs, "inspector.");
    draw_dyn_sel(p, l, d, "inspector.");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 接线表必须与两套开关的真实字段一一对应（改一个没改另一个立刻红）。
    ///
    /// 前三行 → `LayerMask`（三层精灵），后两行 → `CollisionOverlay`（阻挡标记通道）。
    /// 这条同时钉住"后两行不再置灰/不再点了没反应"（上一版它们返回 `None`）。
    #[test]
    fn 五行都接在两套开关上() {
        let mut mask = LayerMask::all();
        let mut overlay = CollisionOverlay::none();
        for i in 0..LAYER_ROWS {
            assert!(layer_wired(i), "第 {i} 行必须已接线");
        }
        assert!(!layer_wired(LAYER_ROWS), "越界行不算接线");

        // 前三行读 LayerMask，后两行读 CollisionOverlay（默认全关）
        assert_eq!(layer_visible(&mask, &overlay, 0), Some(true));
        assert_eq!(layer_visible(&mask, &overlay, 1), Some(true));
        assert_eq!(layer_visible(&mask, &overlay, 2), Some(true));
        assert_eq!(layer_visible(&mask, &overlay, 3), Some(false), "低位阻挡默认关");
        assert_eq!(layer_visible(&mask, &overlay, 4), Some(false), "高位阻挡默认关");
        assert_eq!(layer_visible(&mask, &overlay, 5), None, "越界行没有可见性");

        // 切第 2 行 → front；切第 3/4 行 → overlay 的 low/high（**必须真的动 overlay**）
        assert!(toggle_visibility(&mut mask, &mut overlay, 2));
        assert!(!mask.front, "第 2 行应切到 front");
        assert!(!overlay.low && !overlay.high, "切精灵层不许动阻挡通道");

        assert!(toggle_visibility(&mut mask, &mut overlay, 3));
        assert!(overlay.low, "第 3 行应切到 CollisionOverlay::low");
        assert!(!overlay.high, "切低位阻挡不许动高位通道");
        assert!(!mask.front, "切阻挡层不许动精灵层开关");

        assert!(toggle_visibility(&mut mask, &mut overlay, 4));
        assert!(overlay.high, "第 4 行应切到 CollisionOverlay::high");
        assert!(overlay.low, "切高位阻挡不许动低位通道");

        // 再切一次回到关（眼睛是开关，不是单向）
        assert!(toggle_visibility(&mut mask, &mut overlay, 3));
        assert!(!overlay.low);
        assert!(!toggle_visibility(&mut mask, &mut overlay, 5), "越界行不能被切");
    }

    /// 值行推进与键列推进必须同源：六条值都是 1 行时，行间距 = 行高 + 间隔。
    #[test]
    fn 值行推进与键列同源() {
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let mut d = DocText {
            title: String::new(),
            tab: String::new(),
            pill: String::new(),
            brush_size: 3,
            status_left: String::new(),
            status_tool: String::new(),
            status_layer: String::new(),
            status_brush: String::new(),
            status_hint: String::new(),
            status_hint_warn: false,
            status_coord: String::new(),
            status_lib: String::new(),
            status_format: String::new(),
            banner: None,
            ground_locked: false,
        status_stamp: None,
            status_fill: None,
            status_op: None,
            status_op_warn: false,
            status_sel: None,
            status_sel_tmpl: None,
            zoom: String::new(),
            page: String::new(),
            inspector_head: String::new(),
            inspector_values: Default::default(),
        };
        for v in d.inspector_values.iter_mut() {
            *v = "x".to_string();
        }
        let spans = value_row_spans(&l, &d);
        assert_eq!(spans.len(), 6);
        for w in spans.windows(2) {
            assert!(
                (w[1].0 - w[0].0 - (m::INSPECTOR_LINE_H + m::INSPECTOR_ROW_GAP)).abs() < 0.01,
                "单行值的行距应等于 行高+间隔：{:?}",
                w
            );
        }
        // 某条值 3 行 ⇒ 后面的行整体下移 2 行高
        d.inspector_values[1] = "a\nb\nc".to_string();
        let spans2 = value_row_spans(&l, &d);
        assert_eq!(spans2[1].1, 3);
        assert!(
            (spans2[2].0 - spans[2].0 - 2.0 * m::INSPECTOR_LINE_H).abs() < 0.01,
            "3 行值之后的键应下移 2 行"
        );
    }

    /// **五行都是正文色**（置灰已取消）：接线完成后再灰着，会让人以为"这两行天生不可用"。
    ///
    /// "没开"这件事由眼睛的斜杠表达（见 [`draw_layers`]），不是靠字色。
    #[test]
    fn 图层名一律正文色不再置灰() {
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let p = crate::fonts::measure_ctx();
        let painter = Painter::new(
            p,
            egui::LayerId::debug(),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(m::WINDOW_W, m::WINDOW_H)),
        );
        let labs = labels(&painter, &l);
        for (i, name) in ["layer.0", "layer.1", "layer.2", "layer.3", "layer.4"]
            .iter()
            .enumerate()
        {
            let lab = labs.iter().find(|x| x.name == *name).expect(name);
            assert_eq!(
                lab.color,
                t::ui().text,
                "{name}（行 {i}）必须与其它行同色；置灰已取消"
            );
            assert_ne!(lab.color, t::ui().text_muted, "{name} 不许再是次要色");
        }
        // 眼睛图标的矩形必须落在对应行内（否则点不到）
        for i in 0..LAYER_ROWS {
            assert!(l.layer_rows[i].contains_rect(eye_rect(&l, i)));
        }
    }

    /// **两个动作互不影响**：点眼睛只切可见性、点行只切当前编辑层（五行都是如此）。
    #[test]
    fn 点眼睛与点行是两个动作() {
        use LayerClick::*;
        // 点眼睛 ⇒ 只切可见性（绝不改当前编辑层）
        for i in 0..LAYER_ROWS {
            assert_eq!(dispatch_click(i, true), ToggleVisibility(i), "行 {i} 的眼睛");
        }
        // 点行内其它位置 ⇒ 只切当前编辑层（五行各对应一个 EditLayer）
        assert_eq!(dispatch_click(0, false), SetEditLayer(EditLayer::Ground));
        assert_eq!(dispatch_click(1, false), SetEditLayer(EditLayer::Middle));
        assert_eq!(dispatch_click(2, false), SetEditLayer(EditLayer::Front));
        assert_eq!(
            dispatch_click(3, false),
            SetEditLayer(EditLayer::LowCollision),
            "低位阻挡行必须能设为当前编辑层"
        );
        assert_eq!(
            dispatch_click(4, false),
            SetEditLayer(EditLayer::HighCollision),
            "高位阻挡行必须能设为当前编辑层"
        );
        // 只有越界行才 Ignore（"点了没反应"只允许出现在不存在的行上）
        assert_eq!(dispatch_click(5, false), Ignore, "越界行");
        assert_eq!(dispatch_click(9, true), Ignore, "越界行");
    }

    /// 当前编辑层的选中态可以落在**任意一行**（含低位/高位阻挡），
    /// 金条也必须在行内（否则截图里会看到一根被裁掉一半的条）。
    #[test]
    fn 五行都能当编辑层() {
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        assert_eq!(EditLayer::ALL.len(), LAYER_ROWS, "编辑层数与图层行数必须一致");
        for layer in EditLayer::ALL {
            let i = layer.row();
            assert_eq!(EditLayer::from_row(i), Some(layer), "行 {i} 应能当编辑层");
            assert!(layer_wired(i), "编辑层必须已接线");
            let bar = edit_bar_rect(&l, i);
            assert!(l.layer_rows[i].contains_rect(bar), "金条越出第 {i} 行：{bar:?}");
            assert!((bar.width() - m::LAYER_EDIT_BAR_W).abs() < 0.01);
            // 金条不许压到眼睛图标上（两者语义不同，重合会让"点眼睛"变得含糊）
            assert!(
                !bar.intersects(eye_rect(&l, i)),
                "第 {i} 行的金条与眼睛图标重叠"
            );
        }
        assert_eq!(EditLayer::from_row(LAYER_ROWS), None, "越界行不是编辑层");
    }

    // ───────────────── 本版新增：胶囊与滑杆滑块接真值 ─────────────────

    fn 量布(_l: &Layout) -> Painter {
        let ctx = crate::fonts::measure_ctx();
        Painter::new(
            ctx,
            egui::LayerId::debug(),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(m::WINDOW_W, m::WINDOW_H)),
        )
    }

    /// 造一份最小 DocText（只关心胶囊与尺寸）。
    fn 文案(pill: &str, size: usize) -> DocText {
        DocText {
            title: String::new(),
            tab: String::new(),
            pill: pill.to_string(),
            brush_size: size,
            status_left: String::new(),
            status_tool: String::new(),
            status_layer: String::new(),
            status_brush: String::new(),
            status_hint: String::new(),
            status_hint_warn: false,
            status_coord: String::new(),
            status_lib: String::new(),
            status_format: String::new(),
            banner: None,
            ground_locked: false,
        status_stamp: None,
            status_fill: None,
            status_op: None,
            status_op_warn: false,
            status_sel: None,
            status_sel_tmpl: None,
            zoom: String::new(),
            page: String::new(),
            inspector_head: String::new(),
            inspector_values: Default::default(),
        }
    }

    /// **工具按钮的命中矩形 = 悬停提示的作用域**（R48：`on_hover_text` 挂在 `tool_buttons` 上）。
    ///
    /// 钉三件事：
    /// ① 10 个按钮矩形**互不重叠**（否则 hover 会同时点亮两个按钮 ⇒ 气泡内容不确定）；
    /// ② 每个矩形都落在「工具」面板的**内容区**里（按钮条可见时才有提示，看不见的区域不该有热区）；
    /// ③ 每个按钮都拿得到非空提示（[`crate::icons::tip_of`] 与 `TOOLS` 同源，另有用例）。
    ///
    /// ⚠ 真正"hover 后 egui 画出气泡"这一步**无法在无头测试里断言**：egui 把 tooltip 状态
    /// 存在自己的 `Memory` 里，`Context::run` 的 `FullOutput` 不暴露它；而 `--shot` 运行又会
    /// 锁死输入（`app::input_locked`）⇒ 截图也拿不到真实 hover。可断言的是**文案数据**
    /// （`icons` 侧）与**热区几何**（本条）。
    #[test]
    fn 工具按钮热区不重叠且在面板内() {
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let c = l.p_tools.content;
        let rects: Vec<egui::Rect> = l.tool_buttons.to_vec();
        assert_eq!(rects.len(), 10, "工具按钮应有 10 个");
        for (i, r) in rects.iter().enumerate() {
            assert!(!r.is_negative() && r.width() > 1.0 && r.height() > 1.0, "第 {i} 个按钮是空矩形");
            assert!(
                c.expand(0.5).contains_rect(*r),
                "第 {i} 个按钮 {:?} 不在「工具」面板内容区 {:?} 内",
                r,
                c
            );
            assert!(!crate::icons::tip_of(i).is_empty(), "第 {i} 个按钮没有悬停提示");
        }
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(
                    !rects[i].intersects(rects[j]),
                    "第 {i} 与第 {j} 个按钮热区重叠：{:?} / {:?}",
                    rects[i],
                    rects[j]
                );
            }
        }
    }

    /// **胶囊文字放得进胶囊**：最大尺寸 `31×31` 也必须完整显示（否则会被截成 `…`）。
    ///
    /// 胶囊宽不再是固定 45（原为截图实测 `x92..136`，2026-11-08 按上游 `MainWindow.axaml:6`
    /// 的算式改为动态：`max(MinWidth 35, 文字宽) + Padding 5×2`）—— 这条钉住
    /// "尺寸到顶也不溢出、不被截断"。
    #[test]
    fn 胶囊最大尺寸放得下() {
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let p = 量布(&l);
        for (pill, size) in [
            (tx::fill(tx::PILL_SIZE_TMPL, &[("n", "3")]), 3usize),
            (tx::fill(tx::PILL_SIZE_TMPL, &[("n", "7")]), 7),
            (tx::fill(tx::PILL_SIZE_TMPL, &[("n", "15")]), 15),
            (tx::fill(tx::PILL_SIZE_TMPL, &[("n", "31")]), 31),
            (tx::TOOL_BUCKET.to_string(), 31),
            (tx::PILL_PENCIL.to_string(), 3),
        ] {
            let d = 文案(&pill, size);
            let 胶囊宽 = pill_rect(&p, &l, &d).width();
            let mut labs = dyn_labels(&p, &l, &d);
            let lab = labs
                .iter_mut()
                .find(|x| x.name == "tool_options.pill")
                .expect("tool_options.pill");
            lab.truncate_to_fit(&p);
            assert!(
                !lab.truncated,
                "胶囊文字 {pill:?} 被截断成 {:?}（胶囊宽 {胶囊宽}）",
                lab.text
            );
            assert!(
                crate::paint::text_w(&p, &lab.text, lab.font) <= 胶囊宽,
                "胶囊文字 {pill:?} 比胶囊还宽"
            );
        }
    }

    /// **A17：胶囊尺寸 = 上游算式**（`MainWindow.axaml:6`）—— 5 组输入，期望值**手算**。
    ///
    /// 上游：`<Border CornerRadius="4" Padding="5,2">` 套 `<TextBlock MinWidth="35" …/>`
    /// ⇒ 宽 = `max(35, 文字宽) + 5 + 5`、高 = `文字高 + 2 + 2`。
    /// 表里的期望值不读被测实现（手算），实现一改就红。
    #[test]
    fn 胶囊尺寸按上游算式() {
        // (文字宽, 文字高) → (期望宽, 期望高)
        for (文字宽, 文字高, 期望宽, 期望高, 说明) in [
            // `3×3` 量级：文字宽 < MinWidth ⇒ 宽被 35 兜住，35 + 10 = 45
            (19.0_f32, 12.0_f32, 45.0_f32, 16.0_f32, "3×3 量级"),
            // 30 仍 < 35 ⇒ 同一个 45（MinWidth 覆盖区，宽度对文字宽不敏感）
            (30.0, 12.0, 45.0, 16.0, "接近 MinWidth"),
            // 恰好 35（边界）：max 取等号 ⇒ 45，证明是 max 而不是 "严格大于才用文字宽"
            (35.0, 12.0, 45.0, 16.0, "恰等 MinWidth"),
            // 44 > 35 ⇒ 宽度改由文字宽决定：44 + 10 = 54（两位数×两位数量级）
            (44.0, 12.0, 54.0, 16.0, "两位数×两位数"),
            // 单字符：宽仍被兜住（8+10=18 < 45），高度随行高走 13 + 4 = 17
            (8.0, 13.0, 45.0, 17.0, "单字符"),
        ] {
            let 尺寸 = pill_size(vec2(文字宽, 文字高));
            assert!(
                (尺寸.x - 期望宽).abs() < 0.01,
                "{说明}：文字宽 {文字宽} 的胶囊宽应为 {期望宽}（max(35, {文字宽}) + 5×2），实得 {}",
                尺寸.x
            );
            assert!(
                (尺寸.y - 期望高).abs() < 0.01,
                "{说明}：文字高 {文字高} 的胶囊高应为 {期望高}（{文字高} + 2×2），实得 {}",
                尺寸.y
            );
            // 两条上游口径同时成立
            assert!(
                尺寸.x >= m::PILL_MIN_W,
                "{说明}：胶囊宽不许小于 MinWidth 35，实得 {}",
                尺寸.x
            );
            assert!(
                尺寸.x >= 文字宽 + m::PILL_PAD_X * 2.0 - 0.01,
                "{说明}：胶囊宽必须装得下文字 + 左右 padding"
            );
        }
    }

    /// **A17：真实文案的胶囊 = 本仓实测字形 + 上游算式**（4 组真实文案）。
    ///
    /// 上一条验「算式对不对」，这条验「喂进算式的字形是**真的量出来的**」：
    /// 期望值一律由 [`pill_text_size`] 的 `layout_no_wrap` 实测值现算 ——
    /// 换成任何估算值 / 旧常量 45×22，下面的等式立刻对不上。
    #[test]
    fn 真实文案的胶囊按实测字形撑出() {
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let p = 量布(&l);
        let c = l.p_tool_options.content;

        for (文字, 说明) in [
            (tx::fill(tx::PILL_SIZE_TMPL, &[("n", "3")]), "3×3（缺省）"),
            (tx::fill(tx::PILL_SIZE_TMPL, &[("n", "7")]), "7×7"),
            (tx::fill(tx::PILL_SIZE_TMPL, &[("n", "31")]), "31×31（上限）"),
            (tx::PILL_PENCIL.to_string(), "1×1（铅笔口径）"),
        ] {
            let 实测 = pill_text_size(&p, &文字);
            let r = pill_rect(&p, &l, &文案(&文字, 3));
            assert!(实测.x > 0.0 && 实测.y > 0.0, "{说明}：字形量出来是空盒子");
            // 宽 = max(35, 实测文字宽) + 10
            assert!(
                (r.width() - (实测.x.max(m::PILL_MIN_W) + m::PILL_PAD_X * 2.0)).abs() < 0.01,
                "{说明}：宽应为 max(35, {}) + 10 = {}，实得 {}",
                实测.x,
                实测.x.max(m::PILL_MIN_W) + m::PILL_PAD_X * 2.0,
                r.width()
            );
            // 高 = 实测文字高 + 4
            assert!(
                (r.height() - (实测.y + m::PILL_PAD_Y * 2.0)).abs() < 0.01,
                "{说明}：高应为 {} + 4 = {}，实得 {}",
                实测.y,
                实测.y + m::PILL_PAD_Y * 2.0,
                r.height()
            );
            // 位置口径没变：左边界 = 面板内容左 + PILL_X（84），垂直居中
            assert!(
                (r.left() - (c.left() + m::PILL_X)).abs() < 0.01,
                "{说明}：左边界必须仍是 PILL_X = {}",
                m::PILL_X
            );
            assert!(
                (r.center().y - c.center().y).abs() < 0.01,
                "{说明}：胶囊必须仍在面板内容区垂直居中"
            );
        }

        // ★★ **`max` 真的生效 —— 单独断言**：
        //    `3×3` 的文字宽 + 10 **小于** 45，而胶囊宽**恰是 45**
        //    ⇒ 去掉 MinWidth 那一步，胶囊只会剩「文字宽 + 10」，不可能还是 45。
        let 三 = tx::fill(tx::PILL_SIZE_TMPL, &[("n", "3")]);
        let 三宽 = pill_text_size(&p, &三).x;
        let 三r = pill_rect(&p, &l, &文案(&三, 3));
        let 兜底宽 = m::PILL_MIN_W + m::PILL_PAD_X * 2.0;
        assert!(
            三宽 + m::PILL_PAD_X * 2.0 < 兜底宽,
            "本仓字体下 `3×3` 的文字宽 {三宽} 必须小到「+10 仍不足 45」，否则这条证明不了 max 生效"
        );
        assert!(
            (三r.width() - 兜底宽).abs() < 0.01,
            "`3×3` 的胶囊宽必须被 MinWidth 兜到 {兜底宽}（文字宽只有 {三宽}），实得 {}",
            三r.width()
        );
        // 反向对照：把文字加宽到越过 MinWidth，宽度必须**跟着文字变**（不是恒 45）
        let 长 = "8888×8888";
        let 长实测 = pill_text_size(&p, 长);
        assert!(
            长实测.x > m::PILL_MIN_W,
            "对照文案 {长} 的实测宽 {} 必须越过 MinWidth 35，否则对照组无效",
            长实测.x
        );
        let 长r = pill_rect(&p, &l, &文案(长, 31));
        assert!(
            (长r.width() - (长实测.x + m::PILL_PAD_X * 2.0)).abs() < 0.01,
            "越过 MinWidth 后宽度必须由文字宽撑出：{} + 10，实得 {}",
            长实测.x,
            长r.width()
        );
        assert!(
            (长r.width() - 45.0).abs() > 0.01,
            "对照组宽度必须不等于旧的固定值 45，实得 {}",
            长r.width()
        );
    }

    /// **滑块停在真实位置**：尺寸 3 在最左（= 参考截图那个位置），31 在最右，且都在轨道上。
    #[test]
    fn 滑块位置随尺寸() {
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let c = l.p_tool_options.content;
        let track_left = c.left() + m::SLIDER_X;
        let track_right = track_left + m::SLIDER_W;
        let min = thumb_rect(&l, &文案("3×3", 3));
        assert!(
            (min.left() - track_left).abs() < 0.01,
            "尺寸 3 应贴轨道左端：{} vs {track_left}",
            min.left()
        );
        let max = thumb_rect(&l, &文案("31×31", 31));
        assert!(
            (max.right() - track_right).abs() < 0.01,
            "尺寸 31 应贴轨道右端：{} vs {track_right}",
            max.right()
        );
        // 单调：尺寸越大越靠右，且滑块永远在轨道范围内
        let mut last = f32::NEG_INFINITY;
        for n in (m::BRUSH_MIN..=m::BRUSH_MAX).step_by(m::BRUSH_STEP) {
            let r = thumb_rect(&l, &文案("x", n));
            assert!(r.left() >= last, "尺寸 {n} 的滑块没有右移");
            assert!(r.left() >= track_left - 0.01 && r.right() <= track_right + 0.01);
            last = r.left();
        }
        // 尺寸 7 的位置可以算出来（4/28 行程）
        let seven = thumb_rect(&l, &文案("7×7", 7));
        let want = track_left + (7 - m::BRUSH_MIN) as f32 / 28.0 * (m::SLIDER_W - m::SWATCH_W);
        assert!((seven.left() - want).abs() < 0.01, "尺寸 7 的位置");
    }
}
