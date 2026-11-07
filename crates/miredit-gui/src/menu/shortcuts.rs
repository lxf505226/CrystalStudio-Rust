//! 帮助 → 键盘快捷键：**只列当前真实绑定**的只读浮层。
//!
//! # 为什么必须"只列真的"
//!
//! 官方 `KeyboardShortcuts` 对话框会列出它自己的全套键（含 `Ctrl+N` / `Ctrl+Shift+S` /
//! `Ctrl+Shift+N` 等）。本仓**没有**实现那些功能（见
//! [`crate::menu::model::State::Todo`]），把它们列出来就是**假信息**：
//! 用户照着按，什么都不会发生。
//!
//! ⇒ 本表与下列实现**逐条对应**，加一条必须先在代码里真的绑上：
//!
//! | 表里的行 | 实现位置 |
//! |---|---|
//! | `Ctrl+S` / `Ctrl+O` | [`crate::app::MireditApp::handle_keys`]（打开地图走 `Overlay::OpenMap`）|
//! | `Ctrl+Shift+S`（另存为）| 同上 → `dispatch_menu(Action::SaveAs)` → `Overlay::SaveAs`（2026-11-05 起真实现）|
//! | `Ctrl+Z` / `Ctrl+Shift+Z` / `Ctrl+Y` | 同上（重做同时接受官方键与本仓旧键）|
//! | `F` / `M` / `O` / `[` `]` / `Enter` | 同上 + [`crate::selection_ops`] + [`crate::stamp_place`] |
//! | `B` / `P` / `F` / `E` / `M` / `O` / `V` / `K`（工具，**合并一行**）| [`crate::app::MireditApp::handle_keys`]（2026-11-07 起真绑定；`P`/`K` 的**工具**本身尚未接线，表里如实标注）|
//! | `C`（低位/高位阻挡标记同开同关）| 同上（上游 `MainWindow.axaml.cs:997`）|
//! | `W` `A` `S` `D`（平移，每次 48 屏幕像素）| 同上 → [`crate::canvas_view::CanvasView::pan_by_screen`] |
//!
//! ⚠ **`W A S D` 的按住语义**（R41 定稿）：**每个按键事件走一步（48 屏幕像素），每帧最多一步**。
//! `i.key_pressed()` 含 egui 的 key-repeat 事件 ⇒ 按住不放时系统每发一次重复事件再走一步
//! （与上游 WinForms `KeyDown` 的重复行为一致）；但没有按键事件的帧**不会**自己漂移
//! （不是"按帧 × 速度"那种积分）。表里写「每次 48 屏幕像素」指的就是这个"每次"。
//! | `G`（单元格网格） | 同上 → [`crate::menu::action::apply`] → [`crate::grid`]（2026-11-05 起真实现）|
//! | `Ctrl+C/X/V` / `Delete` / `Ctrl+Shift+V` | [`crate::app::MireditApp::handle_sel_keys`] |
//! | 空格+拖动 / 中键拖动 / 滚轮 | [`crate::app::MireditApp::interact`] |
//! | 按住 `Alt` | [`crate::app::MireditApp::stamp_preview`] |
//! | `F1` / `Esc` | [`crate::app::MireditApp::handle_keys`] + [`crate::menu::handle_key_with`] |
//!
//! ⚠ 菜单自身的导航键（弹层展开时的 `↑ ↓ ← →` / `Enter` / `Esc`）**不列在这里**：
//! 它们只在弹层展开时有效（[`crate::menu::handle_key_with`]），不是全局绑定。
//! 弹层的交互契约（含 `Enter` 遇容器项只展开、`→` 进子菜单、`Esc` 逐级收起）写在
//! [`crate::menu`] 的模块文档里。

use egui::{pos2, vec2, Align2, Color32, Painter, Rect};

use crate::layout::Layout;
use crate::metrics as m;
use crate::paint as pt;
use crate::theme as t;
use crate::ui_text as tx;

/// 「键」列的文字色 **#A8B7F6** = `rgb(168, 183, 246)` —— **深色主题专用**（见下）。
///
/// 上游 `MainWindow.axaml.cs:1248`：
/// `var shortcut = new TextBlock { Text = LocalText.T(key), Foreground = new ImmutableSolidColorBrush(Color.FromRgb(168, 183, 246)) };`
/// —— **写死的字面量，不经 `ThemeService`**。
///
/// ⚠ **订正（2026-11-08）**——本节原文是：
/// > —— **写死的字面量，不经 `ThemeService`** ⇒ 主题无关常量（两套色板同值）。
///
/// **「主题无关」这个推论错了**：字面量不经 `ThemeService` 只说明**上游没给它做浅色版**，
/// 不等于「它在浅色下也合适」。上游 `EditorWindow` 只有深色一套，所以上游**根本没有「浅色键色」这个真值**。
/// 实测对比度（`--shot --shortcuts` 像素取样）：
/// - `#A8B7F6` on 深色 `panel_bg #1C2028` = **8.37 : 1** ✅ 好；
/// - `#A8B7F6` on 浅色 `panel_bg #F5F6F7` = **1.80 : 1** ❌ 远低于 WCAG AA 4.5:1，**键列几乎看不见**。
///
/// ⇒ 现行口径（见 [`draw`] 里的 `key_color`）：**深色取本常量**（上游真值）、
/// **浅色沿用本仓改动前的原值 `pal.text`**（不是新发明的颜色）。
/// 「浅色键色」属**上游无此物 ⇒ 本仓自定**，已登记待拍板（台账 §8.77）。
///
/// 所以它落在本模块，**不进** `crate::theme` 的调色板：那张表是"官方命名刷"的真源，
/// 而这个色在官方代码里就是 `ShortcutsClick` 里的一个字面量，没有刷名。
///
/// ⚠ 别拿 `theme` 的 `tab_selected_underline`（深色 `#A8B6FF`）顶替：
/// 差 9 个蓝阶（246 vs 255），是**两个不同的数**。
pub const KEY_COLOR: Color32 = Color32::from_rgb(168, 183, 246);

/// 标题/说明与第一组之间的 6px 呼吸（**本仓独有**：上游没有说明行，也没有这条分隔线）。
pub const SEP_GAP: f32 = 6.0;

/// 组与组之间的 4px 间隔（**本仓独有**：上游是单列无分组）。
pub const GROUP_GAP: f32 = 4.0;

/// 键文字相对内容左边的缩进 10（**本仓独有**：上游用 `DockPanel` 自动排，无缩进）。
pub const KEY_INDENT: f32 = 10.0;

/// 一条绑定：`键` + `做什么`。
#[derive(Clone, Copy, Debug)]
pub struct Row {
    /// 键（`Ctrl+S` / `滚轮` …）。
    pub keys: &'static str,
    /// 做什么（中文，来自 `ui_text`）。
    pub what: &'static str,
}

/// 一组绑定。
#[derive(Clone, Copy, Debug)]
pub struct Group {
    /// 组名。
    pub title: &'static str,
    /// 组内各行。
    pub rows: &'static [Row],
}

/// 全表（**只含真实绑定**，见模块文档）。
pub const GROUPS: &[Group] = &[
    Group {
        title: tx::SC_GROUP_FILE,
        rows: &[
            Row {
                keys: tx::KEY_CTRL_S,
                what: tx::SC_SAVE,
            },
            Row {
                // 官方 `Ctrl+Shift+S`：2026-11-05 起真绑定（打开另存为路径输入模态）
                keys: tx::KEY_CTRL_SHIFT_S,
                what: tx::SC_SAVE_AS,
            },
            Row {
                keys: tx::KEY_CTRL_O,
                what: tx::SC_OPEN,
            },
        ],
    },
    Group {
        title: tx::SC_GROUP_EDIT,
        rows: &[
            Row {
                keys: tx::KEY_CTRL_Z,
                what: tx::SC_UNDO,
            },
            Row {
                keys: tx::KEY_CTRL_SHIFT_Z,
                what: tx::SC_REDO,
            },
        ],
    },
    Group {
        title: tx::SC_GROUP_VIEW,
        rows: &[
            Row {
                keys: tx::KEY_G,
                what: tx::SC_TOGGLE_GRID,
            },
            Row {
                keys: tx::KEY_C,
                what: tx::SC_TOGGLE_COLLISION,
            },
            Row {
                keys: tx::KEY_WASD,
                what: tx::SC_PAN_KEYS,
            },
        ],
    },
    Group {
        title: tx::SC_GROUP_TOOLS,
        rows: &[
            // R40：八个工具键**合并成一行**（官方 `MainWindow.axaml.cs:989` 本来就是同一行
            // `switch`；分八行会把浮层撑爆，也读不出"这是一组同类绑定"）
            Row {
                keys: tx::KEY_TOOL_KEYS,
                what: tx::SC_TOOL_KEYS,
            },
            Row {
                keys: tx::KEY_BRACKETS,
                what: tx::SC_BRUSH_SIZE,
            },
            Row {
                keys: tx::KEY_ENTER,
                what: tx::SC_ARM_STAMP,
            },
        ],
    },
    Group {
        title: tx::SC_GROUP_SELECTION,
        rows: &[
            Row {
                keys: tx::KEY_CTRL_C,
                what: tx::SC_COPY,
            },
            Row {
                keys: tx::KEY_CTRL_X,
                what: tx::SC_CUT,
            },
            Row {
                keys: tx::KEY_CTRL_V,
                what: tx::SC_PASTE,
            },
            Row {
                keys: tx::KEY_DELETE,
                what: tx::SC_DELETE,
            },
            Row {
                keys: tx::KEY_CTRL_SHIFT_V,
                what: tx::SC_FILL_SEL,
            },
        ],
    },
    Group {
        title: tx::SC_GROUP_MOUSE,
        rows: &[
            Row {
                keys: tx::KEY_SPACE_DRAG,
                what: tx::SC_PAN,
            },
            Row {
                keys: tx::KEY_MIDDLE_DRAG,
                what: tx::SC_PAN,
            },
            Row {
                keys: tx::KEY_WHEEL,
                what: tx::SC_WHEEL_ZOOM,
            },
            Row {
                keys: tx::KEY_ALT,
                what: tx::SC_ALT_OUTLINE,
            },
            Row {
                keys: tx::KEY_F1,
                what: tx::SC_OPEN_OVERLAY,
            },
            Row {
                keys: tx::KEY_ESC,
                what: tx::SC_ESC,
            },
        ],
    },
];

/// 全表行数（标题 + 各行）。
pub fn line_count() -> usize {
    GROUPS
        .iter()
        .map(|g| 1 + g.rows.len())
        .sum()
}

/// 表的总高（从内容顶边到最后一行的底边；**不含**上下内边距）。
///
/// 绘制（[`draw`]）与断言（`tests::表放得进浮层`）**共用这一个函数** ——
/// 以前两边各写一遍加法，改常量时容易只改一处、另一边悄悄失准。
pub fn table_height() -> f32 {
    m::SHORTCUTS_TITLE_H
        + m::SHORTCUTS_NOTE_H
        + SEP_GAP
        + line_count() as f32 * m::SHORTCUTS_ROW_H
        + GROUPS.len() as f32 * GROUP_GAP
}

/// 浮层内容框高（= 浮层高 − 上下内边距）。表必须放得进它，否则行会被画到面板外。
pub fn content_height() -> f32 {
    m::SHORTCUTS_H - m::SHORTCUTS_PAD * 2.0
}

/// 浮层矩形（居中）。
pub fn rect(l: &Layout) -> Rect {
    Rect::from_center_size(
        l.content.center(),
        vec2(m::SHORTCUTS_W, m::SHORTCUTS_H),
    )
}

/// 画浮层（遮罩 + 面板 + 标题 + 说明 + 各组）。
pub fn draw(p: &Painter, l: &Layout) {
    let pal = t::ui();
    // 遮罩：压住整个窗口（浮层是"只读对话框"，底下的东西不该被误点）
    pt::fill(p, l.content, pal.modal_scrim);
    let r = rect(l);
    pt::fill(p, r, pal.panel_bg);
    pt::border(p, r, pal.menu_popup_border);

    let inner_l = r.left() + m::SHORTCUTS_PAD;
    let 起 = r.top() + m::SHORTCUTS_PAD;
    let mut y = 起;

    // 标题。字号仍用本仓 `PANEL_TITLE_FONT`；**行高按上游 FontSize 22 预留**
    // （见 `metrics::SHORTCUTS_TITLE_H` 的算式），这样将来把字号改成上游 22 不必再改高度。
    pt::text(
        p,
        pos2(inner_l, y + m::SHORTCUTS_TITLE_H * 0.5),
        Align2::LEFT_CENTER,
        tx::SHORTCUTS_TITLE,
        m::PANEL_TITLE_FONT,
        pal.text,
    );
    y += m::SHORTCUTS_TITLE_H;
    // 说明
    pt::text(
        p,
        pos2(inner_l, y + m::SHORTCUTS_NOTE_H * 0.5),
        Align2::LEFT_CENTER,
        tx::SHORTCUTS_NOTE,
        m::BODY_FONT,
        pal.text_muted,
    );
    y += m::SHORTCUTS_NOTE_H;
    // 标题下 1px 分隔线
    pt::fill(
        p,
        Rect::from_min_size(pos2(inner_l, y), vec2(r.width() - m::SHORTCUTS_PAD * 2.0, 1.0)),
        pal.separator,
    );
    y += SEP_GAP;

    for g in GROUPS {
        // 组名（强调色，比正文更显眼）
        pt::text(
            p,
            pos2(inner_l, y + m::SHORTCUTS_ROW_H * 0.5),
            Align2::LEFT_CENTER,
            g.title,
            m::BODY_FONT,
            pal.accent_dark,
        );
        y += m::SHORTCUTS_ROW_H;
        // 键色分主题（2026-11-08 订正）：
        //   深色 = 上游 `MainWindow.axaml.cs:1248` 的 `#A8B7F6`（见 [`KEY_COLOR`]）——**上游真值**。
        //   浅色 = **上游无真值**：上游 `EditorWindow` 只有深色一套，`#A8B7F6` 直接搬到浅色
        //          `panel_bg #F5F6F7` 上对比度只有 **1.80 : 1**（WCAG AA 要 4.5），键列几乎看不见。
        //          上游没给浅色版本 ⇒ 属「上游无此物」，**沿用本仓改动前的原值 `pal.text`**
        //          （不是新发明的颜色），并登记待拍板（台账 §8.77）。
        let key_color = if t::is_dark() { KEY_COLOR } else { pal.text };
        for row in g.rows {
            // 键色见上（深色取上游 `#A8B7F6`，浅色取本仓原值）
            pt::text(
                p,
                pos2(inner_l + KEY_INDENT, y + m::SHORTCUTS_ROW_H * 0.5),
                Align2::LEFT_CENTER,
                row.keys,
                m::BODY_FONT,
                key_color,
            );
            pt::text(
                p,
                pos2(inner_l + m::SHORTCUTS_KEY_COL_W, y + m::SHORTCUTS_ROW_H * 0.5),
                Align2::LEFT_CENTER,
                row.what,
                m::BODY_FONT,
                pal.text_muted,
            );
            y += m::SHORTCUTS_ROW_H;
        }
        y += GROUP_GAP;
    }

    // 单源自证：绘制实际消费的高度必须等于 [`table_height`]。
    // 以前绘制与断言各写一遍加法，改常量时容易只改一处、另一边悄悄失准。
    debug_assert!(
        (y - 起 - table_height()).abs() < 0.01,
        "绘制消费 {}px ≠ table_height() {}px",
        y - 起,
        table_height()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// 本仓上屏字体的**实测行高**：`(标题 @ 上游 FontSize 22, 正文 @ BODY_FONT)`。
    ///
    /// 与绘制同一口径（`layout_no_wrap` + 同一份字体），不是估算值 ——
    /// `dock_left.rs::pill_text_size` 同套路。
    ///
    /// 标题按**上游字号 22** 量：本仓当前上屏用 `PANEL_TITLE_FONT`（11），
    /// 但 `metrics::SHORTCUTS_TITLE_H` 的算式是按上游 22 预留的（见那里的文档）。
    fn 实测行高() -> (f32, f32) {
        let ctx = crate::fonts::measure_ctx();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(m::WINDOW_W, m::WINDOW_H));
        let p = Painter::new(ctx, egui::LayerId::debug(), screen);
        let 量 = |s: &str, f: f32| {
            p.layout_no_wrap(s.to_owned(), egui::FontId::proportional(f), Color32::WHITE)
                .size()
                .y
        };
        (量(tx::SHORTCUTS_TITLE, 22.0), 量(tx::SC_SAVE, m::BODY_FONT))
    }

    /// **表必须放得进浮层的内容框**（"列出真实绑定"的物理约束：放不下就会被画到面板外）。
    ///
    /// # 旧口径是什么（2026-11-07 起，本单改掉）
    ///
    /// 旧断言是 `need <= SHORTCUTS_H`，其中 `need` 用**自定**的
    /// `SHORTCUTS_ROW_H = 18` / `SHORTCUTS_TITLE_H = 26` 现加，而 `SHORTCUTS_H` 被钉成 **820**，
    /// 注释理由是「R40 要求 ≥800」—— 它守的是**派单书里的一个数字**：
    /// 表真变长时要长到 820 以上才报，而 820 本身没有任何布局依据（所以高度一改它必红）。
    ///
    /// # 新口径（2026-11-08 回源）
    ///
    /// 两侧都换成可复算的真值：
    /// - 左 = [`table_height`]（行高/标题高按上屏字体实测 + 上游行距，见 `metrics` 各常量文档）；
    /// - 右 = [`content_height`] = `SHORTCUTS_H − 2 × SHORTCUTS_PAD`。
    ///
    /// 意图**不变**：所有快捷键行都在面板内，**一行都不许被裁掉**。
    #[test]
    fn 表放得进浮层() {
        let 表 = table_height();
        let 内容 = content_height();
        assert!(
            表 <= 内容,
            "快捷键表要 {表}px，浮层内容框只有 {内容}px —— 超出的行会被画到面板外\
             （{} 行 × 行高 {} + 标题 {} + 说明 {} + 分隔 {} + 组间 {}）",
            line_count(),
            m::SHORTCUTS_ROW_H,
            m::SHORTCUTS_TITLE_H,
            m::SHORTCUTS_NOTE_H,
            SEP_GAP,
            GROUPS.len() as f32 * GROUP_GAP,
        );
    }

    /// **标题高与行高必须是「实测行高 + 上游行距」算出来的，不是填出来的。**
    ///
    /// 与 `dock_left.rs::pill_size` 同一套路：常量负责算式，这条负责证明"喂进算式的字形
    /// 是真的量出来的"—— 换成任何估算值（旧 26 / 旧 18）下面的等式立刻对不上。
    #[test]
    fn 标题与行高由实测行高加上游行距推出() {
        let (标题, 正文) = 实测行高();
        assert!(标题 > 0.0 && 正文 > 0.0, "字形量出来是空盒子");
        assert!(
            (m::SHORTCUTS_ROW_H - (正文 + 14.0)).abs() < 0.01,
            "行高应为 实测正文行高 {正文} + 上游 Margin bottom 14\
             （`MainWindow.axaml.cs:1246`）= {}，实得 {}",
            正文 + 14.0,
            m::SHORTCUTS_ROW_H
        );
        assert!(
            (m::SHORTCUTS_TITLE_H - (标题 + 18.0)).abs() < 0.01,
            "标题行高应为 实测标题行高 {标题} + 上游 Margin bottom 18\
             （`MainWindow.axaml.cs:1240`）= {}，实得 {}",
            标题 + 18.0,
            m::SHORTCUTS_TITLE_H
        );
    }

    /// **把「本仓高度偏离上游 680」写成可执行断言**（不许它悄悄存在，也不许它悄悄消失）。
    ///
    /// 上游 `MainWindow.axaml.cs:1233` 是 `Height = 680`；本仓取 [`m::SHORTCUTS_H`]。
    /// 偏离的唯一理由是**内容量**（上游 19 行单列、无组名、无说明行；本仓 28 行 = 6 组名 +
    /// 22 条绑定）：本仓表在上游 680 的内容框（`680 − 2×28 = 624`）里**放不下**。
    ///
    /// 这是**反向哨兵**：哪天有人把表缩到 624 以内（例如去掉 6 个组名），它就红了 ——
    /// 那时说明 680 又够用了，该把高度改回上游 680 并删掉本测试。
    #[test]
    fn 上游_680_装不下本仓的表() {
        let 上游内容框 = 680.0 - m::SHORTCUTS_PAD * 2.0;
        assert!(
            table_height() > 上游内容框,
            "本仓表高 {}px 已经装得进上游 680 的内容框（{上游内容框}px）—— \
             那就不该再偏离上游高度了：把 `SHORTCUTS_H` 改回 680，并删掉本测试",
            table_height()
        );
    }

    /// **每行的「键」与「说明」都放得进各自那一栏**（放不下就会被画到面板边界外 ⇒ 截断）。
    ///
    /// 2026-11-07 实测教训：工具键合并成一行后，说明最初写成
    /// 「工具：画笔 / 铅笔 / 填充 / 橡皮 / 选择 / 印章 / 吸管 / 克隆（后二未接线）」，
    /// 排版宽超过说明列，截图里被面板右边界截断。
    /// 这条用**真排版宽度**（`paint::text_w`，与绘制同一个 `BODY_FONT`）守，不靠目测。
    ///
    /// ⚠ 说明列宽随浮层宽走：旧快照是 `560 − 2×18 − 170 = 354px`（2026-11-07）；
    /// 2026-11-08 回源后是 `460 − 2×28 − 170 = **234px**`（实测最宽说明 190.00 ⇒ 余 44px）。
    #[test]
    fn 每行两栏都放得下() {
        let ctx = crate::fonts::measure_ctx();
        let screen = Rect::from_min_size(
            pos2(0.0, 0.0),
            egui::vec2(m::WINDOW_W, m::WINDOW_H),
        );
        let p = Painter::new(ctx, egui::LayerId::debug(), screen);
        let 说明列宽 = m::SHORTCUTS_W - 2.0 * m::SHORTCUTS_PAD - m::SHORTCUTS_KEY_COL_W;
        for g in GROUPS {
            for row in g.rows {
                let 键宽 = crate::paint::text_w(&p, row.keys, m::BODY_FONT);
                assert!(
                    键宽 <= m::SHORTCUTS_KEY_COL_W,
                    "键「{}」排版宽 {键宽}px 超过键列 {}px",
                    row.keys,
                    m::SHORTCUTS_KEY_COL_W
                );
                let 说明宽 = crate::paint::text_w(&p, row.what, m::BODY_FONT);
                assert!(
                    说明宽 <= 说明列宽,
                    "说明「{}」排版宽 {说明宽}px 超过说明列 {说明列宽}px（会被截断）",
                    row.what
                );
            }
        }
    }

    /// 每一行的键与说明都非空，且键**不重复**（重复说明有两处绑同一个键，必然有一处是错的）。
    #[test]
    fn 键唯一且说明非空() {
        let mut keys = BTreeSet::new();
        for g in GROUPS {
            assert!(!g.title.is_empty());
            for row in g.rows {
                assert!(!row.keys.is_empty(), "{} 组里有空键", g.title);
                assert!(!row.what.is_empty(), "键 {} 没有说明", row.keys);
                assert!(keys.insert(row.keys), "键 {} 重复登记", row.keys);
            }
        }
        assert!(keys.len() >= 15, "真实绑定至少 15 条，现在只有 {}", keys.len());
    }

    /// **官方有、本仓没实现的键不许出现在表里**（列了就是假信息）。
    #[test]
    fn 不列未实现的官方键() {
        // 这些键官方有，但本仓对应功能仍是 Todo（见 `menu::model::state`）
        for forbidden in ["Ctrl+N", "Ctrl+Shift+N"] {
            let hit = GROUPS
                .iter()
                .flat_map(|g| g.rows.iter())
                .any(|r| r.keys == forbidden);
            assert!(!hit, "表里不该出现未实现的键：{forbidden}");
        }
        // 反向：真的实现了的必须列出来
        // （`G` = 视图 → 单元格网格 2026-11-05 起真实现；`Ctrl+Shift+S` = 另存为同日真实现）
        for must in [
            "Ctrl+S",
            "Ctrl+Shift+S",
            "Ctrl+Z",
            "G",
            // 2026-11-07（R9-F2 / R40）：八个工具键合并成一行，另加阻挡开关与平移
            "B/P/F/E/M/O/V/K",
            "C",
            "W A S D",
            "Ctrl+C",
            "Delete",
            "F1",
            "Esc",
        ] {
            let hit = GROUPS
                .iter()
                .flat_map(|g| g.rows.iter())
                .any(|r| r.keys == must);
            assert!(hit, "真实绑定 {must} 没列进表");
        }
    }
}
