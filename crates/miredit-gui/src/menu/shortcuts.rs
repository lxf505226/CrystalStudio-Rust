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

use egui::{pos2, vec2, Align2, Painter, Rect};

use crate::layout::Layout;
use crate::metrics as m;
use crate::paint as pt;
use crate::theme as t;
use crate::ui_text as tx;

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
    let mut y = r.top() + m::SHORTCUTS_PAD;

    // 标题
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
    y += 6.0;

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
        for row in g.rows {
            pt::text(
                p,
                pos2(inner_l + 10.0, y + m::SHORTCUTS_ROW_H * 0.5),
                Align2::LEFT_CENTER,
                row.keys,
                m::BODY_FONT,
                pal.text,
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
        y += 4.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// 表必须放得进浮层（**这是"列出真实绑定"的物理约束**：放不下就会被裁掉）。
    #[test]
    fn 表放得进浮层() {
        let need = m::SHORTCUTS_PAD * 2.0
            + m::SHORTCUTS_TITLE_H
            + m::SHORTCUTS_NOTE_H
            + 6.0
            + line_count() as f32 * m::SHORTCUTS_ROW_H
            + GROUPS.len() as f32 * 4.0;
        assert!(
            need <= m::SHORTCUTS_H,
            "快捷键表要 {need}px，浮层只有 {}px（会被裁掉）",
            m::SHORTCUTS_H
        );
    }

    /// **每行的「键」与「说明」都放得进各自那一栏**（放不下就会被画到面板边界外 ⇒ 截断）。
    ///
    /// 2026-11-07 实测教训：工具键合并成一行后，说明最初写成
    /// 「工具：画笔 / 铅笔 / 填充 / 橡皮 / 选择 / 印章 / 吸管 / 克隆（后二未接线）」，
    /// 排版宽超过说明列（560 − 2×18 − 170 = **354px**），截图里被面板右边界截断。
    /// 这条用**真排版宽度**（`paint::text_w`，与绘制同一个 `BODY_FONT`）守，不靠目测。
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
