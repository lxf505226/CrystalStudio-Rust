//! 「视图 → 定位到格…」模态（F-R13-2）：几何 + 绘制 + 输入 + 校验。
//!
//! 上游同语义只有一处实现 —— `_upstream/CrystalStudio/MapViewport.Navigation.cs:5-13` 的 `LocateCell`：
//!
//! ```csharp
//! internal void LocateCell(int x, int y) {
//!     if (!Document.Contains(x, y)) return;      // 越界：什么都不做（**不出声**）
//!     CancelInteraction();                       // 退掉进行中的交互
//!     Zoom = Math.Max(Zoom, .5);                 // 缩放抬到 ≥50%
//!     Selection = new Rect(x, y, 1, 1);          // 1×1 选区
//!     CenterOnWorld(new Point(x * 48 + 24, y * 32 + 16)); // 居中到**格中心**
//!     InvalidateVisual(); Changed?.Invoke();
//! }
//! ```
//!
//! 上游界面上只有 CheckMap 结果面板的「定位」按钮调它（`_upstream/CrystalStudio/MainWindow.MapCheck.cs:33`）。
//! 本仓把语义抽成 `MireditApp::locate_cell`（一处实现，CheckMap 与本模态共用），
//! 并在此另开一个**通用入口**（视图菜单）—— 这是本仓新增的入口，不是上游界面项。
//!
//! 与上游的三点差异（都记档）：
//! 1. **越界不再静默**：上游 `Document.Contains` 失败就 `return`，用户什么也看不到；
//!    本仓的输入模态当场给「超出地图范围：…列的合法区间」，按钮灰显（拒绝必说原因）。
//! 2. 上游从 CheckMap 列表点进来（坐标来自报告），本仓多一条手输路径 ⇒ 必须自己校验。
//! 3. `CancelInteraction()` 在本仓对应「退掉待放置印章 + 结束进行中的笔画」（见 `locate_cell`）。

use egui::{pos2, vec2, Align2, Painter, Rect, Ui};

use crate::layout::Layout;
use crate::menu::Overlay;
use crate::metrics as m;
use crate::paint as pt;
use crate::theme as t;
use crate::ui_text as tx;

/// 面板宽（两格输入，比 640 的路径模态窄一半）。
const 面板宽: f32 = 420.0;

// 初值由 `menu::action` 现算（选区中心 → 否则鼠标格 → 否则 `0,0`），本模块不管。
// 空框仍是**非法**（0,0 是合法坐标，不能拿"空"替用户猜 0）。

/// 每个输入组的高度：标签 18 + 输入框 28 + 组间距 10。
const 组高: f32 = 56.0;

/// 内容高（标题 + 两行提示 + 两组 + 摘要 + 按钮 + 内边距）。
///
/// 抽成函数是为了让 [`tests::面板放得下内容`] 与绘制**用同一个算式**。
#[must_use]
pub fn 内容高() -> f32 {
    m::OPEN_DLG_PAD * 2.0
        + 24.0                // 标题
        + 16.0                // 标题与提示之间
        + 2.0 * 16.0          // 提示两行
        + 2.0 * 组高          // 两组「标签 + 输入框」
        + 8.0                 // 组与摘要之间
        + 22.0                // 摘要一行
        + 12.0                // 摘要与按钮之间
        + m::OPEN_DLG_BTN_H
}

/// 模态里所有可画/可点矩形（绘制与命中测试共用同一份几何）。
#[derive(Clone, Copy, Debug)]
pub struct Rects {
    /// 面板。
    pub panel: Rect,
    /// 两个输入框（顺序 = 列 / 行）。
    pub fields: [Rect; 2],
    /// 摘要行（合法 = 目标坐标与缩放；非法 = 原因）。
    pub summary: Rect,
    /// 主按钮「定位」。
    pub ok: Rect,
    /// 「取消」。
    pub cancel: Rect,
}

/// 算模态几何（居中；按钮贴底，与其它模态同一规则）。
#[must_use]
pub fn rects(l: &Layout) -> Rects {
    let panel = Rect::from_center_size(l.content.center(), vec2(面板宽, 内容高()));
    let inner_l = panel.left() + m::OPEN_DLG_PAD;
    let inner_r = panel.right() - m::OPEN_DLG_PAD;
    let 宽 = inner_r - inner_l;
    let mut y = panel.top() + m::OPEN_DLG_PAD + 24.0 + 16.0 + 2.0 * 16.0;
    let mut fields = [Rect::NOTHING; 2];
    for f in &mut fields {
        y += 18.0; // 标签
        *f = Rect::from_min_size(pos2(inner_l, y), vec2(宽, m::OPEN_DLG_FIELD_H));
        y += m::OPEN_DLG_FIELD_H + 10.0;
    }
    let summary = Rect::from_min_size(pos2(inner_l, y + 8.0), vec2(宽, 22.0));
    let btn_y = panel.bottom() - m::OPEN_DLG_PAD - m::OPEN_DLG_BTN_H;
    let cancel = Rect::from_min_size(
        pos2(inner_r - m::OPEN_DLG_BTN_W, btn_y),
        vec2(m::OPEN_DLG_BTN_W, m::OPEN_DLG_BTN_H),
    );
    let ok = Rect::from_min_size(
        pos2(cancel.left() - 10.0 - m::OPEN_DLG_BTN_W, btn_y),
        vec2(m::OPEN_DLG_BTN_W, m::OPEN_DLG_BTN_H),
    );
    Rects {
        panel,
        fields,
        summary,
        ok,
        cancel,
    }
}

/// 提交判定（上游 `LocateCell` 的 `Document.Contains` + 解析）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum 校验 {
    /// 合法：`(列, 行)`。
    可用 {
        /// 列（0 基）。
        x: u16,
        /// 行（0 基）。
        y: u16,
    },
    /// 非法：文本不是整数（含空着）。
    不是整数,
    /// 非法：越出地图（带中文原因，列出合法区间）。
    越界(String),
}

impl 校验 {
    /// 能不能点「定位」。
    #[must_use]
    pub fn 可用(&self) -> bool {
        matches!(self, 校验::可用 { .. })
    }

    /// 摘要行文案（合法 ⇒ 目标坐标与缩放；非法 ⇒ 原因）。
    #[must_use]
    pub fn 摘要(&self) -> String {
        match *self {
            校验::可用 { x, y } => tx::fill(
                tx::GOTO_SUMMARY_TMPL,
                &[
                    ("x", &x.to_string()),
                    ("y", &y.to_string()),
                    ("pct", &(m::GOTO_MIN_SCALE * 100.0).round().to_string()),
                ],
            ),
            校验::不是整数 => tx::GOTO_ERR_NOT_INT.to_owned(),
            校验::越界(ref why) => why.clone(),
        }
    }

    /// 目标坐标（非法时 `None`）。
    #[must_use]
    pub fn 坐标(&self) -> Option<(u16, u16)> {
        match *self {
            校验::可用 { x, y } => Some((x, y)),
            _ => None,
        }
    }
}

/// 校验两个输入框（`cells` = 地图尺寸，用来判越界）。
#[must_use]
pub fn 校验(文本: &[String; 2], cells: (u16, u16)) -> 校验 {
    let mut 值 = [0i64; 2];
    for (i, s) in 文本.iter().enumerate() {
        let t = s.trim();
        if t.is_empty() {
            // 空串**不许**当 0（0,0 是合法坐标，不能替用户猜）
            return 校验::不是整数;
        }
        match t.parse::<i64>() {
            Ok(v) => 值[i] = v,
            Err(_) => return 校验::不是整数,
        }
    }
    let (w, h) = (i64::from(cells.0), i64::from(cells.1));
    if 值[0] < 0 || 值[1] < 0 || 值[0] >= w || 值[1] >= h {
        return 校验::越界(tx::fill(
            tx::GOTO_ERR_RANGE_TMPL,
            &[
                ("w", &cells.0.to_string()),
                ("h", &cells.1.to_string()),
                ("xmax", &cells.0.saturating_sub(1).to_string()),
                ("ymax", &cells.1.saturating_sub(1).to_string()),
            ],
        ));
    }
    // 已经判过 < w / < h（且 w/h 来自 u16）⇒ 这两次窄化必然成功
    校验::可用 {
        x: u16::try_from(值[0]).unwrap_or(0),
        y: u16::try_from(值[1]).unwrap_or(0),
    }
}

/// 用户在模态里做的一次动作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum 动作 {
    /// 什么都没发生。
    None,
    /// 取消（`Esc` / 点「取消」）。
    Cancel,
    /// 提交：目标格（已校验在图内）。
    Apply {
        /// 列。
        x: u16,
        /// 行。
        y: u16,
    },
}

/// 画模态。
pub fn draw(
    p: &Painter,
    l: &Layout,
    文本: &[String; 2],
    焦点: usize,
    判定: &校验,
    错误: Option<&str>,
) {
    let r = rects(l);
    let pal = t::ui();
    pt::fill(p, l.content, pal.modal_scrim);
    pt::fill(p, r.panel, pal.panel_bg);
    pt::border(p, r.panel, pal.separator);

    let inner_l = r.panel.left() + m::OPEN_DLG_PAD;
    pt::text(
        p,
        pos2(inner_l, r.panel.top() + m::OPEN_DLG_PAD),
        Align2::LEFT_TOP,
        tx::GOTO_DLG_TITLE,
        m::TITLE_FONT,
        pal.text,
    );
    let 提示_y = r.panel.top() + m::OPEN_DLG_PAD + 24.0 + 16.0;
    pt::text(
        p,
        pos2(inner_l, 提示_y),
        Align2::LEFT_TOP,
        tx::GOTO_DLG_HINT,
        m::BODY_FONT,
        pal.text_muted,
    );
    pt::text(
        p,
        pos2(inner_l, 提示_y + 16.0),
        Align2::LEFT_TOP,
        tx::GOTO_DLG_HINT2,
        m::BODY_FONT,
        pal.text_muted,
    );
    for (i, rect) in r.fields.iter().enumerate() {
        pt::text(
            p,
            pos2(rect.left(), rect.top() - 18.0),
            Align2::LEFT_TOP,
            tx::GOTO_FIELDS[i],
            m::BODY_FONT,
            pal.text,
        );
        let 空 = 文本[i].trim().is_empty();
        pt::fill(p, *rect, pal.input_bg);
        pt::border(
            p,
            *rect,
            if 空 {
                pal.warn_banner_edge
            } else if 焦点 == i {
                pal.accent
            } else {
                pal.input_border
            },
        );
        if !空 {
            pt::text(
                p,
                pos2(rect.left() + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                &文本[i],
                m::BODY_FONT,
                pal.text,
            );
        }
    }
    let 合法 = 判定.可用() && 错误.is_none();
    let 摘要 = 错误.map_or_else(|| 判定.摘要(), str::to_owned);
    pt::text(
        p,
        pos2(r.summary.left(), r.summary.center().y),
        Align2::LEFT_CENTER,
        &摘要,
        m::BODY_FONT,
        if 合法 {
            pal.text
        } else {
            pal.warn_banner_text
        },
    );
    pt::fill(p, r.ok, if 合法 { pal.accent } else { pal.input_bg });
    pt::border(
        p,
        r.ok,
        if 合法 {
            pal.accent_dark
        } else {
            pal.input_border
        },
    );
    pt::text(
        p,
        r.ok.center(),
        Align2::CENTER_CENTER,
        tx::GOTO_OK,
        m::BODY_FONT,
        if 合法 {
            pal.accent_button_text
        } else {
            pal.text_muted
        },
    );
    pt::fill(p, r.cancel, pal.input_bg);
    pt::border(p, r.cancel, pal.input_border);
    pt::text(
        p,
        r.cancel.center(),
        Align2::CENTER_CENTER,
        tx::GOTO_CANCEL,
        m::BODY_FONT,
        pal.text,
    );
}

/// 处理模态输入（**只在 `Overlay::GotoCell` 时调用**）。
pub fn interact(ui: &mut Ui, l: &Layout, overlay: &mut Overlay, cells: (u16, u16)) -> 动作 {
    let Overlay::GotoCell {
        文本, focus, error, ..
    } = overlay
    else {
        return 动作::None;
    };
    let r = rects(l);
    let 判定 = 校验(文本, cells);

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return 动作::Cancel;
    }
    if 真点击(ui, r.cancel, ui.id().with(("goto", "cancel"))) {
        return 动作::Cancel;
    }
    if 真点击(ui, r.ok, ui.id().with(("goto", "ok"))) {
        return match 判定.坐标() {
            Some((x, y)) => 动作::Apply { x, y },
            None => 动作::None,
        };
    }
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        return match 判定.坐标() {
            Some((x, y)) => 动作::Apply { x, y },
            None => 动作::None,
        };
    }
    for i in 0..2 {
        if 真点击(ui, r.fields[i], ui.id().with(("goto", "field", i))) {
            *focus = i;
            return 动作::None;
        }
    }
    let (tab, back) = ui.input(|i| {
        let tab = i.key_pressed(egui::Key::Tab);
        (tab, tab && i.modifiers.shift)
    });
    if tab {
        *focus = if back {
            if *focus == 0 {
                1
            } else {
                0
            }
        } else if *focus == 0 {
            1
        } else {
            0
        };
        return 动作::None;
    }
    let (texts, backspace) = ui.input(|i| {
        let mut texts = Vec::new();
        for e in &i.events {
            if let egui::Event::Text(t) = e {
                texts.push(t.clone());
            }
        }
        (texts, i.key_pressed(egui::Key::Backspace))
    });
    if !texts.is_empty() || backspace {
        let 当前 = (*focus).min(1);
        for t in texts {
            for c in t.chars() {
                // 只收 ASCII 数字（坐标是十进制；负号由"越界"分支报，输不进来也无所谓）
                if c.is_ascii_digit() && 文本[当前].len() < 5 {
                    文本[当前].push(c);
                }
            }
        }
        if backspace {
            文本[当前].pop();
        }
        *error = None;
    }
    动作::None
}

/// 只认真实鼠标点击（与其它模态同一口径）。
fn 真点击(ui: &mut Ui, rect: Rect, id: egui::Id) -> bool {
    ui.interact(rect, id, egui::Sense::click()).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn 布局() -> Layout {
        Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ))
    }

    fn 文本(a: [&str; 2]) -> [String; 2] {
        [a[0].to_string(), a[1].to_string()]
    }

    /// 几何：面板在窗口内、两框不重叠、按钮不压摘要、宽度固定。
    #[test]
    fn 几何自洽() {
        let l = 布局();
        let r = rects(&l);
        assert!(l.content.contains_rect(r.panel), "面板越出窗口");
        assert_eq!(r.panel.width(), 面板宽);
        assert!(r.fields[0].bottom() < r.fields[1].top(), "两框重叠");
        assert!(r.fields[1].bottom() < r.summary.top(), "摘要压到输入框");
        assert!(r.summary.bottom() < r.ok.top(), "按钮压到摘要");
        assert!(r.ok.right() <= r.cancel.left(), "两按钮重叠");
        assert!((r.panel.height() - 内容高()).abs() < 0.01);
    }

    /// 校验：空/非数字/负/越界各有原因；合法给坐标。
    #[test]
    fn 校验逐条() {
        let cells = (700, 700);
        assert_eq!(校验(&文本(["", ""]), cells), 校验::不是整数, "空串不许当 0");
        assert_eq!(校验(&文本(["1", ""]), cells), 校验::不是整数);
        assert_eq!(校验(&文本(["x", "1"]), cells), 校验::不是整数);
        // 负号：UI 的输入过滤只收 ASCII 数字（打不进 "-"），但程序化塞进来时要拒绝
        assert!(matches!(校验(&文本(["-1", "0"]), cells), 校验::越界(_)), "负数必须拒绝");
        assert!(matches!(
            校验(&文本(["700", "0"]), cells),
            校验::越界(ref why) if why.contains("700 × 700")
        ));
        assert!(matches!(
            校验(&文本(["0", "700"]), cells),
            校验::越界(_)
        ));
        let ok = 校验(&文本(["699", "699"]), cells);
        assert!(ok.可用(), "右下角应合法：{ok:?}");
        assert_eq!(ok.坐标(), Some((699, 699)));
        assert_eq!(
            ok.摘要(),
            tx::fill(
                tx::GOTO_SUMMARY_TMPL,
                &[("x", "699"), ("y", "699"), ("pct", "50")]
            )
        );
        // 0,0 合法（这也是"空串不许当 0"的意义：0 是真实坐标，不能被空串顶替）
        assert_eq!(校验(&文本(["0", "0"]), cells).坐标(), Some((0, 0)));
    }

    /// `1×1` 地图只有 `(0,0)` 合法（边界值要能过）。
    #[test]
    fn 单格地图边界() {
        let cells = (1, 1);
        assert!(校验(&文本(["0", "0"]), cells).可用());
        assert!(matches!(校验(&文本(["1", "0"]), cells), 校验::越界(_)));
        assert!(matches!(校验(&文本(["0", "1"]), cells), 校验::越界(_)));
    }

    // ───────────────────── 模态输入（真 egui 帧） ─────────────────────

    fn 一帧(t: f64, events: Vec<egui::Event>) -> egui::RawInput {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                vec2(m::WINDOW_W, m::WINDOW_H),
            )),
            events,
            ..Default::default()
        };
        input.time = Some(t);
        input
    }

    fn 跑(ctx: &egui::Context, l: &Layout, overlay: &mut Overlay, input: egui::RawInput) -> 动作 {
        let mut got = 动作::None;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    got = interact(ui, l, overlay, (700, 700));
                });
        });
        got
    }

    fn 键(key: egui::Key) -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]
    }

    fn 浮层(a: [&str; 2]) -> Overlay {
        Overlay::GotoCell {
            文本: 文本(a),
            focus: 0,
            error: None,
        }
    }

    /// 回车只在合法时提交；Esc 取消；非法点按钮不提交。
    #[test]
    fn 回车只在合法时提交() {
        let ctx = egui::Context::default();
        let l = 布局();
        let mut o = 浮层(["", ""]);
        assert_eq!(跑(&ctx, &l, &mut o, 一帧(0.0, 键(egui::Key::Enter))), 动作::None);
        let mut o = 浮层(["700", "0"]);
        assert_eq!(跑(&ctx, &l, &mut o, 一帧(0.2, 键(egui::Key::Enter))), 动作::None, "越界不许提交");
        let mut o = 浮层(["12", "34"]);
        assert_eq!(
            跑(&ctx, &l, &mut o, 一帧(0.4, 键(egui::Key::Enter))),
            动作::Apply { x: 12, y: 34 }
        );
        let mut o = 浮层(["12", "34"]);
        assert_eq!(跑(&ctx, &l, &mut o, 一帧(0.6, 键(egui::Key::Escape))), 动作::Cancel);
        // 非法时点「定位」按钮不提交（先 hover 一帧再按下+松开）
        let r = rects(&l);
        let mut o = 浮层(["", ""]);
        let _ = 跑(&ctx, &l, &mut o, 一帧(0.8, vec![egui::Event::PointerMoved(r.ok.center())]));
        let 点 = vec![
            egui::Event::PointerButton {
                pos: r.ok.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: r.ok.center(),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ];
        assert_eq!(跑(&ctx, &l, &mut o, 一帧(1.0, 点)), 动作::None, "非法不许提交");
    }

    /// Tab 换框；只收 ASCII 数字；一动输入就清旧错误。
    #[test]
    fn 输入与焦点() {
        let ctx = egui::Context::default();
        let l = 布局();
        let mut o = 浮层(["", ""]);
        if let Overlay::GotoCell { error, .. } = &mut o {
            *error = Some("上次失败".to_string());
        }
        let _ = 跑(&ctx, &l, &mut o, 一帧(0.0, vec![egui::Event::Text("12".to_string())]));
        match &o {
            Overlay::GotoCell { 文本, error, .. } => {
                assert_eq!(文本[0], "12");
                assert!(error.is_none(), "改输入要清旧错误");
            }
            other => panic!("浮层变了：{other:?}"),
        }
        let _ = 跑(&ctx, &l, &mut o, 一帧(0.2, 键(egui::Key::Tab)));
        match &o {
            Overlay::GotoCell { focus, .. } => assert_eq!(*focus, 1, "Tab 应换到第二框"),
            other => panic!("浮层变了：{other:?}"),
        }
        let _ = 跑(&ctx, &l, &mut o, 一帧(0.4, vec![egui::Event::Text("7x".to_string())]));
        match &o {
            Overlay::GotoCell { 文本, .. } => assert_eq!(文本[1], "7", "非数字不许进来"),
            other => panic!("浮层变了：{other:?}"),
        }
    }
}
