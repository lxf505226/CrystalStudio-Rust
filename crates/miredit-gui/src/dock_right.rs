//! 右停靠区（宽 356）：小地图 / 资源面板 / 当前资源 三个面板。
//!
//! 全部尺寸来自 [`crate::metrics`]（每条注明截图来源），颜色来自 [`crate::theme`]，
//! **全部文案来自 [`crate::ui_text`]**。
//!
//! > 资源面板本单起是**真图块树**（库下拉 / 缩略图网格 / 图号搜索 / 点选设笔刷）：
//! > 状态、几何与缓存都在 [`crate::palette`]，本模块只负责排版与"该画哪一块"。
//! > 小地图的视口指示框自 2026-11-04（审计 E10）起是**真数据**：地图区按
//! > `map.width/height` 等比映射，框按当前 `pan/scale` 算出的可见格窗口换算，
//! > 地图没打开时**不画框**、只写「未打开地图」（见 [`minimap_rects`] 的注释）。

use egui::{pos2, vec2, Align2, Painter, Rect};
use mir2_assets::WilSource;

use crate::layout::Layout;
use crate::metrics as m;
use crate::paint as pt;
use crate::palette::PaletteState;
use crate::panel;
use crate::screen_text::DocText;
use crate::stamp_panel::{StampPanel, TAB_INDEX as STAMP_TAB};
use crate::text_fit::{DynLabel, Label, LabelStyle};
use crate::theme as t;
use crate::ui_text as tx;

/// 画整个右停靠区。
///
/// `vis` = 7 个面板的显隐（视图菜单的开关）。隐藏的面板既不画也不响应点击
/// （点击的闸门在 [`crate::app`] 的 `interact` 里，两边用同一个判据 `PanelVis::get`）。
// 8 个参数是"画一整块停靠区"所需的最小输入集（painter + 布局 + 文案 + 两个面板状态 +
// 图库源 + 小地图数据 + 面板显隐）；打结构体只是搬家，不减少耦合 ⇒ 直接 `allow`。
#[allow(clippy::too_many_arguments)]
pub fn draw(
    p: &Painter,
    l: &Layout,
    d: &DocText,
    pal: &mut PaletteState,
    stamps: &mut StampPanel,
    src: Option<&WilSource>,
    // 印章库号号段（A-024）：与 `src` 同源，都来自 `mapdoc::Assets`。
    号段: crate::stamp_place::库号号段,
    mini: &MiniMap,
    vis: &crate::menu::panels::PanelVis,
) {
    use crate::menu::panels::PanelId;

    let labs = labels(p, l);

    if vis.get(PanelId::Minimap) {
        panel::draw_panel(p, &l.p_minimap, tx::TITLE_MINIMAP, "panel.minimap", t::ui().panel_bg);
        draw_minimap(p, l, mini);
    }

    if vis.get(PanelId::Palette) {
        panel::draw_panel(p, &l.p_palette, tx::TITLE_PALETTE, "panel.palette", t::ui().panel_bg);
        draw_palette(p, l, &labs, d, pal, stamps, src, 号段);
    }

    if vis.get(PanelId::ActiveAsset) {
        panel::draw_panel(
            p,
            &l.p_active_asset,
            tx::TITLE_ACTIVE_ASSET,
            "panel.active_asset",
            t::ui().panel_bg,
        );
        draw_active_asset(p, l, &labs, pal, stamps, src, 号段);
    }
}

/// 右停靠区的**动态**标签：资源面板分页行中间的「可见帧区间」。
///
/// 当前库的帧数还不知道（库打不开 / 一个库都没登记）时，如实回落到
/// 「资源库 N 个」—— 两条模板都在 [`crate::ui_text`] 里，不会出现临时拼的串。
pub fn dyn_labels(l: &Layout, d: &DocText, pal: &PaletteState) -> Vec<DynLabel> {
    // 非「资源库」页签：不显示帧区间（那一页没有网格内容，显示区间等于假装有内容）
    //
    // 「分类」页签的分页行文字（`第 N 页，共 M 页 · 共 K 枚`）由
    // [`crate::stamp_panel`] 自己画 —— 它要读印章面板的状态，不该经这里转发
    // （本函数的签名被 `text_fit` 的截断验收共用，加参数会牵动六处测试）。
    if pal.tab() != 0 {
        return Vec::new();
    }
    let c = l.p_palette.content;
    let (prev, next) = pager_rects(l);
    let (text, tmpl) = match (pal.current(), pal.current_count()) {
        (Some(_), Some(n)) if n > 0 => {
            let visible = pal.rows_drawn().max(1) * m::PALETTE_GRID_COLS;
            let first = pal.scroll() * m::PALETTE_GRID_COLS;
            let last = (first + visible - 1).min(n.saturating_sub(1) as usize);
            (
                tx::fill(
                    tx::PALETTE_RANGE_TMPL,
                    &[
                        // 1 基图号（= 0 基索引 + 1），见 `ui_text::PALETTE_RANGE_TMPL` 的说明
                        ("a", &(first + 1).to_string()),
                        ("b", &(last + 1).to_string()),
                        ("n", &n.to_string()),
                    ],
                ),
                tx::PALETTE_RANGE_TMPL,
            )
        }
        _ => (d.page.clone(), tx::PAGE_LABEL_TMPL),
    };
    vec![DynLabel::new(
        "palette.page_label",
        tmpl,
        text,
        LabelStyle::new(
            m::BODY_FONT,
            pos2(c.center().x, prev.center().y),
            Align2::CENTER_CENTER,
            Rect::from_min_max(
            pos2(prev.right() + 4.0, prev.top()),
            pos2(next.left() - 4.0, next.bottom()),
        ),
            t::ui().text_muted,
        ),
    )]
}

/// 按 id 前缀取标签并绘制。
fn draw_sel(p: &Painter, labs: &[Label], prefix: &str) {
    for lab in labs.iter().filter(|x| x.name.starts_with(prefix)) {
        lab.draw(p);
    }
}

// ───────────────────────── 几何小工具（绘制与验收共用） ─────────────────────────

/// 素材面板内容区的左右可用边界。
fn palette_x_range(l: &Layout) -> (f32, f32) {
    let c = l.p_palette.content;
    (c.left() + m::RIGHT_PAD_X, c.right() - m::RIGHT_PAD_X)
}

/// 素材面板页签的矩形（宽度按文字量出来）。**交互与绘制共用**（见 [`crate::palette`]）。
pub(crate) fn tab_rects(p: &Painter, l: &Layout) -> Vec<Rect> {
    let c = l.p_palette.content;
    let (x0, _) = palette_x_range(l);
    let tabs_y = c.top() + m::PALETTE_TABS_OFF;
    let mut x = x0 + m::PALETTE_TAB_INSET_L;
    let mut out = Vec::with_capacity(tx::PALETTE_TABS.len());
    for name in tx::PALETTE_TABS {
        let w = pt::text_w(p, name, m::TAB_FONT);
        out.push(Rect::from_min_size(
            pos2(x, tabs_y),
            vec2(w, m::PALETTE_TABS_H),
        ));
        x += w + m::PALETTE_TAB_GAP;
    }
    out
}

/// 库选择（原「地形组」）组合框矩形。
pub(crate) fn combo_rect(l: &Layout) -> Rect {
    let c = l.p_palette.content;
    let (x0, x1) = palette_x_range(l);
    Rect::from_min_size(
        pos2(x0, c.top() + m::PALETTE_COMBO_OFF),
        vec2(x1 - x0, m::COMBO_H),
    )
}

/// 搜索框矩形。
pub(crate) fn search_rect(l: &Layout) -> Rect {
    let c = l.p_palette.content;
    let (x0, x1) = palette_x_range(l);
    Rect::from_min_size(
        pos2(x0, c.top() + m::PALETTE_SEARCH_OFF),
        vec2(x1 - x0, m::SEARCH_H),
    )
}

/// 空态主按钮矩形。
fn primary_btn_rect(l: &Layout) -> Rect {
    let c = l.p_palette.content;
    Rect::from_min_size(
        pos2(
            c.center().x - m::PRIMARY_BTN_W * 0.5,
            c.top() + m::PALETTE_BTN_OFF,
        ),
        vec2(m::PRIMARY_BTN_W, m::PRIMARY_BTN_H),
    )
}

/// 分页行两枚按钮的矩形 `(prev, next)`。
pub(crate) fn pager_rects(l: &Layout) -> (Rect, Rect) {
    let c = l.p_palette.content;
    let (x0, x1) = palette_x_range(l);
    let pager_y = c.bottom() - m::PALETTE_BOTTOM_PAD - m::PAGER_ROW_H;
    let prev = Rect::from_min_size(
        pos2(x0 + m::PAGER_BTN_INSET, pager_y),
        vec2(m::PAGER_BTN_W, m::PAGER_ROW_H),
    );
    let next = Rect::from_min_size(
        pos2(x1 - m::PAGER_BTN_INSET - m::PAGER_BTN_W, pager_y),
        vec2(m::PAGER_BTN_W, m::PAGER_ROW_H),
    );
    (prev, next)
}

/// 当前素材卡片与其右侧文字区的左边界。
fn aa_card_and_text(l: &Layout) -> (Rect, f32) {
    let c = l.p_active_asset.content;
    let card = Rect::from_min_size(
        pos2(c.left() + m::AA_CARD_OFF.0, c.top() + m::AA_CARD_OFF.1),
        vec2(m::AA_CARD_W, m::AA_CARD_H),
    );
    let preview = Rect::from_min_size(
        pos2(
            card.left() + m::AA_PREVIEW_OFF,
            card.top() + m::AA_PREVIEW_OFF,
        ),
        vec2(m::AA_PREVIEW_W, m::AA_PREVIEW_H),
    );
    (card, preview.right() + m::AA_TEXT_GAP)
}

/// 右停靠区全部文字标签。
pub fn labels(p: &Painter, l: &Layout) -> Vec<Label> {
    let mut out = Vec::with_capacity(24);

    // ① 三个面板标题
    out.push(panel::title_label(&l.p_minimap, tx::TITLE_MINIMAP, "panel.minimap"));
    out.push(panel::title_label(&l.p_palette, tx::TITLE_PALETTE, "panel.palette"));
    out.push(panel::title_label(
        &l.p_active_asset,
        tx::TITLE_ACTIVE_ASSET,
        "panel.active_asset",
    ));

    // ② 素材面板：页签
    let c = l.p_palette.content;
    let (x0, x1) = palette_x_range(l);
    for (i, r) in tab_rects(p, l).iter().enumerate() {
        out.push(Label {
            name: ["palette.tab.library", "palette.tab.favourites", "palette.tab.recent", "palette.tab.categories"][i],
            text: tx::PALETTE_TABS[i],
            font: m::TAB_FONT,
            pos: r.center(),
            anchor: Align2::CENTER_CENTER,
            // 页签盒宽 = 量出来的文字宽，外扩 `TEXT_CLIP_SLACK` 免得抗锯齿被切（间距 10，够放）
            clip: r.expand(m::TEXT_CLIP_SLACK),
            color: t::ui().text,
        });
    }

    // ③ 类别 + 数量（左/右各占半幅，互不干扰，验收时也各查各的）
    let label_y = c.top() + m::PALETTE_LABEL_OFF;
    let mid_x = c.center().x;
    out.push(Label {
        name: "palette.lib_label",
        text: tx::PALETTE_LIB_LABEL,
        font: m::PALETTE_LABEL_FONT,
        pos: pos2(x0, label_y),
        anchor: Align2::LEFT_TOP,
        clip: Rect::from_min_max(pos2(x0, label_y), pos2(mid_x, label_y + 16.0)),
        color: t::ui().text,
    });
    // ③b 「分类」页签的同一行：左标签换成「印章分类」（几何与上一行逐像素相同，
    //     两个页签共用同一个面板骨架；绘制时按当前页签二选一）
    out.push(Label {
        name: "palette.stamp_label",
        text: tx::STAMP_KIND_LABEL,
        font: m::PALETTE_LABEL_FONT,
        pos: pos2(x0, label_y),
        anchor: Align2::LEFT_TOP,
        clip: Rect::from_min_max(pos2(x0, label_y), pos2(mid_x, label_y + 16.0)),
        color: t::ui().text,
    });

    // ③b 网格区的两条"没内容"说明（居中）；只在对应状态下画，见后面 `draw_palette`
    let grid = PaletteState::grid_rect(l);
    out.push(Label {
        name: "palette.tab_unimplemented",
        text: tx::PALETTE_TAB_UNIMPLEMENTED,
        font: m::BODY_FONT,
        pos: pos2(grid.center().x, grid.top() + grid.height() * 0.4),
        anchor: Align2::CENTER_CENTER,
        clip: grid,
        color: t::ui().text_muted,
    });
    out.push(Label {
        name: "palette.lib_empty",
        text: tx::PALETTE_LIB_EMPTY,
        font: m::BODY_FONT,
        pos: pos2(grid.center().x, grid.top() + grid.height() * 0.4),
        anchor: Align2::CENTER_CENTER,
        clip: grid,
        color: t::ui().text_muted,
    });

    // ④ 搜索框占位文字 + 清除符
    //
    // 裁剪盒要避开搜索框自身的 1px 边框（上下各留 2px），否则"文字右边界"会被量成边框线。
    let search = search_rect(l);
    let hint_x = search.left() + m::SEARCH_TEXT_INSET;
    let inner_h = Rect::from_min_max(
        pos2(search.left() + 2.0, search.top() + 2.0),
        pos2(search.right() - 2.0, search.bottom() - 2.0),
    );
    out.push(Label {
        name: "palette.search_hint",
        text: tx::SEARCH_HINT,
        font: m::BODY_FONT,
        pos: pos2(hint_x, search.center().y),
        anchor: Align2::LEFT_CENTER,
        clip: Rect::from_min_max(inner_h.min, pos2(search.right() - 26.0, inner_h.max.y)),
        color: t::ui().text_muted,
    });
    // ④b 「分类」页签的搜索占位符（同一位置，语义不同：那边搜名称/编号，这边搜图号）
    out.push(Label {
        name: "palette.stamp_search_hint",
        text: tx::STAMP_SEARCH_HINT,
        font: m::BODY_FONT,
        pos: pos2(hint_x, search.center().y),
        anchor: Align2::LEFT_CENTER,
        clip: Rect::from_min_max(inner_h.min, pos2(search.right() - 26.0, inner_h.max.y)),
        color: t::ui().text_muted,
    });
    out.push(Label {
        name: "palette.search_clear",
        text: tx::SEARCH_CLEAR,
        font: m::BODY_FONT,
        pos: pos2(search.right() - m::SEARCH_TEXT_INSET, search.center().y),
        anchor: Align2::RIGHT_CENTER,
        clip: Rect::from_min_max(
            pos2(search.right() - 26.0, inner_h.min.y),
            pos2(search.right() - 2.0, inner_h.max.y),
        ),
        color: t::ui().text_muted,
    });

    // ⑤ 空态：标题 + 两行说明 + 主按钮文字
    let empty_clip_x = (x0, x1);
    out.push(Label {
        name: "palette.empty_title",
        text: tx::EMPTY_TITLE,
        font: m::BODY_FONT,
        pos: pos2(c.center().x, c.top() + m::PALETTE_EMPTY_TITLE_OFF),
        anchor: Align2::CENTER_TOP,
        clip: Rect::from_min_max(
            pos2(empty_clip_x.0, c.top() + m::PALETTE_EMPTY_TITLE_OFF),
            pos2(
                empty_clip_x.1,
                c.top() + m::PALETTE_EMPTY_TITLE_OFF + m::BODY_FONT * 1.6,
            ),
        ),
        color: t::ui().text,
    });
    for (i, line) in tx::EMPTY_DESC.iter().enumerate() {
        let y = c.top() + m::PALETTE_EMPTY_DESC_OFF + i as f32 * m::EMPTY_DESC_LINE_H;
        out.push(Label {
            name: ["palette.empty_desc0", "palette.empty_desc1"][i],
            text: line,
            font: m::BODY_FONT,
            pos: pos2(c.center().x, y),
            anchor: Align2::CENTER_TOP,
            clip: Rect::from_min_max(
                pos2(empty_clip_x.0, y),
                pos2(empty_clip_x.1, y + m::EMPTY_DESC_LINE_H),
            ),
            color: t::ui().text,
        });
    }
    let btn = primary_btn_rect(l);
    out.push(Label {
        name: "palette.primary_btn",
        text: tx::EMPTY_ACTION,
        font: m::BODY_FONT,
        pos: btn.center(),
        anchor: Align2::CENTER_CENTER,
        clip: btn,
        color: t::ui().accent_button_text,
    });

    // ⑥ 分页行：两个箭头 + 中间文字
    let (prev, next) = pager_rects(l);
    out.push(Label {
        name: "palette.pager_prev",
        text: tx::PAGER_PREV,
        font: m::PAGER_ARROW_FONT,
        pos: prev.center(),
        anchor: Align2::CENTER_CENTER,
        clip: prev,
        color: t::ui().text_muted,
    });
    out.push(Label {
        name: "palette.pager_next",
        text: tx::PAGER_NEXT,
        font: m::PAGER_ARROW_FONT,
        pos: next.center(),
        anchor: Align2::CENTER_CENTER,
        clip: next,
        color: t::ui().text_muted,
    });
    // 分页行中间的文字（`资源库 N 个`）是动态的，见 [`dyn_labels`]。


    // ⑦ 当前素材：三行 + 星标
    //
    // 每行一个**纵向窄带**裁剪盒（高 16）：既防止行间重叠，也让"文字右边界 vs 裁剪盒右边界"
    // 的核对能逐行做，不会把三行墨迹混成一带。
    let (card, text_x) = aa_card_and_text(l);
    let line_clip = |y: f32| {
        Rect::from_min_max(
            pos2(text_x, y - 8.0),
            pos2(card.right() - 4.0, y + 8.0),
        )
    };
    let caption_y = card.top() + m::AA_LINE_CAPTION_Y;
    let label_y = card.top() + m::AA_LINE_LABEL_Y;
    let pin_y = card.top() + m::AA_LINE_PIN_Y;
    out.push(Label {
        name: "active_asset.caption",
        text: tx::TITLE_ACTIVE_ASSET,
        font: m::AA_LINE_FONT,
        pos: pos2(text_x, caption_y),
        anchor: Align2::LEFT_CENTER,
        clip: line_clip(caption_y),
        color: t::ui().text,
    });
    out.push(Label {
        name: "active_asset.label",
        text: tx::AA_LABEL,
        font: m::AA_LINE_FONT,
        pos: pos2(text_x, label_y),
        anchor: Align2::LEFT_CENTER,
        clip: line_clip(label_y),
        color: t::ui().text,
    });
    // 「分类」页签的同一位置：「还没选印章」时的提示（与上一行同几何，二选一绘制）
    out.push(Label {
        name: "active_asset.stamp_label",
        text: tx::STAMP_AA_PROMPT,
        font: m::AA_LINE_FONT,
        pos: pos2(text_x, label_y),
        anchor: Align2::LEFT_CENTER,
        clip: line_clip(label_y),
        color: t::ui().text,
    });
    // 星标与 `收藏` 同一行并排：星占左边 AA_STAR_TEXT_OFF 宽，文字从 AA_STAR_TEXT_OFF 起
    out.push(Label {
        name: "active_asset.star",
        text: tx::PIN_STAR,
        font: m::AA_LINE_FONT,
        pos: pos2(text_x, pin_y),
        anchor: Align2::LEFT_CENTER,
        clip: Rect::from_min_max(
            pos2(text_x, pin_y - 8.0),
            pos2(text_x + m::AA_STAR_TEXT_OFF, pin_y + 8.0),
        ),
        color: t::ui().text_muted,
    });
    out.push(Label {
        name: "active_asset.pin",
        text: tx::AA_PIN,
        font: m::AA_LINE_FONT,
        pos: pos2(text_x + m::AA_STAR_TEXT_OFF, pin_y),
        anchor: Align2::LEFT_CENTER,
        clip: Rect::from_min_max(
            pos2(text_x + m::AA_STAR_TEXT_OFF, pin_y - 8.0),
            pos2(card.right() - 4.0, pin_y + 8.0),
        ),
        color: t::ui().text_muted,
    });

    out
}

// ───────────────────────── Minimap ─────────────────────────

/// 小地图要画的**真实数据**（一个常量都没有）。
///
/// ## 为什么必须把数据传进来（审计 E10，2026-11-04）
/// 上一版 [`draw_minimap`] 只吃 `Layout`，于是"视口指示框"只能是一个写死的 14×14 方块，
/// 永远钉在地图块左上角 —— 几何形状在替数据说话（"你的视口在整图左上角"），
/// 而这句话与 `pan` / `scale` / 地图尺寸**全都无关**。那正是项目明令禁止的假数据。
///
/// 本单走**方案 A（真数据）**而不是"写句未实现"：真实几何全都在手 ——
/// 地图尺寸来自 `DocState`（`.map` 头里的 `width/height`），
/// 视口窗口来自 `CanvasView::visible_cells`（= 真实 `pan`/`scale`/画布矩形算出来的）。
/// 既然能算准，就不该退回"不画"。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MiniMap {
    /// 地图尺寸（世界格）；`None` = **没打开地图**（那时不画任何框，只写「未打开地图」）。
    pub map: Option<(u16, u16)>,
    /// 当前视口覆盖的世界格窗口 `(原点 x, 原点 y, 宽, 高)`
    /// —— 直接就是 `CanvasView::visible_cells(canvas)` 的返回值。
    pub view: (i32, i32, u32, u32),
}

impl MiniMap {
    /// 没打开地图时的小地图状态（`view` 全 0，不参与任何计算）。
    pub const fn closed() -> Self {
        Self {
            map: None,
            view: (0, 0, 0, 0),
        }
    }

    /// 真实构造：地图格数 + 可见格窗口。
    pub const fn new(map: Option<(u16, u16)>, view: (i32, i32, u32, u32)) -> Self {
        Self { map, view }
    }
}

/// 小地图的两块矩形：`(地图区, 视口框)`；没打开地图 ⇒ `None`（**一个框都不画**）。
///
/// - **地图区**：按 `map.width × map.height` 的**世界像素**等比缩放进 `block.shrink(6)`
///   （保持长宽比、居中）—— 上游 `MapMinimap.Avalonia.cs:19-21` 的 `MapBounds` 用
///   `(ActualWidth - 12) / w` 定倍数、`(ActualWidth - w*scale) / 2` 定位置，
///   `-12` 是两边合计 ⇒ 等价于每边内缩 [`m::MINIMAP_FIT_INSET`] = 6。
///   所以地图区是"这张图的形状"，不是装饰；返回值就是后续绘制/交互要用的**实际**矩形。
/// - **视口框**：把 `view` 的格窗口按同一比例映射到地图区上，再**夹进**地图区 ——
///   视口可以比整图大（缩到 25% 时确实看得见全图），那时框 = 整块地图区，是实话。
///
/// `block` 传的是地图块矩形（[`minimap_block_rect`]，200×138，**尺寸不变**），不是整个面板。
pub fn minimap_rects(block: Rect, mini: &MiniMap) -> Option<(Rect, Rect)> {
    let (mw, mh) = mini.map?;
    // 可用区 = 块每边内缩 6（上游的 `-12`）；块太小/退化时连图都放不下 ⇒ 不画
    let inner = block.shrink(m::MINIMAP_FIT_INSET);
    if mw == 0 || mh == 0 || !inner.is_positive() {
        return None;
    }
    // 世界像素尺寸：一格 = 48×32（与 `miredit_render` 同源，不另编一个比例）
    let world_w = f32::from(mw) * m::CELL_W;
    let world_h = f32::from(mh) * m::CELL_H;
    // 等比 fit 进可用区（取小的那个比例，保证整图都在块里），居中
    let k = (inner.width() / world_w).min(inner.height() / world_h);
    let map = Rect::from_center_size(inner.center(), vec2(world_w * k, world_h * k));

    // 视口：世界格 → 世界像素 → 小地图像素，再夹到地图区里
    let (vx, vy, vw, vh) = mini.view;
    if vw == 0 || vh == 0 {
        // 视口未知（还没布局/没地图）：不画一个"猜"的框
        return Some((map, Rect::NOTHING));
    }
    let px = |gx: f32| map.left() + gx * k;
    let py = |gy: f32| map.top() + gy * k;
    let x0 = px(vx as f32 * m::CELL_W).clamp(map.left(), map.right());
    let y0 = py(vy as f32 * m::CELL_H).clamp(map.top(), map.bottom());
    let x1 = px((vx + vw as i32) as f32 * m::CELL_W).clamp(map.left(), map.right());
    let y1 = py((vy + vh as i32) as f32 * m::CELL_H).clamp(map.top(), map.bottom());
    // 亚像素视口（缩到极小时宽/高可能 <1px）也留 1px，否则"看不见框"同样是失真
    let m1 = m::MINIMAP_VIEWBOX_MIN_PX;
    let vb = Rect::from_min_max(
        pos2(x0, y0),
        pos2(
            x1.max(x0 + m1).min(map.right()),
            y1.max(y0 + m1).min(map.bottom()),
        ),
    );
    Some((map, vb))
}

/// 小地图像素 → 世界像素（上游 `MapMinimap.Avalonia.cs:70-75` 的 `WorldAt` **逐字**）。
///
/// 上游原文（`_upstream/CrystalStudio/`，文件 sha256 前 8 位 `11f92126`）：
///
/// ```csharp
/// private Point WorldAt(Point point)
/// {
///     var map = MapBounds; var source = Viewport!;
///     return new(Math.Clamp((point.X - map.X) / map.Width, 0, 1) * source.Document.Width * 48.0,
///         Math.Clamp((point.Y - map.Y) / map.Height, 0, 1) * source.Document.Height * 32.0);
/// }
/// ```
///
/// 即 `wx = clamp((p.x - map.x) / map.w, 0.0, 1.0) * world_w`，`wy` 同理。
/// **点在区外**（左/上/右/下）时被夹到边界 —— 上游那个 `Math.Clamp(…, 0, 1)` 就是干这个的，
/// 所以永远不会算出负坐标或超过 `world_w/h` 的值。
///
/// - `map`：传 [`minimap_rects`] 返回的**地图区**（fit 进块内那个矩形），不是 200×138 的地图块。
///   上游同样拿 `MapBounds`（fit 后的图）算，块内那圈留白属于"图外"，夹到边界正合适。
/// - `world`：世界像素尺寸，调用方按 `(W as f32 * 48.0, H as f32 * 32.0)` 给
///   （与 [`m::CELL_W`] 和 [`m::CELL_H`] 同源；上游就在这里直接写 `* 48.0` / `* 32.0`）。
/// - `map` 是空矩形（没打开地图时 `MapBounds` 走 `default`）⇒ 返回 `(0, 0)`，**绝不除零**。
pub fn minimap_world_at(p: egui::Pos2, map: egui::Rect, world: (f32, f32)) -> (f32, f32) {
    // 上游没有这道 guard（它先 `if (map.Width <= 0 …) return;` 就画完了），
    // 本仓把"空矩形"的语义收在这里：退化矩形 / 非有限输入一律 (0,0)，不外抛 NaN。
    if !map.is_positive()
        || !(p.x.is_finite() && p.y.is_finite())
        || !(world.0.is_finite() && world.1.is_finite())
    {
        return (0.0, 0.0);
    }
    let fx = ((p.x - map.left()) / map.width()).clamp(0.0, 1.0);
    let fy = ((p.y - map.top()) / map.height()).clamp(0.0, 1.0);
    (fx * world.0, fy * world.1)
}

/// 小地图里那块「地图块」矩形（200×138，水平/垂直居中于面板内容区）。
///
/// 抽成函数是为了让绘制与单测用**同一份**几何（单测要断言"框随 pan 移动"，
/// 不能自己再写一遍矩形算法）。
pub fn minimap_block_rect(l: &Layout) -> Rect {
    let c = l.p_minimap.content;
    let inner = Rect::from_min_max(
        pos2(
            c.left() + m::MINIMAP_INNER_INSET,
            c.top() + m::MINIMAP_INNER_INSET,
        ),
        pos2(
            c.right() - m::MINIMAP_INNER_INSET,
            c.bottom() - m::MINIMAP_INNER_INSET,
        ),
    );
    Rect::from_center_size(inner.center(), vec2(m::MINIMAP_MAP_W, m::MINIMAP_MAP_H))
}

/// 画小地图：**有地图** ⇒ 按真实几何画地图区 + 视口框；**没地图** ⇒ 只画空块 + 如实文案。
fn draw_minimap(p: &Painter, l: &Layout, mini: &MiniMap) {
    let c = l.p_minimap.content;
    // 浅底内框（截图 `#F9FAFB` 区段 x1146..1481，相对面板左右各内缩 10）
    let inner = Rect::from_min_max(
        pos2(
            c.left() + m::MINIMAP_INNER_INSET,
            c.top() + m::MINIMAP_INNER_INSET,
        ),
        pos2(
            c.right() - m::MINIMAP_INNER_INSET,
            c.bottom() - m::MINIMAP_INNER_INSET,
        ),
    );
    pt::fill(p, inner, t::ui().view_surround);

    // 地图块 200×138（尺寸/位置都不变；图按 `minimap_rects` fit 进块内每边内缩 6 的可用区）
    let block = minimap_block_rect(l);
    pt::fill(p, block, t::ui().canvas_bg);
    pt::border(p, block, t::ui().separator);

    let Some((map, vb)) = minimap_rects(block, mini) else {
        // 没打开地图：**不画任何框**，只如实说一句（旧版这里画的是恒在左上角的假框）
        pt::text(
            p,
            block.center(),
            Align2::CENTER_CENTER,
            tx::STATUS_NO_MAP,
            m::PALETTE_LABEL_FONT,
            t::ui().text_muted,
        );
        return;
    };

    // 地图区（按地图长宽比等比缩出来的真形状）：底色略深一档，好与"块"区分开
    pt::fill(p, map, t::ui().status_bar);
    pt::border(p, map, t::ui().checkbox_border);
    // 视口框：位置/大小全部来自真实 pan/scale/地图尺寸
    if vb.is_positive() {
        pt::fill(p, vb, t::ui().title_bar_text);
        pt::border(p, vb, t::ui().menu_bar_text);
    }
}

// ───────────────────────── Palette（素材面板） ─────────────────────────

// 8 个参数是"画一整块素材面板"所需的最小输入集（画笔 + 布局 + 标签 + 文档文案 + 面板状态 +
// 印章面板 + 图库源 + 库号号段）；打结构体只是搬家，不减少耦合 ⇒ 与本文件 [`draw`] 同口径直接 `allow`。
#[allow(clippy::too_many_arguments)]
fn draw_palette(
    p: &Painter,
    l: &Layout,
    labs: &[Label],
    d: &DocText,
    pal: &mut PaletteState,
    stamps: &mut StampPanel,
    src: Option<&WilSource>,
    号段: crate::stamp_place::库号号段,
) {
    let c = l.p_palette.content;
    let (_, x1) = palette_x_range(l);
    let stamp_tab = pal.tab() == STAMP_TAB;

    // ① 页签行 + **当前**页签下划线（截图 y=312..315 的金色；本单起跟着选中页签走）
    let tabs = tab_rects(p, l);
    if let Some(active) = tabs.get(pal.tab()) {
        let ul = Rect::from_min_size(
            pos2(
                active.left() - m::PALETTE_UNDERLINE_PAD,
                c.top() + m::PALETTE_TABS_OFF + m::PALETTE_TABS_H - m::PALETTE_UNDERLINE_H,
            ),
            vec2(
                active.width() + m::PALETTE_UNDERLINE_PAD * 2.0,
                m::PALETTE_UNDERLINE_H,
            ),
        );
        pt::fill(p, ul, t::ui().accent);
    }
    draw_sel(p, labs, "palette.tab.");

    // ② 选择行的左标签（静态）+ 右端真实计数（动态）
    //
    //    ⚠ 两个页签在这一行显示**两套不同的数据**（资源库名/帧数 vs 印章分类/印章数），
    //    绝不复用同一个字段 —— 那会把"帧 31775"写到印章页签上。
    let label_y = c.top() + m::PALETTE_LABEL_OFF;
    let mid_x = c.center().x;
    if stamp_tab {
        draw_sel(p, labs, "palette.stamp_label");
        let text = tx::fill(
            tx::STAMP_COUNT_TMPL,
            &[("n", &stamps.total().to_string())],
        );
        let mut lab = DynLabel::new(
            "palette.stamp_count",
            tx::STAMP_COUNT_TMPL,
            text,
            LabelStyle::new(
                m::PALETTE_LABEL_FONT,
                pos2(x1, label_y),
                Align2::RIGHT_TOP,
                Rect::from_min_max(
                pos2(mid_x, label_y),
                pos2(x1 + m::TEXT_CLIP_SLACK, label_y + 16.0),
            ),
                t::ui().text,
            ),
        );
        lab.truncate_to_fit(p);
        lab.draw(p);
    } else {
        draw_sel(p, labs, "palette.lib_label");
        if pal.current().is_some() {
            let text = match pal.current_count() {
                Some(n) => tx::fill(tx::PALETTE_FRAMES_TMPL, &[("n", &n.to_string())]),
                None => tx::VAL_UNKNOWN.to_owned(),
            };
            let mut lab = DynLabel::new(
                "palette.frames",
                tx::PALETTE_FRAMES_TMPL,
                text,
                LabelStyle::new(
                    m::PALETTE_LABEL_FONT,
                    pos2(x1, label_y),
                    Align2::RIGHT_TOP,
                    Rect::from_min_max(
                    pos2(mid_x, label_y),
                    pos2(x1 + m::TEXT_CLIP_SLACK, label_y + 16.0),
                ),
                    t::ui().text,
                ),
            );
            lab.truncate_to_fit(p);
            lab.draw(p);
        }
    }

    // ③ 组合框（下拉）：先铺底与边框，再写当前选择（顺序错了文字会被底色盖掉）
    let combo = combo_rect(l);
    pt::fill(p, combo, t::ui().input_bg);
    pt::border(p, combo, t::ui().input_border);
    if stamp_tab {
        let pc = p.with_clip_rect(Rect::from_min_max(
            pos2(combo.left() + m::SEARCH_TEXT_INSET, combo.top()),
            pos2(combo.right() - 26.0, combo.bottom()),
        ));
        pt::text(
            &pc,
            pos2(combo.left() + m::SEARCH_TEXT_INSET, combo.center().y),
            Align2::LEFT_CENTER,
            &stamps.filter().label(),
            m::BODY_FONT,
            t::ui().text,
        );
    } else if let Some(e) = pal.current() {
        let name = tx::fill(
            tx::PALETTE_LIB_ROW_TMPL,
            &[("stem", &e.stem), ("fam", e.family_label())],
        );
        let pc = p.with_clip_rect(Rect::from_min_max(
            pos2(combo.left() + m::SEARCH_TEXT_INSET, combo.top()),
            pos2(combo.right() - 26.0, combo.bottom()),
        ));
        pt::text(
            &pc,
            pos2(combo.left() + m::SEARCH_TEXT_INSET, combo.center().y),
            Align2::LEFT_CENTER,
            &name,
            m::BODY_FONT,
            t::ui().text,
        );
    }
    // 右侧 chevron
    let cx = combo.right() - 16.0;
    let cy = combo.center().y;
    p.line_segment(
        [pos2(cx - 4.0, cy - 2.0), pos2(cx, cy + 2.0)],
        egui::Stroke::new(1.2_f32, t::ui().text),
    );
    p.line_segment(
        [pos2(cx, cy + 2.0), pos2(cx + 4.0, cy - 2.0)],
        egui::Stroke::new(1.2_f32, t::ui().text),
    );

    // ④ 搜索框：有输入就画输入内容（裁剪到框内），否则画占位提示
    let search = search_rect(l);
    pt::fill_round(p, search, 3.0, t::ui().input_search_bg);
    pt::border(p, search, t::ui().input_border);
    // 两个页签的搜索语义不同（资源库 = 搜图号；分类 = 搜印章名称/编号），占位符与内容各画各的
    let (typed, hint, focused) = if stamp_tab {
        (
            stamps.search_text(),
            "palette.stamp_search_hint",
            stamps.focused(),
        )
    } else {
        (pal.search_text(), "palette.search_hint", pal.focused())
    };
    if typed.is_empty() {
        draw_sel(p, labs, hint);
    } else {
        let pc = p.with_clip_rect(Rect::from_min_max(
            pos2(search.left() + 2.0, search.top() + 2.0),
            pos2(search.right() - 26.0, search.bottom() - 2.0),
        ));
        pt::text(
            &pc,
            pos2(search.left() + m::SEARCH_TEXT_INSET, search.center().y),
            Align2::LEFT_CENTER,
            typed,
            m::BODY_FONT,
            t::ui().text,
        );
        // 光标：有焦点时在文字后画一条 1px 竖线（提示"这里能打字"）
        if focused {
            let w = pt::text_w(p, typed, m::BODY_FONT);
            let cx = search.left() + m::SEARCH_TEXT_INSET + w + 1.0;
            pt::fill(
                &pc,
                Rect::from_min_max(
                    pos2(cx, search.top() + 5.0),
                    pos2(cx + 1.0, search.bottom() - 5.0),
                ),
                t::ui().text,
            );
        }
    }
    draw_sel(p, labs, "palette.search_clear");

    // ⑤ 主体：「分类」页签 = 印章网格；「资源库」页签 = 真网格；其它页签 = 未实现；
    //    库打不开 = 如实说没有；没库 = 空态
    if stamp_tab {
        stamps.draw(p, l, src, 号段);
    } else if pal.libs().is_empty() {
        // 空态：金色网格图标 + 标题 + 说明 + 金色按钮（一个库都没登记时才走这里）
        let icon_c = pos2(
            c.center().x,
            c.top() + m::PALETTE_EMPTY_ICON_OFF + m::EMPTY_ICON_SIZE * 0.5,
        );
        draw_grid_icon(p, icon_c, t::ui().empty_icon);
        draw_sel(p, labs, "palette.empty_");
        let btn = primary_btn_rect(l);
        pt::fill_round(p, btn, 3.0, t::ui().accent_dark);
        pt::fill_round(p, btn.shrink(1.0), 2.0, t::ui().accent);
        draw_sel(p, labs, "palette.primary_btn");
    } else if pal.tab() != 0 {
        draw_sel(p, labs, "palette.tab_unimplemented");
    } else if pal.current_count().is_none_or(|n| n == 0) {
        draw_sel(p, labs, "palette.lib_empty");
    } else {
        pal.draw(p, l, src);
    }

    // ⑥⑦ 提示行 + 分页行：只有两个"有网格"的页签才有
    //    （收藏 / 最近使用没有网格，画帧区间等于假装有内容）
    if stamp_tab {
        draw_stamp_footer(p, l, labs, stamps, src.is_some());
    } else if pal.tab() == 0 {
        // ⑥ 提示行：优先显示面板提示（搜索/拒绝设笔刷），否则报空帧数（真数据）
        let note = PaletteState::note_rect(l);
        let (note_text, note_color) = match pal.note() {
            Some(s) => (Some(s.to_owned()), t::ui().accent_dark),
            None => {
                let n = pal.empty_count();
                if n > 0 {
                    (
                        Some(tx::fill(
                            tx::PALETTE_EMPTY_FRAMES_TMPL,
                            &[("n", &n.to_string())],
                        )),
                        t::ui().text_muted,
                    )
                } else {
                    (None, t::ui().text_muted)
                }
            }
        };
        if let Some(text) = note_text {
            let pc = p.with_clip_rect(note);
            pt::text(
                &pc,
                pos2(note.left(), note.center().y),
                Align2::LEFT_CENTER,
                &text,
                m::PALETTE_LABEL_FONT,
                note_color,
            );
        }

        // ⑦ 底部分页行
        let (prev, next) = pager_rects(l);
        for r in [prev, next] {
            pt::fill_round(p, r, 3.0, t::ui().input_bg);
        }
        draw_sel(p, labs, "palette.pager_");
        for mut lab in dyn_labels(l, d, pal) {
            lab.truncate_to_fit(p);
            lab.draw(p);
        }
    }

    // ⑧ 下拉弹层**最后画**（它压在网格之上；两个页签各弹各的）
    if stamp_tab {
        stamps.draw_combo_popup(p, l);
    } else {
        pal.draw_combo_popup(p, l);
    }
}

/// 「分类」页签的提示行 + 分页行（`第 N 页，共 M 页 · 共 K 枚`）。
///
/// 为什么单独一个函数而不是塞进 `draw_palette` 的 if 里：这两行的**内容来源完全不同**
/// （印章面板 vs 资源库面板），混在一段里最容易在改动时把两套数字串台。
fn draw_stamp_footer(p: &Painter, l: &Layout, labs: &[Label], stamps: &StampPanel, has_source: bool) {
    // ① 提示行：装载中 / 装载失败 / 图库未装载 / 未登记分组说明（全部来自 `ui_text`）
    let note = StampPanel::note_rect(l);
    if let Some(text) = stamps.note(has_source) {
        let color = if stamps.is_ready() {
            t::ui().text_muted
        } else {
            t::ui().accent_dark
        };
        let pc = p.with_clip_rect(note);
        pt::text(
            &pc,
            pos2(note.left(), note.center().y),
            Align2::LEFT_CENTER,
            &text,
            m::PALETTE_LABEL_FONT,
            color,
        );
    }

    // ② 分页行：两枚按钮（复用与资源库页签同一批静态标签）+ 中间那行真实页码
    let (prev, next) = pager_rects(l);
    for r in [prev, next] {
        pt::fill_round(p, r, 3.0, t::ui().input_bg);
    }
    draw_sel(p, labs, "palette.pager_");
    let band = Rect::from_min_max(
        pos2(prev.right() + 4.0, prev.top()),
        pos2(next.left() - 4.0, next.bottom()),
    );
    let text = tx::fill(
        tx::STAMP_PAGE_TMPL,
        &[
            ("page", &(stamps.page() + 1).to_string()),
            ("pages", &stamps.pages().to_string()),
            ("total", &stamps.total().to_string()),
        ],
    );
    let pc = p.with_clip_rect(band);
    pt::text(
        &pc,
        pos2(band.center().x, band.center().y),
        Align2::CENTER_CENTER,
        &text,
        m::BODY_FONT,
        t::ui().text_muted,
    );
}

/// Palette 空态的 ▦ 网格图标（4×4 小方格，金色）。截图 (1308,488)-(1330,508) 是金色像素簇。
fn draw_grid_icon(p: &Painter, c: egui::Pos2, color: egui::Color32) {
    const N: usize = 4;
    let cell = m::EMPTY_ICON_SIZE / N as f32;
    let origin = pos2(
        c.x - m::EMPTY_ICON_SIZE * 0.5,
        c.y - m::EMPTY_ICON_SIZE * 0.5,
    );
    for r in 0..N {
        for q in 0..N {
            let rect = Rect::from_min_size(
                pos2(origin.x + q as f32 * cell, origin.y + r as f32 * cell),
                vec2(cell, cell),
            );
            pt::fill(p, rect.shrink(0.8), color);
        }
    }
}

// ───────────────────────── Active Asset（当前资源） ─────────────────────────

fn draw_active_asset(
    p: &Painter,
    l: &Layout,
    labs: &[Label],
    pal: &mut PaletteState,
    stamps: &mut StampPanel,
    src: Option<&WilSource>,
    号段: crate::stamp_place::库号号段,
) {
    let (card, text_x) = aa_card_and_text(l);
    // 卡片底（截图 `#EFF1F3` 卡片 x1147..1480 / y787..895）
    pt::fill_round(p, card, 4.0, t::ui().input_bg);

    // 卡片内预览框（截图 `#F7F7F8` 区段 x1157..1344）
    let preview = Rect::from_min_size(
        pos2(
            card.left() + m::AA_PREVIEW_OFF,
            card.top() + m::AA_PREVIEW_OFF,
        ),
        vec2(m::AA_PREVIEW_W, m::AA_PREVIEW_H),
    );
    pt::fill_round(p, preview, 3.0, t::ui().status_bar);
    pt::border(p, preview, t::ui().separator);

    // 「分类」页签 + 已选中一枚印章 ⇒ 这一格显示**印章**的信息（名称 · 中文分类 · 格数 · 包围盒）。
    // 两个页签的选中项是**两套数据**，绝不混着显示（否则会拿帧号冒充印章）。
    if pal.tab() == STAMP_TAB {
        draw_sel(p, labs, "active_asset.caption");
        if stamps.selected().is_some() {
            draw_stamp_active(p, &card, text_x, &preview, stamps, src, 号段);
            return;
        }
        // 该页签还没选：如实写占位提示（不借用资源库那句"选择要绘制的地形组合"）
        draw_sel(p, labs, "active_asset.stamp_label");
        return;
    }

    // 预览框里放**当前高亮帧的真缩略图**（没选就是空的，不画假图）
    if let Some((id, w, h)) = pal.selected_texture(p.ctx(), src) {
        let dst = crate::palette::fit_rect(preview.shrink(4.0), w, h);
        p.with_clip_rect(preview).image(
            id,
            dst,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    // 三行文字：标题 / （已选中 ⇒ 真实层·库·图号，否则占位提示）/ 星标 + 收藏
    draw_sel(p, labs, "active_asset.caption");
    match pal.selected() {
        Some(key) => {
            let fam = pal
                .libs()
                .iter()
                .find(|e| e.lib == key.lib)
                .map(|e| e.family_label())
                .unwrap_or(tx::VAL_UNKNOWN);
            let stem = pal
                .libs()
                .iter()
                .find(|e| e.lib == key.lib)
                .map(|e| e.stem.clone())
                .unwrap_or_else(|| tx::VAL_UNKNOWN.to_owned());
            let y = card.top() + m::AA_LINE_LABEL_Y;
            let mut lab = DynLabel::new(
                "active_asset.label",
                tx::AA_SELECTED_TMPL,
                tx::fill(
                    tx::AA_SELECTED_TMPL,
                    &[
                        ("fam", fam),
                        ("stem", &stem),
                        // 1 基图号（= 选中帧的 0 基索引 + 1），见 `ui_text::AA_SELECTED_TMPL`
                        ("img", &(key.index + 1).to_string()),
                    ],
                ),
                LabelStyle::new(
                    m::AA_LINE_FONT,
                    pos2(text_x, y),
                    Align2::LEFT_CENTER,
                    Rect::from_min_max(pos2(text_x, y - 8.0), pos2(card.right() - 4.0, y + 8.0)),
                    t::ui().text,
                ),
            );
            lab.truncate_to_fit(p);
            lab.draw(p);
        }
        None => draw_sel(p, labs, "active_asset.label"),
    }
    draw_sel(p, labs, "active_asset.star");
    draw_sel(p, labs, "active_asset.pin");
}

/// 「分类」页签选中一枚印章时的「当前资源」：预览缩略图 + 两行真实信息。
///
/// 两行内容（任务书 §三.4）：`名称 · 中文分类` / `N 格 · 包围盒 (x0,y0,x1,y1)`。
/// 为什么拆两行：文字区只有 128 px 宽（见 [`m::STAMP_AA_FONT`] 的注释），
/// 一行塞不下 —— 宁可两行，也不让关键数字被截成 `…`。
///
/// 收藏（星标 + 加入收藏）在本页签**不画**：印章的收藏是任务书 §三.6 明确不做的功能，
/// 画一个点不动的按钮就是假功能。
fn draw_stamp_active(
    p: &Painter,
    card: &Rect,
    text_x: f32,
    preview: &Rect,
    stamps: &mut StampPanel,
    src: Option<&WilSource>,
    号段: crate::stamp_place::库号号段,
) {
    // 预览框：选中那枚的**真缩略图**（走与网格同一份缓存；取不到就留空，不画假图）
    if let Some((id, w, h)) = stamps.selected_texture(p.ctx(), src, 号段) {
        let dst = crate::palette::fit_rect(preview.shrink(4.0), w, h);
        p.with_clip_rect(*preview).image(
            id,
            dst,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    let Some(sel) = stamps.selected().cloned() else {
        return;
    };
    let line_clip = |y: f32| {
        Rect::from_min_max(
            pos2(text_x, y - 8.0),
            pos2(card.right() - 4.0, y + 8.0),
        )
    };
    let y1 = card.top() + m::AA_LINE_LABEL_Y;
    let y2 = card.top() + m::AA_LINE_PIN_Y;

    let mut name_line = DynLabel::new(
        "active_asset.stamp_name",
        tx::STAMP_AA_TMPL,
        tx::fill(
            tx::STAMP_AA_TMPL,
            &[("name", &sel.name), ("kind", &sel.kind_zh)],
        ),
        LabelStyle::new(
            m::STAMP_AA_FONT,
            pos2(text_x, y1),
            Align2::LEFT_CENTER,
            line_clip(y1),
            t::ui().text,
        ),
    );
    name_line.truncate_to_fit(p);
    name_line.draw(p);

    let geom = tx::fill(
        tx::STAMP_AA_GEOM_TMPL,
        &[
            (
                "n",
                &sel.cells
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| tx::VAL_UNKNOWN.to_owned()),
            ),
            (
                "x0",
                &sel.bounds
                    .map(|b| b.0.to_string())
                    .unwrap_or_else(|| tx::VAL_UNKNOWN.to_owned()),
            ),
            (
                "y0",
                &sel.bounds
                    .map(|b| b.1.to_string())
                    .unwrap_or_else(|| tx::VAL_UNKNOWN.to_owned()),
            ),
            (
                "x1",
                &sel.bounds
                    .map(|b| b.2.to_string())
                    .unwrap_or_else(|| tx::VAL_UNKNOWN.to_owned()),
            ),
            (
                "y1",
                &sel.bounds
                    .map(|b| b.3.to_string())
                    .unwrap_or_else(|| tx::VAL_UNKNOWN.to_owned()),
            ),
        ],
    );
    let mut geom_line = DynLabel::new(
        "active_asset.stamp_geom",
        tx::STAMP_AA_GEOM_TMPL,
        geom,
        LabelStyle::new(
            m::STAMP_AA_FONT,
            pos2(text_x, y2),
            Align2::LEFT_CENTER,
            line_clip(y2),
            t::ui().text_muted,
        ),
    );
    geom_line.truncate_to_fit(p);
    geom_line.draw(p);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_view::CanvasView;
    use egui::vec2;

    fn window() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(m::WINDOW_W, m::WINDOW_H))
    }

    /// **没打开地图 ⇒ 一个框都不画**（审计 E10：旧实现恒画一个 14×14 的假框）。
    #[test]
    fn 没打开地图时不画视口框() {
        let l = Layout::compute(window());
        let block = minimap_block_rect(&l);
        assert!(
            minimap_rects(block, &MiniMap::closed()).is_none(),
            "没地图时不许返回任何矩形（否则就会画出一个假框）"
        );
        // 地图尺寸为 0（坏文件）也一样
        assert!(minimap_rects(block, &MiniMap::new(Some((0, 0)), (0, 0, 10, 10))).is_none());
    }

    /// **地图块（布局基线）不动**：200×138，左上 (1222, 107)。
    ///
    /// 这是"不动布局、不动人眼基线"的钉子：块尺寸/位置一变，
    /// [`m::MINIMAP_MAP_W`] / [`m::MINIMAP_MAP_H`] 的截图基线就失效了。
    #[test]
    fn 地图块尺寸位置不变() {
        let l = Layout::compute(window());
        let block = minimap_block_rect(&l);
        assert_eq!(m::MINIMAP_MAP_W, 200.0, "地图块宽 200 是截图基线");
        assert_eq!(m::MINIMAP_MAP_H, 138.0, "地图块高 138 是截图基线");
        assert!(
            (block.width() - 200.0).abs() < 0.01 && (block.height() - 138.0).abs() < 0.01,
            "块必须是 200×138，实际 {:.3}×{:.3}",
            block.width(),
            block.height()
        );
        assert!(
            (block.left() - 1222.0).abs() < 0.01
                && (block.top() - 107.0).abs() < 0.01
                && (block.right() - 1422.0).abs() < 0.01
                && (block.bottom() - 245.0).abs() < 0.01,
            "块的四边必须还在截图基线上：{block:?}"
        );
    }

    /// 宽扁图（200×100 格 ⇒ 世界 9600×3200，3:1）等比 fit 进 `block.shrink(6)` 并居中。
    ///
    /// 四边用**具体数字**钉住（不是"非空"）：`block` 1222..1422 / 107..245，
    /// `shrink(6)` ⇒ 可用区 1228..1416 / 113..239（188×126），3:1 比可用区更宽
    /// ⇒ **宽度先顶满 188**，高度 188/3 ≈ 62.667 居中 ⇒ y 144.667..207.333。
    #[test]
    fn 宽扁地图等比fit并居中() {
        let l = Layout::compute(window());
        let block = minimap_block_rect(&l);
        let (map, _vb) = minimap_rects(block, &MiniMap::new(Some((200, 100)), (0, 0, 20, 20)))
            .expect("有地图就要有矩形");
        assert!(
            (map.width() / map.height() - 3.0).abs() < 1e-3,
            "地图区长宽比应为 3:1，实际 {:.3}",
            map.width() / map.height()
        );
        // 左右顶满 shrink(6) 后的可用区；上下按比例居中
        assert!((map.left() - 1228.0).abs() < 0.01, "左边 {}", map.left());
        assert!((map.right() - 1416.0).abs() < 0.01, "右边 {}", map.right());
        assert!((map.top() - 144.666_67).abs() < 0.01, "上边 {}", map.top());
        assert!((map.bottom() - 207.333_33).abs() < 0.01, "下边 {}", map.bottom());
        assert!((map.width() - 188.0).abs() < 0.01, "宽度 {}", map.width());
        assert!((map.height() - 62.666_664).abs() < 0.01, "高度 {}", map.height());
    }

    /// 高窄图（10×100 格 ⇒ 世界 480×3200，0.15:1）⇒ **高度先顶满**，横向居中。
    ///
    /// 这条钉的是"两种极端比例都等比、都居中"：宽 188/世界 480 与 高 126/世界 3200
    /// 取小者 ⇒ k = 126/3200，宽 18.9 居中 ⇒ x 1312.55..1331.45，y 113..239。
    #[test]
    fn 高窄地图等比fit并居中() {
        let l = Layout::compute(window());
        let block = minimap_block_rect(&l);
        let (map, _vb) = minimap_rects(block, &MiniMap::new(Some((10, 100)), (0, 0, 20, 20)))
            .expect("有地图就要有矩形");
        assert!((map.top() - 113.0).abs() < 0.01, "上边 {}", map.top());
        assert!((map.bottom() - 239.0).abs() < 0.01, "下边 {}", map.bottom());
        assert!((map.left() - 1312.55).abs() < 0.01, "左边 {}", map.left());
        assert!((map.right() - 1331.45).abs() < 0.01, "右边 {}", map.right());
        assert!((map.height() - 126.0).abs() < 0.01, "高度 {}", map.height());
        assert!((map.width() - 18.9).abs() < 0.01, "宽度 {}", map.width());
        // 长宽比仍是世界的比例（480:3200 = 0.15）
        assert!(
            (map.width() / map.height() - 0.15).abs() < 1e-3,
            "长宽比应为 0.15，实际 {:.4}",
            map.width() / map.height()
        );
    }

    /// `shrink(6)` **真的生效**：地图区最多 188×126，每边比旧块正好少 6。
    ///
    /// 旧实现是 fit 进整块（200×138），新实现 fit 进 `block.shrink(6)`（188×126）——
    /// 判据不是"矩形变小了"，而是**两个方向各自顶满时，块与图之差恰好 12（= 6×2）**。
    #[test]
    fn 地图区每边内缩六像素() {
        let l = Layout::compute(window());
        let block = minimap_block_rect(&l);
        let inner = block.shrink(m::MINIMAP_FIT_INSET);
        assert_eq!(m::MINIMAP_FIT_INSET, 6.0, "上游 `(ActualWidth - 12)` 是两边合计");
        // 宽扁图：宽度方向顶满 ⇒ 块宽 - 图宽 = 12（每边 6）
        let (wide, _) = minimap_rects(block, &MiniMap::new(Some((200, 100)), (0, 0, 20, 20))).unwrap();
        assert!(
            (block.width() - wide.width() - 12.0).abs() < 0.01,
            "宽扁图应每边内缩 6：块 {:.3} - 图 {:.3} = {:.3}",
            block.width(),
            wide.width(),
            block.width() - wide.width()
        );
        assert!((wide.center().y - block.center().y).abs() < 0.01, "纵向居中");
        // 高窄图：高度方向顶满 ⇒ 块高 - 图高 = 12（每边 6）
        let (tall, _) = minimap_rects(block, &MiniMap::new(Some((10, 100)), (0, 0, 20, 20))).unwrap();
        assert!(
            (block.height() - tall.height() - 12.0).abs() < 0.01,
            "高窄图应每边内缩 6：块 {:.3} - 图 {:.3} = {:.3}",
            block.height(),
            tall.height(),
            block.height() - tall.height()
        );
        assert!((tall.center().x - block.center().x).abs() < 0.01, "横向居中");
        // 任何比例都必须完整落在可用区里，且至少一个方向顶满（fit 的本义）
        for (w, h) in [(200_u16, 100_u16), (10, 100), (100, 100), (700, 700), (1, 1)] {
            let (map, _) = minimap_rects(block, &MiniMap::new(Some((w, h)), (0, 0, 20, 20)))
                .expect("有地图就要有矩形");
            assert!(
                map.left() >= inner.left() - 0.01 && map.right() <= inner.right() + 0.01,
                "{w}×{h} 横向越出可用区：{map:?}"
            );
            assert!(
                map.top() >= inner.top() - 0.01 && map.bottom() <= inner.bottom() + 0.01,
                "{w}×{h} 纵向越出可用区：{map:?}"
            );
            let fills_w = (map.width() - inner.width()).abs() < 0.05;
            let fills_h = (map.height() - inner.height()).abs() < 0.05;
            assert!(fills_w || fills_h, "{w}×{h} 两轴都没顶满：{map:?}");
        }
    }

    /// `minimap_world_at`：四角 / 中心 / 区外四方向 / 空矩形（上游 `MapMinimap.Avalonia.cs:70-75`）。
    ///
    /// 用**真实**的 `minimap_rects` 输出当 `map`（不是另编一个矩形），
    /// 于是"四角 ⇒ 世界四角"同时验证了 fit 与换算两段的组合。
    #[test]
    fn 小地图像素换算世界像素() {
        let l = Layout::compute(window());
        let block = minimap_block_rect(&l);
        // 200×100 格 ⇒ 世界 9600×3200（与调用方给的口径一致：W*48, H*32）
        let (map, _) = minimap_rects(block, &MiniMap::new(Some((200, 100)), (0, 0, 20, 20))).unwrap();
        let world = (200.0 * m::CELL_W, 100.0 * m::CELL_H);
        assert_eq!(world, (9600.0, 3200.0));

        // ① 四角 ⇒ 恰好世界四角
        let tl = minimap_world_at(pos2(map.left(), map.top()), map, world);
        assert!((tl.0 - 0.0).abs() < 1e-3 && (tl.1 - 0.0).abs() < 1e-3, "左上角 {tl:?}");
        let tr = minimap_world_at(pos2(map.right(), map.top()), map, world);
        assert!((tr.0 - 9600.0).abs() < 1e-2 && (tr.1 - 0.0).abs() < 1e-3, "右上角 {tr:?}");
        let bl = minimap_world_at(pos2(map.left(), map.bottom()), map, world);
        assert!((bl.0 - 0.0).abs() < 1e-3 && (bl.1 - 3200.0).abs() < 1e-2, "左下角 {bl:?}");
        let br = minimap_world_at(pos2(map.right(), map.bottom()), map, world);
        assert!((br.0 - 9600.0).abs() < 1e-2 && (br.1 - 3200.0).abs() < 1e-2, "右下角 {br:?}");

        // ② 中心 ⇒ 一半
        let c = minimap_world_at(map.center(), map, world);
        assert!(
            (c.0 - 4800.0).abs() < 1e-2 && (c.1 - 1600.0).abs() < 1e-2,
            "中心应为 (4800, 1600)，实际 {c:?}"
        );

        // ③ 区外四方向 ⇒ clamp 到边界（不许负值 / 不许越界）
        let left = minimap_world_at(pos2(map.left() - 50.0, map.center().y), map, world);
        let top = minimap_world_at(pos2(map.center().x, map.top() - 50.0), map, world);
        let right = minimap_world_at(pos2(map.right() + 50.0, map.center().y), map, world);
        let bottom = minimap_world_at(pos2(map.center().x, map.bottom() + 50.0), map, world);
        assert!((left.0 - 0.0).abs() < 1e-3 && (left.1 - 1600.0).abs() < 1e-2, "左侧外 {left:?}");
        assert!((top.0 - 4800.0).abs() < 1e-2 && (top.1 - 0.0).abs() < 1e-3, "上方外 {top:?}");
        assert!((right.0 - 9600.0).abs() < 1e-2 && (right.1 - 1600.0).abs() < 1e-2, "右侧外 {right:?}");
        assert!(
            (bottom.0 - 4800.0).abs() < 1e-2 && (bottom.1 - 3200.0).abs() < 1e-2,
            "下方外 {bottom:?}"
        );
        for (name, v) in [("左", left), ("上", top), ("右", right), ("下", bottom)] {
            assert!(v.0 >= 0.0 && v.0 <= 9600.0, "{name}侧外 x 越界：{}", v.0);
            assert!(v.1 >= 0.0 && v.1 <= 3200.0, "{name}侧外 y 越界：{}", v.1);
        }

        // ④ 空矩形（没打开地图）⇒ (0,0)，绝不除零
        for empty in [
            Rect::NOTHING,
            Rect::from_min_size(pos2(10.0, 20.0), vec2(0.0, 0.0)),
            Rect::from_min_size(pos2(10.0, 20.0), vec2(-5.0, 30.0)),
        ] {
            assert_eq!(
                minimap_world_at(pos2(500.0, 500.0), empty, world),
                (0.0, 0.0),
                "空矩形 {empty:?} 必须返回 (0,0)"
            );
        }
    }

    /// 视口框 = 可见格窗口按同一比例映射（含"框随 pan 移动"这条 E10 验收）。
    #[test]
    fn 视口框随真实可见窗口移动() {
        let l = Layout::compute(window());
        let block = minimap_block_rect(&l);
        let canvas = l.viewport;

        let mut v = CanvasView::default();
        v.pan_x = 0.0;
        v.pan_y = 0.0;
        let (_, a) = minimap_rects(
            block,
            &MiniMap::new(Some((100, 100)), v.visible_cells(canvas)),
        )
        .unwrap();

        // 拖两下（走的是与鼠标拖动完全相同的 `pan_by`）
        v.pan_by(vec2(-600.0, -400.0));
        v.pan_by(vec2(-600.0, -400.0));
        let (_, b) = minimap_rects(
            block,
            &MiniMap::new(Some((100, 100)), v.visible_cells(canvas)),
        )
        .unwrap();

        assert!(
            b.left() > a.left() + 1.0 && b.top() > a.top() + 1.0,
            "往右下拖之后视口框必须往右下走：a={a:?} b={b:?}"
        );
        assert!(
            b.width() > 1.0 && b.height() > 1.0,
            "框要有可见尺寸：{b:?}"
        );
    }

    /// 视口比整图还大（缩到最小/看全图）⇒ 框 = 整块地图区，**如实不夸大**。
    #[test]
    fn 视口大于整图时框等于地图区() {
        let l = Layout::compute(window());
        let block = minimap_block_rect(&l);
        let (map, vb) = minimap_rects(block, &MiniMap::new(Some((10, 10)), (-50, -50, 200, 200)))
            .expect("有地图就要有矩形");
        assert!(
            (vb.left() - map.left()).abs() < 0.01
                && (vb.top() - map.top()).abs() < 0.01
                && (vb.right() - map.right()).abs() < 0.01
                && (vb.bottom() - map.bottom()).abs() < 0.01,
            "视口盖住整图时框必须正好等于地图区：map={map:?} vb={vb:?}"
        );
    }

    /// 视口未知（`w/h == 0`）⇒ 只画地图区、不画框（绝不猜一个位置）。
    #[test]
    fn 视口未知时只画地图区() {
        let l = Layout::compute(window());
        let block = minimap_block_rect(&l);
        let (map, vb) = minimap_rects(block, &MiniMap::new(Some((100, 100)), (0, 0, 0, 0)))
            .expect("有地图就要有矩形");
        assert!(map.is_positive());
        assert!(!vb.is_positive(), "视口未知时不许画框：{vb:?}");
    }
}
