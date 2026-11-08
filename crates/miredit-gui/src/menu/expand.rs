//! 「地图扩边」模态（A-015 Expand / R36）：几何 + 绘制 + 输入。
//!
//! 上游 `CrystalStudio/MapExpansionWindow.cs`（`@901dbe2f`）：
//!
//! | 段 | 上游 | 本模块 |
//! |---|---|---|
//! | 面板 | `Width = 440`、`CanResize = false`、`SizeToContent = Height`、居中 | [`rects`]：440 宽、高度按内容算（标题 + 提示 + 4 组「标签+输入框」+ 摘要 + 按钮） |
//! | 四个输入 | `Add left/top/right/bottom (cells)`，初值 `"0"` | 同（初值 [`初值`]） |
//! | 实时校验 | `int.TryParse` + `MapDimensions.CanExpand` ⇒ 非法时 `ApplyButton.IsEnabled = false` | [`校验`]：四条边非负、偶数、至少一条 > 0，且新尺寸不越上限（判据全在 `miredit_core::expand_target`） |
//! | 摘要 | 合法 ⇒ `"Expanded map: {0} × {1} cells"`；非法 ⇒ 那句原因 | 合法 ⇒ `扩边后：{w} × {h} 格`；非法 ⇒ 一句中文原因（**同一处**画） |
//! | 按钮 | `Cancel`（`IsCancel`）+ `Expand Map`（`IsDefault`，非法时禁用） | 「取消」+「地图扩边」（灰显时**点击也不响应**，且**不关窗**） |
//! | 提交 | `DialogResult = true` ⇒ `Viewport.ExpandMap(...)` | 返回 [`动作::Apply`]，由 `menu::action` 落到 `EditSession::expand_by`（一步撤销 + 保存源同步） |
//!
//! ⚠ 与上游的两点差异（都记档）：
//! 1. 上游用 `int.Parse`（非法文本直接抛异常）；本仓只收 ASCII 数字，非数字进不来 ⇒ 不存在"抛异常"；
//! 2. 上游 `SizeToContent` 由布局引擎算高；本仓是**纯 Painter 自绘**，高度按固定行高算死
//!    （[`内容高`]），并有 [`tests::面板放得下内容`] 钉住 —— 改行数/行高忘了改高度会当场红。

use egui::{pos2, vec2, Align2, Painter, Rect, Ui};

use crate::layout::Layout;
use crate::menu::Overlay;
use crate::metrics as m;
use crate::paint as pt;
use crate::theme as t;
use crate::ui_text as tx;

/// 面板宽（官方 `Width = 440`）。
const 面板宽: f32 = 440.0;

/// 四个输入框的初值（官方每个 `Text = "0"`）。
pub const 初值: [&str; 4] = ["0", "0", "0", "0"];

/// 四条边的顺序（与 [`初值`] / 四个输入框 / 摘要一致）：左、上、右、下。
pub const 方向: [&str; 4] = ["左", "上", "右", "下"];

/// 每个输入组的高度：标签 18 + 输入框 28 + 组间距 10。
const 组高: f32 = 56.0;

/// 内容高（标题 + 提示两行 + 四组 + 摘要 + 按钮 + 内边距）。
///
/// 抽成函数是为了让 [`tests::面板放得下内容`] 与绘制**用同一个算式**。
#[must_use]
pub fn 内容高() -> f32 {
    m::OPEN_DLG_PAD * 2.0      // 上下内边距
        + 24.0                 // 标题
        + 16.0                 // 标题与提示之间
        + 2.0 * 16.0           // 提示两行
        + 4.0 * 组高           // 四组「标签 + 输入框」
        + 8.0                  // 组与摘要之间
        + 22.0                 // 摘要一行
        + 12.0                 // 摘要与按钮之间
        + m::OPEN_DLG_BTN_H    // 按钮
}

/// 模态里所有可画/可点矩形（绘制与命中测试共用同一份几何）。
#[derive(Clone, Copy, Debug)]
pub struct Rects {
    /// 面板。
    pub panel: Rect,
    /// 四个输入框（顺序 = 左/上/右/下）。
    pub fields: [Rect; 4],
    /// 摘要行（合法 = 目标尺寸；非法 = 原因）。
    pub summary: Rect,
    /// 主按钮「地图扩边」。
    pub ok: Rect,
    /// 「取消」。
    pub cancel: Rect,
}

/// 算模态几何（居中；按钮贴底，与其它模态同一规则）。
#[must_use]
pub fn rects(l: &Layout) -> Rects {
    let 高 = 内容高();
    let panel = Rect::from_center_size(l.content.center(), vec2(面板宽, 高));
    let inner_l = panel.left() + m::OPEN_DLG_PAD;
    let inner_r = panel.right() - m::OPEN_DLG_PAD;
    let 宽 = inner_r - inner_l;
    let mut y = panel.top() + m::OPEN_DLG_PAD + 24.0 + 16.0 + 2.0 * 16.0;
    let mut fields = [Rect::NOTHING; 4];
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

/// 提交目标的判定结果（= 上游 `Validate()`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum 校验 {
    /// 合法：`(左, 上, 右, 下, 目标宽, 目标高)`。
    可用 {
        /// 左加。
        left: i32,
        /// 上加。
        top: i32,
        /// 右加。
        right: i32,
        /// 下加。
        bottom: i32,
        /// 目标宽。
        width: u16,
        /// 目标高。
        height: u16,
    },
    /// 非法：文本**不是整数**（含空白）—— 上游 `int.TryParse` 对空串同样返回 `false`。
    不是整数,
    /// 非法：四向判据不过（负数 / 奇数 / 全 0 / 越上限），带中文原因。
    越界(String),
}

impl 校验 {
    /// 能不能点「地图扩边」（上游 `ApplyButton.IsEnabled`）。
    #[must_use]
    pub fn 可用(&self) -> bool {
        matches!(self, 校验::可用 { .. })
    }

    /// 摘要行文案（合法 ⇒ 目标尺寸；非法 ⇒ 原因）。
    #[must_use]
    pub fn 摘要(&self) -> String {
        match self {
            校验::可用 { width, height, .. } => tx::fill(
                tx::EXPAND_SUMMARY_TMPL,
                &[("w", &width.to_string()), ("h", &height.to_string())],
            ),
            校验::不是整数 => tx::EXPAND_ERR_NOT_NUMBER.to_owned(),
            校验::越界(why) => why.clone(),
        }
    }

    /// 四个输入值的元组（非法时给 `None`）。
    #[must_use]
    pub fn 四向(&self) -> Option<(i32, i32, i32, i32)> {
        match *self {
            校验::可用 {
                left,
                top,
                right,
                bottom,
                ..
            } => Some((left, top, right, bottom)),
            _ => None,
        }
    }
}

/// 校验四个输入框的文本。
///
/// # 空串**不许**当 0（2026-11-07 P3 订正）
///
/// 上游 `_upstream/CrystalStudio/MapExpansionWindow.cs:39-41` 是 `int.TryParse(LeftInput.Text, out int left) && …`：
/// 空串 ⇒ `TryParse` 返回 `false` ⇒ `ApplyButton.IsEnabled = false`。本仓旧实现把空串当 `0`，
/// **与上游相反**，已改成 [`校验::不是整数`]（摘要行同时说明「不能为空」）。
///
/// `expand_target` 的入参是 `i32`（四个已解析的数），**结构性看不到文本** ⇒ 空串不可能漏到
/// 内核去当 0；拦截点只有本函数（GUI 层）。
#[must_use]
pub fn 校验(文本: &[String; 4], cells: (u16, u16)) -> 校验 {
    let mut 值 = [0i32; 4];
    for (i, s) in 文本.iter().enumerate() {
        let t = s.trim();
        if t.is_empty() {
            // 上游 `int.TryParse("")` == false ⇒ 禁用 Apply，**不猜 0**
            return 校验::不是整数;
        }
        match t.parse::<i32>() {
            Ok(v) => 值[i] = v,
            Err(_) => return 校验::不是整数,
        }
    }
    match miredit_core::expand_target(cells.0, cells.1, 值[0], 值[1], 值[2], 值[3]) {
        Ok((width, height)) => 校验::可用 {
            left: 值[0],
            top: 值[1],
            right: 值[2],
            bottom: 值[3],
            width,
            height,
        },
        Err(e) => 校验::越界(e.to_string()),
    }
}

/// 用户在模态里做的一次动作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum 动作 {
    /// 什么都没发生。
    None,
    /// 取消（`Esc` / 点「取消」）。
    Cancel,
    /// 提交：四向扩边量（已校验过）。
    Apply {
        /// 左加。
        left: i32,
        /// 上加。
        top: i32,
        /// 右加。
        right: i32,
        /// 下加。
        bottom: i32,
    },
}

/// 画模态（遮罩 + 面板 + 标题 + 提示 + 四组输入 + 摘要 + 按钮）。
pub fn draw(
    p: &Painter,
    l: &Layout,
    文本: &[String; 4],
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
    // ① 标题
    pt::text(
        p,
        pos2(inner_l, r.panel.top() + m::OPEN_DLG_PAD),
        Align2::LEFT_TOP,
        tx::EXPAND_TITLE,
        m::TITLE_FONT,
        pal.text,
    );
    // ② 提示两行
    let 提示_y = r.panel.top() + m::OPEN_DLG_PAD + 24.0 + 16.0;
    pt::text(
        p,
        pos2(inner_l, 提示_y),
        Align2::LEFT_TOP,
        tx::EXPAND_HINT,
        m::BODY_FONT,
        pal.text_muted,
    );
    pt::text(
        p,
        pos2(inner_l, 提示_y + 16.0),
        Align2::LEFT_TOP,
        tx::EXPAND_HINT2,
        m::BODY_FONT,
        pal.text_muted,
    );
    // ③ 四组输入
    for (i, rect) in r.fields.iter().enumerate() {
        pt::text(
            p,
            pos2(rect.left(), rect.top() - 18.0),
            Align2::LEFT_TOP,
            tx::EXPAND_FIELDS[i],
            m::BODY_FONT,
            pal.text,
        );
        pt::fill(p, *rect, pal.input_bg);
        // 空着的框用警示色边框（**不再画 "0" 占位** —— 空≠0，上游也是禁用 Apply）
        let 空 = 文本[i].trim().is_empty();
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
    // ④ 摘要：合法 = 目标尺寸（正文色）；非法 / 被内核拒绝 = 原因（警示色）
    //
    // 「被内核拒绝」（`错误`）优先显示：那是点过「地图扩边」之后才知道的原因
    // （该格式不支持改尺寸 / 有未解释尾块），光看四个输入框是看不出来的。
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
    // ⑤ 两个按钮（「地图扩边」非法时灰显；点击也不响应，见 `interact`）
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
        tx::MENU_EXPAND_MAP,
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
        tx::EXPAND_CANCEL,
        m::BODY_FONT,
        pal.text,
    );
}

/// 处理模态输入（**只在 `Overlay::ExpandMap` 时调用**）。
///
/// 键盘：`Esc` 取消、`Enter` 提交（**仅在合法时**，非法时留在窗口里）、`Tab`/`Shift+Tab` 换框、
/// 数字进当前焦点框、退格删一位。鼠标：点框换焦点、点「取消」关窗、点「地图扩边」提交
/// （**非法时点不动、也不关窗** —— 与官方 `IsEnabled=false` 同义）。
pub fn interact(ui: &mut Ui, l: &Layout, overlay: &mut Overlay, cells: (u16, u16)) -> 动作 {
    let Overlay::ExpandMap {
        文本, focus, error, ..
    } = overlay
    else {
        return 动作::None;
    };
    let r = rects(l);
    let 判定 = 校验(文本, cells);

    // ① Esc = 取消
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return 动作::Cancel;
    }
    // ② 「取消」
    if 真点击(ui, r.cancel, ui.id().with(("expand", "cancel"))) {
        return 动作::Cancel;
    }
    // ③ 「地图扩边」：只有合法才响应（非法点击**不关窗、不改图**）
    if 真点击(ui, r.ok, ui.id().with(("expand", "ok"))) {
        return match 判定.四向() {
            Some((left, top, right, bottom)) => 动作::Apply {
                left,
                top,
                right,
                bottom,
            },
            None => 动作::None,
        };
    }
    // ④ 回车 = 提交（同样只在合法时）
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        return match 判定.四向() {
            Some((left, top, right, bottom)) => 动作::Apply {
                left,
                top,
                right,
                bottom,
            },
            None => 动作::None,
        };
    }
    // ⑤ 点框换焦点
    for i in 0..4 {
        if 真点击(ui, r.fields[i], ui.id().with(("expand", "field", i))) {
            *focus = i;
            return 动作::None;
        }
    }
    // ⑥ Tab / Shift+Tab 换焦点（四个框循环）
    let (tab, back) = ui.input(|i| {
        let tab = i.key_pressed(egui::Key::Tab);
        (tab, tab && i.modifiers.shift)
    });
    if tab {
        let cur = (*focus).min(3);
        *focus = if back {
            if cur == 0 {
                3
            } else {
                cur - 1
            }
        } else if cur >= 3 {
            0
        } else {
            cur + 1
        };
        return 动作::None;
    }
    // ⑦ 文本（只收 ASCII 数字）与退格
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
        let 当前 = (*focus).min(3);
        for t in texts {
            for c in t.chars() {
                if c.is_ascii_digit() && 文本[当前].len() < 5 {
                    文本[当前].push(c);
                }
            }
        }
        if backspace {
            文本[当前].pop();
        }
        // 一动输入就把上次的拒绝原因清掉（用户正在改，旧原因已经不适用了）
        *error = None;
    }
    动作::None
}

/// 只认真实鼠标点击（与 `menu::cellprops` / `menu::dialog` 同一口径）。
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

    fn 文本(a: [&str; 4]) -> [String; 4] {
        [a[0].to_string(), a[1].to_string(), a[2].to_string(), a[3].to_string()]
    }

    /// 几何：面板在窗口内、四组输入框互不重叠、按钮不压摘要、440 宽。
    #[test]
    fn 几何自洽() {
        let l = 布局();
        let r = rects(&l);
        assert!(l.content.contains_rect(r.panel), "面板越出窗口：{:?}", r.panel);
        assert_eq!(r.panel.width(), 面板宽);
        for i in 1..4 {
            assert!(
                r.fields[i - 1].bottom() < r.fields[i].top(),
                "第 {i} 个输入框压到上一个"
            );
        }
        assert!(r.fields[3].bottom() < r.summary.top(), "摘要压到输入框");
        assert!(r.summary.bottom() < r.ok.top(), "按钮压到摘要");
        assert!(r.ok.right() <= r.cancel.left(), "两个按钮重叠");
    }

    /// 面板高度 = 内容高（改行数/行高忘了改高度会在这里红）。
    #[test]
    fn 面板放得下内容() {
        let l = 布局();
        let r = rects(&l);
        assert!((r.panel.height() - 内容高()).abs() < 0.01);
        assert!(r.cancel.bottom() <= r.panel.bottom() - m::OPEN_DLG_PAD + 0.01);
    }

    /// 校验：0/0/0/0 ⇒ 拒（全 0）；奇/负/越界各有中文原因；合法给目标尺寸。
    #[test]
    fn 校验逐条() {
        let cells = (10, 10);
        assert!(matches!(
            校验(&文本(["0", "0", "0", "0"]), cells),
            校验::越界(ref why) if why.contains("不能全为 0")
        ));
        assert!(matches!(
            校验(&文本(["1", "0", "0", "0"]), cells),
            校验::越界(ref why) if why.contains("偶数")
        ));
        assert!(matches!(
            校验(&文本(["-2", "0", "0", "0"]), cells),
            校验::越界(ref why) if why.contains("负数")
        ));
        assert_eq!(校验(&文本(["x", "0", "0", "0"]), cells), 校验::不是整数);
        let ok = 校验(&文本(["2", "4", "6", "8"]), cells);
        assert!(ok.可用());
        assert_eq!(
            ok.摘要(),
            tx::fill(tx::EXPAND_SUMMARY_TMPL, &[("w", "18"), ("h", "22")])
        );
        assert_eq!(ok.四向(), Some((2, 4, 6, 8)));
    }

    /// **空串不许当 0**（P3 订正；上游 `int.TryParse("")` == false ⇒ Apply 禁用）。
    ///
    /// 逐格清空都要被拦：任何一格的原文是空串/纯空白 ⇒ [`校验::不是整数`]、`可用() == false`、
    /// 摘要说「不能为空」、`四向()` 给 `None`（⇒ 调用方拿不到可提交的四个数）。
    #[test]
    fn 空串不许当零() {
        let cells = (10, 10);
        for i in 0..4 {
            for 空白 in ["", " ", "\t"] {
                let mut a = ["2", "2", "2", "2"];
                a[i] = 空白;
                let 判定 = 校验(&文本(a), cells);
                assert_eq!(判定, 校验::不是整数, "第 {i} 格填 {空白:?} 必须判非法");
                assert!(!判定.可用(), "第 {i} 格填 {空白:?} ⇒ Apply 必须禁用");
                assert!(判定.四向().is_none());
                assert!(判定.摘要().contains("不能为空"), "{}", 判定.摘要());
            }
        }
        // 全空也一样（初值是 "0"，但用户清空后不能靠"当 0"混过去）
        assert_eq!(
            校验(&文本(["", "", "", ""]), cells),
            校验::不是整数
        );
        // 对照：四格都有数 ⇒ 可用（否则上面那条可能只是"全都不可用"）
        assert!(校验(&文本(["0", "0", "0", "2"]), cells).可用());
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
                    got = interact(ui, l, overlay, (10, 10));
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

    fn 浮层(a: [&str; 4]) -> Overlay {
        Overlay::ExpandMap {
            文本: 文本(a),
            focus: 0,
            error: None,
        }
    }

    /// `Esc` 取消；回车在**合法**时提交、**非法**时什么都不做（不关窗）。
    #[test]
    fn 回车只在合法时提交() {
        let ctx = egui::Context::default();
        let l = 布局();
        // 非法（全 0）⇒ 回车不动
        let mut o = 浮层(["0", "0", "0", "0"]);
        assert_eq!(跑(&ctx, &l, &mut o, 一帧(0.0, 键(egui::Key::Enter))), 动作::None);
        // 奇数 ⇒ 回车不动
        let mut o = 浮层(["1", "0", "0", "0"]);
        assert_eq!(跑(&ctx, &l, &mut o, 一帧(0.2, 键(egui::Key::Enter))), 动作::None);
        // 合法 ⇒ 提交四向
        let mut o = 浮层(["2", "4", "6", "8"]);
        assert_eq!(
            跑(&ctx, &l, &mut o, 一帧(0.4, 键(egui::Key::Enter))),
            动作::Apply {
                left: 2,
                top: 4,
                right: 6,
                bottom: 8
            }
        );
        // Esc 取消
        let mut o = 浮层(["2", "0", "0", "0"]);
        assert_eq!(跑(&ctx, &l, &mut o, 一帧(0.6, 键(egui::Key::Escape))), 动作::Cancel);
    }

    /// 「地图扩边」按钮：合法才响应；非法时点它**什么都不做**（不关窗、不提交）。
    #[test]
    fn 非法时点扩边按钮不提交() {
        let ctx = egui::Context::default();
        let l = 布局();
        let r = rects(&l);
        // 先 hover 一帧再按下+松开（egui 的 clicked() 要求按下前已 hover）
        let mut o = 浮层(["0", "0", "0", "0"]);
        let _ = 跑(
            &ctx,
            &l,
            &mut o,
            一帧(0.0, vec![egui::Event::PointerMoved(r.ok.center())]),
        );
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
        assert_eq!(跑(&ctx, &l, &mut o, 一帧(0.1, 点)), 动作::None, "非法时点按钮不许提交");
        // 「取消」永远可点
        let _ = 跑(
            &ctx,
            &l,
            &mut o,
            一帧(0.2, vec![egui::Event::PointerMoved(r.cancel.center())]),
        );
        let 点取消 = vec![
            egui::Event::PointerButton {
                pos: r.cancel.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: r.cancel.center(),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ];
        assert_eq!(跑(&ctx, &l, &mut o, 一帧(0.3, 点取消)), 动作::Cancel);
    }

    /// **清空任一格 ⇒ 回车与「地图扩边」都不提交**（不关窗、不进动作层）。
    #[test]
    fn 清空任一格则不提交() {
        let ctx = egui::Context::default();
        let l = 布局();
        let r = rects(&l);
        // ⚠ 帧时间必须**单调递增**（emath 的 `Undoer` 在时间倒退时会 panic），
        // 所以每轮用互不重叠的时间片。
        for i in 0..4 {
            let t0 = i as f64 * 10.0;
            let mut a = ["2", "2", "2", "2"];
            a[i] = "";
            let mut o = 浮层(a);
            assert_eq!(
                跑(&ctx, &l, &mut o, 一帧(t0, 键(egui::Key::Enter))),
                动作::None,
                "第 {i} 格为空时回车不许提交"
            );
            // 点按钮同样不提交（先 hover 一帧再按下+松开）
            let _ = 跑(
                &ctx,
                &l,
                &mut o,
                一帧(t0 + 0.1, vec![egui::Event::PointerMoved(r.ok.center())]),
            );
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
            assert_eq!(
                跑(&ctx, &l, &mut o, 一帧(t0 + 0.2, 点)),
                动作::None,
                "第 {i} 格为空时点「地图扩边」不许提交"
            );
        }
    }

    /// 只收 ASCII 数字；退格删一位；一动输入就清掉上次的拒绝原因。
    #[test]
    fn 只收数字且改输入清错误() {
        let ctx = egui::Context::default();
        let l = 布局();
        let mut o = 浮层(["0", "0", "0", "0"]);
        if let Overlay::ExpandMap { error, .. } = &mut o {
            *error = Some("上一次失败".to_string());
        }
        let 事件 = vec![
            egui::Event::Text("1".to_string()),
            egui::Event::Text("2".to_string()),
            egui::Event::Text("x".to_string()),
            egui::Event::Text("٣".to_string()), // 非 ASCII 数字，也不许进
        ];
        let _ = 跑(&ctx, &l, &mut o, 一帧(0.0, 事件));
        match &o {
            Overlay::ExpandMap { 文本, error, .. } => {
                assert_eq!(文本[0], "012", "只该多收 1 和 2：{}", 文本[0]);
                assert!(error.is_none(), "改输入要清掉上次的拒绝原因");
            }
            other => panic!("浮层变了：{other:?}"),
        }
        // 退格
        let _ = 跑(&ctx, &l, &mut o, 一帧(0.2, 键(egui::Key::Backspace)));
        match &o {
            Overlay::ExpandMap { 文本, .. } => assert_eq!(文本[0], "01"),
            other => panic!("浮层变了：{other:?}"),
        }
    }

    /// 上限：新尺寸越界 ⇒ 拒（原因来自 core 的 `Dimensions`）。
    #[test]
    fn 校验上限() {
        let ok = miredit_core::expand_target(32766, 2, 2, 0, 0, 0);
        assert!(ok.is_err(), "32766+2 > 32767 必须拒");
        let 判定 = 校验(&文本(["2", "0", "0", "0"]), (32766, 2));
        assert!(!判定.可用(), "{判定:?}");
        assert!(判定.摘要().contains("越界"), "{}", 判定.摘要());
    }
}
