//! **悬停地砖信息浮层**（视图 → 地砖信息悬浮）。上游实现在 `MainWindow.CellHover.cs`。
//!
//! # 一、上游契约（逐条对照，改这里前先回核）
//!
//! | 环节 | 出处 | 内容 |
//! |---|---|---|
//! | 菜单开关 | `MainWindow.axaml` 的 `ViewMenu` 段（`CellHoverToggle`） | `ToggleType="CheckBox"`、**没写 `IsChecked`**（缺省不勾）；位置在深色模式之后、单元格网格之前，且**它前面有一条分隔线** |
//! | 触发 | `_upstream/CrystalStudio/MainWindow.CellHover.cs:9-19` | 指针移动 / 悬停 / 文档变化 / 窗口失焦 ⇒ 刷新；指针移出视口 ⇒ 隐藏 |
//! | 显示条件 | `:31-42` | 开关勾上 **且** 指针在视口内 **且** 该格在地图内；任一不满足 ⇒ 隐藏（**不画空框**） |
//! | 内容 | `:56-63` | ① 坐标行（`:58` 的 `"Cell {0:N0}, {1:N0}"` ⇒ **千分位**，见 [`crate::cellinfo::千分位`]） ② 可行走行（不可行走用**红**） ③ 六组「标题: 值」 |
//! | 几何 | `_upstream/CrystalStudio/MainWindow.axaml:87` + `_upstream/CrystalStudio/MainWindow.CellHover.cs:45-53` | 宽 **350** / Padding **12** / CornerRadius **6**；面板放在指针 **+16 右 / +8 下**，右侧越界就**翻到左侧**，四边各夹 **8**；最大高 = 窗口高 **−16** |
//!
//! # 二、本仓实现的三段（取数 / 排版 / 绘制分开，与既有模块同一纪律）
//!
//! 1. **取数**：[`crate::cellinfo::hover_text`]（纯函数，无窗口也能测）；
//! 2. **排版**：[`layout`]（纯几何 + 逐行文字，[`rect_for`] 是位置算式的唯一实现处）；
//! 3. **绘制**：[`draw`]（底 → 圆角 → 1px 边 → 逐行字），文字标签同时供
//!    [`crate::text_fit`] 的排版自检取用（[`dyn_labels`]）。
//!
//! # 三、与上游的两处差异（如实登记，不是漏做）
//!
//! 1. **六组明细每组压成一行**：官方每组是一条「标题: 值」（组内本来就是单行）；
//!    本仓常驻检查器的值模板有为竖向面板排的 `\n`，浮层只有 350px 宽，照搬会把框撑到
//!    十几行 ⇒ 压成一行（信息一个不少），见 [`crate::ui_text::HOVER_DETAIL_JOIN`]。
//! 2. **「可行走」的红色来自主题板**：官方「不可行走」直接写 `Brushes.Red`（不经
//!    `ThemeService`），本仓放进色板的 `hover_blocked_text`（两套板同值，由
//!    `theme` 的 `主题无关字段取官方常量` 钉住 RGB）；「可行走」用官方 `ThemeE5E9F0`
//!    （= 本板的 `text`）。
//!
//! **坐标行千分位已不是差异**（A16 已对齐）：上游 `:58` 是 `"Cell {0:N0}, {1:N0}"`，
//! 本仓现按 `N0` 输出（`单元格 1,234,5,678` 那种）。格式化在
//! [`crate::cellinfo::千分位`] 里做 —— [`tx::fill`] 只做 `{name}` 字面替换、**不认 `N0`**，
//! 所以 [`crate::ui_text::HOVER_COORD_TMPL`] 里仍然只有 `{x}`/`{y}` 两个占位符。
//!
//! # 四、**不持久化**
//!
//! 上游 `CellHoverToggle` 只是运行期勾选态（`_upstream/CrystalStudio/MainWindow.CellHover.cs:31` 每帧读 `IsChecked`），
//! `SettingsStore` 里没有它。本仓同口径：开关只在内存里
//! （[`crate::menu::settings::Settings`] 仍然只有 `dark` + `recent` 两个字段）。

use egui::{pos2, vec2, Align2, Painter, Pos2, Rect, Vec2};

use crate::cellinfo::HoverText;
use crate::metrics as m;
use crate::paint as pt;
use crate::text_fit::{DynLabel, LabelStyle};
use crate::theme as t;
use crate::ui_text as tx;

/// 一行浮层文字的角色（决定字号 / 行高 / 颜色 / 裁剪盒）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// 坐标行（`单元格 x,y`）。
    Coord,
    /// 可行走状态行（`可行走` / `不可行走`）。
    Walkable,
    /// 一组明细（`标题: 值`）。
    Detail,
}

/// 排版结果：面板矩形 + 逐行 (角色, 文字)。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Panel {
    /// 面板矩形（已按 [`rect_for`] 夹进窗口内）。
    pub rect: Rect,
    /// 逐行内容（顺序 = 绘制顺序）。
    pub rows: Vec<(Row, String)>,
    /// 是否因为窗口太矮而丢掉了尾部明细（正常恒 `false`）。
    pub truncated: bool,
}

impl Panel {
    /// 该行的高（坐标/状态行与明细行不同档）。
    fn row_h(kind: Row) -> f32 {
        match kind {
            Row::Detail => m::HOVER_DETAIL_LINE_H,
            Row::Coord | Row::Walkable => m::HOVER_HEAD_LINE_H,
        }
    }

    /// 该行的字号。
    fn row_font(kind: Row) -> f32 {
        match kind {
            Row::Detail => m::HOVER_DETAIL_FONT,
            Row::Coord | Row::Walkable => m::HOVER_HEAD_FONT,
        }
    }

    /// 该行的文字颜色。
    fn row_color(kind: Row) -> egui::Color32 {
        match kind {
            Row::Coord => t::ui().text,
            Row::Detail => t::ui().text_muted,
            // 官方 `_upstream/CrystalStudio/MainWindow.CellHover.cs:61`：不可行走 = `Brushes.Red`，可行走 = `ThemeE5E9F0`
            Row::Walkable => t::ui().text,
        }
    }

    /// 某一行的**裁剪盒**（绘制与 [`dyn_labels`] 共用同一份计算）。
    fn line_clip(&self, index: usize) -> Rect {
        let mut top = self.rect.top() + m::HOVER_PAD;
        for (i, (kind, _)) in self.rows.iter().enumerate() {
            let h = Self::row_h(*kind);
            if i == index {
                return Rect::from_min_max(
                    pos2(self.rect.left() + m::HOVER_PAD, top),
                    pos2(
                        (self.rect.right() - m::HOVER_PAD).max(self.rect.left() + m::HOVER_PAD + 1.0),
                        top + h,
                    ),
                );
            }
            top += h;
        }
        Rect::NOTHING
    }
}

/// **位置算式**（官方 `_upstream/CrystalStudio/MainWindow.CellHover.cs:47-53` 的逐条翻译），纯几何、可单测。
///
/// ```text
/// left = anchor.x + 16                 // :49
/// if left + w > screen.right() - 8     // :51  ⇒ 右侧放不下就翻到指针左边
///     left = anchor.x - w - 16
/// left = clamp(left, 8, max(8, screen.right() - w - 8))     // :52
/// top  = clamp(anchor.y + 8, 8, max(8, screen.bottom() - h - 8))  // :53
/// ```
///
/// `screen` 是**窗口客户区**（官方的浮层 Canvas 是 `Grid.RowSpan="5"` 的整窗叠加层，
/// 不是画布矩形）—— 所以靠近状态栏/右停靠区时浮层仍然贴着指针走，只被窗口边夹住。
pub fn rect_for(anchor: Pos2, size: Vec2, screen: Rect) -> Rect {
    let w = size.x;
    let h = size.y;
    let mut left = anchor.x + m::HOVER_OFFSET_X;
    if left + w > screen.right() - m::HOVER_EDGE_PAD {
        left = anchor.x - w - m::HOVER_OFFSET_X;
    }
    let left_max = (screen.right() - w - m::HOVER_EDGE_PAD).max(m::HOVER_EDGE_PAD);
    let left = left.clamp(m::HOVER_EDGE_PAD, left_max);
    let top_max = (screen.bottom() - h - m::HOVER_EDGE_PAD).max(m::HOVER_EDGE_PAD);
    let top = (anchor.y + m::HOVER_OFFSET_Y).clamp(m::HOVER_EDGE_PAD, top_max);
    Rect::from_min_size(pos2(left, top), vec2(w, h))
}

/// 面板宽度：官方 350，但**屏比它窄时收到「屏宽 − 2×8」**（官方靠 `ClipToBounds` 兜底，
/// 本仓没有裁剪盒，只能自己收窄，否则会画出窗口外）。
pub fn panel_w(screen: Rect) -> f32 {
    (screen.width() - 2.0 * m::HOVER_EDGE_PAD).clamp(1.0, m::HOVER_W)
}

/// 排版：把取好的文字摆成一块面板；**没有任何可显示行时返回 `None`**（不画空框）。
///
/// 高度上限照官方 `:45`（窗口高 − 16）：放不下时**从尾部丢明细行**并在最后一行补
/// [`tx::TRUNC_SUFFIX`]（先保坐标与可行走，那两行是"这一格是什么"的核心）。
pub fn layout(text: &HoverText, anchor: Pos2, screen: Rect) -> Option<Panel> {
    if screen.width() <= 0.0 || screen.height() <= 0.0 {
        return None;
    }
    let mut rows: Vec<(Row, String)> = Vec::with_capacity(2 + text.details.len());
    rows.push((Row::Coord, text.coords.clone()));
    rows.push((Row::Walkable, text.walkable_text.to_owned()));
    for d in &text.details {
        rows.push((Row::Detail, d.clone()));
    }
    if rows.is_empty() {
        return None;
    }

    let w = panel_w(screen);
    let max_h = (screen.height() - m::HOVER_MAX_H_MARGIN).max(m::HOVER_HEAD_LINE_H);
    let mut truncated = false;
    loop {
        let h = 2.0 * m::HOVER_PAD + rows.iter().map(|(k, _)| Panel::row_h(*k)).sum::<f32>();
        if h <= max_h || rows.len() <= 2 {
            let rect = rect_for(anchor, vec2(w, h.min(max_h)), screen);
            return Some(Panel {
                rect,
                rows,
                truncated,
            });
        }
        // 只丢明细行（坐标/可行走不丢）；丢掉之后在**新的末行**补省略号
        rows.pop();
        truncated = true;
        if let Some((kind, last)) = rows.last_mut() {
            if *kind == Row::Detail && !last.ends_with(tx::TRUNC_SUFFIX) {
                last.push_str(tx::TRUNC_SUFFIX);
            }
        }
    }
}

/// 逐行的动态标签（**参与 `text_fit` 的排版自检**：文字不许溢出裁剪盒）。
pub fn dyn_labels(panel: &Panel) -> Vec<DynLabel> {
    panel
        .rows
        .iter()
        .enumerate()
        .map(|(i, (kind, text))| {
            let clip = panel.line_clip(i);
            let (name, tmpl) = match kind {
                Row::Coord => ("hover.coords", tx::HOVER_COORD_TMPL),
                Row::Walkable => (
                    "hover.walkable",
                    if text == tx::HOVER_WALKABLE {
                        tx::HOVER_WALKABLE
                    } else {
                        tx::HOVER_NOT_WALKABLE
                    },
                ),
                Row::Detail => ("hover.detail", tx::HOVER_DETAIL_TMPL),
            };
            DynLabel::new(
                name,
                tmpl,
                text.clone(),
                LabelStyle::new(
                    Panel::row_font(*kind),
                    pos2(clip.left(), clip.center().y),
                    Align2::LEFT_CENTER,
                    clip,
                    match kind {
                        // 不可行走那一行必须**真的红**：颜色是这条功能的语义之一
                        Row::Walkable if *text != tx::HOVER_WALKABLE => t::ui().hover_blocked_text,
                        k => Panel::row_color(*k),
                    },
                ),
            )
        })
        .collect()
}

/// 画浮层：底 → 1px 圆角边 → 逐行文字。
///
/// ⚠ 必须在全部面板/画布**之后**调用（官方 `ZIndex="100"`），否则会被后画的纹理顶掉。
pub fn draw(p: &Painter, panel: &Panel) {
    let r = panel.rect;
    pt::fill_round(p, r, m::HOVER_RADIUS, t::ui().panel_bg);
    pt::border(p, r, t::ui().input_border);
    for mut lab in dyn_labels(panel) {
        lab.truncate_to_fit(p);
        lab.draw(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cellinfo::CellLines;

    fn 窗口() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(m::WINDOW_W, m::WINDOW_H))
    }

    /// 一份最小可用的悬停内容（六条明细，文字都不长）。
    fn 内容(walkable: bool) -> HoverText {
        HoverText {
            coords: tx::fill(tx::HOVER_COORD_TMPL, &[("x", "265"), ("y", "261")]),
            walkable,
            walkable_text: if walkable {
                tx::HOVER_WALKABLE
            } else {
                tx::HOVER_NOT_WALKABLE
            },
            details: (0..6)
                .map(|i| tx::fill(tx::HOVER_DETAIL_TMPL, &[("title", "地表"), ("value", &i.to_string())]))
                .collect(),
        }
    }

    /// **右侧放不下就翻到指针左边**（官方 `:51`），且四边永不越出窗口。
    #[test]
    fn 靠右时翻到左侧() {
        let screen = 窗口();
        let anchor = pos2(screen.right() - 4.0, 100.0);
        let r = rect_for(anchor, vec2(m::HOVER_W, 100.0), screen);
        assert!(
            r.right() <= screen.right() - m::HOVER_EDGE_PAD + 0.01,
            "右侧放不下时不许顶出窗口：{r:?}"
        );
        assert!(
            r.right() <= anchor.x - m::HOVER_OFFSET_X + 0.01,
            "翻侧后必须整体在指针左边：{r:?} vs {anchor:?}"
        );
        // 正常位置（左边有地方）不翻侧
        let r2 = rect_for(pos2(100.0, 100.0), vec2(m::HOVER_W, 100.0), screen);
        assert!((r2.left() - (100.0 + m::HOVER_OFFSET_X)).abs() < 0.01);
        assert!((r2.top() - (100.0 + m::HOVER_OFFSET_Y)).abs() < 0.01);
    }

    /// 四角都要夹进窗口（官方 `:52-53` 的 `Math.Clamp`）。
    #[test]
    fn 四角都夹进窗口() {
        let screen = 窗口();
        let size = vec2(m::HOVER_W, 120.0);
        for anchor in [
            pos2(0.0, 0.0),
            pos2(screen.right(), 0.0),
            pos2(0.0, screen.bottom()),
            pos2(screen.right(), screen.bottom()),
        ] {
            let r = rect_for(anchor, size, screen);
            assert!(r.left() >= m::HOVER_EDGE_PAD - 0.01, "左边越界：{r:?}");
            assert!(r.top() >= m::HOVER_EDGE_PAD - 0.01, "上边越界：{r:?}");
            assert!(
                r.right() <= screen.right() - m::HOVER_EDGE_PAD + 0.01,
                "右边越界：{r:?}"
            );
            assert!(
                r.bottom() <= screen.bottom() - m::HOVER_EDGE_PAD + 0.01,
                "下边越界：{r:?}"
            );
        }
    }

    /// 屏比面板窄时面板跟着收窄（否则会画出窗口外）。
    #[test]
    fn 窄屏收窄面板() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 600.0));
        assert!((panel_w(screen) - (200.0 - 2.0 * m::HOVER_EDGE_PAD)).abs() < 0.01);
        assert!((panel_w(窗口()) - m::HOVER_W).abs() < 0.01);
    }

    /// **三行结构**：坐标 → 可行走 → 六条明细；不可行走时那一行必须走红色。
    #[test]
    fn 行序与红字() {
        let screen = 窗口();
        let anchor = pos2(300.0, 300.0);
        let p = layout(&内容(true), anchor, screen).expect("有内容就必须排版");
        assert_eq!(p.rows.len(), 8, "2 行头 + 6 条明细");
        assert_eq!(p.rows[0].0, Row::Coord);
        assert_eq!(p.rows[1].0, Row::Walkable);
        assert!(p.rows[2..].iter().all(|(k, _)| *k == Row::Detail));
        // 面板在指针**右下**（+16 / +8），指针自身不在框内 —— 官方 `_upstream/CrystalStudio/MainWindow.CellHover.cs:49,53` 同款
        assert!(
            p.rect.left() >= anchor.x + m::HOVER_OFFSET_X - 0.01,
            "面板必须在指针右侧：{p:?}"
        );
        assert!(p.rect.top() >= anchor.y + m::HOVER_OFFSET_Y - 0.01, "面板必须在指针下方");

        let 红 = layout(&内容(false), anchor, screen).expect("排版");
        let labels = dyn_labels(&红);
        assert_eq!(labels[1].text, tx::HOVER_NOT_WALKABLE);
        assert_eq!(
            labels[1].color,
            t::ui().hover_blocked_text,
            "不可行走那一行必须是官方红（`Brushes.Red`）"
        );
        let 绿 = dyn_labels(&layout(&内容(true), anchor, screen).expect("排版"));
        assert_eq!(绿[1].color, t::ui().text, "可行走那一行是官方 ThemeE5E9F0");
        assert_ne!(labels[1].text, 绿[1].text, "两个状态词必须不同");
    }

    /// 窗口太矮时**只丢明细**（坐标与可行走必须留着），且补省略号。
    #[test]
    fn 太矮时丢明细保头两行() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 60.0));
        let p = layout(&内容(true), pos2(10.0, 10.0), screen).expect("排版");
        assert!(p.truncated, "60px 高的窗口放不下 8 行 ⇒ 必须标记截断");
        assert_eq!(p.rows[0].0, Row::Coord, "坐标行不许丢");
        assert_eq!(p.rows[1].0, Row::Walkable, "可行走行不许丢");
        assert!(p.rect.height() <= screen.height(), "面板不许超出窗口");
    }

    /// 真正画一遍：不 panic，且面板底色在（含圆角填充与 1px 边）。
    #[test]
    fn 能画出来() {
        let ctx = crate::fonts::measure_ctx();
        let screen = 窗口();
        let p = egui::Painter::new(ctx, egui::LayerId::debug(), screen);
        let panel = layout(&内容(true), pos2(300.0, 300.0), screen).expect("排版");
        draw(&p, &panel);
    }

    /// `CellLines` 的六行与浮层六条明细**一一对应**（防止将来检查器加行、浮层漏接）。
    #[test]
    fn 明细条数跟检查器一致() {
        let lines = CellLines::unknown();
        assert_eq!(lines.values.len(), tx::INSPECTOR_KEYS.len());
        let text = crate::cellinfo::hover_text(0, 0, &mir2_map::Cell::default(), &lines);
        assert_eq!(text.details.len(), lines.values.len());
    }
}
