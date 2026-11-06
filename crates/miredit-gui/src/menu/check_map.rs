//! 工具 → **检查地图**（A-015 / R32-R33）：几何 + 绘制 + 输入 + `--check-map` 自动化。
//!
//! 上游：`CrystalStudio/MapCheck.cs:1-101`（三项检查）+ `MainWindow.MapCheck.cs:1-58`
//! （760×480 对话框：summary / ListBox / Locate / Close / 陈旧提示）+ `MainWindow.axaml`
//! 的 Tools 组（`CheckMap` 夹在生成器组与 `Identify` 之间，带分隔线）。
//!
//! # 本模块只做「界面那一半」
//!
//! 检查逻辑全在 `miredit_core::check_map`（只读 `&RawMap` + 注入探针），本模块负责：
//! 1. **探针适配**：[`WilProbe`] 把 `miredit_core::MapLibRef`（族 + **族内页号**）翻成
//!    `mir2_assets::LibRef`（同口径，恒等换名），并把 **1 基图号 → 0 基帧号**；
//! 2. **五态映射**：[`态映射`] 把 `mir2_assets::ImageState` 映到 `miredit_core::ImageState`。
//!    ⚠ 不能用 `impl From`：两个类型对 `miredit-gui` 都是**外来类型**（孤儿规则不许），
//!    而且 `miredit-core` 按依赖白名单**不许**依赖 `mir2-assets` ⇒ 只能在宿主侧显式映射。
//!    `match` 不写通配臂 ⇒ 任一侧加/改一个变体都会**编译失败**（这就是防漂移的钉子）；
//! 3. **模态**：summary 两处（总数/显示数 + 未检查/无图库/已陈旧）、滚动列表、行选中、
//!    「定位」= 画布居中 + 1×1 选区 + 关模态、「关闭」= 关模态；行数上限 = core 的 1000。
//!
//! # 三处**有意差异**（都记在案，不静默）
//!
//! | 项 | 上游 | 本仓 |
//! |---|---|---|
//! | 列表控件 | Avalonia `ListBox`（自带滚动条） | 自绘：行高 [`m::CHECK_MAP_ROW_H`] + 滚轮 + 可见行窗口，**不画滚动条**（与面板其它列表同口径） |
//! | 陈旧判定 | `PickupRevision` + 文档引用相等 | `map_gen` 快照（每次真改图 +1，见 `app::touch_map`） |
//! | 千分位 | `{0:N0}` ⇒ `1,234` | 纯整数（与状态栏/悬停浮层同一套数字口径） |
//!
//! # 为什么不做「后台线程」
//!
//! 上游用 `Task.Run` + `CancellationToken` 是因为它有 1 万格以上的真图；本仓实测
//! （`acceptance` 里的 `0.map` 用例）整张图跑完在毫秒级，且探针只读帧头 ⇒ 同步跑更简单，
//! 也避免「检查跑完时文档已经被改」这类竞态（见 R32 的「性能」一行）。

use egui::{pos2, vec2, Align2, Painter, Rect, Ui};

use miredit_core::{LibFamily, MapCheckReport};

use crate::layout::Layout;
use crate::menu::Overlay;
use crate::metrics as m;
use crate::paint as pt;
use crate::theme as t;
use crate::ui_text as tx;

// ───────────────────────────── 探针适配 ─────────────────────────────

/// `mir2_assets::ImageState` → `miredit_core::ImageState`（五态一一对应）。
///
/// 见模块文档第 2 条：为什么不是 `impl From`，以及 `match` 为什么**不写通配臂**。
#[must_use]
pub fn 态映射(态: mir2_assets::ImageState) -> miredit_core::ImageState {
    use mir2_assets::ImageState as 资产;
    use miredit_core::ImageState as 内核;
    match 态 {
        资产::Ok => 内核::Ok,
        资产::Empty => 内核::Empty,
        资产::Unreadable => 内核::Unreadable,
        资产::OutOfRange => 内核::OutOfRange,
        资产::MissingLibrary => 内核::MissingLibrary,
    }
}

/// `MapLibRef`（core：族 + 族内页号）→ `mir2_assets::LibRef`（图源：同口径）。
///
/// **恒等换名，不做任何加减**：两边的 `index` 都是「族内 0 基页号」
/// （`lib_paths.rs:103-113`：`0` = 基名本身、`N` = `{Stem}{N+1}`）。
/// ⚠ C# 的扁平全局号（`area + 2` / `+120` / `+200` …）**只用于对照**，
/// 拿它去查 `LibRef` 会去查错的库（例如 `Objects2`）。
#[must_use]
pub fn 资产定位键(r: miredit_core::MapLibRef) -> mir2_assets::LibRef {
    match r.family {
        LibFamily::Ground => mir2_assets::LibRef::ground(r.index),
        LibFamily::Middle => mir2_assets::LibRef::middle(r.index),
        LibFamily::Objects => mir2_assets::LibRef::surface(r.index),
    }
}

/// GUI 侧的图库探针：包一层 [`mir2_assets::SpriteSource`]（`None` = 没有图库表）。
///
/// 图号是 **1 基**（地图格字段口径）⇒ 转成图源的 0 基帧号；`0` 由
/// `miredit_core::check_map` 先跳过，这里再兜一次（防御性，不改语义）。
pub struct WilProbe<'a> {
    /// 图源（`None` ⇒ 一律 `MissingLibrary`；调用方其实该改传 `NoLibProbe`）。
    pub src: Option<&'a dyn mir2_assets::SpriteSource>,
}

impl WilProbe<'_> {
    /// 从 `Assets::source()` 的结果建一个探针。
    #[must_use]
    pub fn new(src: Option<&dyn mir2_assets::SpriteSource>) -> WilProbe<'_> {
        WilProbe { src }
    }
}

impl miredit_core::LibProbe for WilProbe<'_> {
    /// 有没有图库表 = 有没有图源（与 `check_map` 的 `libraries == null` 语义一致）。
    fn has_libraries(&self) -> bool {
        self.src.is_some()
    }

    fn frame_state(
        &self,
        lib: miredit_core::MapLibRef,
        image_1based: u32,
    ) -> miredit_core::ImageState {
        let Some(src) = self.src else {
            return miredit_core::ImageState::MissingLibrary;
        };
        if image_1based == 0 {
            // 上游 `if (image == 0 || libraries == null) return;`
            return miredit_core::ImageState::Ok;
        }
        态映射(src.frame_state(资产定位键(lib), image_1based - 1))
    }
}

// ───────────────────────────── 几何 ─────────────────────────────

/// 模态里所有可画/可点矩形（绘制与命中测试共用同一份几何）。
#[derive(Clone, Copy, Debug)]
pub struct Rects {
    /// 面板（官方 760×480，居中）。
    pub panel: Rect,
    /// 标题行。
    pub title: Rect,
    /// 汇总区（最多 4 行：汇总 + 3 条「未检查」/「无图库」提示）。
    pub summary: Rect,
    /// 列表（可滚动；行高 [`m::CHECK_MAP_ROW_H`]）。
    pub list: Rect,
    /// 「定位」按钮。
    pub locate: Rect,
    /// 「关闭」按钮。
    pub close: Rect,
}

/// 算模态几何（居中；按钮贴底，与其它模态同一规则）。
#[must_use]
pub fn rects(l: &Layout) -> Rects {
    let panel = Rect::from_center_size(l.content.center(), vec2(m::CHECK_MAP_W, m::CHECK_MAP_H));
    let inner_l = panel.left() + m::CHECK_MAP_PAD;
    let inner_r = panel.right() - m::CHECK_MAP_PAD;
    let 宽 = inner_r - inner_l;
    let title = Rect::from_min_size(
        pos2(inner_l, panel.top() + m::CHECK_MAP_PAD),
        vec2(宽, m::CHECK_MAP_TITLE_H),
    );
    let summary = Rect::from_min_size(
        pos2(inner_l, title.bottom() + m::CHECK_MAP_TITLE_GAP),
        vec2(宽, m::CHECK_MAP_SUMMARY_H),
    );
    let btn_y = panel.bottom() - m::CHECK_MAP_PAD - m::OPEN_DLG_BTN_H;
    let close = Rect::from_min_size(
        pos2(inner_r - m::OPEN_DLG_BTN_W, btn_y),
        vec2(m::OPEN_DLG_BTN_W, m::OPEN_DLG_BTN_H),
    );
    let locate = Rect::from_min_size(
        pos2(close.left() - 10.0 - m::OPEN_DLG_BTN_W, btn_y),
        vec2(m::OPEN_DLG_BTN_W, m::OPEN_DLG_BTN_H),
    );
    let list = Rect::from_min_max(
        pos2(inner_l, summary.bottom() + 6.0),
        pos2(inner_r, btn_y - m::CHECK_MAP_LIST_GAP),
    );
    Rects {
        panel,
        title,
        summary,
        list,
        locate,
        close,
    }
}

/// 列表一次能显示多少行（至少 1 行；`0` 高的列表按 1 行算，免得除零/空窗口）。
#[must_use]
pub fn 可见行数(list: Rect) -> usize {
    let n = (list.height() / m::CHECK_MAP_ROW_H).floor();
    if n.is_finite() && n >= 1.0 {
        n as usize
    } else {
        1
    }
}

// ───────────────────────────── 文案 ─────────────────────────────

/// 汇总行（官方 `"{total} findings; showing {shown}. …"`）。
///
/// `stale` ⇒ 换整句「地图已变化，请重新检查。」（官方 `MainWindow.MapCheck.cs:43`）。
#[must_use]
pub fn summary_text(report: &MapCheckReport, stale: bool) -> String {
    if stale {
        return tx::CHECK_MAP_STALE.to_owned();
    }
    tx::fill(
        tx::CHECK_MAP_SUMMARY_TMPL,
        &[
            ("total", &report.total.to_string()),
            ("shown", &report.issues.len().to_string()),
        ],
    )
}

/// 汇总区下面那几行提示（最多 3 行）：
/// 1. 每一族「未检查」各一行（R33 要求按层如实标出，含原因）；
/// 2. 没有图库时追加官方那句「加载图库后才能检查图号引用。」。
#[must_use]
pub fn hint_lines(report: &MapCheckReport, has_libs: bool) -> Vec<String> {
    let mut out: Vec<String> = report
        .未检查
        .iter()
        .map(|(family, why)| {
            tx::fill(
                tx::CHECK_MAP_UNCHECKED_TMPL,
                &[(
                    "layers",
                    &tx::fill(
                        tx::CHECK_MAP_UNCHECKED_LAYER_TMPL,
                        &[("layer", family.label()), ("why", why)],
                    ),
                )],
            )
        })
        .collect();
    if !has_libs {
        out.push(tx::CHECK_MAP_NO_LIBS.to_owned());
    }
    out
}

// ───────────────────────────── 绘制 ─────────────────────────────

/// 画模态（遮罩 + 面板 + 标题 + 汇总/提示 + 列表 + 两个按钮）。
///
/// `stale` = 地图已变（`map_gen` 与检查时不同）⇒ summary 换成「请重新检查」，
/// **列表不画**（上游在那种情况下根本没往 `ListBox` 里填东西）。
#[allow(clippy::too_many_arguments)] // 6 个都是"这一帧的输入"，与 `menu::popup::draw` 同口径
pub fn draw(
    p: &Painter,
    l: &Layout,
    report: &MapCheckReport,
    selected: Option<usize>,
    scroll: usize,
    stale: bool,
    has_libs: bool,
) {
    let r = rects(l);
    let pal = t::ui();
    // 遮罩：压住整个窗口（模态，底下的东西不该被误点）
    pt::fill(p, l.content, pal.modal_scrim);
    pt::fill(p, r.panel, pal.panel_bg);
    pt::border(p, r.panel, pal.separator);

    // ① 标题
    pt::text(
        p,
        pos2(r.title.left(), r.title.center().y),
        Align2::LEFT_CENTER,
        tx::CHECK_MAP_TITLE,
        m::TITLE_FONT,
        pal.text,
    );

    // ② 汇总 + 提示（逐行；行高 = 汇总区高 / 4，最多 4 行）
    let 行高 = m::CHECK_MAP_SUMMARY_H / 4.0;
    let mut lines = vec![summary_text(report, stale)];
    if !stale {
        lines.extend(hint_lines(report, has_libs));
    }
    let 警示 = stale || !report.未检查.is_empty() || !has_libs;
    for (i, line) in lines.iter().take(4).enumerate() {
        pt::text(
            p,
            pos2(r.summary.left(), r.summary.top() + 行高 * (i as f32 + 0.5)),
            Align2::LEFT_CENTER,
            line,
            m::BODY_FONT,
            if 警示 { pal.warn_banner_text } else { pal.text_muted },
        );
    }

    // ③ 列表（裁剪到列表矩形，行窗口从 `scroll` 开始）
    let clip = p.with_clip_rect(r.list);
    if stale {
        // 地图变了：上游不填列表 ⇒ 这里也一格不画（不是"空列表占位"，是"没有结果"）
    } else if report.issues.is_empty() {
        pt::text(
            &clip,
            pos2(r.list.left() + 4.0, r.list.top() + m::CHECK_MAP_ROW_H * 0.5),
            Align2::LEFT_CENTER,
            tx::CHECK_MAP_EMPTY,
            m::BODY_FONT,
            pal.text_muted,
        );
    } else {
        let n = 可见行数(r.list);
        for (i, issue) in report.issues.iter().skip(scroll).take(n).enumerate() {
            let idx = scroll + i;
            let top = r.list.top() + i as f32 * m::CHECK_MAP_ROW_H;
            let row = Rect::from_min_size(
                pos2(r.list.left(), top),
                vec2(r.list.width(), m::CHECK_MAP_ROW_H),
            );
            if Some(idx) == selected {
                pt::fill(&clip, row, pal.menu_popup_hover);
            }
            pt::text(
                &clip,
                pos2(row.left() + 4.0, row.center().y),
                Align2::LEFT_CENTER,
                &issue.line(),
                m::BODY_FONT,
                if Some(idx) == selected { pal.text } else { pal.text_muted },
            );
        }
    }

    // ④ 两个按钮（「定位」在没选中 / 已陈旧时画灰 —— 官方 `IsEnabled` 同义）
    //    ⚠ 按钮在列表矩形**之外**，必须用未裁剪的 `p` 画（用 `clip` 会被整块裁掉）
    let 可定位 = selected.is_some_and(|i| !stale && i < report.issues.len());
    pt::fill(p, r.locate, if 可定位 { pal.accent } else { pal.input_bg });
    pt::border(
        p,
        r.locate,
        if 可定位 { pal.accent_dark } else { pal.input_border },
    );
    pt::text(
        p,
        r.locate.center(),
        Align2::CENTER_CENTER,
        tx::CHECK_MAP_LOCATE,
        m::BODY_FONT,
        if 可定位 {
            pal.accent_button_text
        } else {
            pal.text_muted
        },
    );
    pt::fill(p, r.close, pal.input_bg);
    pt::border(p, r.close, pal.input_border);
    pt::text(
        p,
        r.close.center(),
        Align2::CENTER_CENTER,
        tx::CHECK_MAP_CLOSE,
        m::BODY_FONT,
        pal.text,
    );
}


// ───────────────────────────── 输入 ─────────────────────────────

/// 用户在模态里做的一次动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum 动作 {
    /// 什么都没发生。
    None,
    /// 选中了第 `n` 行（**状态已写进 overlay**，返回值供测试与宿主记日志）。
    Select(usize),
    /// 「定位」：把这个世界格居中 + 设成 1×1 选区 + 关模态。
    Locate {
        /// 世界格 X。
        x: u16,
        /// 世界格 Y。
        y: u16,
    },
    /// 关闭模态。
    Close,
}

/// 处理模态输入（**只在 `Overlay::CheckMap` 时调用**）。
///
/// 键盘：`Esc` 关、`↑↓` 换选中行、`Home/End` 跳到首/末行、`Enter` **不做定位**
/// （与官方一致：`Locate` 是显式按钮，回车不提交 —— 避免误把对话框当"确定"按掉）。
pub fn interact(ui: &mut Ui, l: &Layout, overlay: &mut Overlay, 当前gen: u64) -> 动作 {
    let Overlay::CheckMap {
        report,
        selected,
        scroll,
        gen,
        ..
    } = overlay
    else {
        return 动作::None;
    };
    let stale = *gen != 当前gen;
    let r = rects(l);
    let 行数 = report.issues.len();
    let n = 可见行数(r.list);

    // ① Esc = 关闭
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return 动作::Close;
    }
    // ② 「关闭」按钮
    if 真点击(ui, r.close, ui.id().with(("check_map", "close"))) {
        return 动作::Close;
    }
    // ③ 「定位」按钮（灰显时不响应，也不改状态）
    if 真点击(ui, r.locate, ui.id().with(("check_map", "locate"))) {
        if let Some(i) = *selected {
            if let Some(issue) = report.issues.get(i) {
                if !stale {
                    return 动作::Locate {
                        x: issue.x,
                        y: issue.y,
                    };
                }
            }
        }
        return 动作::None;
    }
    // ④ 键盘换选中（陈旧时不动：那时列表是空的，上游也没得选）
    if !stale && 行数 > 0 {
        let (down, up, home, end) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
            )
        });
        if down || up || home || end {
            let cur = selected.unwrap_or(0);
            let next = if home {
                0
            } else if end {
                行数 - 1
            } else if down {
                (cur + 1).min(行数 - 1)
            } else {
                cur.saturating_sub(1)
            };
            *selected = Some(next);
            // 让选中行保持在可见窗口内（向上越界 ⇒ 它成为首行；向下越界 ⇒ 它成为末行）
            if next < *scroll {
                *scroll = next;
            } else if next >= *scroll + n {
                *scroll = next + 1 - n;
            }
            *scroll = 夹滚动(*scroll, 行数, n);
            return 动作::Select(next);
        }
    }
    // ⑤ 滚轮（`smooth_scroll_delta.y` 正 = 向上）
    let 滚 = ui.input(|i| i.smooth_scroll_delta.y);
    if 滚.abs() > f32::EPSILON {
        let 步 = if 滚 > 0.0 { -1isize } else { 1isize };
        let 新 = if 步 < 0 {
            scroll.saturating_sub(1)
        } else {
            *scroll + 1
        };
        *scroll = 夹滚动(新, 行数, n);
        return 动作::None;
    }
    // ⑥ 点列表：命中第几行
    if ui.input(|i| i.pointer.any_click()) && !stale {
        if let Some(pos) = ui
            .input(|i| i.pointer.interact_pos())
            .or_else(|| ui.ctx().pointer_hover_pos())
        {
            if r.list.contains(pos) && 行数 > 0 {
                let 行 = ((pos.y - r.list.top()) / m::CHECK_MAP_ROW_H).floor();
                if 行 >= 0.0 {
                    let idx = *scroll + 行 as usize;
                    if idx < 行数 {
                        *selected = Some(idx);
                        return 动作::Select(idx);
                    }
                }
            }
        }
    }
    // ⑦ 点面板外：**不关**（模态对话框；要关请按 Esc / 关闭）
    动作::None
}

/// 把滚动位置夹到 `[0, 行数 - 可见行数]`（行数不够滚动 ⇒ 0）。
fn 夹滚动(值: usize, 行数: usize, 可见: usize) -> usize {
    值.min(行数.saturating_sub(可见))
}

/// 只认真实鼠标点击（与 `menu::cellprops` / `menu::dialog` 同一口径）。
fn 真点击(ui: &mut Ui, rect: Rect, id: egui::Id) -> bool {
    ui.interact(rect, id, egui::Sense::click()).clicked()
}

// ───────────────────────── `--check-map` 自动化 ─────────────────────────

/// `--check-map` 默认最多打印多少条（够看清问题类型，又不至于刷屏；`--check-map-show` 可改）。
pub const CHECK_MAP_SHOW_DEFAULT: usize = 40;

/// `--check-map`：打开地图（可选）→ 跑一次**只读**检查 → 把 summary + 前 `show` 条
/// 打到 `stderr`，返回进程退出码（`0` = 跑完；`2` = 没图 / 读不出 / 快照失败）。
///
/// 为什么走 stderr：与 `--menu-dump` / `--probe` 同一口径（诊断都在 stderr，
/// 自动化脚本用退出码判成败）。**只读**：不写盘、不置脏、不进撤销栈。
pub fn run_check_map(map: Option<&std::path::Path>, show: usize) -> i32 {
    let st = crate::mapdoc::DocState::load(map);
    let Some(d) = st.doc.as_ref() else {
        eprintln!(
            "{}",
            st.map_error
                .as_deref()
                .unwrap_or(tx::STATUS_CHECK_MAP_NEEDS_MAP)
        );
        return 2;
    };
    let raw = match d.raw() {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "{}",
                tx::fill(tx::STATUS_CHECK_MAP_RAW_FAIL_TMPL, &[("err", &e)])
            );
            return 2;
        }
    };
    let src = st.assets.source();
    let has_libs = src.is_some();
    let probe = WilProbe::new(src.map(|s| s as &dyn mir2_assets::SpriteSource));
    // 「没有图库」时换成 NoLibProbe（= 上游 `libraries == null`），不刷假缺库
    const 无库: miredit_core::NoLibProbe = miredit_core::NoLibProbe;
    let 探针: &dyn miredit_core::LibProbe = if has_libs { &probe } else { &无库 };
    let report = miredit_core::check_map(&raw, 探针);
    // 「有没有库」以**报告**为唯一真源（`check_map` 从 `probe.has_libraries()` 落的值）
    let has_libs = !report.无图库;
    eprintln!("{}", summary_text(&report, false));
    for line in hint_lines(&report, has_libs) {
        eprintln!("{line}");
    }
    for issue in report.issues.iter().take(show) {
        eprintln!("{}", issue.line());
    }
    if report.truncated() > 0 {
        eprintln!("…（还有 {} 条未打印）", report.truncated());
    }
    0
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

    /// **几何自洽**：面板在窗口内、四块互不重叠、列表有正高度。
    #[test]
    fn 几何自洽() {
        let l = 布局();
        let r = rects(&l);
        assert!(l.content.contains_rect(r.panel), "面板越出窗口：{:?}", r.panel);
        assert_eq!(r.panel.width(), m::CHECK_MAP_W);
        assert_eq!(r.panel.height(), m::CHECK_MAP_H);
        assert!(r.title.bottom() <= r.summary.top());
        assert!(r.summary.bottom() < r.list.top(), "汇总压到列表上");
        assert!(r.list.bottom() < r.locate.top(), "列表压到按钮上");
        assert!(r.list.width() > 0.0 && r.list.height() > 0.0);
        assert!(r.locate.right() <= r.close.left(), "两个按钮重叠");
        assert!(r.close.right() <= r.panel.right() - m::CHECK_MAP_PAD + 0.01);
        assert!(可见行数(r.list) >= 10, "760×480 至少要能显示 10 行");
    }

    /// **五态映射一一对应**（这条是"防漂移"的编译期钉子：任一侧加变体 ⇒ 编译失败）。
    #[test]
    fn 五态映射一一对应() {
        for (资产态, 内核态) in [
            (mir2_assets::ImageState::Ok, miredit_core::ImageState::Ok),
            (mir2_assets::ImageState::Empty, miredit_core::ImageState::Empty),
            (
                mir2_assets::ImageState::Unreadable,
                miredit_core::ImageState::Unreadable,
            ),
            (
                mir2_assets::ImageState::OutOfRange,
                miredit_core::ImageState::OutOfRange,
            ),
            (
                mir2_assets::ImageState::MissingLibrary,
                miredit_core::ImageState::MissingLibrary,
            ),
        ] {
            assert_eq!(态映射(资产态), 内核态, "{资产态:?} 映射错了");
        }
    }

    /// **族内页号恒等换名**（不是 C# 全局号）：
    /// `ground(3)` → `Tiles{4}`、`middle(2)` → `SmTiles{3}`、`surface(0)` → `Objects`。
    #[test]
    fn 资产定位键恒等换名() {
        assert_eq!(
            资产定位键(miredit_core::MapLibRef::ground(3)),
            mir2_assets::LibRef::ground(3)
        );
        assert_eq!(
            资产定位键(miredit_core::MapLibRef::middle(2)),
            mir2_assets::LibRef::middle(2)
        );
        assert_eq!(
            资产定位键(miredit_core::MapLibRef::surface(0)),
            mir2_assets::LibRef::surface(0)
        );
        // 三个族不会串（同名索引在不同族是不同文件）
        assert_ne!(
            资产定位键(miredit_core::MapLibRef::ground(0)),
            资产定位键(miredit_core::MapLibRef::surface(0))
        );
    }

    /// 汇总文案：正常 / 陈旧两态，数字来自报告原文（不写死）。
    #[test]
    fn 汇总文案跟随报告() {
        let report = MapCheckReport {
            issues: vec![miredit_core::MapIssue {
                x: 3,
                y: 4,
                message: miredit_core::MSG_LIB_MISSING.to_owned(),
            }],
            total: 7,
            ..Default::default()
        };
        let s = summary_text(&report, false);
        assert!(s.contains("共 7 条"), "{s}");
        assert!(s.contains("显示 1 条"), "{s}");
        assert_eq!(summary_text(&report, true), tx::CHECK_MAP_STALE);
    }

    /// 提示行：逐层「未检查」各一行 + 无图库追加一行；都有时两样都在。
    #[test]
    fn 提示行按层与图库状态() {
        let mut report = MapCheckReport::default();
        assert!(hint_lines(&report, true).is_empty(), "都查了就不该有提示");

        report.未检查 = vec![
            (LibFamily::Ground, miredit_core::UNCHECKED_MIR3_BLOCK_GROUND),
            (LibFamily::Objects, miredit_core::UNCHECKED_CRYSTALCS),
        ];
        let lines = hint_lines(&report, true);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains(LibFamily::Ground.label()), "{}", lines[0]);
        assert!(
            lines[0].contains(miredit_core::UNCHECKED_MIR3_BLOCK_GROUND),
            "原因要点名：{}",
            lines[0]
        );
        assert!(lines[1].contains(LibFamily::Objects.label()), "{}", lines[1]);

        let lines = hint_lines(&report, false);
        assert_eq!(lines.len(), 3, "无图库时追加官方那句：{lines:?}");
        assert_eq!(lines[2], tx::CHECK_MAP_NO_LIBS);
    }

    /// `WilProbe`：1 基图号 → 0 基帧号；没有图源 ⇒ 一律缺库；图号 0 ⇒ 直接 Ok。
    #[test]
    fn 探针把一基图号折成零基帧号() {
        use std::cell::RefCell;

        struct 假源(RefCell<Vec<(mir2_assets::LibRef, u32)>>);
        impl mir2_assets::SpriteSource for 假源 {
            fn frame(&self, _lib: mir2_assets::LibRef, _index: u32) -> Option<mir2_assets::Frame> {
                None
            }
            fn frame_state(
                &self,
                lib: mir2_assets::LibRef,
                index: u32,
            ) -> mir2_assets::ImageState {
                self.0.borrow_mut().push((lib, index));
                mir2_assets::ImageState::Empty
            }
        }
        let 源 = 假源(RefCell::new(Vec::new()));
        let probe = WilProbe::new(Some(&源));
        let 态 = miredit_core::LibProbe::frame_state(
            &probe,
            miredit_core::MapLibRef::surface(3),
            1,
        );
        assert_eq!(态, miredit_core::ImageState::Empty);
        assert_eq!(
            源.0.borrow().as_slice(),
            &[(mir2_assets::LibRef::surface(3), 0)],
            "1 基图号要折成 0 基帧号（上游 ReadHeader(image - 1)）"
        );
        // 图号 0 不查
        let 态 = miredit_core::LibProbe::frame_state(
            &probe,
            miredit_core::MapLibRef::surface(3),
            0,
        );
        assert_eq!(态, miredit_core::ImageState::Ok);
        assert_eq!(源.0.borrow().len(), 1, "图号 0 不该进图源");
        // 没有图源 ⇒ 缺库（直接调用时），且 `has_libraries()` 为 false
        let 空 = WilProbe::new(None);
        assert_eq!(
            miredit_core::LibProbe::frame_state(&空, miredit_core::MapLibRef::ground(0), 5),
            miredit_core::ImageState::MissingLibrary
        );
        assert!(!miredit_core::LibProbe::has_libraries(&空), "没图源 = 没有库表");
        assert!(
            miredit_core::LibProbe::has_libraries(&probe),
            "有图源 = 有库表（照常查图号）"
        );
    }
    // ───────────────────── 模态输入（真 egui 帧，模拟点击/按键） ─────────────────────


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

    /// 在浮层上跑一帧 `interact`，返回它给出的动作。
    fn 跑(
        ctx: &egui::Context,
        l: &Layout,
        overlay: &mut Overlay,
        当前gen: u64,
        input: egui::RawInput,
    ) -> 动作 {
        let mut got = 动作::None;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    got = interact(ui, l, overlay, 当前gen);
                });
        });
        got
    }

    /// 一次"按下 + 松开"（同一点，同一帧）——与 `menu::mod` 的点击用例同一套事件。
    fn 点(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }

    /// 三条问题的报告（坐标各不相同，便于断言"定位到哪一格"）。
    fn 三条报告() -> MapCheckReport {
        MapCheckReport {
            issues: (0..3)
                .map(|i| miredit_core::MapIssue {
                    x: 10 + i,
                    y: 20 + i,
                    message: miredit_core::MSG_DOOR_NO_WALKABLE.to_owned(),
                })
                .collect(),
            total: 3,
            ..Default::default()
        }
    }

    fn 浮层(report: MapCheckReport, gen: u64) -> Overlay {
        Overlay::CheckMap {
            report,
            selected: None,
            scroll: 0,
            gen,
            has_libs: true,
        }
    }

    /// `Esc` 关闭（官方 `IsCancel = true` 那只按钮的键盘等价物）。
    #[test]
    fn esc关闭() {
        let ctx = egui::Context::default();
        let l = 布局();
        let mut o = 浮层(三条报告(), 5);
        let got = 跑(
            &ctx,
            &l,
            &mut o,
            5,
            一帧(
                0.0,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            ),
        );
        assert_eq!(got, 动作::Close);
    }

    /// 点第 1 行 ⇒ 选中它（并且**写回浮层**，宿主只需重绘）。
    #[test]
    fn 点行选中并把状态写回浮层() {
        let ctx = egui::Context::default();
        let l = 布局();
        let r = rects(&l);
        let 行1 = pos2(
            r.list.left() + 20.0,
            r.list.top() + m::CHECK_MAP_ROW_H * 1.5,
        );
        let mut o = 浮层(三条报告(), 5);
        let got = 跑(&ctx, &l, &mut o, 5, 一帧(0.2, 点(行1)));
        assert_eq!(got, 动作::Select(1), "点第 1 行要选中第 1 行");
        match &o {
            Overlay::CheckMap { selected, .. } => assert_eq!(*selected, Some(1)),
            other => panic!("浮层变了：{other:?}"),
        }
    }

    /// 地图已变（`map_gen` 不同）⇒ 点行**不再选中**（列表是空的，上游也没得选）。
    #[test]
    fn 陈旧时点行不选中() {
        let ctx = egui::Context::default();
        let l = 布局();
        let r = rects(&l);
        let 行0 = pos2(r.list.left() + 20.0, r.list.top() + 2.0);
        let mut o = 浮层(三条报告(), 5);
        let got = 跑(&ctx, &l, &mut o, 6, 一帧(0.4, 点(行0)));
        assert_eq!(got, 动作::None, "陈旧结果不许再被选中");
        match &o {
            Overlay::CheckMap { selected, .. } => assert_eq!(*selected, None),
            other => panic!("浮层变了：{other:?}"),
        }
    }

    /// 「定位」：选中且不陈旧 ⇒ 给出那一格的坐标；未选中 / 已陈旧 ⇒ 什么都不做。
    #[test]
    fn 定位按钮只在可定位时给坐标() {
        let ctx = egui::Context::default();
        let l = 布局();
        let r = rects(&l);
        // 未选中 ⇒ 点定位不动
        let mut o = 浮层(三条报告(), 5);
        assert_eq!(
            两帧点(&ctx, &l, &mut o, 5, 0.6, r.locate.center()),
            动作::None
        );
        // 选中第 2 行 ⇒ 定位到 (12, 22)
        let mut o = Overlay::CheckMap {
            report: 三条报告(),
            selected: Some(2),
            scroll: 0,
            gen: 5,
            has_libs: true,
        };
        assert_eq!(
            两帧点(&ctx, &l, &mut o, 5, 0.8, r.locate.center()),
            动作::Locate { x: 12, y: 22 }
        );
        // 同一份结果 + 地图已变 ⇒ 定位失效
        let mut o = Overlay::CheckMap {
            report: 三条报告(),
            selected: Some(2),
            scroll: 0,
            gen: 5,
            has_libs: true,
        };
        assert_eq!(
            两帧点(&ctx, &l, &mut o, 9, 1.0, r.locate.center()),
            动作::None
        );
    }

    /// 先 `PointerMoved` 到 `pos` 建 hover，再在同一位置按下+松开（**两帧**）。
    ///
    /// 为什么不能一帧搞定：egui 的 `interact().clicked()` 要求控件**在按下之前**
    /// 就已经是 hover 态；同一帧里"凭空按下再松开"读不到（这是 egui 的帧语义，
    /// 不是本模块的逻辑 —— 与 `menu::mod` 的点击用例同一套两帧写法）。
    fn 两帧点(
        ctx: &egui::Context,
        l: &Layout,
        overlay: &mut Overlay,
        当前gen: u64,
        t: f64,
        pos: egui::Pos2,
    ) -> 动作 {
        let _ = 跑(
            ctx,
            l,
            overlay,
            当前gen,
            一帧(t, vec![egui::Event::PointerMoved(pos)]),
        );
        跑(ctx, l, overlay, 当前gen, 一帧(t + 0.01, 点(pos)))
    }

    /// 「关闭」按钮。
    #[test]
    fn 关闭按钮() {
        let ctx = egui::Context::default();
        let l = 布局();
        let r = rects(&l);
        let mut o = 浮层(三条报告(), 5);
        assert_eq!(
            两帧点(&ctx, &l, &mut o, 5, 1.2, r.close.center()),
            动作::Close
        );
    }

    // ───────────────────── `--check-map` 的退出码（R34 复核项 3） ─────────────────────

    /// **没有地图 ⇒ 退出码 2**（`--check-map` 不给 `--map`，或路径不存在）。
    #[test]
    fn check_map无图时退出码2() {
        assert_eq!(run_check_map(None, 3), 2, "不给 --map ⇒ 2");
        let 不存在 = std::env::temp_dir().join("miredit_check_map_不存在.map");
        assert_eq!(run_check_map(Some(&不存在), 3), 2, "路径不存在 ⇒ 2");
    }

    /// **有地图 ⇒ 退出码 0**（有/无问题都算"跑完"），且 summary 打到 stderr。
    #[test]
    fn check_map有图时退出码0() {
        let dir = std::env::temp_dir().join(format!("miredit_check_map_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let p = dir.join("4x4.map");
        let mut b = vec![0u8; 52 + 12 * 16];
        b[0..2].copy_from_slice(&4u16.to_le_bytes());
        b[2..4].copy_from_slice(&4u16.to_le_bytes());
        std::fs::write(&p, &b).expect("写合成图");
        assert_eq!(run_check_map(Some(&p), 3), 0, "能读的图 ⇒ 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `↓` 换选中行，并把选中行滚进可见窗口（选中在窗口下沿之外时）。
    #[test]
    fn 下箭头换选中行() {
        let ctx = egui::Context::default();
        let l = 布局();
        let mut report = 三条报告();
        report.issues = (0..100)
            .map(|i| miredit_core::MapIssue {
                x: i,
                y: 0,
                message: miredit_core::MSG_DOOR_NO_WALKABLE.to_owned(),
            })
            .collect();
        report.total = 100;
        let mut o = Overlay::CheckMap {
            report,
            selected: Some(0),
            scroll: 0,
            gen: 5,
            has_libs: true,
        };
        let 下 = |t: f64| {
            一帧(
                t,
                vec![egui::Event::Key {
                    key: egui::Key::ArrowDown,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            )
        };
        assert_eq!(跑(&ctx, &l, &mut o, 5, 下(1.4)), 动作::Select(1));
        // 连按到超出可见行数 ⇒ 滚动位置必须跟上（否则选中行看不见）
        for i in 0..60 {
            let _ = 跑(&ctx, &l, &mut o, 5, 下(1.5 + i as f64 * 0.01));
        }
        match &o {
            Overlay::CheckMap { selected, scroll, .. } => {
                let sel = selected.expect("必须仍选中");
                assert!(sel > 50, "应已下移：{sel}");
                assert!(*scroll > 0, "选中行超出可见窗口时必须滚动：scroll={scroll}");
                assert!(
                    sel >= *scroll && sel < *scroll + 可见行数(rects(&l).list),
                    "选中行 {sel} 必须在可见窗口 [{scroll}, …) 内"
                );
            }
            other => panic!("浮层变了：{other:?}"),
        }
    }
}
