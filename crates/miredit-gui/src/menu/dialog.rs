//! 三个浮层的绘制与输入：**打开地图模态**、**另存为模态**（都可输入）与**快捷键浮层**（只读）。
//!
//! # 为什么"打开地图"要一个输入框而不是文件对话框
//!
//! 本 crate 的依赖纪律是「只用 egui/eframe + workspace 内库」（见 `Cargo.toml` 顶部注释），
//! 而 egui 自身**没有**文件选择器，`rfd` 之类是新增依赖 ⇒ 不加。
//! 于是这里做的是**真输入**：用户把 `.map` 的完整路径敲进输入框，回车后走
//! [`crate::menu::action::open_path`]（与启动时完全相同的 `Document::load`）。
//! 它不是假控件：输入的文字真的进了 `path`，真的被读盘、真的换图。
//!
//! # 「另存为」是同一套输入的两个实例（2026-11-05）
//!
//! 官方 `文件 → 另存为` 弹的是系统文件对话框（`SaveAs_04C37C25`），本仓同样没有那个依赖，
//! 所以走**同一套**输入：标题/提示/按钮文案换成 [`crate::ui_text::SAVE_AS_DLG_TITLE`] 那三条，
//! 回车后提交 [`DialogAction::SaveAs`] → [`crate::menu::action::save_as_path`] →
//! [`crate::save::save_as_session`]。
//!
//! ⚠ **几何、焦点、光标、退格、`Esc`/`Enter` 只有一份实现**
//! （[`dialog_rects`] + [`interact_path_dialog`]），两个模态的差别只有 [`Kind`] 里的四条文案
//! 与提交出来的动作 —— 不许各写一套（那样"打开能退格、另存为不能"这类偏差会立刻出现）。
//!
//! # 「导出小地图」多一组**尺寸输入**（2026-11-06，N F-R14-1）
//!
//! 官方 `MinimapExportWindow.cs:20-41@6450da90` 除了路径，还要「预设下拉 + 自定义宽高」：
//! 预设 `Small (¼) / Medium (½) / Large (1:1)`（除数 `{4,2,1}`，**默认 Small**，选中即把
//! `document.Width*48/divisor` × `document.Height*32/divisor` 写进两个 TextBox），
//! 提交前 `Validate`（`int.TryParse` ∧ `MinimapExporter.ValidSize`）。
//! ⇒ 这个模态的排版**比通用路径模态高三行**（[`minimap_rects`]），三档预设 / 宽高文本框 /
//! 错误提示带都是**真控件**：点击真改状态、非法尺寸当场拒绝且**一个字节都不写**
//! （[`MinimapDlg::resolve`]，倍率与校验全部复用 [`crate::export`]，这里不另算）。
//!
//! # 键盘怎么进到输入框
//!
//! 与资源面板搜索框同一套做法（`palette.rs` 手写焦点 + `Event::Text`）：
//! 本仓没有用 `egui::TextEdit`（它是 widget，要 `Ui` 布局），
//! 而是直接读 `Event::Text` / `Backspace` —— 好处是弹层的排版仍由 `metrics` 常量决定。
//!
//! ⚠ 模态打开期间 `app::handle_keys` **必须整体让位**（否则敲 `f` 会切工具、敲 `o` 会切印章），
//! 只保留 `Esc` / `Enter` / `Tab` / `Backspace` / 文本输入。

use egui::{pos2, vec2, Align2, Painter, Rect, Ui};

use crate::export::{MinimapPreset, MinimapSize};
use crate::layout::Layout;
use crate::menu::Overlay;
use crate::metrics as m;
use crate::paint as pt;
use crate::theme as t;
use crate::ui_text as tx;

/// 路径输入模态的关键矩形（绘制与命中测试共用；两个模态**同一套几何**）。
#[derive(Clone, Copy, Debug)]
pub struct DialogRects {
    /// 面板整体。
    pub panel: Rect,
    /// 路径输入框。
    pub field: Rect,
    /// 主按钮（打开 / 另存为）。
    pub ok: Rect,
    /// 「取消」按钮。
    pub cancel: Rect,
}

/// 算路径输入模态的几何（居中）。
pub fn dialog_rects(l: &Layout) -> DialogRects {
    dialog_rects_tall(l, m::OPEN_DLG_H)
}

/// 按给定**面板高度**算「面板 + 路径框 + 两个按钮」这一套（其它模态与小地图模态共用）。
///
/// 路径框与标题仍贴面板顶、两个按钮仍贴面板底 ⇒ 面板一变高，多出来的空档全落在
/// 「提示行」与「按钮」之间；导出小地图模态正好把那一段摆成预设行 / 宽高行 / 错误行
/// （见 [`minimap_rects`]）。**横向几何与 `OPEN_DLG_H` 那份逐像素相同。**
fn dialog_rects_tall(l: &Layout, panel_h: f32) -> DialogRects {
    let panel = Rect::from_center_size(l.content.center(), vec2(m::OPEN_DLG_W, panel_h));
    let inner_l = panel.left() + m::OPEN_DLG_PAD;
    let inner_r = panel.right() - m::OPEN_DLG_PAD;
    // 纵向：标题 24 → 输入框 → 提示 18 → 按钮
    let field_top = panel.top() + m::OPEN_DLG_PAD + 24.0;
    let field = Rect::from_min_size(
        pos2(inner_l, field_top),
        vec2(inner_r - inner_l, m::OPEN_DLG_FIELD_H),
    );
    let btn_y = panel.bottom() - m::OPEN_DLG_PAD - m::OPEN_DLG_BTN_H;
    let cancel = Rect::from_min_size(
        pos2(inner_r - m::OPEN_DLG_BTN_W, btn_y),
        vec2(m::OPEN_DLG_BTN_W, m::OPEN_DLG_BTN_H),
    );
    let ok = Rect::from_min_size(
        pos2(cancel.left() - 10.0 - m::OPEN_DLG_BTN_W, btn_y),
        vec2(m::OPEN_DLG_BTN_W, m::OPEN_DLG_BTN_H),
    );
    DialogRects {
        panel,
        field,
        ok,
        cancel,
    }
}

/// 打开地图模态的几何（= [`dialog_rects`]，保留这个名字给既有调用方/用例）。
pub fn open_rects(l: &Layout) -> DialogRects {
    dialog_rects(l)
}

/// 另存为模态的几何（= [`dialog_rects`]；两个模态的排版**必须一致**）。
pub fn save_as_rects(l: &Layout) -> DialogRects {
    dialog_rects(l)
}

// ═══════════════ 「导出小地图」模态：几何 + 尺寸输入（2026-11-06，N F-R14-1） ═══════════════
//
// 官方 `MinimapExportWindow.cs:20-41@6450da90` = 预设下拉（Small ¼ / Medium ½ / Large 1:1，
// 除数 {4,2,1}，默认 Small）+ 自定义宽/高 TextBox + `Validate`（`int.TryParse` ∧ `ValidSize`）。

/// 三档预设的**显示顺序**（官方 ComboBox 顺序：Small → Medium → Large）。
pub const MINIMAP_PRESETS: [MinimapPreset; 3] = [
    MinimapPreset::Small,
    MinimapPreset::Medium,
    MinimapPreset::Large,
];

/// 三档预设的显示名（**与官方 `MinimapExportWindow.cs:21` 逐字相同**，不译）。
pub fn preset_label(p: MinimapPreset) -> &'static str {
    match p {
        MinimapPreset::Small => tx::MINIMAP_PRESET_SMALL,
        MinimapPreset::Medium => tx::MINIMAP_PRESET_MEDIUM,
        MinimapPreset::Large => tx::MINIMAP_PRESET_LARGE,
    }
}

/// 「导出小地图」模态里三个输入框的焦点（同一时刻只有一个吃键盘）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiniFocus {
    /// 目标 `.png` 路径框（初值 = `<地图名>_小地图.png`）。
    Path,
    /// 自定义宽度框。
    Width,
    /// 自定义高度框。
    Height,
}

impl MiniFocus {
    /// `Tab` 的下一格：路径 → 宽 → 高 → 路径。
    pub fn next(self) -> Self {
        match self {
            MiniFocus::Path => MiniFocus::Width,
            MiniFocus::Width => MiniFocus::Height,
            MiniFocus::Height => MiniFocus::Path,
        }
    }

    /// `Shift+Tab` 的上一格。
    pub fn prev(self) -> Self {
        match self {
            MiniFocus::Path => MiniFocus::Height,
            MiniFocus::Width => MiniFocus::Path,
            MiniFocus::Height => MiniFocus::Width,
        }
    }
}

/// 「导出小地图」模态的**尺寸输入状态**（三档预设 + 两个可手改的宽高 + 错误提示）。
///
/// 官方流程照抄：选预设只是把 `W*48/divisor` × `H*32/divisor` **写进两个文本框**，
/// 提交时再由 `Validate` 解析文本框。所以这里存的也是「预选项 + 文本框原文」，
/// 折成 [`MinimapSize`] 只发生在 [`MinimapDlg::resolve`]。
///
/// ⚠ **倍率与校验一个字都不在这里算**：
/// - 预设像素 = [`MinimapSize::Preset`] + [`MinimapSize::resolve`]（内部用 `MinimapPreset::divisor`）；
/// - 合法性 = [`crate::export::valid_minimap_size`]（`MINIMAP_MAX_PIXELS` + 更严的既有护栏）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinimapDlg {
    /// 预设的当前项（**默认 `Small`**，与官方默认一致）。
    preset: MinimapPreset,
    /// 宽度框原文（选中预设时被重写成该档像素数；可手改）。
    width_text: String,
    /// 高度框原文。
    height_text: String,
    /// 键盘焦点。
    focus: MiniFocus,
    /// 最近一次提交被拒的中文原因（画在模态里；用户一动输入就清掉）。
    error: Option<String>,
}

impl MinimapDlg {
    /// 新开模态：预设 = **Small**（官方 `MinimapExportWindow.cs:21` 的默认项），
    /// 两个文本框 = 该档算出的像素尺寸。
    pub fn new(cells: (u32, u32)) -> Self {
        let mut dlg = Self {
            preset: MinimapPreset::Small,
            width_text: String::new(),
            height_text: String::new(),
            focus: MiniFocus::Path,
            error: None,
        };
        dlg.select(MinimapPreset::Small, cells);
        dlg
    }

    /// 当前选中的预设（只读出口，给测试与绘制读）。
    pub fn preset(&self) -> MinimapPreset {
        self.preset
    }

    /// 宽度框原文。
    pub fn width_text(&self) -> &str {
        &self.width_text
    }

    /// 高度框原文。
    pub fn height_text(&self) -> &str {
        &self.height_text
    }

    /// 最近一次被拒的中文原因（`None` = 没有错误）。
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// 选一档预设：把该档像素数写进两个文本框（官方 `MinimapExportWindow.cs:36-38`）。
    ///
    /// 算不出来（地图格数 × 48/32 溢出 `u32`）时**清空文本框并给出中文原因**，
    /// 不静默留上一个档位的数字。写完立刻跑一次 [`MinimapDlg::refresh`]
    /// —— 官方 `SelectionChanged` 之后紧接着就是 `Validate()`（`:31-41`）。
    pub fn select(&mut self, preset: MinimapPreset, cells: (u32, u32)) {
        self.preset = preset;
        match MinimapSize::Preset(preset).resolve(cells.0, cells.1) {
            Ok((w, h)) => {
                self.width_text = w.to_string();
                self.height_text = h.to_string();
                self.error = None;
            }
            Err(why) => {
                self.width_text.clear();
                self.height_text.clear();
                self.error = Some(why);
            }
        }
        self.refresh(cells);
    }

    /// **活校验**（官方 `MinimapExportWindow.cs:44-51` 的 `Validate()`）：
    /// 每次改动文本框/预选后立刻算一遍，合法就清空错误、不合法就把中文原因摆出来，
    /// 同时决定「导出」按钮是不是可用（绘制侧读 [`MinimapDlg::is_valid`]）。
    ///
    /// ⚠ 官方 `Validate()` 是在 `PropertyChanged` 事件里调的（每敲一个字符一次），
    /// 本仓是同口径：`interact_export_minimap` 每次改完就调这个方法。
    pub fn refresh(&mut self, cells: (u32, u32)) {
        self.error = self.resolve(cells).err();
    }

    /// 当前输入是否**可以导出**（官方 `ExportButton.IsEnabled`）。
    pub fn is_valid(&self, cells: (u32, u32)) -> bool {
        self.resolve(cells).is_ok()
    }

    /// 官方 `Validate` 的等价：两个文本框都要是 `int`、且过 `ValidSize`；**失败绝不写盘**。
    ///
    /// 成功返回本次要用的口径：宽高**仍等于所选预设**算出的像素 ⇒ `Preset(p)`
    /// （默认 Small 与 CLI 缺省 `MinimapSize::Default` 逐像素相同），手改过 ⇒ `Explicit`。
    pub fn resolve(&self, cells: (u32, u32)) -> Result<MinimapSize, String> {
        let width = parse_side(&self.width_text, tx::EXPORT_MINI_DLG_WIDTH)?;
        let height = parse_side(&self.height_text, tx::EXPORT_MINI_DLG_HEIGHT)?;
        crate::export::valid_minimap_size(width, height)
            .map_err(|why| format!("{why}{}", tx::EXPORT_MINI_ERR_SIZE_SUFFIX))?;
        let unchanged = MinimapSize::Preset(self.preset).resolve(cells.0, cells.1).ok()
            == Some((width, height));
        Ok(if unchanged {
            MinimapSize::Preset(self.preset)
        } else {
            MinimapSize::Explicit { width, height }
        })
    }

    /// 当前焦点对应的输入缓冲。
    ///
    /// 路径框的缓冲是 [`Overlay::ExportMinimap::path`]，所以单独传进来（两处借用互不相干）。
    fn focus_mut<'a>(&'a mut self, path: &'a mut String) -> &'a mut String {
        match self.focus {
            MiniFocus::Path => path,
            MiniFocus::Width => &mut self.width_text,
            MiniFocus::Height => &mut self.height_text,
        }
    }
}

/// 官方 `int.TryParse` 的等价解析（**Int32**：#6450da90 的 `out int` 就是 32 位）：
/// 前后空白忽略、可带正负号；非数字 / 超出 `i32` ⇒ 中文原因。
///
/// 0 与负数**能解析**但过不了 `w > 0` ⇒ 走「必须是正整数」那条原因，
/// 与官方 `ValidSize` 的 `width > 0 && height > 0` 对得上。
fn parse_side(text: &str, field: &str) -> Result<u32, String> {
    let trimmed = text.trim();
    let Ok(v) = trimmed.parse::<i32>() else {
        return Err(tx::fill(
            tx::EXPORT_MINI_ERR_SIZE_PARSE_TMPL,
            &[("field", field), ("got", trimmed)],
        ));
    };
    if v <= 0 {
        return Err(tx::fill(
            tx::EXPORT_MINI_ERR_SIZE_POSITIVE_TMPL,
            &[("field", field), ("got", trimmed)],
        ));
    }
    Ok(v as u32)
}

/// 「导出小地图」模态的关键矩形（绘制与命中测试共用）。
#[derive(Clone, Copy, Debug)]
pub struct MinimapRects {
    /// 面板 / 路径框 / 两个按钮（与其它三个模态**同一套横向几何**）。
    pub base: DialogRects,
    /// 三档预设按钮（顺序 = [`MINIMAP_PRESETS`]）。
    pub presets: [Rect; 3],
    /// 宽输入框。
    pub width: Rect,
    /// 高输入框。
    pub height: Rect,
    /// 错误提示带（没有错误时不画，位置固定）。
    pub error: Rect,
}

/// 算「导出小地图」模态的几何：路径框与两个按钮沿用通用几何，中间**多出三行**。
///
/// 纵向（面板顶为 0）：`16(PAD) → 24(标题) → 28(路径框) → 12+18(提示) → 26(预设行)
/// → 10 → 26(宽高行) → 10 → 32(错误行) → 10 → 26(按钮) → 16`，合计 [`m::MINIMAP_DLG_H`]。
pub fn minimap_rects(l: &Layout) -> MinimapRects {
    let base = dialog_rects_tall(l, m::MINIMAP_DLG_H);
    let inner_l = base.panel.left() + m::OPEN_DLG_PAD;

    // ① 预设行：左侧标签列 + 三个等宽按钮（标签在绘制时右对齐到列内）
    let preset_top = base.field.bottom() + 12.0 + 18.0;
    let preset_x = inner_l + m::MINIMAP_DLG_PRESET_COL_W + m::MINIMAP_DLG_LABEL_GAP;
    let mut presets = [Rect::ZERO; 3];
    for (i, slot) in presets.iter_mut().enumerate() {
        let x = preset_x + i as f32 * (m::MINIMAP_DLG_PRESET_W + m::MINIMAP_DLG_LABEL_GAP);
        *slot = Rect::from_min_size(
            pos2(x, preset_top),
            vec2(m::MINIMAP_DLG_PRESET_W, m::MINIMAP_DLG_ROW_H),
        );
    }

    // ② 宽高行：`宽 [__] px   高 [__] px`
    let size_top = preset_top + m::MINIMAP_DLG_ROW_H + m::MINIMAP_DLG_GAP;
    let width = Rect::from_min_size(
        pos2(
            inner_l + m::MINIMAP_DLG_SIZE_LABEL_W + m::MINIMAP_DLG_LABEL_GAP,
            size_top,
        ),
        vec2(m::MINIMAP_DLG_FIELD_W, m::MINIMAP_DLG_ROW_H),
    );
    let height_left = width.right()
        + m::MINIMAP_DLG_UNIT_W
        + m::MINIMAP_DLG_GAP * 2.0
        + m::MINIMAP_DLG_SIZE_LABEL_W
        + m::MINIMAP_DLG_LABEL_GAP;
    let height = Rect::from_min_size(
        pos2(height_left, size_top),
        vec2(m::MINIMAP_DLG_FIELD_W, m::MINIMAP_DLG_ROW_H),
    );

    // ③ 错误提示带（横跨面板内宽）
    let error = Rect::from_min_size(
        pos2(
            inner_l,
            size_top + m::MINIMAP_DLG_ROW_H + m::MINIMAP_DLG_GAP,
        ),
        vec2(
            base.panel.right() - m::OPEN_DLG_PAD - inner_l,
            m::MINIMAP_DLG_ERR_H,
        ),
    );

    MinimapRects {
        base,
        presets,
        width,
        height,
        error,
    }
}

/// 模态里的一次输入结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogAction {
    /// 什么都不做。
    None,
    /// 取消（Esc / 点取消）。
    Cancel,
    /// 打开这个路径（回车 / 点打开）。
    Open(String),
    /// 另存为到这个路径（回车 / 点另存为）。
    SaveAs(String),
    /// 导出选区到这个路径（回车 / 点导出）。
    ExportSelection(String),
    /// 导出小地图到这个路径、**按这个尺寸口径**（回车 / 点导出）。
    ///
    /// 尺寸是 [`MinimapDlg::resolve`] 校验过的结果：预设仍报 `Preset(p)`（默认 Small）、
    /// 手改宽高报 `Explicit { .. }`。非法尺寸**不会**走到这里（模态里就拒了，一个字节不写）。
    ExportMinimap {
        /// 目标 `.png` 路径。
        path: String,
        /// 尺寸口径（宽高像素 / 三档预设）。
        size: MinimapSize,
    },
}

/// 路径输入模态的**四种**用途（差别只有四条文案与提交出来的动作）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 「文件 → 打开地图…」。
    Open,
    /// 「文件 → 另存为…」。
    SaveAs,
    /// 「文件 → 导出选区(E)…」。
    ExportSelection,
    /// 「文件 → 导出小地图(M)…」。
    ExportMinimap,
}

impl Kind {
    /// 模态标题。
    fn title(self) -> &'static str {
        match self {
            Kind::Open => tx::OPEN_DLG_TITLE,
            Kind::SaveAs => tx::SAVE_AS_DLG_TITLE,
            Kind::ExportSelection => tx::EXPORT_SEL_DLG_TITLE,
            Kind::ExportMinimap => tx::EXPORT_MINI_DLG_TITLE,
        }
    }

    /// 输入框下方那句提示。
    fn hint(self) -> &'static str {
        match self {
            Kind::Open => tx::OPEN_DLG_HINT,
            Kind::SaveAs => tx::SAVE_AS_DLG_HINT,
            Kind::ExportSelection => tx::EXPORT_SEL_DLG_HINT,
            Kind::ExportMinimap => tx::EXPORT_MINI_DLG_HINT,
        }
    }

    /// 主按钮文字。
    fn ok_text(self) -> &'static str {
        match self {
            Kind::Open => tx::OPEN_DLG_OK,
            Kind::SaveAs => tx::SAVE_AS_DLG_OK,
            Kind::ExportSelection => tx::EXPORT_SEL_DLG_OK,
            Kind::ExportMinimap => tx::EXPORT_MINI_DLG_OK,
        }
    }

    /// 两个按钮的 egui id 前缀（**每个模态不同 id**：同一帧只有一个浮层，但 id 撞了会让
    /// `interact` 的点击归属变得依赖绘制顺序）。
    fn id_prefix(self) -> &'static str {
        match self {
            Kind::Open => "open_dlg",
            Kind::SaveAs => "save_as_dlg",
            Kind::ExportSelection => "export_sel_dlg",
            Kind::ExportMinimap => "export_mini_dlg",
        }
    }

    /// 把输入框里的文字折成提交动作（**只服务三种路径模态**）。
    ///
    /// ⚠ 导出小地图**不在这条路上**：它还要尺寸，提交统一走
    /// [`interact_export_minimap`] → [`DialogAction::ExportMinimap`]。
    /// 这里给 `None` + [`interact_path_dialog`] 的 `debug_assert_ne!` 一起保证它到不了。
    fn submit(self, path: &str) -> DialogAction {
        match self {
            Kind::Open => DialogAction::Open(path.to_owned()),
            Kind::SaveAs => DialogAction::SaveAs(path.to_owned()),
            Kind::ExportSelection => DialogAction::ExportSelection(path.to_owned()),
            Kind::ExportMinimap => DialogAction::None,
        }
    }
}

/// 从浮层里取出**该种类**的输入缓冲（种类与变体不匹配 ⇒ `None`，调用方什么都不做）。
fn path_mut(overlay: &mut Overlay, kind: Kind) -> Option<&mut String> {
    match (kind, overlay) {
        (Kind::Open, Overlay::OpenMap { path }) => Some(path),
        (Kind::SaveAs, Overlay::SaveAs { path }) => Some(path),
        (Kind::ExportSelection, Overlay::ExportSelection { path }) => Some(path),
        (Kind::ExportMinimap, Overlay::ExportMinimap { path, .. }) => Some(path),
        _ => None,
    }
}

/// 输入框的底 + 边。`focused` ⇒ 边用强调色（多输入框模态用它标"这一格吃键盘"）。
fn draw_field_frame(p: &Painter, r: Rect, focused: bool) {
    let pal = t::ui();
    pt::fill(p, r, pal.input_search_bg);
    pt::border(p, r, if focused { pal.accent } else { pal.input_border });
}

/// 输入框里的文字，**返回光标应落的 x**。
///
/// 太长就从左边裁：只画尾部（用户正在敲的通常是最右边那几个字）。
fn draw_field_text(p: &Painter, r: Rect, text: &str) -> f32 {
    let pal = t::ui();
    let text_x = r.left() + 8.0;
    let text_right = r.right() - 8.0;
    let w = pt::text_w(p, text, m::BODY_FONT);
    let visible = if w > (text_right - text_x) {
        let cut = ((w - (text_right - text_x)) / m::BODY_FONT).ceil() as usize + 1;
        text.chars().skip(cut.min(text.chars().count())).collect::<String>()
    } else {
        text.to_owned()
    };
    pt::text(
        p,
        pos2(text_x, r.center().y),
        Align2::LEFT_CENTER,
        &visible,
        m::BODY_FONT,
        pal.text,
    );
    (text_x + pt::text_w(p, &visible, m::BODY_FONT)).min(text_right)
}

/// 输入框右端那条 1px 竖线（"正在输入"的那一格）。
fn draw_caret(p: &Painter, r: Rect, caret_x: f32) {
    pt::fill(
        p,
        Rect::from_min_size(pos2(caret_x, r.top() + 5.0), vec2(1.0, r.height() - 10.0)),
        t::ui().accent_dark,
    );
}

/// 这一帧有没有一个**真鼠标点击**落在这个控件上。
///
/// ⚠ **为什么必须有这一层**：egui 会把 `Enter` / `Space` 折成**假点击**打到"当前拿到键盘
/// 焦点"的可点控件上（`egui-0.33.3/src/context.rs:1335-1342` 的 `FAKE_PRIMARY_CLICKED`）。
/// 本仓的模态**自己管焦点**、还用 `Tab` 换输入框 ⇒ egui 自己的焦点会落到「取消」按钮上，
/// 于是「Tab 一下再回车」变成**点取消**（2026-11-06 实测：`interact_open_map` 返回 `Cancel`，
/// 而同一条用例换个焦点状态又会返回确认 —— 全看 egui 上一帧把焦点给了谁）。
/// 真鼠标点击一定伴随本帧的 pointer click、假点击没有 ⇒ 用这条把假点击挡掉，
/// `Enter` 就只走各模态自己的「回车 = 确认」分支，`Esc` 只走取消分支。
fn 真点击(ui: &Ui, rect: Rect, id: egui::Id) -> bool {
    ui.interact(rect, id, egui::Sense::click()).clicked() && ui.input(|i| i.pointer.any_click())
}

/// 画路径输入模态（打开 / 另存为 / 导出选区共用）。
fn draw_path_dialog(p: &Painter, l: &Layout, path: &str, kind: Kind) {
    let pal = t::ui();
    pt::fill(p, l.content, pal.modal_scrim);
    let r = dialog_rects(l);
    pt::fill(p, r.panel, pal.panel_bg);
    pt::border(p, r.panel, pal.menu_popup_border);

    let inner_l = r.panel.left() + m::OPEN_DLG_PAD;
    // 标题
    pt::text(
        p,
        pos2(inner_l, r.panel.top() + m::OPEN_DLG_PAD + 12.0),
        Align2::LEFT_CENTER,
        kind.title(),
        m::PANEL_TITLE_FONT,
        pal.text,
    );

    // 输入框：底 + 边 + 文字（右端画一个竖线当光标，因为它是"正在输入"的那一格）
    //
    // ⚠ 边框**恒用 `input_border`**（不是强调色）：这三个模态只有一个输入框、天然聚焦，
    // 换成强调色会平白改掉既有观感；焦点强调色只给「导出小地图」那种多输入框模态。
    draw_field_frame(p, r.field, false);
    let caret_x = draw_field_text(p, r.field, path);
    draw_caret(p, r.field, caret_x);

    // 提示
    pt::text(
        p,
        pos2(inner_l, r.field.bottom() + 12.0),
        Align2::LEFT_CENTER,
        kind.hint(),
        m::BODY_FONT,
        pal.text_muted,
    );

    // 两个按钮：主按钮用强调色底 + `accent_button_text`，次按钮用输入框底
    pt::fill(p, r.ok, pal.accent);
    pt::border(p, r.ok, pal.accent_dark);
    pt::text(
        p,
        r.ok.center(),
        Align2::CENTER_CENTER,
        kind.ok_text(),
        m::BODY_FONT,
        pal.accent_button_text,
    );
    pt::fill(p, r.cancel, pal.input_bg);
    pt::border(p, r.cancel, pal.input_border);
    pt::text(
        p,
        r.cancel.center(),
        Align2::CENTER_CENTER,
        tx::OPEN_DLG_CANCEL,
        m::BODY_FONT,
        pal.text,
    );
}

/// 画打开地图模态。
pub fn draw_open_map(p: &Painter, l: &Layout, path: &str) {
    draw_path_dialog(p, l, path, Kind::Open);
}

/// 画另存为模态（与打开模态同一套排版，文案见 [`Kind::SaveAs`]）。
pub fn draw_save_as(p: &Painter, l: &Layout, path: &str) {
    draw_path_dialog(p, l, path, Kind::SaveAs);
}

/// 画「导出选区」模态（同一套排版，文案见 [`Kind::ExportSelection`]）。
pub fn draw_export_selection(p: &Painter, l: &Layout, path: &str) {
    draw_path_dialog(p, l, path, Kind::ExportSelection);
}

/// 把一条中文原因折成**最多两行**（按字符贪心，第二行放不下的尾巴用 `…` 收掉）。
///
/// 为什么要折：`valid_minimap_size` 最长那条原因约 70 字，640 宽的面板一行放不下；
/// 直接截断会把「超过什么上限」这半句吃掉，等于没告诉用户原因。
fn wrap_two(p: &Painter, text: &str, font: f32, max_w: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        let mut probe = cur.clone();
        probe.push(c);
        if !cur.is_empty() && pt::text_w(p, &probe, font) > max_w {
            lines.push(std::mem::take(&mut cur));
            if lines.len() >= 2 {
                lines[1].push('…');
                return lines;
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// 画「导出小地图」模态：路径框 + 三档预设 + 宽/高 + 错误提示（排版见 [`minimap_rects`]）。
///
/// 与 [`draw_path_dialog`] 的差别只有中间那三行；标题/提示/按钮的文字与配色同一套。
/// 尺寸非法时「导出」按钮按官方 `ExportButton.IsEnabled = false`（`MinimapExportWindow.cs:48`）
/// 画成灰底灰字 —— 与 `interact_export_minimap` 里「非法不提交」是同一个判据（[`MinimapDlg::is_valid`]）。
pub fn draw_export_minimap(
    p: &Painter,
    l: &Layout,
    path: &str,
    dlg: &MinimapDlg,
    cells: (u32, u32),
) {
    let pal = t::ui();
    let r = minimap_rects(l);
    pt::fill(p, l.content, pal.modal_scrim);
    pt::fill(p, r.base.panel, pal.panel_bg);
    pt::border(p, r.base.panel, pal.menu_popup_border);

    let inner_l = r.base.panel.left() + m::OPEN_DLG_PAD;
    // 标题
    pt::text(
        p,
        pos2(inner_l, r.base.panel.top() + m::OPEN_DLG_PAD + 12.0),
        Align2::LEFT_CENTER,
        Kind::ExportMinimap.title(),
        m::PANEL_TITLE_FONT,
        pal.text,
    );

    // ① 路径框（焦点在它上面才画光标 + 强调色边）
    let path_focus = dlg.focus == MiniFocus::Path;
    draw_field_frame(p, r.base.field, path_focus);
    let caret_x = draw_field_text(p, r.base.field, path);
    if path_focus {
        draw_caret(p, r.base.field, caret_x);
    }

    // ② 提示行
    pt::text(
        p,
        pos2(inner_l, r.base.field.bottom() + 12.0),
        Align2::LEFT_CENTER,
        Kind::ExportMinimap.hint(),
        m::BODY_FONT,
        pal.text_muted,
    );

    // ③ 预设行：左标签 + 三个按钮（选中项 = 强调色底，与主按钮同一套配色）
    pt::text(
        p,
        pos2(
            inner_l,
            r.presets[0].center().y,
        ),
        Align2::LEFT_CENTER,
        tx::EXPORT_MINI_DLG_PRESET_LABEL,
        m::BODY_FONT,
        pal.text,
    );
    for (i, preset) in MINIMAP_PRESETS.iter().enumerate() {
        let selected = dlg.preset == *preset;
        let (bg, border, fg) = if selected {
            (pal.accent, pal.accent_dark, pal.accent_button_text)
        } else {
            (pal.input_bg, pal.input_border, pal.text)
        };
        pt::fill(p, r.presets[i], bg);
        pt::border(p, r.presets[i], border);
        pt::text(
            p,
            r.presets[i].center(),
            Align2::CENTER_CENTER,
            preset_label(*preset),
            m::BODY_FONT,
            fg,
        );
    }

    // ④ 宽高行：`宽 [__] px   高 [__] px`（标签右对齐到框左边，单位左对齐到框右边）
    let width_focus = dlg.focus == MiniFocus::Width;
    pt::text(
        p,
        pos2(r.width.left() - m::MINIMAP_DLG_LABEL_GAP, r.width.center().y),
        Align2::RIGHT_CENTER,
        tx::EXPORT_MINI_DLG_WIDTH,
        m::BODY_FONT,
        pal.text,
    );
    draw_field_frame(p, r.width, width_focus);
    let caret_x = draw_field_text(p, r.width, &dlg.width_text);
    if width_focus {
        draw_caret(p, r.width, caret_x);
    }
    pt::text(
        p,
        pos2(r.width.right() + m::MINIMAP_DLG_LABEL_GAP, r.width.center().y),
        Align2::LEFT_CENTER,
        tx::EXPORT_MINI_DLG_PX,
        m::BODY_FONT,
        pal.text_muted,
    );

    let height_focus = dlg.focus == MiniFocus::Height;
    pt::text(
        p,
        pos2(r.height.left() - m::MINIMAP_DLG_LABEL_GAP, r.height.center().y),
        Align2::RIGHT_CENTER,
        tx::EXPORT_MINI_DLG_HEIGHT,
        m::BODY_FONT,
        pal.text,
    );
    draw_field_frame(p, r.height, height_focus);
    let caret_x = draw_field_text(p, r.height, &dlg.height_text);
    if height_focus {
        draw_caret(p, r.height, caret_x);
    }
    pt::text(
        p,
        pos2(r.height.right() + m::MINIMAP_DLG_LABEL_GAP, r.height.center().y),
        Align2::LEFT_CENTER,
        tx::EXPORT_MINI_DLG_PX,
        m::BODY_FONT,
        pal.text_muted,
    );

    // ⑤ 错误提示带（有错误才画；两行折行，尾巴用 `…`）
    if let Some(why) = dlg.error.as_deref() {
        pt::fill(p, r.error, pal.warn_banner_bg);
        pt::border(p, r.error, pal.warn_banner_edge);
        let lines = wrap_two(p, why, m::BODY_FONT, r.error.width() - 12.0);
        for (i, line) in lines.iter().enumerate() {
            pt::text(
                p,
                pos2(r.error.left() + 6.0, r.error.top() + 3.0 + 14.0 * i as f32),
                Align2::LEFT_TOP,
                line,
                m::BODY_FONT,
                pal.warn_banner_text,
            );
        }
    }

    // ⑥ 两个按钮（与通用模态同一套配色；尺寸非法时「导出」按官方 `IsEnabled=false` 画灰）
    let (ok_bg, ok_border, ok_fg) = if dlg.is_valid(cells) {
        (pal.accent, pal.accent_dark, pal.accent_button_text)
    } else {
        (pal.input_bg, pal.input_border, pal.text_muted)
    };
    pt::fill(p, r.base.ok, ok_bg);
    pt::border(p, r.base.ok, ok_border);
    pt::text(
        p,
        r.base.ok.center(),
        Align2::CENTER_CENTER,
        Kind::ExportMinimap.ok_text(),
        m::BODY_FONT,
        ok_fg,
    );
    pt::fill(p, r.base.cancel, pal.input_bg);
    pt::border(p, r.base.cancel, pal.input_border);
    pt::text(
        p,
        r.base.cancel.center(),
        Align2::CENTER_CENTER,
        tx::OPEN_DLG_CANCEL,
        m::BODY_FONT,
        pal.text,
    );
}

/// 路径输入模态的输入处理（**只在对应的浮层变体在场时调用**）。
///
/// 顺序有因果：先判 `Esc` / `Enter` / 按钮点击，再吃文本与退格 ——
/// 否则同一次按键既会被当成"确认"又会被写进路径。
fn interact_path_dialog(
    ui: &mut Ui,
    l: &Layout,
    overlay: &mut Overlay,
    kind: Kind,
) -> DialogAction {
    // 导出小地图还要尺寸 ⇒ 它的提交必须走 `interact_export_minimap`（见 `Kind::submit`）
    debug_assert_ne!(
        kind,
        Kind::ExportMinimap,
        "导出小地图模态不能走通用路径提交（它会丢掉尺寸）"
    );
    let Some(path) = path_mut(overlay, kind) else {
        return DialogAction::None;
    };
    let r = dialog_rects(l);

    // ① Esc / Enter
    let (esc, enter) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::Escape),
            i.key_pressed(egui::Key::Enter),
        )
    });
    if esc {
        return DialogAction::Cancel;
    }
    // ② 按钮点击（只认**真鼠标**，见 [`真点击`] —— 否则 `Tab` 后的回車会假点击「取消」）
    if 真点击(ui, r.cancel, ui.id().with((kind.id_prefix(), "cancel"))) {
        return DialogAction::Cancel;
    }
    if 真点击(ui, r.ok, ui.id().with((kind.id_prefix(), "ok"))) {
        return kind.submit(path);
    }
    // ③ 回车 = 确认（放在按钮之后：点按钮那次点击的同一帧也可能带 Enter）
    if enter {
        return kind.submit(path);
    }
    // ④ 文本与退格
    let (texts, backspace) = ui.input(|i| {
        let mut texts = Vec::new();
        for e in &i.events {
            if let egui::Event::Text(t) = e {
                texts.push(t.clone());
            }
        }
        (texts, i.key_pressed(egui::Key::Backspace))
    });
    for t in texts {
        // 只收可见 ASCII + 中文路径里可能出现的字符；换行/制表一律丢掉
        for c in t.chars() {
            if !c.is_control() {
                path.push(c);
            }
        }
    }
    if backspace {
        path.pop();
    }
    DialogAction::None
}

/// 处理打开地图模态的输入（**只在 `Overlay::OpenMap` 时调用**）。
pub fn interact_open_map(ui: &mut Ui, l: &Layout, overlay: &mut Overlay) -> DialogAction {
    interact_path_dialog(ui, l, overlay, Kind::Open)
}

/// 处理另存为模态的输入（**只在 `Overlay::SaveAs` 时调用**）。
pub fn interact_save_as(ui: &mut Ui, l: &Layout, overlay: &mut Overlay) -> DialogAction {
    interact_path_dialog(ui, l, overlay, Kind::SaveAs)
}

/// 处理「导出选区」模态的输入（**只在 `Overlay::ExportSelection` 时调用**）。
pub fn interact_export_selection(ui: &mut Ui, l: &Layout, overlay: &mut Overlay) -> DialogAction {
    interact_path_dialog(ui, l, overlay, Kind::ExportSelection)
}

/// 提交「导出小地图」：先走官方 `Validate` 的等价，**失败只把原因写进模态**（不调导出、不写盘）。
fn submit_minimap(path: &str, dlg: &mut MinimapDlg, cells: (u32, u32)) -> DialogAction {
    match dlg.resolve(cells) {
        Ok(size) => DialogAction::ExportMinimap {
            path: path.to_owned(),
            size,
        },
        Err(why) => {
            dlg.error = Some(why);
            DialogAction::None
        }
    }
}

/// 处理「导出小地图」模态的输入（**只在 `Overlay::ExportMinimap` 时调用**）。
///
/// `cells` = 当前地图的格数 `(宽, 高)`：选预设时按它算像素数（官方
/// `document.Width * 48 / divisor`）。它由调用方每帧传入（`app.rs` 从文档读），
/// 不往模态状态里塞快照 —— 免得「地图换了、模态里的数字还是旧图的」。
///
/// 键盘：`Esc` 取消、`Enter` 提交、`Tab`/`Shift+Tab` 换输入框、其余文本进当前焦点框。
pub fn interact_export_minimap(
    ui: &mut Ui,
    l: &Layout,
    overlay: &mut Overlay,
    cells: (u32, u32),
) -> DialogAction {
    let Overlay::ExportMinimap { path, size: dlg } = overlay else {
        return DialogAction::None;
    };
    let r = minimap_rects(l);

    // ① Esc = 取消
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return DialogAction::Cancel;
    }
    // ② 两个按钮（只认真鼠标点击，见 [`真点击`]）
    if 真点击(ui, r.base.cancel, ui.id().with(("export_mini_dlg", "cancel"))) {
        return DialogAction::Cancel;
    }
    if 真点击(ui, r.base.ok, ui.id().with(("export_mini_dlg", "ok"))) && dlg.is_valid(cells) {
        return submit_minimap(path, dlg, cells);
    }
    // ③ 三档预设：点一下就把该档像素数写进宽高框（官方 `MinimapExportWindow.cs:36-38`）
    for (i, preset) in MINIMAP_PRESETS.iter().enumerate() {
        if 真点击(
            ui,
            r.presets[i],
            ui.id().with(("export_mini_dlg", "preset", preset.key())),
        ) {
            dlg.select(*preset, cells);
            return DialogAction::None;
        }
    }
    // ④ 三个输入框：点一下换焦点（**不改内容**）
    for (focus, rect, tag) in [
        (MiniFocus::Path, r.base.field, "field_path"),
        (MiniFocus::Width, r.width, "field_width"),
        (MiniFocus::Height, r.height, "field_height"),
    ] {
        if 真点击(ui, rect, ui.id().with(("export_mini_dlg", tag))) {
            dlg.focus = focus;
            return DialogAction::None;
        }
    }
    // ⑤ 回车 = 提交（放在按钮之后：点按钮那次点击的同一帧也可能带 Enter）
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        return submit_minimap(path, dlg, cells);
    }
    // ⑥ Tab / Shift+Tab = 换焦点
    let (tab, back) = ui.input(|i| {
        let tab = i.key_pressed(egui::Key::Tab);
        (tab, tab && i.modifiers.shift)
    });
    if tab {
        dlg.focus = if back {
            dlg.focus.prev()
        } else {
            dlg.focus.next()
        };
        return DialogAction::None;
    }
    // ⑦ 文本与退格：只进**当前焦点**那个框（三个框共用一个缓冲出口 `focus_mut`）
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
                // 换行/制表一律丢掉（与路径模态同一口径）
                if !c.is_control() {
                    dlg.focus_mut(path).push(c);
                }
            }
        }
        if backspace {
            dlg.focus_mut(path).pop();
        }
        // 官方每敲一个字符就 `Validate()` 一次（`MinimapExportWindow.cs:31-32`）
        dlg.refresh(cells);
    }
    DialogAction::None
}

/// 画快捷键浮层（内容在 [`crate::menu::shortcuts`]）。
pub fn draw_shortcuts(p: &Painter, l: &Layout) {
    crate::menu::shortcuts::draw(p, l);
}

/// 快捷键浮层里「关闭」的可点区域（整个浮层外都可点关闭，这里给出浮层矩形供判据用）。
pub fn shortcuts_rect(l: &Layout) -> Rect {
    crate::menu::shortcuts::rect(l)
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

    /// 打开地图模态的几何自洽：整块在窗口内、输入框/按钮互不重叠、按钮在面板内。
    #[test]
    fn 打开模态几何自洽() {
        let l = 布局();
        let r = open_rects(&l);
        assert!(l.content.contains_rect(r.panel), "模态越出窗口");
        assert!(r.panel.contains_rect(r.field), "输入框越出面板");
        assert!(r.panel.contains_rect(r.ok), "打开按钮越出面板");
        assert!(r.panel.contains_rect(r.cancel), "取消按钮越出面板");
        assert!(!r.ok.intersects(r.cancel), "两个按钮重叠");
        assert!(!r.field.intersects(r.ok), "输入框压到按钮上");
        assert!(r.field.bottom() < r.ok.top(), "输入框在按钮之上");
    }

    /// **另存为模态与打开模态逐像素同几何**（同一套排版，只是文案不同）。
    ///
    /// 这条挡的是"另存为另写一套几何"——那样两个模态的输入框/按钮会一点点错位，
    /// 用户在两个几乎一样的框里点到不同的地方。
    #[test]
    fn 另存为模态与打开模态同几何() {
        let l = 布局();
        let a = open_rects(&l);
        let b = save_as_rects(&l);
        assert_eq!(a.panel, b.panel);
        assert_eq!(a.field, b.field);
        assert_eq!(a.ok, b.ok);
        assert_eq!(a.cancel, b.cancel);
        // 同一套判据也要过（另存为是同一个 `dialog_rects`，不许被特判放宽）
        assert!(l.content.contains_rect(b.panel));
        assert!(b.panel.contains_rect(b.field));
        assert!(!b.ok.intersects(b.cancel));
        assert!(b.field.bottom() < b.ok.top());
    }

    /// 两个模态的文案**各不相同**且都来自 `ui_text`（标题/提示/主按钮各一条）。
    #[test]
    fn 两个模态的文案各自独立() {
        assert_eq!(Kind::Open.title(), tx::OPEN_DLG_TITLE);
        assert_eq!(Kind::SaveAs.title(), tx::SAVE_AS_DLG_TITLE);
        assert_eq!(Kind::Open.hint(), tx::OPEN_DLG_HINT);
        assert_eq!(Kind::SaveAs.hint(), tx::SAVE_AS_DLG_HINT);
        assert_eq!(Kind::Open.ok_text(), tx::OPEN_DLG_OK);
        assert_eq!(Kind::SaveAs.ok_text(), tx::SAVE_AS_DLG_OK);
        // 标题与提示不许写成同一句（那会让用户分不清自己开的是哪个模态）
        assert_ne!(Kind::Open.title(), Kind::SaveAs.title());
        assert_ne!(Kind::Open.hint(), Kind::SaveAs.hint());
        // 另存为的提示必须点名"原文件不动"（这是它与「保存地图」的唯一差别）
        assert!(Kind::SaveAs.hint().contains("原文件不动"), "{}", Kind::SaveAs.hint());
        // 两个模态的 egui id 前缀必须不同（点击归属不许依赖绘制顺序）
        assert_ne!(Kind::Open.id_prefix(), Kind::SaveAs.id_prefix());
    }

    /// **提交动作按种类分开**：打开模态只会提交 `Open`，另存为只会提交 `SaveAs`。
    #[test]
    fn 提交动作按种类分开() {
        assert_eq!(
            Kind::Open.submit(r"I:\a.map"),
            DialogAction::Open(r"I:\a.map".to_owned())
        );
        assert_eq!(
            Kind::SaveAs.submit(r"I:\b.map"),
            DialogAction::SaveAs(r"I:\b.map".to_owned())
        );
        // 种类与浮层变体不匹配 ⇒ 什么都不做（不许把另存为的输入喂给打开动作）
        let mut ov = Overlay::OpenMap {
            path: "x".to_owned(),
        };
        assert!(path_mut(&mut ov, Kind::SaveAs).is_none());
        assert_eq!(path_mut(&mut ov, Kind::Open).map(|s| s.as_str()), Some("x"));
        let mut ov = Overlay::SaveAs {
            path: "y".to_owned(),
        };
        assert!(path_mut(&mut ov, Kind::Open).is_none());
        assert_eq!(path_mut(&mut ov, Kind::SaveAs).map(|s| s.as_str()), Some("y"));
        // 别的浮层（快捷键 / 无浮层）一律 `None`
        let mut ov = Overlay::Shortcuts;
        assert!(path_mut(&mut ov, Kind::Open).is_none());
        assert!(path_mut(&mut ov, Kind::SaveAs).is_none());
    }

    /// **四个模态的文案与 egui id 互不相同**（2026-11-05 加了两个导出模态）。
    ///
    /// 这条挡两件事：① 两个模态标题写成同一句 ⇒ 用户分不清自己开的是哪个；
    /// ② id 前缀撞了 ⇒ `interact` 的点击归属变成依赖绘制顺序（同一帧只有一个浮层，
    /// 但 id 撞了会在切换模态的那一帧串味）。
    #[test]
    fn 四个模态文案与id互不相同() {
        let kinds = [
            Kind::Open,
            Kind::SaveAs,
            Kind::ExportSelection,
            Kind::ExportMinimap,
        ];
        for i in 0..kinds.len() {
            assert!(!kinds[i].title().is_empty());
            assert!(!kinds[i].hint().is_empty());
            assert!(!kinds[i].ok_text().is_empty());
            for j in (i + 1)..kinds.len() {
                assert_ne!(kinds[i].title(), kinds[j].title(), "标题撞了：{:?}", kinds[i]);
                assert_ne!(kinds[i].hint(), kinds[j].hint(), "提示撞了：{:?}", kinds[i]);
                assert_ne!(
                    kinds[i].id_prefix(),
                    kinds[j].id_prefix(),
                    "id 前缀撞了：{:?}",
                    kinds[i]
                );
            }
        }
        // 两个导出的提示都要点名目标扩展名（它们是 PNG，不是 .map）
        assert!(Kind::ExportSelection.hint().contains(".png"));
        assert!(Kind::ExportMinimap.hint().contains(".png"));
        // 提交动作必须按种类分开（不许把导出的输入喂给另存为）
        assert_eq!(
            Kind::ExportSelection.submit(r"I:\a.png"),
            DialogAction::ExportSelection(r"I:\a.png".to_owned())
        );
        // ⚠ 导出小地图**不走**通用提交：它还要尺寸，统一在 `interact_export_minimap` 里折成
        // `DialogAction::ExportMinimap { path, size }`（见 `Kind::submit` 与 `debug_assert_ne!`）。
        assert_eq!(
            Kind::ExportMinimap.submit(r"I:\b.png"),
            DialogAction::None,
            "小地图模态不能走通用路径提交（会丢掉尺寸）"
        );
        let mut ov = Overlay::ExportSelection {
            path: "x.png".to_owned(),
        };
        assert!(path_mut(&mut ov, Kind::ExportMinimap).is_none());
        assert_eq!(path_mut(&mut ov, Kind::ExportSelection).map(|s| s.as_str()), Some("x.png"));
        let mut ov = Overlay::export_minimap("y.png".to_owned(), (4, 4));
        assert!(path_mut(&mut ov, Kind::ExportSelection).is_none());
        assert_eq!(path_mut(&mut ov, Kind::ExportMinimap).map(|s| s.as_str()), Some("y.png"));
    }

    /// 快捷键浮层也在窗口内，且**居中**。
    #[test]
    fn 快捷键浮层居中且在窗口内() {
        let l = 布局();
        let r = shortcuts_rect(&l);
        assert!(l.content.contains_rect(r));
        assert!((r.center().x - l.content.center().x).abs() < 0.01);
        assert!((r.center().y - l.content.center().y).abs() < 0.01);
    }

    // ══════════ 导出小地图的尺寸输入（2026-11-06，N F-R14-1） ══════════

    /// **默认预设 = Small**，且两个文本框 = `W*48/4 × H*32/4`
    /// （官方 `MinimapExportWindow.cs:21-22` 的默认项 + `:36-38` 的除数 `4`）。
    ///
    /// 同时钉住「默认 Small ≡ CLI 缺省 `MinimapSize::Default`」：两条口径算出的像素
    /// **逐位相同**（`W*48/4 == W*12`、`H*32/4 == H*8`，48/32 都能被 4 整除，无余数）。
    #[test]
    fn 默认预设是小档且宽高等于官方公式() {
        for cells in [(700u32, 700u32), (701, 699), (1, 1)] {
            let dlg = MinimapDlg::new(cells);
            assert_eq!(dlg.preset(), MinimapPreset::Small, "默认预设必须是 Small");
            let want_w = (u64::from(cells.0) * 48 / 4).to_string();
            let want_h = (u64::from(cells.1) * 32 / 4).to_string();
            assert_eq!(dlg.width_text(), want_w, "默认宽 ≠ W*48/4（{:?}）", cells);
            assert_eq!(dlg.height_text(), want_h, "默认高 ≠ H*32/4（{:?}）", cells);
            // 提交口径 = Preset(Small)，且与官方缺省 `Default` 逐像素相同
            let size = dlg.resolve(cells).expect("默认尺寸必须合法");
            assert_eq!(size, MinimapSize::Preset(MinimapPreset::Small));
            assert_eq!(
                size.resolve(cells.0, cells.1).unwrap(),
                MinimapSize::Default.resolve(cells.0, cells.1).unwrap(),
                "Small 预设必须与 CLI 缺省（W*12 × H*8）同尺寸（{:?}）",
                cells
            );
            assert!(dlg.error().is_none());
        }
    }

    /// **切换三档**：每个档位都把 `W*48/divisor × H*32/divisor` 写进宽高框，
    /// 除数只来自 [`MinimapPreset::divisor`]（这里用它复算，不另写倍率）。
    ///
    /// 用 100×100 格：三档都放得进本仓的 512 MB / 单边 32768 护栏，才能验证"切档即改数字"。
    /// 大图的三档行为单独见 [`大图的中大档会被本仓内存护栏拒在提交时`]。
    #[test]
    fn 切换三档预设重写宽高() {
        let cells = (100u32, 100u32);
        let mut dlg = MinimapDlg::new(cells);
        for preset in MINIMAP_PRESETS {
            dlg.select(preset, cells);
            assert_eq!(dlg.preset(), preset);
            let d = u64::from(preset.divisor());
            assert_eq!(dlg.width_text(), (100u64 * 48 / d).to_string(), "{preset:?} 宽不对");
            assert_eq!(dlg.height_text(), (100u64 * 32 / d).to_string(), "{preset:?} 高不对");
            assert_eq!(
                dlg.resolve(cells).unwrap(),
                MinimapSize::Preset(preset),
                "{preset:?} 提交口径不对"
            );
        }
        // 三档的除数就是官方 `{4,2,1}`（`MinimapExportWindow.cs:21`）
        assert_eq!(
            (
                MinimapPreset::Small.divisor(),
                MinimapPreset::Medium.divisor(),
                MinimapPreset::Large.divisor()
            ),
            (4, 2, 1)
        );
        // 100×100 格的具体数值：Small 1200×800 / Medium 2400×1600 / Large 4800×3200
        let mut dlg = MinimapDlg::new(cells);
        assert_eq!((dlg.width_text(), dlg.height_text()), ("1200", "800"));
        dlg.select(MinimapPreset::Medium, cells);
        assert_eq!((dlg.width_text(), dlg.height_text()), ("2400", "1600"));
        dlg.select(MinimapPreset::Large, cells);
        assert_eq!((dlg.width_text(), dlg.height_text()), ("4800", "3200"));
        dlg.select(MinimapPreset::Small, cells);
        assert_eq!((dlg.width_text(), dlg.height_text()), ("1200", "800"));
    }

    /// **大图的中/大档会在"点导出"时被本仓更严的内存护栏拒掉**（中文原因、不写盘）。
    ///
    /// 700×700 格：Medium = `16800×11200` = 752 MB > 本仓 `MAX_EXPORT_BYTES`（512 MiB），
    /// Large 更大。官方 `ValidSize` 只卡 `Array.MaxLength/4`（≈2 GiB），本仓这条**更严**且
    /// R4 已核、本单**不动数值口径** ⇒ 模态把实情说出来，而不是静默降档或硬写出打不开的图。
    #[test]
    fn 大图的中大档会被本仓内存护栏拒在提交时() {
        let cells = (700u32, 700u32);
        let mut dlg = MinimapDlg::new(cells);
        // Small 放得下（8400×5600 = 188 MB）
        assert_eq!(
            dlg.resolve(cells).unwrap(),
            MinimapSize::Preset(MinimapPreset::Small)
        );
        for preset in [MinimapPreset::Medium, MinimapPreset::Large] {
            dlg.select(preset, cells);
            let err = dlg.resolve(cells).expect_err("大图的中/大档必须被拒");
            assert!(err.contains("超过本仓"), "原因里要点出本仓上限：{err}");
            assert!(err.contains("没有写盘"), "原因里要点明没有写盘：{err}");
        }
    }

    /// **自定义宽高**：手改后提交成 `Explicit`；改回预设原值又回到 `Preset`
    /// （官方 `Validate` 只认文本框里的数字，预设只是"帮你填好"）。
    #[test]
    fn 自定义宽高合法时提交为显式尺寸() {
        let cells = (700u32, 700u32);
        let mut dlg = MinimapDlg::new(cells);
        dlg.focus = MiniFocus::Width;
        dlg.width_text = " 321 ".to_owned(); // 前后空白按官方 `NumberStyles.Integer` 忽略
        dlg.height_text = "123".to_owned();
        assert_eq!(
            dlg.resolve(cells).unwrap(),
            MinimapSize::Explicit { width: 321, height: 123 }
        );
        // 改回 Small 的原值 ⇒ 仍报 `Preset(Small)`（口径不因"手打过数字"而降级成 Explicit）
        dlg.width_text = "8400".to_owned();
        dlg.height_text = "5600".to_owned();
        assert_eq!(
            dlg.resolve(cells).unwrap(),
            MinimapSize::Preset(MinimapPreset::Small)
        );
    }

    /// **自定义宽高非法一律拒绝**（官方 `Validate` = `int.TryParse` ∧ `ValidSize`），
    /// 且原因里**写明没有写盘** —— 这就是"拒绝且不写文件"的用户可见判据。
    #[test]
    fn 自定义宽高非法时拒绝且不写盘() {
        let cells = (700u32, 700u32);
        let cases: [(&str, &str, &str); 6] = [
            ("", "5600", tx::EXPORT_MINI_DLG_WIDTH),           // 空
            ("0", "5600", tx::EXPORT_MINI_DLG_WIDTH),          // 0（ValidSize 要求 > 0）
            ("-5", "5600", tx::EXPORT_MINI_DLG_WIDTH),         // 负数（TryParse 过、ValidSize 拒）
            ("abc", "5600", tx::EXPORT_MINI_DLG_WIDTH),        // 非数字
            ("8400", "99999999999", tx::EXPORT_MINI_DLG_HEIGHT), // 超 i32（官方 out int）
            // 超 MINIMAP_MAX_PIXELS：两边都在 i32 内，但乘积越过 RGBA8 可分配上限
            ("2000000000", "2000000000", "Rust RGBA8 可分配上限"),
        ];
        for (w, h, want) in cases {
            let mut dlg = MinimapDlg::new(cells);
            dlg.width_text = w.to_owned();
            dlg.height_text = h.to_owned();
            let err = dlg.resolve(cells).expect_err(&format!("{w}×{h} 必须被拒"));
            assert!(err.contains(want), "{w}×{h} 的原因不对：{err}");
            assert!(
                err.contains("没有写盘"),
                "{w}×{h} 的原因必须点明没有写盘：{err}"
            );
        }
        // 两个文本框分别报"宽"/"高"（用户要知道是哪一个错了）
        let mut dlg = MinimapDlg::new(cells);
        dlg.height_text = "0".to_owned();
        let err = dlg.resolve(cells).unwrap_err();
        assert!(err.contains(tx::EXPORT_MINI_DLG_HEIGHT), "必须点名是「高」错了：{err}");
    }

    /// **提交闸门**：合法尺寸才产出 [`DialogAction::ExportMinimap`]（才会去写盘）；
    /// 非法尺寸只把中文原因留在模态里，返回 `None` ⇒ 调用方不会调导出、一个字节都不写。
    #[test]
    fn 提交闸门只在尺寸合法时放行() {
        let cells = (700u32, 700u32);
        // 非法：0 × 5600 ⇒ 不提交、原因留在模态里
        let mut dlg = MinimapDlg::new(cells);
        dlg.height_text = "0".to_owned();
        assert_eq!(
            submit_minimap(r"I:\x.png", &mut dlg, cells),
            DialogAction::None,
            "非法尺寸不许提交"
        );
        let err = dlg.error().expect("模态里必须留下原因").to_owned();
        assert!(err.contains("没有写盘"), "原因要点明没有写盘：{err}");
        // 合法：默认 Small ⇒ 提交 path + Preset(Small)，且错误被清掉
        let mut dlg = MinimapDlg::new(cells);
        assert_eq!(
            submit_minimap(r"I:\x.png", &mut dlg, cells),
            DialogAction::ExportMinimap {
                path: r"I:\x.png".to_owned(),
                size: MinimapSize::Preset(MinimapPreset::Small),
            }
        );
        assert!(dlg.error().is_none());
        // 合法自定义 ⇒ 提交 Explicit（模态里的错误不会被带上）
        let mut dlg = MinimapDlg::new(cells);
        dlg.width_text = "800".to_owned();
        dlg.height_text = "600".to_owned();
        assert_eq!(
            submit_minimap(r"I:\y.png", &mut dlg, cells),
            DialogAction::ExportMinimap {
                path: r"I:\y.png".to_owned(),
                size: MinimapSize::Explicit { width: 800, height: 600 },
            }
        );
    }

    /// **活校验 + 导出按钮可用性**（官方 `MinimapExportWindow.cs:31-32,44-51`：
    /// 每敲一个字符 `Validate()` 一次，`ExportButton.IsEnabled = valid`）。
    ///
    /// 这条挡的是"只有点了导出才知道错"那种迟钝 UX：文本框一变，错误与按钮态**当场**跟着变。
    #[test]
    fn 活校验与导出按钮可用性() {
        let cells = (100u32, 100u32);
        let mut dlg = MinimapDlg::new(cells);
        assert!(dlg.is_valid(cells), "默认 Small 必须可用");
        assert!(dlg.error().is_none());
        // 清空宽 ⇒ 当场非法（还没点导出）
        dlg.width_text.clear();
        dlg.refresh(cells);
        assert!(!dlg.is_valid(cells), "空宽必须当场判非法");
        assert!(
            dlg.error().unwrap().contains(tx::EXPORT_MINI_DLG_WIDTH),
            "原因要点名「宽」：{:?}",
            dlg.error()
        );
        // 补回合法数字 ⇒ 当场恢复
        dlg.width_text = "640".to_owned();
        dlg.refresh(cells);
        assert!(dlg.is_valid(cells));
        assert!(dlg.error().is_none());

        // 预设本身放不下（700×700 的 Medium = 752 MB）⇒ **选完预设当场就非法**
        let mut 大 = MinimapDlg::new((700, 700));
        assert!(大.is_valid((700, 700)), "Small（188 MB）放得下");
        大.select(MinimapPreset::Medium, (700, 700));
        assert!(!大.is_valid((700, 700)));
        assert!(大.error().is_some(), "选完预设必须当场报错");
        // 换回 Small ⇒ 当场恢复
        大.select(MinimapPreset::Small, (700, 700));
        assert!(大.is_valid((700, 700)));
        assert!(大.error().is_none());
    }
    /// 三档预设的显示名**与官方 `MinimapExportWindow.cs:21` 逐字相同**。
    #[test]
    fn 三档预设标签与官方文案一致() {
        assert_eq!(preset_label(MinimapPreset::Small), "Small (¼)");
        assert_eq!(preset_label(MinimapPreset::Medium), "Medium (½)");
        assert_eq!(preset_label(MinimapPreset::Large), "Large (1:1)");
        // 顺序 = 官方 ComboBox 顺序，且与 `MinimapPreset::divisor` 一一对应 {4,2,1}
        assert_eq!(MINIMAP_PRESETS.map(|p| p.divisor()), [4, 2, 1]);
    }

    /// **导出小地图模态的几何自洽**（多出的三行不许压到路径框/按钮上）。
    ///
    /// 判据：面板在窗口内、每个控件都在面板内、纵向严格递增且互不相交、按钮在错误带之下。
    #[test]
    fn 小地图模态几何自洽() {
        let l = 布局();
        let r = minimap_rects(&l);
        assert!(l.content.contains_rect(r.base.panel), "面板越出窗口");
        assert!(r.base.panel.contains_rect(r.base.field), "路径框越出面板");
        for (i, preset) in r.presets.iter().enumerate() {
            assert!(
                r.base.panel.contains_rect(*preset),
                "第 {i} 个预设按钮越出面板"
            );
        }
        assert!(r.base.panel.contains_rect(r.width), "宽输入框越出面板");
        assert!(r.base.panel.contains_rect(r.height), "高输入框越出面板");
        assert!(r.base.panel.contains_rect(r.error), "错误带越出面板");
        assert!(r.base.panel.contains_rect(r.base.ok), "导出按钮越出面板");
        assert!(r.base.panel.contains_rect(r.base.cancel), "取消按钮越出面板");
        // 纵向：路径框 → 预设行 → 宽高行 → 错误带 → 按钮（逐段递增且不相交）
        assert!(r.base.field.bottom() < r.presets[0].top(), "预设行压到路径框");
        assert!(r.presets[0].bottom() < r.width.top(), "宽高行压到预设行");
        assert_eq!(r.width.top(), r.height.top(), "宽高必须同一行");
        assert!(r.width.bottom() < r.error.top(), "错误带压到宽高行");
        assert!(r.error.bottom() < r.base.ok.top(), "按钮压到错误带");
        // 横向：三个预设互不相交、宽高框互不相交、按钮不撞
        for i in 1..r.presets.len() {
            assert!(!r.presets[i - 1].intersects(r.presets[i]), "预设按钮重叠");
            assert!(r.presets[i - 1].right() < r.presets[i].left(), "预设按钮顺序颠倒");
        }
        assert!(r.width.right() < r.height.left(), "宽高输入框重叠");
        assert!(!r.base.ok.intersects(r.base.cancel), "两个按钮重叠");
        // 三个模态的**横向几何**必须与通用模态一模一样（同一份 `dialog_rects_tall`）。
        //
        // 纵向不比较：面板更高且居中 ⇒ 内容整体上移、按钮贴底 ⇒ y 本来就该不同。
        let common = dialog_rects(&l);
        assert_eq!(r.base.field.left(), common.field.left(), "路径框左边界必须一致");
        assert_eq!(r.base.field.width(), common.field.width(), "路径框宽度必须一致");
        assert_eq!(r.base.field.height(), common.field.height());
        for (a, b) in [
            (r.base.ok, common.ok),
            (r.base.cancel, common.cancel),
        ] {
            assert_eq!(a.left(), b.left(), "按钮左边界必须一致");
            assert_eq!(a.width(), b.width(), "按钮宽度必须一致");
            assert_eq!(a.height(), b.height(), "按钮高度必须一致");
            // 贴底规则也一样：按钮底距面板底 = OPEN_DLG_PAD
            assert!((m::OPEN_DLG_PAD - (r.base.panel.bottom() - a.bottom())).abs() < 0.01);
        }
        // 面板更高，但居中 ⇒ 中心点不变
        assert!((r.base.panel.center() - common.panel.center()).length() < 0.01);
        assert!((r.base.panel.height() - m::MINIMAP_DLG_H).abs() < 0.01);
        assert!(r.base.panel.height() > common.panel.height());
    }

    /// **整个模态能真的画出来**（含错误带）：不 panic、两行折行的每行都放得进错误带。
    ///
    /// 这条补的是"只有截图才能发现"的那类缺陷（文字冲出面板 / 折行算错），
    /// 用与运行时同一份中文字体做纯 CPU 排版来测，不需要开窗口。
    #[test]
    fn 小地图模态绘制不越界() {
        let ctx = crate::fonts::measure_ctx();
        let l = 布局();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(m::WINDOW_W, m::WINDOW_H));
        let p = egui::Painter::new(ctx, egui::LayerId::debug(), screen);
        let r = minimap_rects(&l);

        // ① 没有错误：照常画（错误带留白）
        let dlg = MinimapDlg::new((13, 16));
        draw_export_minimap(&p, &l, r"I:\29_小地图.png", &dlg, (13, 16));

        // ② 有错误：最长那条原因（超 MINIMAP_MAX_PIXELS）也要折进错误带
        let mut dlg = MinimapDlg::new((700, 700));
        dlg.width_text = "2000000000".to_owned();
        dlg.height_text = "2000000000".to_owned();
        dlg.refresh((700, 700));
        assert!(dlg.error().is_some(), "活校验必须当场报错");
        draw_export_minimap(&p, &l, r"I:\29_小地图.png", &dlg, (700, 700));

        let max_w = r.error.width() - 12.0;
        let lines = wrap_two(&p, dlg.error().unwrap(), m::BODY_FONT, max_w);
        assert!(!lines.is_empty() && lines.len() <= 2, "最多两行：{lines:?}");
        for line in &lines {
            assert!(
                pt::text_w(&p, line, m::BODY_FONT) <= max_w + 0.5,
                "一行超出错误带内宽 {max_w}：{line}"
            );
        }
        // 放不下的尾巴必须显式省略，不许悄悄吞掉：要么两行装完，要么第二行以 `…` 收尾
        if lines.len() == 2 {
            let joined: usize = lines.iter().map(|s| s.chars().count()).sum();
            let total = dlg.error().unwrap().chars().count();
            if joined < total {
                assert!(
                    lines[1].ends_with('…'),
                    "两行没装完就必须以 … 收尾（装 {joined}/{total} 字）：{lines:?}"
                );
            }
        }
    }

    /// 合成输入的**单调时钟**（egui 要求帧时间不许倒退，否则 panic）。
    struct 时钟(f64);

    impl 时钟 {
        fn 下一帧(&mut self) -> f64 {
            self.0 += 1.0 / 60.0;
            self.0
        }
    }

    /// 一帧原始输入（全屏 + 给定事件 + 给定时间）。
    fn 帧(t: f64, events: Vec<egui::Event>) -> egui::RawInput {
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

    /// 在当前浮层上跑一帧对应模态的输入处理，返回它给出的动作。
    fn 跑帧(
        ctx: &egui::Context,
        l: &Layout,
        overlay: &mut Overlay,
        input: egui::RawInput,
    ) -> DialogAction {
        let mut got = DialogAction::None;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    got = match overlay {
                        Overlay::ExportMinimap { .. } => {
                            interact_export_minimap(ui, l, overlay, (100, 100))
                        }
                        Overlay::OpenMap { .. } => interact_open_map(ui, l, overlay),
                        Overlay::SaveAs { .. } => interact_save_as(ui, l, overlay),
                        Overlay::ExportSelection { .. } => interact_export_selection(ui, l, overlay),
                        _ => DialogAction::None,
                    };
                });
        });
        got
    }

    /// 敲一串事件（一帧），返回这一帧的动作。
    fn 敲(
        ctx: &egui::Context,
        l: &Layout,
        overlay: &mut Overlay,
        clock: &mut 时钟,
        events: Vec<egui::Event>,
    ) -> DialogAction {
        let input = 帧(clock.下一帧(), events);
        跑帧(ctx, l, overlay, input)
    }

    /// 一串 `Event::Text`（每个字符一条，与真打字一致）。
    fn 文本事件(s: &str) -> Vec<egui::Event> {
        s.chars()
            .map(|c| egui::Event::Text(c.to_string()))
            .collect()
    }

    /// `n` 条退格键事件（同帧多条 = 产品实现只吃一次，改用 [`连退格`]）。
    #[allow(dead_code)]
    fn 退格事件(n: usize) -> Vec<egui::Event> {
        (0..n).map(|_| 按键事件(egui::Key::Backspace)).collect()
    }

    /// 连续退格 `n` 格：**一帧一次**（与产品实现同口径 —— 一帧只吃一次退格）。
    fn 连退格(
        ctx: &egui::Context,
        l: &Layout,
        overlay: &mut Overlay,
        clock: &mut 时钟,
        n: usize,
    ) -> DialogAction {
        let mut act = DialogAction::None;
        for _ in 0..n {
            act = 敲(ctx, l, overlay, clock, vec![按键事件(egui::Key::Backspace)]);
            if act != DialogAction::None {
                return act;
            }
        }
        act
    }

    /// 一条"按下"的键盘事件。
    fn 按键事件(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    /// 模态里的焦点（测试用只读出口：`MiniFocus` 是私有字段，用行为间接断言也行，
    /// 但直接读更清楚）。
    fn 焦点(ov: &Overlay) -> MiniFocus {
        match ov {
            Overlay::ExportMinimap { size, .. } => size.focus,
            _ => panic!("不是导出小地图模态"),
        }
    }

    /// 宽/高框原文。
    fn 宽高(ov: &Overlay) -> (String, String) {
        match ov {
            Overlay::ExportMinimap { size, .. } => {
                (size.width_text().to_owned(), size.height_text().to_owned())
            }
            _ => panic!("不是导出小地图模态"),
        }
    }

    /// **真跑一遍键盘交互**（合成 egui 事件，不开窗）：Tab 换焦点 → 退格 → 打字 → 回车提交。
    ///
    /// 这条补的是"只有真按键才暴露"的那类缺陷：Tab 没换焦点、打字进了别的框、
    /// 回车不提交或提交时丢掉尺寸。走 `ctx.run(RawInput{events})` + `CentralPanel` 里的真 `Ui`。
    ///
    /// ⚠ **只测键盘**：egui 0.33 的 `Ui::interact().clicked()` 依赖**上一帧**的 widget 命中表
    /// （`context.rs:1384` 的 `viewport.interact_widgets.clicked`），在单测宿主的合成上下文里
    /// 复现不出来（实测 hovered 为真、clicked 恒假）；鼠标点击由开窗实测覆盖（见任务回执）。
    #[test]
    fn 键盘交互_切焦打字回车提交() {
        let ctx = crate::fonts::measure_ctx();
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let mut 钟 = 时钟(0.0);
        // 100×100 格 ⇒ 默认 Small = 1200 × 800，焦点在路径框
        let mut ov = Overlay::export_minimap("x.png".to_owned(), (100, 100));
        assert_eq!(焦点(&ov), MiniFocus::Path, "默认焦点必须能继续敲路径");
        assert_eq!(宽高(&ov), ("1200".to_owned(), "800".to_owned()));

        // ① Tab ⇒ 宽框
        assert_eq!(
            敲(&ctx, &l, &mut ov, &mut 钟, vec![按键事件(egui::Key::Tab)]),
            DialogAction::None
        );
        assert_eq!(焦点(&ov), MiniFocus::Width);
        // ② 退格 4 次 + 敲 "900" ⇒ 只动宽框
        assert_eq!(
            连退格(&ctx, &l, &mut ov, &mut 钟, 4),
            DialogAction::None
        );
        assert_eq!(
            敲(&ctx, &l, &mut ov, &mut 钟, 文本事件("900")),
            DialogAction::None
        );
        assert_eq!(宽高(&ov), ("900".to_owned(), "800".to_owned()));
        match &ov {
            Overlay::ExportMinimap { path, .. } => assert_eq!(path, "x.png", "焦点不在路径框，路径不许动"),
            _ => unreachable!(),
        }
        // ③ Tab ⇒ 高框；退格 3 次 + 敲 "600"
        assert_eq!(
            敲(&ctx, &l, &mut ov, &mut 钟, vec![按键事件(egui::Key::Tab)]),
            DialogAction::None
        );
        assert_eq!(焦点(&ov), MiniFocus::Height);
        assert_eq!(
            连退格(&ctx, &l, &mut ov, &mut 钟, 3),
            DialogAction::None
        );
        assert_eq!(
            敲(&ctx, &l, &mut ov, &mut 钟, 文本事件("600")),
            DialogAction::None
        );
        // ④ 回车 ⇒ 提交 path + Explicit（手改过就不报预设）
        assert_eq!(
            敲(&ctx, &l, &mut ov, &mut 钟, vec![按键事件(egui::Key::Enter)]),
            DialogAction::ExportMinimap {
                path: "x.png".to_owned(),
                size: MinimapSize::Explicit {
                    width: 900,
                    height: 600
                },
            }
        );
    }

    /// **键盘 `Tab` 之后的回车必须还是"确认"**（不许被 egui 的假点击打成「取消」）。
    ///
    /// 这条钉的是一个**既有缺陷**（2026-11-06 实测，修在 [`真点击`] 里）：egui 会把 `Enter`
    /// 折成假点击打到「当前拿到键盘焦点」的可点控件上（`egui-0.33.3/src/context.rs:1335-1342`
    /// 的 `FAKE_PRIMARY_CLICKED`），而路径模态里第一个可点控件正是「取消」⇒
    /// 「`Tab` 一下再回车」会返回 `Cancel`（实测 `interact_open_map` 就是这样，且换个焦点
    /// 状态又会变成确认 —— 全看 egui 上一帧把焦点给了谁）。三个路径模态各测一遍。
    #[test]
    fn 路径模态回车不被假点击打成取消() {
        let ctx = crate::fonts::measure_ctx();
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let mut 钟 = 时钟(0.0);
        let cases: [(Overlay, DialogAction); 3] = [
            (
                Overlay::OpenMap {
                    path: r"I:\a.map".to_owned(),
                },
                DialogAction::Open(r"I:\a.map".to_owned()),
            ),
            (
                Overlay::SaveAs {
                    path: r"I:\b.map".to_owned(),
                },
                DialogAction::SaveAs(r"I:\b.map".to_owned()),
            ),
            (
                Overlay::ExportSelection {
                    path: r"I:\c.png".to_owned(),
                },
                DialogAction::ExportSelection(r"I:\c.png".to_owned()),
            ),
        ];
        for (mut ov, want) in cases {
            // Tab（egui 自己的焦点也会跟着动）→ 回车：必须还是「确认」
            assert_eq!(
                敲(&ctx, &l, &mut ov, &mut 钟, vec![按键事件(egui::Key::Tab)]),
                DialogAction::None
            );
            assert_eq!(
                敲(&ctx, &l, &mut ov, &mut 钟, vec![按键事件(egui::Key::Enter)]),
                want,
                "Tab 后回车必须提交，不许变成取消"
            );
            // Esc 仍然必须取消（别把真取消也一起挡掉了）
            assert_eq!(
                敲(&ctx, &l, &mut ov, &mut 钟, vec![按键事件(egui::Key::Escape)]),
                DialogAction::Cancel
            );
        }
    }

    /// **非法尺寸回车**：不提交（`DialogAction::None`）、原因落在模态里供下一帧画出来；
    /// 再敲一个数字就把错误清掉。
    #[test]
    fn 键盘交互_非法尺寸回车不提交() {
        let ctx = crate::fonts::measure_ctx();
        let l = Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let mut 钟 = 时钟(0.0);
        let mut ov = Overlay::export_minimap("x.png".to_owned(), (100, 100));
        // Tab 到宽框 → 清空 → 回车（宽为空 ⇒ TryParse 失败）
        assert_eq!(
            敲(&ctx, &l, &mut ov, &mut 钟, vec![按键事件(egui::Key::Tab)]),
            DialogAction::None
        );
        assert_eq!(
            连退格(&ctx, &l, &mut ov, &mut 钟, 4),
            DialogAction::None
        );
        assert_eq!(
            敲(&ctx, &l, &mut ov, &mut 钟, vec![按键事件(egui::Key::Enter)]),
            DialogAction::None,
            "宽为空时不许提交"
        );
        match &ov {
            Overlay::ExportMinimap { size, .. } => {
                let err = size.error().expect("原因必须留给模态画出来");
                assert!(err.contains(tx::EXPORT_MINI_DLG_WIDTH), "要点名是「宽」：{err}");
                assert!(err.contains("没有写盘"), "要点明没有写盘：{err}");
            }
            _ => unreachable!(),
        }
        // 再敲数字 ⇒ 错误自动清掉
        assert_eq!(
            敲(&ctx, &l, &mut ov, &mut 钟, 文本事件("12")),
            DialogAction::None
        );
        match &ov {
            Overlay::ExportMinimap { size, .. } => assert!(size.error().is_none()),
            _ => unreachable!(),
        }
        // Esc ⇒ 取消（模态该关）
        assert_eq!(
            敲(&ctx, &l, &mut ov, &mut 钟, vec![按键事件(egui::Key::Escape)]),
            DialogAction::Cancel
        );
    }

    /// 焦点循环：`Tab` 路径 → 宽 → 高 → 路径；`Shift+Tab` 反向（三个框都要能进）。
    #[test]
    fn 焦点循环覆盖三个输入框() {
        let mut f = MiniFocus::Path;
        let mut seen = vec![f];
        for _ in 0..2 {
            f = f.next();
            seen.push(f);
        }
        assert_eq!(seen, vec![MiniFocus::Path, MiniFocus::Width, MiniFocus::Height]);
        assert_eq!(f.next(), MiniFocus::Path, "Tab 必须回到路径框");
        assert_eq!(MiniFocus::Path.prev(), MiniFocus::Height);
        assert_eq!(MiniFocus::Height.prev(), MiniFocus::Width);
        // 文本只进当前焦点框
        let cells = (4u32, 4u32);
        let mut dlg = MinimapDlg::new(cells);
        let mut path = "a.png".to_owned();
        dlg.focus = MiniFocus::Width;
        dlg.width_text.clear();
        dlg.focus_mut(&mut path).push_str("48");
        assert_eq!((dlg.width_text(), path.as_str()), ("48", "a.png"));
        dlg.focus_mut(&mut path).pop();
        assert_eq!(dlg.width_text(), "4");
        assert_eq!(path, "a.png", "焦点不在路径框时不许改路径");
    }
}
