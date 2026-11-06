//! 「单元格属性」模态（R12-F1）：几何 + 绘制 + 输入。
//!
//! 上游：`CrystalStudio/MainWindow.axaml.cs:740-786`（`OpenCellProperties`）——
//! 一个 340 宽的自适应高对话框、三个 0–255 的文本框、水印 `Mixed — leave unchanged`、
//! 非法值给一句提示且**不提交**，「应用」时**与初值逐字比较**、只把改过的字段交给
//! `ApplyMetadata`（见 [`crate::cell_props`] 的纯函数与 `miredit_core::EditSession::apply_metadata`）。
//!
//! 与既有路径模态（[`super::dialog`]）的差别：
//! * 三个输入框（不是一条路径），`Tab` 在三个框之间转；
//! * 框里数字非法 ⇒ **模态内就拒**（红字 + 不提交、不关窗），不是"提交后报错"。
//!
//! 为什么单独一个模块：`dialog.rs` 已经 1000+ 行（路径模态 + 小地图模态），
//! 这里只服务一个模态，独立成文件才能一眼看完。

use egui::{pos2, vec2, Align2, Painter, Rect, Ui};

use crate::cell_props::字段;
use crate::layout::Layout;
use crate::menu::Overlay;
use crate::metrics as m;
use crate::paint as pt;
use crate::theme as t;
use crate::ui_text as tx;

/// 模态宽度（官方 `Width = 340`）。
const 面板宽: f32 = 340.0;
/// 面板高度：标题 24 + 头行 22 + 三个字段（16 + 32 + 10）×3 + 说明行 20 + 错误行 24 + 按钮 40 + 内边距。
const 面板高: f32 = 24.0 + 22.0 + 3.0 * 58.0 + 34.0 + 24.0 + 40.0 + 2.0 * 24.0;


/// 模态里所有可点/可画矩形（绘制与命中测试共用同一份几何）。
#[derive(Clone, Copy, Debug)]
pub struct Rects {
    /// 面板整体。
    pub panel: Rect,
    /// 三个输入框（顺序 = [`字段::全`]）。
    pub fields: [Rect; 3],
    /// Mir3 说明行（没有门字节时才有内容，仍占位以免几何跳动）。
    pub note: Rect,
    /// 非法值提示带。
    pub error: Rect,
    /// 主按钮「应用属性」。
    pub ok: Rect,
    /// 「取消」。
    pub cancel: Rect,
}

/// 算模态几何（居中，与路径模态同一套内边距/按钮尺寸）。
#[must_use]
pub fn rects(l: &Layout) -> Rects {
    let panel = Rect::from_center_size(l.content.center(), vec2(面板宽, 面板高));
    let inner_l = panel.left() + m::OPEN_DLG_PAD;
    let inner_r = panel.right() - m::OPEN_DLG_PAD;
    let 宽 = inner_r - inner_l;
    let mut y = panel.top() + m::OPEN_DLG_PAD + 24.0 + 22.0;
    let mut fields = [Rect::NOTHING; 3];
    for f in &mut fields {
        y += 16.0; // 字段标题
        *f = Rect::from_min_size(pos2(inner_l, y), vec2(宽, m::OPEN_DLG_FIELD_H));
        y += m::OPEN_DLG_FIELD_H + 10.0;
    }
    let note = Rect::from_min_size(pos2(inner_l, y), vec2(宽, 34.0));
    let error = Rect::from_min_size(pos2(inner_l, note.bottom()), vec2(宽, 24.0));
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
        note,
        error,
        ok,
        cancel,
    }
}

/// 用户在模态里做的一次动作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum 动作 {
    /// 什么都没发生。
    None,
    /// 取消（`Esc` / 点取消）。
    Cancel,
    /// 提交：三个字段的**最终取值**（`None` = 这一字段不动 ⇒ 混合值保留）。
    Apply {
        /// 光照。
        light: Option<u8>,
        /// 门索引。
        door: Option<u8>,
        /// 门偏移。
        offset: Option<u8>,
    },
}

/// 画模态。`头行` 由调用方算（`cell_props::头行`，需要地图尺寸）。
pub fn draw(
    p: &Painter,
    l: &Layout,
    头行: &str,
    文本: &[String; 3],
    焦点: usize,
    error: Option<&str>,
    有门字节: bool,
) {
    let r = rects(l);
    let pal = t::ui();
    // ① 面板 + 标题
    pt::fill(p, r.panel, pal.panel_bg);
    pt::border(p, r.panel, pal.separator);
    pt::text(
        p,
        pos2(
            r.panel.left() + m::OPEN_DLG_PAD,
            r.panel.top() + m::OPEN_DLG_PAD,
        ),
        Align2::LEFT_TOP,
        tx::CELL_PROPS_TITLE,
        m::TITLE_FONT,
        pal.text,
    );
    // ② 头行（单选一格 / 选区 N 格）
    pt::text(
        p,
        pos2(
            r.panel.left() + m::OPEN_DLG_PAD,
            r.panel.top() + m::OPEN_DLG_PAD + 24.0,
        ),
        Align2::LEFT_TOP,
        头行,
        m::BODY_FONT,
        pal.text_muted,
    );
    // ③ 三个字段：标题 + 输入框（禁用时画灰底灰字）
    for (i, f) in 字段::全.iter().enumerate() {
        let 可用 = f.可用(有门字节).is_ok();
        let rect = r.fields[i];
        pt::text(
            p,
            pos2(rect.left(), rect.top() - 16.0),
            Align2::LEFT_TOP,
            f.标签(),
            m::BODY_FONT,
            if 可用 { pal.text } else { pal.text_muted },
        );
        pt::fill(p, rect, pal.input_bg);
        pt::border(p, rect, if 焦点 == i { pal.accent } else { pal.input_border });
        let 文本 = &文本[i];
        if 文本.is_empty() && 可用 {
            // 水印（官方 `Watermark = "Mixed — leave unchanged"`）
            pt::text(
                p,
                pos2(rect.left() + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                tx::CELL_PROPS_MIXED,
                m::BODY_FONT,
                pal.text_muted,
            );
        } else {
            pt::text(
                p,
                pos2(rect.left() + 8.0, rect.center().y),
                Align2::LEFT_CENTER,
                文本,
                m::BODY_FONT,
                if 可用 { pal.text } else { pal.text_muted },
            );
        }
    }
    // ④ Mir3 说明行（两行：为什么门框禁用 + 光照的有效范围）
    //
    // 只有没有门字节的格式（Mir3 两档）才画；其余格式这一行留空
    // （不放"混合值"解释 —— 输入框水印已经说了）。
    if !有门字节 {
        pt::text(
            p,
            r.note.min,
            Align2::LEFT_TOP,
            tx::CELL_PROPS_NO_DOOR,
            m::BODY_FONT,
            pal.warn_banner_text,
        );
        pt::text(
            p,
            pos2(r.note.left(), r.note.top() + 14.0),
            Align2::LEFT_TOP,
            tx::CELL_PROPS_LIGHT_HINT_MIR3,
            m::BODY_FONT,
            pal.text_muted,
        );
    }
    // ⑤ 非法值红字（只在有错时画）
    if let Some(why) = error {
        pt::fill(p, r.error, pal.warn_banner_bg);
        pt::border(p, r.error, pal.warn_banner_edge);
        pt::text(
            p,
            pos2(r.error.left() + 6.0, r.error.top() + 4.0),
            Align2::LEFT_TOP,
            why,
            m::BODY_FONT,
            pal.warn_banner_text,
        );
    }
    // ⑥ 两个按钮
    pt::fill(p, r.ok, pal.accent);
    pt::border(p, r.ok, pal.accent_dark);
    pt::text(
        p,
        r.ok.center(),
        Align2::CENTER_CENTER,
        tx::CELL_PROPS_OK,
        m::BODY_FONT,
        pal.accent_button_text,
    );
    pt::fill(p, r.cancel, pal.input_bg);
    pt::border(p, r.cancel, pal.input_border);
    pt::text(
        p,
        r.cancel.center(),
        Align2::CENTER_CENTER,
        tx::CELL_PROPS_CANCEL,
        m::BODY_FONT,
        pal.text,
    );
}

/// 处理模态输入（**只在 `Overlay::CellProps` 时调用**）。
///
/// 键盘：`Esc` 取消、`Enter` 提交、`Tab`/`Shift+Tab` 换框、其余文本进当前焦点框。
/// 提交时用 [`cell_props::决定写入`]：与初值逐字相同的字段 ⇒ `None`（不写），
/// 任何一个字段非法 ⇒ 红字 + **不提交、不关窗**（官方 `MessageBox` 的等价物）。
pub fn interact(ui: &mut Ui, l: &Layout, overlay: &mut Overlay) -> 动作 {
    let Overlay::CellProps {
        area: _,
        初值,
        文本,
        focus,
        error,
        有门字节,
    } = overlay
    else {
        return 动作::None;
    };
    let r = rects(l);
    let 有门 = *有门字节;

    // ① Esc = 取消
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return 动作::Cancel;
    }
    // ② 两个按钮（只认真鼠标点击）
    if 真点击(ui, r.cancel, ui.id().with(("cell_props", "cancel"))) {
        return 动作::Cancel;
    }
    if 真点击(ui, r.ok, ui.id().with(("cell_props", "ok"))) {
        return 提交(初值, 文本, 有门, error);
    }
    // ③ 三个输入框：点一下换焦点（禁用的门框点不动）
    for (i, f) in 字段::全.iter().enumerate() {
        if f.可用(有门).is_err() {
            continue;
        }
        if 真点击(ui, r.fields[i], ui.id().with(("cell_props", "field", i))) {
            *focus = i;
            return 动作::None;
        }
    }
    // ④ 回车 = 提交（放在按钮之后：点按钮那一帧也可能带 Enter）
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        return 提交(初值, 文本, 有门, error);
    }
    // ⑤ Tab / Shift+Tab 换焦点（跳过禁用的门框）
    let (tab, back) = ui.input(|i| {
        let tab = i.key_pressed(egui::Key::Tab);
        (tab, tab && i.modifiers.shift)
    });
    if tab {
        // 0 → 1 → 2 → 0（Shift+Tab 反向）；禁用的框不在链上也没关系 —— 第 ⑥ 段会拦
        let cur = (*focus).min(2);
        *focus = if back {
            if cur == 0 { 2 } else { cur - 1 }
        } else if cur >= 2 {
            0
        } else {
            cur + 1
        };
        return 动作::None;
    }
    // ⑥ 文本与退格：只进当前焦点框；禁用的框吃不到
    let 当前 = *focus;
    if 字段::全[当前].可用(有门).is_err() {
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
        for t in texts {
            for c in t.chars() {
                // 只收 ASCII 数字（官方 TextBox 也是数字输入；非数字反正会在提交时被拒）
                if !c.is_control() {
                    文本[当前].push(c);
                }
            }
        }
        if backspace {
            文本[当前].pop();
        }
        // 一动输入就把上次的红字清掉（官方每敲一下就重验）
        *error = None;
    }
    动作::None
}

/// 提交：逐字段 [`cell_props::决定写入`]；任一字段非法 ⇒ 红字 + 不提交。
fn 提交(
    初值: &[String; 3],
    文本: &[String; 3],
    _有门: bool,
    error: &mut Option<String>,
) -> 动作 {
    let mut 结果 = [None; 3];
    for (i, f) in 字段::全.iter().enumerate() {
        match crate::cell_props::决定写入(&文本[i], &初值[i]) {
            crate::cell_props::取值::不动 => {}
            crate::cell_props::取值::设为(v) => 结果[i] = Some(v),
            crate::cell_props::取值::非法 => {
                *error = Some(tx::fill(tx::CELL_PROPS_ERR_RANGE, &[("field", f.标签())]));
                return 动作::None;
            }
        }
    }
    *error = None;
    动作::Apply {
        light: 结果[字段::光照.下标()],
        door: 结果[字段::门索引.下标()],
        offset: 结果[字段::门偏移.下标()],
    }
}

/// 只认真实鼠标点击（与 [`super::dialog`] 同一口径：`clicked()` 会被键盘焦点误触发）。
fn 真点击(ui: &mut Ui, rect: Rect, id: egui::Id) -> bool {
    ui.interact(rect, id, egui::Sense::click()).clicked()
}
