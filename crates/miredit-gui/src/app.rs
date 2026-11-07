//! 应用主体：把各模块的绘制与交互串起来。
//!
//! 绘制顺序（自底向上）：
//! 窗口底 → 左右分隔线 → 左停靠区 → 中列（页签条/真实地图/悬停高亮/缩放浮层）→ 右停靠区
//! → 菜单栏 → 标题栏 → 状态栏 → 关闭拦截模态（有未保存修改时才画）。
//!
//! # 每帧的固定次序（**别调换，有因果**）
//!
//! 1. **快捷键 + 关窗拦截**（`Ctrl+Z/Y/S`、`Esc`）——必须在交互之前，这样本帧的撤销/保存
//!    就参与本轮布局与重渲判定；
//! 2. **自动化驱动**（`--autodrag` 时按帧平移）；
//! 3. **交互**（拖拽/滚轮/点击/图层开关/**工具落格**）；
//! 4. **悬停格**（由交互后的视口算出）；
//! 5. **重渲调度**（[`CanvasView::ensure`]，只在视口/图层/地图变化时真渲）；
//! 6. **装配文案**（`screen_text::build`，全部来自 `ui_text` 模板）；
//! 7. **绘制**；
//! 8. **截图/拖动的自动化收尾**。
//!
//! # 本单接线的编辑闭环（详见 [`crate::edit`] / [`crate::save`]）
//!
//! | 动作 | 入口 | 粒度 |
//! |---|---|---|
//! | 吸管取格 | 左键点画布（工具 = 吸管）| 即时 |
//! | 笔刷落格 | 左键按下并拖动（工具 = 画笔）| 一次拖动 = 一步撤销；**每格按 N×N 落** |
//! | 橡皮清图号 | 左键按下并拖动（工具 = 橡皮）| 同上 |
//! | **填充** | 左键点画布（工具 = 填充，快捷键 `F`）| **整次填充 = 一步撤销** |
//! | **笔刷尺寸** | `[` / `]`（各 ±2，夹 3..=31）| 只改落笔范围，**不重渲** |
//! | 撤销 / 重做 | `Ctrl+Z` / `Ctrl+Y` | 一步 |
//! | 保存写回 | `Ctrl+S`（原子替换，仅在 `dirty` 时真写）| — |
//! | 关闭拦截 | 右上角 `×` / 系统关窗 | `dirty` 时先弹模态 |
//!
//! ## 印章放置（2026-11-05 新增，详见 [`crate::stamp_place`]）
//!
//! | 动作 | 入口 | 说明 |
//! |---|---|---|
//! | 选中一枚印章 | 资源面板「分类」页签里点缩略图 | 只选中，**不放置** |
//! | 进入**待放置态** | `Enter`（搜索框有焦点时不抢）| 状态栏 + 画布徽标双提示 |
//! | **落位** | 待放置态下左键点画布 | 读 `.X` → 注册 → 落位；**整次 = 一步撤销** |
//! | 看轮廓 | 待放置态下**按住 `Alt`** | 只描边、不填像素 |
//! | 退出待放置态 | `Esc` / 点别的工具 / 落位成功 | — |
//!
//! ## 矩形选区 + 剪贴板（2026-11-05 新增，详见 [`crate::selection_ops`]）
//!
//! | 动作 | 入口 | 说明 |
//! |---|---|---|
//! | 画选区 | 工具列第 5 个「选择」+ 左键拖动 | 金色虚线框 + 半透明填充；**不置脏** |
//! | 复制 / 剪切 / 粘贴 | `Ctrl+C` / `Ctrl+X` / `Ctrl+V`（粘贴锚点 = 鼠标格按源奇偶对齐，内核 `Clipboard::anchor`）| 各一步撤销 |
//! | 删除选区内容 | `Delete` | 一步撤销 |
//! | 填充选区 | `Ctrl+Shift+V`（**无官方依据，本仓自定**）| 一步撤销 |
//! | 取消选区 | `Esc` | **不清剪贴板** |
//!
//! 所有几何由 [`Layout::compute`] 一次性算出，所有颜色来自 [`crate::theme`]。

use std::path::PathBuf;
use std::time::Instant;

use egui::{Align2, Rect, Sense, Vec2};
use mir2_assets::SpriteSource;
use miredit_render::{MapLighting, RenderOptions};

use crate::banner;
use crate::canvas::{self, CanvasCtx};
use crate::canvas_view::CanvasView;
use crate::cell_hover;
use crate::cellinfo::{self, CellLines};
use crate::cli::Cli;
use crate::dock_left;
use crate::dock_right;
use crate::edit::{self, EditSession, Tool};
use crate::layout::Layout;
use crate::mapdoc::{Assets, DocState};
use crate::maplist::{self, MapListPanel};
use crate::menu::panels::{PanelId, PanelVis};
use crate::menu::{self, Overlay};
use crate::menubar;
use crate::metrics as m;
use crate::paint as pt;
use crate::palette::{PaletteAction, PaletteState};
use crate::stamp_panel::{self, StampAction, StampPanel};
use crate::save;
use crate::screen_text::{self, DocText};
use crate::selection_ops::{self, Cmd as SelCmd, Drag as SelDrag};
use crate::selfcheck;
use crate::stamp_place::{self, Armed};
use crate::statusbar;
use crate::theme as t;
use crate::titlebar;
use crate::ui_text as tx;

/// 自动化动作（截图 / 拖动实测 / 素材滚动实测）的状态机。
#[derive(Default)]
struct Auto {
    /// 截图落盘路径（`--shot`）。
    shot: Option<PathBuf>,
    /// 第几帧请求截图（`--frames`，默认 30）。
    frames: u64,
    /// 是否已发出截图请求（只发一次）。
    requested: bool,
    /// 截图是否已落盘。
    saved: bool,
    /// 客户端截图路径（`--gate2-client`）。
    gate2_client: Option<PathBuf>,
    /// 客户端截图的世界格中心（`--gate2-at`）。
    gate2_at: Option<(i32, i32)>,
    /// 拖动实测秒数（`--autodrag`）。
    autodrag: Option<f32>,
    /// 拖动开始时刻。
    drag_started: Option<Instant>,
    /// 拖动报告是否已打印（只打一次）。
    drag_reported: bool,
    /// 素材面板滚动实测秒数（`--palette-autoscroll`）。
    palette_scroll: Option<f32>,
    /// 素材滚动开始时刻。
    palette_started: Option<Instant>,
    /// 素材滚动报告是否已打印（只打一次）。
    palette_reported: bool,
    /// `--palette-report`：退出/截图前打一次素材面板统计。
    palette_report: bool,
    /// `--stamp-report`：退出/截图前打一次印章面板统计。
    stamp_report: bool,
}

/// `--stamp-kind/--stamp-search/--stamp-page/--stamp-pick` 的原始取值。
///
/// 为什么不能在建面板时直接应用：目录要读 5.09 MB JSON（48~83 ms），
/// 而首帧不能等它 —— 所以这些开关一律**等装载完成后再落**（见 [`apply_stamp_cli`]）。
#[derive(Clone, Debug, Default)]
struct PendingStamp {
    /// `--stamp-kind <all|unclassified|unlisted|英文 kind>`。
    kind: Option<String>,
    /// `--stamp-search <文字>`。
    search: Option<String>,
    /// `--stamp-page <1 基页码>`。
    page: Option<usize>,
    /// `--stamp-pick <当前页第几个，0 基>`。
    pick: Option<usize>,
    /// `--stamp-arm`：装载完成后进入待放置态（= 按 `Enter`）。
    arm: bool,
    /// `--stamp-place-at <x,y>`：装载完成后在 `(x,y)` 落位（隐含 `arm`）。
    place_at: Option<(u16, u16)>,
}

/// 编辑器应用状态。
pub struct MireditApp {
    /// 文档 + 图库。
    pub doc: DocState,
    /// 画布视口（平移/缩放/图层/纹理缓存）。
    pub view: CanvasView,
    /// 编辑会话（工具 / 笔刷 / 撤销重做 / 脏标记）。
    pub edit: EditSession,
    /// 素材面板（真图库枚举 / 缩略图缓存 / 虚拟滚动 / 搜索）。
    pub palette: PaletteState,
    /// 资源面板「分类」页签 = 印章浏览器（9,159 枚 `.X`，后台装载 + LRU 缩略图）。
    pub stamps: StampPanel,
    /// 当前选中的工具下标（0..10，对应 `icons::TOOLS`）。
    pub selected_tool: usize,
    /// 鼠标所在世界格（不在图上为 `None`）。
    pub hover: Option<(u16, u16)>,
    /// `--hover` 指定的伪造悬停格（截图取证用，优先于真实鼠标）。
    forced_hover: Option<(u16, u16)>,
    /// **地砖信息悬浮**开关（视图 → 地砖信息悬浮；真源，**不持久化**）。
    ///
    /// 与 `grid` / `doors_open` / `lighting` 同一套口径：菜单翻转它、绘制读它
    /// （`crate::cell_hover`），勾选态由 `menu::model::check_with_hover` 从这里取真值。
    /// 与它们不同的是**没有启动参数**：上游 `CellHoverToggle` 也只有菜单一条入口。
    pub cell_hover: bool,
    /// 悬停浮层的**锚点**（上一帧的指针位置）。
    ///
    /// `None` = 这一帧没有真实指针（`--hover` 伪造悬停 / 截图取证）⇒ 浮层改钉在
    /// 该格的**中心**（否则截图里浮层会跑到 (0,0)）。
    hover_anchor: Option<egui::Pos2>,
    /// 当前单元格信息（缓存，见 [`Self::refresh_cell_lines`]）。
    lines: CellLines,
    /// 上面那份信息对应的 `(x, y, 地图代号)`；变了才重算（要问图库要帧，不能每帧算）。
    lines_key: Option<(u16, u16, u64)>,
    /// 这一帧装配好的全部动态文字。
    text: DocText,
    /// 首次布局后把视口对准目标格（只做一次）。
    pending_center: Option<(f32, f32)>,
    /// 是否已做过首次对准。
    centered: bool,
    /// 已渲染帧数（拖动实测的分母）。
    frames: u64,
    /// 帧间隔累计（毫秒）。
    frame_gap_ms: f64,
    /// `update()` 自耗时累计（毫秒）。
    update_ms: f64,
    /// 上一帧的时刻（算帧间隔）。
    last_frame: Option<Instant>,
    /// 最近一次布局算出的画布矩形（截图比对要用）。
    last_canvas: Rect,
    /// 最近一次布局算出的缩放浮层矩形（截图比对时要排除它）。
    last_overlay: Rect,
    /// **上一帧的画布尺寸**（A-012：`None` = 还没布局过）。
    ///
    /// 只用于「窗口尺寸是否变了」这一个判断 —— `Some(旧) != 当前` 时才走一次
    /// [`CanvasView::refit_on_resize`]，**不是每帧都 fit**（否则用户手动缩放/平移会被抢掉）。
    canvas_size: Option<Vec2>,
    /// **Alt 落位预览的内容纹理缓存**（A-011）：`(印章 id, 纹理, 宽, 高)`。
    ///
    /// 换选（`id` 变了）⇒ 失效重渲染；不按帧渲染（`render_stamp` 一次 ~10ms 级）。
    preview_tex: Option<(String, egui::TextureId, u32, u32)>,
    /// `--preview-alt`：把「按住 Alt」伪造成常真（**只服务取证截图**）。
    preview_alt: bool,
    /// 关闭拦截模态是否已经弹出（有未保存修改时点第一次 × 变成 `true`）。
    confirm_close: bool,
    /// 已发给窗口系统的标题（变了才发 `ViewportCommand::Title`，避免每帧发指令）。
    shown_title: String,
    /// 自动化状态机。
    auto: Auto,
    /// `--stamp-*` 的待应用项：印章目录是**后台装载**的，启动时还没就绪
    /// ⇒ 先存着，等装载完成的那一帧再落到面板上（只做一次）。
    pending_stamp: Option<PendingStamp>,
    /// **印章待放置态**（`None` = 不在待放置态）。
    ///
    /// 进入：选中一枚印章后按 `Enter`（[`Self::handle_keys`]）。
    /// 退出：点画布落位成功、按 `Esc`、或改选别的工具。
    armed: Option<Armed>,
    /// 矩形选区的拖动起点（`None` = 当前没在拖）。
    sel_drag: Option<SelDrag>,
    /// **最近一次选区 / 印章操作的回执**：`(文案, 是否拒绝)`。
    ///
    /// 为什么存在宿主里而不是会话里：内核的 `note` 会被状态栏「落笔」那一段消费，
    /// 若两段都说同一句话，用户会以为发生了两件事。所以选区/印章操作的回执由
    /// [`crate::selection_ops::take_note`] 从内核**取走**，存在这里，
    /// 由 `update` 填进 [`DocText::status_op`]；撤销/重做时清掉（它描述的是一步已不存在的操作）。
    op: Option<(String, bool)>,
    /// **菜单系统状态**（展开的是哪个菜单、键盘/鼠标高亮哪一项）。
    menu: menu::MenuState,
    /// **7 个面板的显隐**（视图菜单的 7 个开关；默认全显示）。
    panels: PanelVis,
    /// **深色模式开关**（真源；`theme::set_dark` 由 `dispatch_menu` 同步）。
    dark: bool,
    /// **单元格网格开关**（真源；`--cell-grid` / `--no-cell-grid` 置初值，`G` / 菜单 /
    /// `--menu-click` 翻转）。
    ///
    /// 默认**开**（官方 `MainWindow.axaml:5` 的 `IsChecked="True"`；2026-11-05 之前本仓默认关，
    /// 审查判定为形态偏离后订正 —— 判据在 [`crate::cli::Cli::cell_grid_enabled`]，
    /// 要无网格基线请显式传 `--no-cell-grid`）。
    /// 画布每帧读它决定画不画格线（[`crate::canvas::CanvasCtx::grid`]），
    /// 菜单弹层的勾也读它（[`crate::menu::model::check`]）—— **同一个字段，不许两套**。
    grid: bool,
    /// **开启门开关**（真源；`--open-doors` 置初值，菜单 / `--menu-click view.open_doors` 翻转）。
    ///
    /// 默认 **`false`** = 闭门（`miredit_render::RenderOptions::default()`）⇒ 与接线前
    /// **逐字节相同**（门偏移恒 0）。
    ///
    /// 三条纪律：
    /// 1. **同一个字段**：菜单勾（`model::check_with` 的 `doors`）、画布渲染
    ///    （`CanvasView::ensure` 的 `doors_open`，进缓存键 + `RenderOptions`）、
    ///    `--open-doors` 的初值 —— 三处读的都是它，不许有第二份；
    /// 2. **进缓存键**：门开合只改前景层帧号，视口/图层/地图都没变 ⇒ 不进键就不会重渲
    ///    （见 `canvas_view` 模块文档 §六）；
    /// 3. **不改地图**：门态是渲染开关，不是编辑 ⇒ 不许 `touch_map()`。
    doors_open: bool,
    /// **地图光照档位**（真源；`--lighting` 置初值，菜单三子项 / `--menu-click
    /// view.light_*` 切档）。
    ///
    /// 缺省 [`MapLighting::Day`]（render crate 的 `RenderOptions::default()`）⇒ 与接线前
    /// **逐字节相同**（白天档连遮罩都不画）。
    ///
    /// 四条纪律（与 `doors_open` 同一套口径）：
    /// 1. **同一个字段**：菜单勾（`model::check_with` 的 `lighting`）、画布渲染
    ///    （`CanvasView::ensure` 的 `RenderOptions`，进缓存键 + 渲染选项）、导出
    ///    （`export::export_*` 收同一份 `RenderOptions`）、`--lighting` 的初值 ——
    ///    四处读的都是它，不许有第二份；
    /// 2. **进缓存键**：切档只叠遮罩，视口/图层/地图都没变 ⇒ 不进键就不会重渲；
    /// 3. **不改地图**：光照是渲染开关，不是编辑 ⇒ 不许 `touch_map()`；
    /// 4. **不持久化**：官方 SettingsStore 里没有这个偏好（见 `miredit_render::lighting`
    ///    的模块文档），本仓同样只在内存里留这一档。
    lighting: MapLighting,
    /// 当前浮层（打开地图模态 / 快捷键浮层）。
    overlay: Overlay,
    /// 设置文件路径（`%LOCALAPPDATA%\miredit\settings.json`，可用 `--settings` 覆盖）。
    settings_path: PathBuf,
    /// **最近打开的地图列表**（最近优先；真源 = 设置文件里的 `recent`）。
    ///
    /// 谁改它：
    /// * 启动时读一次（[`menu::settings::load`]）；
    /// * 打开成功 ⇒ [`menu::action::open_path`] 提升/追加并落盘；
    /// * 点了一条已不存在的历史 ⇒ [`menu::action::open_recent_index`] 移除并落盘。
    ///
    /// 谁读它：菜单「文件 → 最近打开」的子菜单行（[`Self::sub_state`]）——
    /// **画的行数与键盘能走的行数必须是同一份**，否则 `↑↓` 会走到不存在的行上。
    recent: Vec<String>,
    /// `--menu-click` 的待执行队列 `(第几帧执行, 菜单项 id)`。
    ///
    /// 为什么要等：首次布局算出画布矩形之后，缩放/适应才有锚点。
    /// 为什么要帧号：验收「同一会话里切换开关 ⇒ 画面重新渲染」必须**先渲一帧、再切**，
    /// 全部在第 1 帧执行时切换发生在首帧重渲之前，缓存键失效观察不到
    /// （语法见 [`crate::cli::parse_menu_click`]，缺省第 1 帧 = 旧行为）。
    pending_menu_clicks: Vec<(u64, String)>,
    /// `--open-map` 的待执行路径（要等首次布局算出画布矩形才能 fit）。
    pending_open_map: Option<PathBuf>,
    /// **地图列表面板**（2026-11-05，上游 `901dbe2f` 移植；状态 + 分帧扫描器）。
    ///
    /// 面板**显不显示**不归它管：真源是 `panels.get(PanelId::Maps)`（与视图菜单同一份），
    /// 这里只装"扫到了什么、滚到第几行"。两者分开的理由见 [`crate::maplist`] 模块文档 §四。
    maps: MapListPanel,
    /// `--map-list-root` 是否**钉住了**根（钉住后不再跟着当前地图换目录）。
    maps_root_pinned: bool,
}

/// **地图列表面板的扫描根**：`--map-list-root` > 当前地图所在目录 > `None`。
///
/// 为什么不复用 `menu::settings` 的 `LibraryFolder`（上游是从设置里的游戏目录
/// `DataPaths.FindMapFolder` 找 `Map` 子目录）：本仓的设置文件只存深色偏好 + 最近地图
/// （`menu::settings::Settings`），**没有**"游戏目录"这个概念；再为它加一个持久字段
/// 会把"设置文件格式"这件事也拖进本单。当前地图所在目录是**零成本且永远正确**的根
/// —— 你正在看哪张图，列表就列它旁边那些图。显式指定走 `--map-list-root`。
pub fn maps_root(cli: &Cli, doc: &DocState) -> Option<PathBuf> {
    if let Some(r) = cli.map_list_root.as_ref() {
        return Some(r.clone());
    }
    let p = doc.doc.as_ref()?.path.clone();
    match p.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => Some(dir.to_path_buf()),
        _ => None,
    }
}

/// **地表落笔前端拦截的判据**（纯函数：`app` 与验收用例共用这一份，绝不各写一套）。
///
/// # 为什么要在前端拦（而不是等内核拒绝）
///
/// 任务书 §三.2：「`ground_paint_supported == false`（Type5）⇒ 地表图层行加「不可落笔」标记，
/// 且**点画布前先拦**（别等 core 拒绝）」。理由：内核拒绝发生在 `touch()` 里，
/// 那时画笔已经 `begin()` 过一笔、指针已经走过一串格 —— 用户看到的是"画了半天没反应"，
/// 而内核只会给一句 `note`。前端拦则是"按下即说明原因"，且**一条命令都不发**。
///
/// # 判据（两条同时成立才拦）
///
/// 1. `MapDoc::ground_paint_supported() == false` —— 九种格式里只有 Type5 / `Mir3Wemade`；
/// 2. 当前编辑层写的是**地表**（`EditLayer::Ground`，落笔字段 `bk_img`）。
///    Type5 的中层 / 物体 / 低位阻挡 / 高位阻挡**照常可写**（`mid_img` / `fr_img` 都在格内）。
///
/// 返回 `Some(原因)` = 拦下，调用方**必须直接 return**（一条编辑命令都不许发）；
/// `None` = 放行。原因文案见 [`tx::STATUS_GROUND_BLOCKED_TMPL`]。
pub fn ground_block_reason(doc: &DocState, layer: edit::EditLayer) -> Option<String> {
    let d = doc.doc.as_ref()?;
    if d.ground_paint_supported() {
        return None;
    }
    if layer != edit::EditLayer::Ground {
        return None;
    }
    Some(tx::fill(
        tx::STATUS_GROUND_BLOCKED_TMPL,
        &[("fmt", d.format_label())],
    ))
}

impl MireditApp {
    /// 按命令行开关构造应用（**先装载再开窗**：标题栏/画布第一帧就是真实数据）。
    pub fn new(cli: &Cli) -> Self {
        let mut doc = DocState::load(cli.map.as_deref());

        let mut view = CanvasView::default();
        if let Some(s) = cli.scale {
            view.scale = s.clamp(m::CANVAS_MIN_SCALE, m::CANVAS_MAX_SCALE);
        }
        if let Some([g, mid, f]) = cli.layers {
            view.mask = miredit_render::LayerMask {
                ground: g,
                middle: mid,
                front: f,
            };
        }
        // `--collision`：初始**阻挡标记通道**（缺省两个都关 ⇒ 与上一版逐字节一致）。
        // 截图取证时鼠标被锁死，点不了眼睛，只能靠它把通道打开。
        if let Some(overlay) = cli.collision {
            view.overlay = overlay;
        }

        // 编辑会话：先按 `--tool`（缺省第 0 个 = 画笔）选中工具。
        let mut edit = EditSession::default();
        // ⓪ **把真文档绑进会话**（2026-11-05 九格式收口，必须在任何落笔/选区之前）：
        //    内核的库号字节还原、Type5 地表闸门、选区入口裁剪读的都是 `raw`（字节层），
        //    不绑定的话 `raw` 停在 1×1 `Old12` 占位，三处全按错误格式判定。
        //    失败要**说出来**（保存仍然正确，但"按格式判定的编辑语义"会退化成占位档位）。
        if let Some(d) = doc.doc.as_ref() {
            if !edit.bind_document(&d.path, d.orig_bytes()) {
                eprintln!(
                    "{}",
                    tx::fill(
                        tx::DIAG_EDIT_BIND_FAIL,
                        &[("path", &d.path.display().to_string())]
                    )
                );
            }
        }
        let selected_tool = cli.tool.unwrap_or(0).min(crate::icons::TOOLS.len() - 1);
        edit.select(selected_tool);
        // `--layer`：本次运行的**当前编辑层**（缺省 = 地表层）
        if let Some(layer) = cli.layer {
            edit.set_layer(layer);
        }
        // `--brush-size`：本次运行的**笔刷尺寸**（缺省 3；夹进 3..=31 的奇数）
        if let Some(n) = cli.brush_size {
            edit.set_brush_size(n);
        }

        // 素材面板：库列表来自**已装载的登记表**（装载失败 ⇒ 一个库都没有 ⇒ 面板走空态）。
        //
        // ⚠ 必须在 `--edit-*` 脚本**之前**建好并处理 `--palette-pick`：
        //   脚本里的 `--edit-brush` 要吃掉"当前笔刷"，而笔刷正是从素材面板选的。
        let mut palette = PaletteState::new(match &doc.assets {
            Assets::Ready { paths, .. } => Some(paths),
            Assets::Failed(_) => None,
        });
        apply_palette_cli(&mut palette, &mut edit, cli, doc.assets.source());

        // 再执行 `--edit-*` 脚本（GUI 里同样执行，截图取证靠它把"改动痕迹"画上去）。
        if !cli.edits.is_empty() {
            let _ = edit::apply_script(&mut doc, &mut edit, &cli.edits);
            // 脚本会按指令切工具，这里恢复成命令行/缺省选中的那个（状态栏显示的才是"当前工具"）
            edit.select(selected_tool);
        }

        // `--select <x0,y0,x1,y1>` / `--select-copy`：**走与鼠标拖动完全相同**的
        // `EditSession::set_selection`（内核在入口处裁剪一次），不是另写一条"直接赋值"的捷径
        // —— 否则命令行能造出"整块在图外"的非法选区，而鼠标造不出来。
        if let Some((x0, y0, x1, y1)) = cli.select {
            let mut ok = false;
            if let Some(d) = doc.doc.as_mut() {
                ok = edit.set_selection(&mut d.map, miredit_core::Selection::new(x0, y0, x1, y1));
            }
            if cli.select_copy {
                if let Some(d) = doc.doc.as_mut() {
                    let _ = edit.copy_selection(&mut d.map);
                }
            }
            let clip = match edit.clipboard() {
                Some(c) => format!("{}×{}", c.width(), c.height()),
                None => tx::VAL_UNKNOWN.to_owned(),
            };
            eprintln!(
                "{}",
                tx::fill(
                    tx::DIAG_SEL_CLI,
                    &[
                        ("x0", &x0.to_string()),
                        ("y0", &y0.to_string()),
                        ("x1", &x1.to_string()),
                        ("y1", &y1.to_string()),
                        ("ok", if ok { "1" } else { "0" }),
                        ("clip", &clip),
                    ]
                )
            );
        }

        let text = screen_text::build(&doc, &view, None, &CellLines::unknown(), &edit);

        // ── 主题 + 面板显隐 + 菜单初始态（2026-11-05） ──
        //
        // ① 设置文件：读失败**回落浅色**（`settings::load` 已把原因打进 stderr），绝不崩；
        // ② 命令行 `--theme` / `--dark` 覆盖设置文件（取证要能强制浅/深）；
        // ③ `theme::set_dark` 是绘制取色的唯一开关，必须在**任何一帧之前**设好。
        let settings_path = cli
            .settings
            .clone()
            .unwrap_or_else(menu::settings::default_path);
        // 一次读盘拿两个字段（深色偏好 + 最近地图列表）：读失败 ⇒ 回落浅色 + 空列表，
        // 并把这句原因打到 stderr（**不静默**，见 `menu::settings` 的三条纪律）。
        let (mut dark, recent) = match menu::settings::load(&settings_path) {
            Ok(s) => (s.dark, s.recent),
            Err(e) => {
                eprintln!(
                    "{}",
                    tx::fill(
                        tx::DIAG_SETTINGS_FALLBACK,
                        &[("path", &settings_path.display().to_string()), ("err", &e)]
                    )
                );
                (false, Vec::new())
            }
        };        if cli.dark {
            dark = true;
        }
        if let Some(d) = cli.theme {
            dark = d;
        }
        t::set_dark(dark);

        // 面板显隐：`--panel-hide` 逐个关（认不出的键在 cli 里已经报过错）
        let mut panels = PanelVis::default();
        let hid = menu::action::hide_panels(&mut panels, &cli.panel_hide);
        if !cli.panel_hide.is_empty() {
            eprintln!(
                "[面板] --panel-hide 生效 {} / {} 个：{}",
                hid.len(),
                cli.panel_hide.len(),
                hid.join(",")
            );
        }

        // `--show-maps`：展开地图列表面板（它默认关；截图/验收要它开着）。
        // 与视图菜单勾选走**同一个字段**（`PanelVis`），不另存一份开关。
        if cli.show_maps {
            panels.set(PanelId::Maps, true);
        }

        // 菜单初始态：`--menu-open` 展开某个菜单、`--shortcuts` 直接开浮层
        let mut menu_state = menu::MenuState::default();
        if let Some(i) = cli.menu_open {
            menu_state.open(i);
        }
        // `--menu-hover`：把某一项设为高亮项（子菜单取证用；只在 `--shot`/`--menu-open`
        // 同时给时生效，避免影响正常交互）。越界/没展开菜单 ⇒ 打一条诊断并忽略。
        if let Some(i) = cli.menu_hover {
            // 两个条件合并成一个 `if`（clippy `collapsible_if`）：`&&` 的求值顺序与
            // 嵌套 `if` 相同 —— 先看有没有 `--shot`/`--menu-open`，再看 `focus(i)`
            // 成不成立，所以 `focus` 的副作用（把第 i 项设为高亮）不会多发生一次。
            if (cli.shot.is_some() || cli.menu_open.is_some()) && !menu_state.focus(i) {
                eprintln!(
                    "{}",
                    tx::fill(
                        tx::DIAG_BAD_ARG,
                        &[("arg", &format!("--menu-hover {i}（当前展开的菜单没有这一项）"))]
                    )
                );
            }
        }
        // `--menu-sub-focus <父项>,<行>`：把**键盘焦点**送进子菜单并停在第 <行> 行
        // （子行高亮的截图取证入口；只在 `--shot` + `--menu-open` 同给时生效）。
        //
        // 走**真键盘路径**（`menu::handle_key_with`：先 `→` 进子菜单，再按 <行> 次 `↓`），
        // **不是**直接写 `MenuState::sub_focus`（那是私有字段，也不该为截图开后门）
        // ⇒ 截图证的是"用户按 → 再按 ↓ 之后屏幕上长什么样"，与手动操作同一条链。
        if let (Some(mi), Some((parent, row))) = (cli.menu_open, cli.menu_sub_focus) {
            if cli.shot.is_some() {
                let 有子菜单 = menu::model::MENUS
                    .get(mi)
                    .and_then(|m| m.items.get(parent))
                    .map(|it| it.sub.is_some())
                    .unwrap_or(false);
                let sub_state = menu::model::SubState {
                    recent: recent.clone(),
                    ..menu::model::SubState::default()
                };
                let mut ok = 有子菜单 && menu_state.focus(parent);
                if ok {
                    ok = menu::handle_key_with(&mut menu_state, menu::MenuKey::Right, &sub_state)
                        == menu::KeyResult::Changed;
                }
                for _ in 0..row {
                    if !ok {
                        break;
                    }
                    ok = menu::handle_key_with(&mut menu_state, menu::MenuKey::Down, &sub_state)
                        == menu::KeyResult::Changed;
                }
                if !ok || menu_state.sub_focus() != Some((parent, row)) {
                    eprintln!(
                        "{}",
                        tx::fill(
                            tx::DIAG_BAD_ARG,
                            &[(
                                "arg",
                                &format!(
                                    "--menu-sub-focus {parent},{row}（菜单 {mi} 第 {parent} 项没有这个子行，\
                                     或没展开/没有子菜单；当前 sub_focus={:?}）",
                                    menu_state.sub_focus()
                                )
                            )]
                        )
                    );
                }
            }
        }
        let overlay = if cli.shortcuts {
            Overlay::Shortcuts
        } else {
            Overlay::None
        };

        // ── 地图列表面板（2026-11-05，上游 `901dbe2f` 移植） ──
        //
        // 根：`--map-list-root` > 当前地图所在目录。扫描**立刻开始**（分帧推进，
        // 见 `maplist::Scanner`）：1,097 张实测 10~14 ms，摊到每帧 4 ms 只要 3~4 帧，
        // 所以"启动就开始扫"既不会拖慢开窗，也让 `--shot` 那一帧的列表**已经扫完**。
        let mut maps = MapListPanel::new();
        let maps_root_pinned = cli.map_list_root.is_some();
        if let Some(root) = maps_root(cli, &doc) {
            maps.start(&root);
        }

        Self {
            doc,
            view,
            edit,
            palette,
            stamps: StampPanel::new(),
            selected_tool,
            hover: None,
            forced_hover: cli.hover,
            // 视图 → 地砖信息悬浮：官方 `CellHoverToggle` 没写 `IsChecked` ⇒ **缺省关**；
            // 不持久化 ⇒ 每次启动都是关（不从设置文件读）。
            cell_hover: false,
            hover_anchor: None,
            lines: CellLines::unknown(),
            lines_key: None,
            text,
            pending_center: cli.center,
            centered: false,
            frames: 0,
            frame_gap_ms: 0.0,
            update_ms: 0.0,
            last_frame: None,
            last_canvas: Rect::NOTHING,
            canvas_size: None,
            preview_tex: None,
            preview_alt: cli.preview_alt,
            last_overlay: Rect::NOTHING,
            confirm_close: false,
            shown_title: String::new(),
            auto: Auto {
                shot: cli.shot.clone(),
                frames: cli.frames.unwrap_or(30),
                gate2_client: cli.gate2_client.clone(),
                gate2_at: cli.gate2_at,
                autodrag: cli.autodrag,
                palette_scroll: cli.palette_autoscroll,
                palette_report: cli.palette_report,
                stamp_report: cli.stamp_report,
                ..Default::default()
            },
            pending_stamp: Some(PendingStamp {
                kind: cli.stamp_kind.clone(),
                search: cli.stamp_search.clone(),
                page: cli.stamp_page,
                pick: cli.stamp_pick,
                arm: cli.stamp_arm || cli.stamp_place_at.is_some(),
                place_at: cli.stamp_place_at,
            }),
            armed: None,
            sel_drag: None,
            op: None,
            menu: menu_state,
            panels,
            dark,
            // 单元格网格初值：官方 `MainWindow.axaml:5` 的 `IsChecked="True"` ⇒ **默认开**。
            // 优先级 `--no-cell-grid` > `--cell-grid` > 默认 true（判据在
            // [`crate::cli::Cli::cell_grid_enabled`]，不是在这里另写一套）。
            grid: cli.cell_grid_enabled(),
            // 开启门的初值：`--open-doors` ⇒ true，缺省 false（闭门，与改动前逐字节相同）。
            // 不产生回执：回执会改状态栏文字，那样"开/关两张图的像素差"就不止落在门帧上。
            doors_open: cli.open_doors,
            // 地图光照初值：`--lighting <档>` ⇒ 那一档；缺省 Day（与改动前逐字节相同）。
            // 不产生回执：回执会改状态栏文字，那样"三档截图的像素差"就不止落在遮罩上。
            lighting: cli.lighting,
            overlay,
            settings_path,
            recent,
            pending_menu_clicks: {
                // `--menu-click` 的值可能带 `@<帧号>` 后缀（见 `cli::parse_menu_click`）。
                // 帧号晚于截图帧 ⇒ 这一条**永远不会执行**，必须当场说出来（不静默）。
                let shot_frame = cli.frames.unwrap_or(30);
                let mut q = Vec::with_capacity(cli.menu_clicks.len());
                for raw in &cli.menu_clicks {
                    let (frame, id) = crate::cli::parse_menu_click(raw);
                    if cli.shot.is_some() && frame > shot_frame {
                        eprintln!(
                            "{}",
                            tx::fill(
                                tx::DIAG_CLICK_TOO_LATE,
                                &[
                                    ("id", id),
                                    ("frame", &frame.to_string()),
                                    ("shot", &shot_frame.to_string()),
                                ]
                            )
                        );
                    }
                    q.push((frame, id.to_owned()));
                }
                q
            },
            pending_open_map: cli.open_map.clone(),
            maps,
            maps_root_pinned,
        }
    }

    /// 这一帧的**子菜单状态**（菜单绘制 / 命中 / 键盘共用同一份）。
    ///
    /// 为什么每次都现建：`SubState` 里只有 `recent` 是运行期数据（语言取官方缺省，
    /// 光照档位已提到 [`Self::lighting`] 真源，见 [`menu::model::SubState`] 的文档），
    /// 而 `recent` 必须与 `self.recent` **同一份**
    /// —— 否则画出来的行数与键盘能走的行数会不一致。
    fn sub_state(&self) -> menu::model::SubState {
        menu::model::SubState {
            recent: self.recent.clone(),
            ..menu::model::SubState::default()
        }
    }

    /// 当前窗口标题（= 标题栏文字；由真实文件名与格数装配）。
    pub fn window_title(&self) -> &str {
        &self.text.title
    }

    /// 按当前窗口尺寸算布局（地图列表面板开着时画布让出一列）。
    ///
    /// **`maps_open` 取自 `panels.get(PanelId::Maps)`** —— 与视图菜单勾选、`--show-maps`、
    /// `--panel-hide maps` 是同一个字段，不存在"菜单勾着但画布不让位"这种两套状态。
    fn layout(&self, screen: Rect) -> Layout {
        Layout::compute_with(screen, self.panels.get(PanelId::Maps))
    }

    /// 首次布局后把视口对准目标：有 `--center` 用它，否则对准地图中心。
    fn init_view(&mut self, l: &Layout) {
        if self.centered {
            return;
        }
        if let Some((x, y)) = self.pending_center {
            self.view.center_on_cell(l.viewport, x, y);
        } else if let Some(d) = self.doc.doc.as_ref() {
            self.view.center_on_cell(
                l.viewport,
                d.map.width as f32 * 0.5,
                d.map.height as f32 * 0.5,
            );
        }
        self.centered = true;
    }

    /// **窗口尺寸变化后修 fit 不变量**（A-012，对齐上游 `MapViewport.cs:169-179`）。
    ///
    /// 触发条件与上游 `OnSizeChanged` 相同 —— **画布矩形尺寸真的变了**才动手：
    ///
    /// | 情形 | 行为 |
    /// |---|---|
    /// | 首次布局（`canvas_size == None`） | 只**记住**尺寸，不重 fit（初次对准归 `init_view` / 开图时的 `fit_map`） |
    /// | 尺寸没变 | **什么都不做**（保证不每帧抢用户的手动缩放/平移） |
    /// | 尺寸变了 | 交给 [`CanvasView::refit_on_resize`]（它自带"当前倍数已经够大就不动"的 guard） |
    fn refit_after_resize(&mut self, l: &Layout) {
        let 当前 = l.viewport.size();
        match self.canvas_size {
            None => self.canvas_size = Some(当前),
            Some(旧) if (旧.x - 当前.x).abs() < 0.5 && (旧.y - 当前.y).abs() < 0.5 => {}
            Some(旧) => {
                self.canvas_size = Some(当前);
                if let Some(d) = self.doc.doc.as_ref() {
                    if self
                        .view
                        .refit_on_resize(旧, l.viewport, d.map.width, d.map.height)
                    {
                        eprintln!(
                            "[视口] 窗口尺寸变化 ⇒ 重适应：缩放 {:.0}%",
                            self.view.scale * 100.0
                        );
                    }
                }
            }
        }
    }

    /// 重渲调度：只在视口/图层/**渲染选项（门态 + 光照档）**/地图变化时真调渲染库
    /// （见 [`CanvasView::ensure`]）。
    fn ensure_texture(&mut self, ctx: &egui::Context, l: &Layout) {
        let map = self.doc.doc.as_ref().map(|d| &d.map);
        let src = self
            .doc
            .assets
            .source()
            .map(|s| s as &dyn SpriteSource);
        // `doors_open` / `lighting` 是**唯一**真源：菜单勾、缓存键、渲染选项、导出都读它们。
        // 打包成整份 `RenderOptions` 一次交给 `ensure`（它整体比较缓存键 + 原样传渲染库），
        // 两个 option 一起生效，不许拆成两条路径把门态弄丢。
        let options = RenderOptions::with_doors_and_lighting(self.doors_open, self.lighting);
        self.view
            .ensure(ctx, map, src, l.viewport, self.doc.map_gen, options);
    }

    /// 悬停格：`--hover` 优先，否则取真实指针位置并做地图范围校验。
    fn update_hover(&mut self, ctx: &egui::Context, l: &Layout) {
        let cell = if let Some((x, y)) = self.forced_hover {
            Some((i32::from(x), i32::from(y)))
        } else {
            ctx.pointer_hover_pos()
                .and_then(|p| self.view.screen_to_cell(l.viewport, p))
        };
        // 浮层锚点 = **真实指针**位置；`--hover` 伪造悬停时没有指针 ⇒ `None`
        // （画的时候改用该格中心，否则截图里的浮层会跑到窗口左上角）。
        self.hover_anchor = if self.forced_hover.is_some() {
            None
        } else {
            ctx.pointer_hover_pos()
        };
        self.hover = cell.and_then(|(x, y)| match self.doc.doc.as_ref() {
            Some(d) => {
                if x >= 0 && y >= 0 && x < i32::from(d.map.width) && y < i32::from(d.map.height) {
                    Some((x as u16, y as u16))
                } else {
                    None
                }
            }
            None => None,
        });
        self.refresh_cell_lines();
    }

    /// 悬停浮层的排版结果（`None` = 这一帧不画）。
    ///
    /// 三条前提**缺一不可**（官方 `CellHover.cs:31-42`）：开关勾着、指针在一个真格上、
    /// 该格在地图内。不满足就**一个像素都不画**（不画空框、不画"无数据"）。
    fn hover_panel(&self, l: &Layout) -> Option<cell_hover::Panel> {
        if !self.cell_hover {
            return None;
        }
        // **模态浮层打开时一个像素都不画**（A-008 P3，2026-11-07）：
        //
        // 上游的模态是**独立窗口**（`MainWindow.axaml` 里的对话框），悬停框永远不可能
        // 从它的边缘露出来；本仓的模态是画在同一张画布上的**浮层**，如果不拦，
        // 对话框旁边会露出半截悬停面板（错位的六行信息 + 压在对话框上的边框）。
        //
        // 判据用 `Overlay::is_some()` 而不是逐个变体列举：**将来新增的模态自动被拦**，
        // 不需要记得回来改这里（这正是"按开关真值判"而不是"按枚举清单判"的理由）。
        if self.overlay.is_some() {
            return None;
        }
        let (x, y) = self.hover?;
        let d = self.doc.doc.as_ref()?;
        let cell = d.map.cell(x, y)?;
        // 锚点：真实指针优先；没有指针（`--hover` 取证）⇒ 该格中心
        let anchor = self.hover_anchor.unwrap_or_else(|| {
            self.view
                .cell_rect(l.viewport, i32::from(x), i32::from(y))
                .center()
        });
        let text = cellinfo::hover_text(x, y, cell, &self.lines);
        cell_hover::layout(&text, anchor, l.content)
    }

    /// 悬停格变了才重算六行信息（它要问图库要帧尺寸，每帧重算会把鼠标拖卡）。
    fn refresh_cell_lines(&mut self) {
        let key = self.hover.map(|(x, y)| (x, y, self.doc.map_gen));
        if key == self.lines_key {
            return;
        }
        self.lines_key = key;
        self.lines = match (self.hover, self.doc.doc.as_ref()) {
            (Some((x, y)), Some(d)) => {
                let src = self.doc.assets.source().map(|s| s as &dyn SpriteSource);
                cellinfo::cell_lines(x, y, d.map.cell(x, y), d.map.format, src)
            }
            _ => CellLines::unknown(),
        };
    }

    // ───────────────────────── 编辑：快捷键 / 落格 / 保存 ─────────────────────────

    /// 改写地图后统一走这里：**地图代号 +1** 且**强刷单元格信息**。
    ///
    /// 为什么用 `map_gen`：画布纹理缓存与单元格信息的缓存键里都有它
    /// （见 [`CanvasView::ensure`] 与 [`Self::refresh_cell_lines`]），
    /// 于是"改了图就重渲一次"是自动的，而且**只在真改过的帧里发生**——
    /// 不会每帧重渲（重渲一次在 0.25× 下要出 12000 格，约 20~40 ms）。
    /// **提交「单元格属性」模态的值**（R12-F1）：返回 `(状态栏回执, 是否警示色)`。
    ///
    /// 语义全在 `miredit_core::EditSession::apply_metadata`（三者全空 ⇒ 不动；
    /// 区域夹紧；只写给了的字段；值没变跳过；整批 = 一步撤销）。
    /// 本方法只做三件宿主的事：① 把 `DocState.map` 交给会话；② 改了图就 `touch_map()`
    /// （画布缓存键 + 单元格信息都靠它）；③ 折一句回执（内核的 `note` 原样附在后面，
    /// 例如 Mir3 上"门字段被忽略"那句）。
    fn apply_cell_props(
        &mut self,
        light: Option<u8>,
        door: Option<u8>,
        offset: Option<u8>,
    ) -> (String, bool) {
        let Overlay::CellProps { area, .. } = self.overlay else {
            // 只有模态开着才会走到这里；关着就是"没有目标"，直接说清而不是静默
            return (tx::CELL_PROPS_NO_TARGET.to_owned(), true);
        };
        let Some(d) = self.doc.doc.as_mut() else {
            return (tx::CELL_PROPS_NO_TARGET.to_owned(), true);
        };
        let n = self
            .edit
            .apply_metadata(&mut d.map, light, door, offset, area);
        let mut text = if n == 0 {
            tx::CELL_PROPS_NOOP.to_owned()
        } else {
            tx::fill(tx::CELL_PROPS_DONE_TMPL, &[("n", &n.to_string())])
        };
        if let Some(note) = self.edit.note() {
            text.push_str("；另：");
            text.push_str(note);
        }
        if n > 0 {
            self.touch_map();
            self.refresh_cell_lines();
        }
        (text, n == 0)
    }

    fn touch_map(&mut self) {
        self.doc.map_gen = self.doc.map_gen.wrapping_add(1);
        self.lines_key = None;
    }

    /// 把「地表落笔被前端拦下」这件事**同时**报到 stderr 与状态栏回执段。
    ///
    /// 为什么要两个出口：状态栏只在窗口里看得见，而无人值守验收读的是 stderr ——
    /// "拦下了"必须有**可机器抓**的输出，否则只能靠人工看截图（与 [`tx::DIAG_EDIT_REJECT`]
    /// 同一条纪律）。
    ///
    /// ⚠ 回执用**警示色**（`warn = true`）：这是一次被拒绝的操作，不是成功回执。
    fn report_ground_blocked(&mut self, why: &str) {
        let fmt = self
            .doc
            .doc
            .as_ref()
            .map(|d| d.format_label())
            .unwrap_or(tx::VAL_UNKNOWN);
        eprintln!(
            "{}",
            tx::fill(tx::DIAG_EDIT_GROUND_BLOCKED, &[("fmt", fmt), ("why", why)])
        );
        self.op = Some((why.to_owned(), true));
    }

    /// `Ctrl+Z` / `Ctrl+Y` / `Ctrl+S` / `Esc` + 工具快捷键（菜单栏的下拉另单做）。
    ///
    /// 工具快捷键**照官方**（`MainWindow.axaml.cs:861`）：
    /// `Key.F => EditorTool.Bucket` —— 本单只接 `F`（填充）；
    /// 笔刷 `B` / 铅笔 `P` / 橡皮 `E` / 吸管 `V` 官方也有，但本仓的图标顺序与官方不同，
    /// 逐个接会引入"按 B 选到第 0 个还是第几个"的新歧义，故只接本单新增的那一个。
    ///
    /// 笔刷尺寸 `[` / `]`：官方 `MainWindow.axaml.cs:871`
    /// `BrushSize.Value = Math.Clamp(BrushSize.Value + (OemOpenBrackets ? -2 : 2), 3, 31);`
    /// ⇒ 一次走 2 格、夹在 3..31（见 [`EditSession::grow_brush_size`]）。
    fn handle_keys(&mut self, ctx: &egui::Context) {
        // ⓪ **浮层优先**：打开地图模态 / 快捷键浮层在的时候，键盘整体让给它
        //    （只留 Esc/回车/退格/文本，由 `menu::dialog` 处理）。
        //    不让位的话，敲路径里的 `f` 会切工具、`o` 会切印章 —— 那是真实踩过的坑。
        if self.overlay.is_some() {
            return;
        }
        // ⓪b **菜单键盘**：弹层展开时 ↑↓←→ / Enter / Esc 归菜单，且**吃掉这次按键**
        //     （否则 Esc 会顺带清选区、↑↓ 会滚画布）。
        if self.menu.open_index().is_some() {
            let key = ctx.input(|i| {
                if i.key_pressed(egui::Key::Escape) {
                    Some(menu::MenuKey::Escape)
                } else if i.key_pressed(egui::Key::ArrowDown) {
                    Some(menu::MenuKey::Down)
                } else if i.key_pressed(egui::Key::ArrowUp) {
                    Some(menu::MenuKey::Up)
                } else if i.key_pressed(egui::Key::ArrowLeft) {
                    Some(menu::MenuKey::Left)
                } else if i.key_pressed(egui::Key::ArrowRight) {
                    Some(menu::MenuKey::Right)
                } else if i.key_pressed(egui::Key::Enter) {
                    Some(menu::MenuKey::Enter)
                } else {
                    None
                }
            });
            if let Some(k) = key {
                let canvas = self.last_canvas;
                // 子菜单状态（`recent` 决定「最近打开」有几行可走）——
                // 键盘与绘制必须问**同一份**，否则 ↑↓ 会走到不存在的行上。
                let sub = self.sub_state();
                match menu::handle_key_with(&mut self.menu, k, &sub) {
                    menu::KeyResult::Run(a) => self.dispatch_menu(a, canvas),
                    // 回车选了一条最近地图 ⇒ 打开**那一条**（与鼠标点击同一条路）
                    menu::KeyResult::RunRecent(i) => self.open_recent_at(i, canvas),
                    menu::KeyResult::Changed => {}
                    menu::KeyResult::None => {}
                }
                ctx.request_repaint();
                return;
            }
        }
        // ⓪c `F1`：帮助 → 键盘快捷键（官方手势，见 `Action::KeyboardShortcuts`）
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            let canvas = self.last_canvas;
            self.dispatch_menu(menu::model::Action::KeyboardShortcuts, canvas);
            ctx.request_repaint();
            return;
        }
        // ⓪d `Ctrl+O`：文件 → 打开地图（官方手势）
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            let canvas = self.last_canvas;
            self.dispatch_menu(menu::model::Action::OpenMap, canvas);
            ctx.request_repaint();
            return;
        }
        // ⓪e `G`：视图 → 单元格网格（官方手势 `MainWindow.axaml` 的 `InputGesture="G"`）。
        //
        // 只认**不带修饰键**的裸 `G`：`Ctrl+G` / `Alt+G` 是别的程序里的习惯（本仓也没绑），
        // 抢过来只会让人误触。走的仍是 `dispatch_menu` ⇒ 与鼠标点菜单项**同一条**路径
        // （回执、诊断行、状态翻转全一致，不存在"键盘能开、菜单开不了"）。
        if ctx.input(|i| {
            !i.modifiers.command && !i.modifiers.alt && !i.modifiers.shift
                && i.key_pressed(egui::Key::G)
        }) {
            let canvas = self.last_canvas;
            self.dispatch_menu(menu::model::Action::CellGrid, canvas);
            ctx.request_repaint();
            return;
        }
        let (undo, redo, save, save_as, esc) = ctx.input(|i| {
            (
                // 撤销：`Ctrl+Z`，且**不带 Shift**（带 Shift 是重做）
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
                // 重做：官方 `Ctrl+Shift+Z`，本仓另接受旧键 `Ctrl+Y`（两者都真实可用，
                // 快捷键浮层里也是这么写的）
                i.modifiers.command
                    && ((i.modifiers.shift && i.key_pressed(egui::Key::Z))
                        || i.key_pressed(egui::Key::Y)),
                // 保存：`Ctrl+S`，**不带 Shift**（带 Shift 是另存为，见下一条）
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S),
                // **另存为**：官方 `Ctrl+Shift+S`（`MainWindow.axaml` 的 `InputGesture`）。
                // 2026-11-05 起真绑定：走与鼠标点「文件 → 另存为…」**完全相同**的
                // `dispatch_menu(Action::SaveAs)`（打开同一个路径输入模态），不另写一条捷径。
                i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S),
                i.key_pressed(egui::Key::Escape),
            )
        });
        // 工具 / 尺寸快捷键：带 Ctrl/Alt 时不算（那是别的命令的修饰键）
        let (fill_key, select_key, stamp_key, shrink, grow) = ctx.input(|i| {
            let plain = !i.modifiers.command && !i.modifiers.alt;
            (
                plain && i.key_pressed(egui::Key::F),
                // 官方 `MainWindow.axaml.cs:861`：`Key.M => EditorTool.Select`、`Key.O => EditorTool.Stamp`
                plain && i.key_pressed(egui::Key::M),
                plain && i.key_pressed(egui::Key::O),
                plain && i.key_pressed(egui::Key::OpenBracket),
                plain && i.key_pressed(egui::Key::CloseBracket),
            )
        });
        if fill_key {
            self.selected_tool = edit::TOOL_BUCKET_IDX;
            self.edit.select(edit::TOOL_BUCKET_IDX);
            ctx.request_repaint();
        }
        // `M` / `O`：选择 / 印章（两支宿主层工具，内核 `Tool` 里没有；见 `selection_ops` §一）
        for (hit, slot) in [
            (select_key, selection_ops::TOOL_SELECT_IDX),
            (stamp_key, selection_ops::TOOL_STAMP_IDX),
        ] {
            if hit {
                self.selected_tool = slot;
                self.edit.select(slot);
                ctx.request_repaint();
            }
        }
        if shrink || grow {
            // `[` 减 2、`]` 加 2（官方同一行代码里的三元表达式）
            let delta = if shrink {
                -(m::BRUSH_STEP as isize)
            } else {
                m::BRUSH_STEP as isize
            };
            let before = self.edit.brush_size();
            let now = self.edit.grow_brush_size(delta);
            eprintln!(
                "{}",
                tx::fill(tx::DIAG_EDIT_SIZE, &[("n", &now.to_string())])
            );
            // ⚠ **不 `touch_map()`**：改尺寸不改地图一个字节 ⇒ 不该让画布丢纹理缓存重渲一遍
            //    （见 [`EditSession::set_brush_size`] 的文档）。只重画界面（胶囊文字变了）。
            if now != before {
                ctx.request_repaint();
            }
        }
        // ── 工具键 `B/P/E/V/K` + 阻挡开关 `C` + 平移 `W/A/S/D`（R9-F2，2026-11-07） ──
        //
        // 上游 `MainWindow.axaml.cs:989`：`B=>Brush / P=>Pencil / F=>Bucket / E=>Eraser /
        // M=>Select / O=>Stamp / V=>Pickup / K=>Clone`；`:997` 单独一条 `C`：
        // `bool show = !Viewport.CollisionVisible; LowCollisionVisible.IsChecked = show;
        //  HighCollisionVisible.IsChecked = show;`（`MapViewport.cs:82-85`：getter = 低 **或** 高，
        //  setter 同时写两边）；`:978-984` 的 `W/A/S/D` 走 `Viewport.PanBy(...)` 每次 **48 屏幕像素**。
        //
        // 三条纪律（别改）：
        // ① 全部**裸键**（`plain` = 不带 Ctrl/Alt）—— `Ctrl+C` 是复制、`Ctrl+S` 是保存、
        //    `Ctrl+Shift+V` 是填充选区，它们各有归属，与这三个键不冲突；
        // ② `C` **只切阻挡通道、不动当前工具**（上游同一条：只改两个 CheckBox）；
        // ③ 平移按**屏幕像素**（`pan_by_screen`），不按格 —— 缩放 25% 时一次 W 也只走 48 屏幕像素；
        // ④ 一律用 `key_pressed`（= 一次按下走一步），**不按帧**：按住不放只走一步，
        //    连按才连走（上游 WinForms 的 `KeyDown` 同语义，不靠 `repeat`）。
        // R40 守卫：这些"裸键"只在真正该归画布时生效 ——
        // ① `--shot` / `--autodrag` 锁死输入（[`Self::input_locked`]）；
        // ② 两个搜索框有焦点时键盘归它们（否则在搜索框里敲 `B` 会顺手换工具）；
        // ③ 浮层 / 菜单已由本函数开头让位（`overlay.is_some()` / `menu.open_index()`）。
        let 键位让位 =
            self.input_locked() || self.palette.focused() || self.stamps.focused();
        let (工具键, 切阻挡, 平移) = ctx.input(|i| {
            // `!command` 分流 `Ctrl+V/C/S`（粘贴/复制/保存各有归属）；`Alt` 也跳过（上游同款）
            let plain = !i.modifiers.command && !i.modifiers.alt && !键位让位;
            let 工具键 = if !plain {
                None
            } else if i.key_pressed(egui::Key::B) {
                Some(miredit_core::TOOL_BRUSH_IDX)
            } else if i.key_pressed(egui::Key::P) {
                // ⚠ 铅笔尚未接线（`Tool::Unwired`）：键只切槽位，状态栏如实说"未接线"
                Some(edit::TOOL_PENCIL_IDX)
            } else if i.key_pressed(egui::Key::E) {
                Some(miredit_core::TOOL_ERASER_IDX)
            } else if i.key_pressed(egui::Key::V) {
                Some(miredit_core::TOOL_PICKUP_IDX)
            } else if i.key_pressed(egui::Key::K) {
                // ⚠ 克隆尚未接线，同铅笔
                Some(edit::TOOL_CLONE_IDX)
            } else {
                None
            };
            // 平移：符号照抄上游（W=(0,+48) / A=(+48,0) 作用在"相机原点"上）
            // ⇒ 换算到本仓"视口左上角的世界像素"时四条边一起取反。
            let 平移 = if !plain {
                (0.0, 0.0)
            } else if i.key_pressed(egui::Key::W) {
                (0.0, -m::PAN_STEP_PX)
            } else if i.key_pressed(egui::Key::S) {
                (0.0, m::PAN_STEP_PX)
            } else if i.key_pressed(egui::Key::A) {
                (-m::PAN_STEP_PX, 0.0)
            } else if i.key_pressed(egui::Key::D) {
                (m::PAN_STEP_PX, 0.0)
            } else {
                (0.0, 0.0)
            };
            (工具键, plain && i.key_pressed(egui::Key::C), 平移)
        });
        if let Some(slot) = 工具键 {
            self.selected_tool = slot;
            self.edit.select(slot);
            ctx.request_repaint();
        }
        if 切阻挡 {
            // 上游 `MapViewport.cs:82-85`：`CollisionVisible` getter = 低 **或** 高，setter 同写两边
            let show = !(self.view.overlay.low || self.view.overlay.high);
            self.view.overlay.low = show;
            self.view.overlay.high = show;
            let text = if show {
                tx::MENU_COLLISION_ON
            } else {
                tx::MENU_COLLISION_OFF
            };
            eprintln!(
                "{}",
                tx::fill(
                    tx::DIAG_KEY_ACTION,
                    &[("key", "C"), ("what", text)]
                )
            );
            // 成功回执（阻断标记不是"改地图"，用正文色）
            self.op = Some((text.to_owned(), false));
            // 阻挡通道在画布缓存键里（`canvas_view` 的 `Cache.overlay`）⇒ 重画即可重渲
            ctx.request_repaint();
        }
        if 平移 != (0.0, 0.0) {
            self.view.pan_by_screen(平移.0, 平移.1);
            ctx.request_repaint();
        }
        // ── 选区 / 剪贴板快捷键（2026-11-05） ──
        //
        // 判定顺序有因果：`Ctrl+Shift+V`（填充选区）必须**先于** `Ctrl+V`（粘贴）判，
        // 否则 shift 按住时两个条件同时成立，会先粘贴再填充。
        self.handle_sel_keys(ctx);
        // ── 印章待放置态：`Enter` 进入 / `Esc` 退出 ──
        self.handle_arm_keys(ctx);
        if esc {
            self.confirm_close = false;
            // `Esc` 的第三层语义：取消选区（官方 `Deselect`，`MapViewport.cs:103-108`）。
            // 顺序：先关模态 → 再退待放置态 → 最后才清选区（越"重"的越先撤）。
            if self.armed.is_none() && self.edit.selection().is_some() {
                self.edit.clear_selection();
                ctx.request_repaint();
            }
        }
        if undo {
            // ⚠ 走 `apply_undo_like`（不是裸 `edit.undo`）：尺寸变化是整档步骤，
            // 撤销越过它时**保存源**也要回旧尺寸，否则保存会因形状不符被拒（R37 P2）。
            let n = match self.doc.doc.as_mut() {
                Some(d) => self.edit.apply_undo_like(d, false),
                None => 0,
            };
            // 撤销之后"上一次操作的回执"就不再成立了（它描述的那一步已经没了）
            self.op = None;
            if n > 0 {
                self.touch_map();
            }
        }
        if redo {
            let n = match self.doc.doc.as_mut() {
                Some(d) => self.edit.apply_undo_like(d, true),
                None => 0,
            };
            self.op = None;
            if n > 0 {
                self.touch_map();
            }
        }
        if save {
            self.do_save();
        }
        if save_as {
            // 与菜单项同一条路径：打开「另存为」模态（输完路径回车才真写盘）
            let canvas = self.last_canvas;
            self.dispatch_menu(menu::model::Action::SaveAs, canvas);
            ctx.request_repaint();
        }
    }

    // ───────────────────── 菜单系统 / 深色模式（2026-11-05） ─────────────────────

    /// **执行一条菜单动作**（鼠标点击、键盘回车、`--menu-click` 三条路径的唯一出口）。
    ///
    /// 真正干活的判定在 [`crate::menu::action::apply`]；这里只负责
    /// ① 拼上下文 ② 把结果落到宿主状态（回执 / 重渲 / 诊断行）。
    fn dispatch_menu(&mut self, action: menu::model::Action, canvas: Rect) {
        let out = {
            let mut cx = menu::action::Ctx {
                doc: &mut self.doc,
                view: &mut self.view,
                edit: &mut self.edit,
                panels: &mut self.panels,
                dark: &mut self.dark,
                grid: &mut self.grid,
                cell_hover: &mut self.cell_hover,
                doors_open: &mut self.doors_open,
                lighting: &mut self.lighting,
                canvas,
                overlay: &mut self.overlay,
                settings_path: &self.settings_path,
                recent: &mut self.recent,
                tool: self.selected_tool,
                hover: self.hover,
            };
            menu::action::apply(action, &mut cx)
        };
        self.finish_menu(action, out);
    }

    /// 把一次菜单动作的结果落到宿主状态。
    ///
    /// ⚠ **灰显项也要有诊断行**：`--menu-click file.exit` 这种"点了但没做"必须能被
    /// 脚本抓到（验收门 §四.4 的机器判据就是这一行 + 状态栏那句）。
    fn finish_menu(&mut self, action: menu::model::Action, out: menu::action::Outcome) {
        let result = match &out.receipt {
            Some((text, warn)) => format!("{text}{}", if *warn { "（警示）" } else { "" }),
            None => "无回执".to_owned(),
        };
        eprintln!(
            "{}",
            tx::fill(
                tx::DIAG_MENU_CLICK,
                &[
                    ("id", action.id()),
                    ("label", action.label()),
                    ("result", &result),
                ]
            )
        );
        if out.touched_map {
            self.touch_map();
        }
        if let Some((text, warn)) = out.receipt {
            self.op = Some((text, warn));
        }
    }

    /// 打开一张地图（模态里点「打开」/ 回车）。
    fn open_map_path(&mut self, path: &str, canvas: Rect) {
        let out = {
            let mut cx = menu::action::Ctx {
                doc: &mut self.doc,
                view: &mut self.view,
                edit: &mut self.edit,
                panels: &mut self.panels,
                dark: &mut self.dark,
                grid: &mut self.grid,
                cell_hover: &mut self.cell_hover,
                doors_open: &mut self.doors_open,
                lighting: &mut self.lighting,
                canvas,
                overlay: &mut self.overlay,
                settings_path: &self.settings_path,
                recent: &mut self.recent,
                tool: self.selected_tool,
                hover: self.hover,
            };
            menu::action::open_path(&mut cx, path)
        };
        self.finish_menu(menu::model::Action::OpenMap, out);
        // 打开成功（文档真的换了）⇒ 地图列表跟着换目录；失败不动（当前图还在）
        if self.doc.doc.is_some() {
            self.follow_map_dir();
        }
    }

    /// **地图列表跟着当前地图换目录**（只在根没被 `--map-list-root` 钉住时）。
    ///
    /// 为什么要有它：列表根默认取"当前地图所在目录"。用户从模态打开了**别的目录**的图
    /// 之后，若列表还停在旧目录，点一行就会跳回旧目录 —— 那不是"列表"该有的行为。
    /// 换根是**重新扫**（`MapListPanel::start`），不是增量补几条：旧条目的相对路径
    /// 全都失效了，留着只会显示错名字。
    fn follow_map_dir(&mut self) {
        if self.maps_root_pinned {
            return;
        }
        let Some(d) = self.doc.doc.as_ref() else {
            return;
        };
        let Some(dir) = d.path.parent() else {
            return;
        };
        if dir.as_os_str().is_empty() {
            return;
        }
        let same = self
            .maps
            .root()
            .map(|r| maplist::same_path(r, dir))
            .unwrap_or(false);
        if !same {
            let root = dir.to_path_buf();
            self.maps.start(&root);
        }
    }

    /// **打开「最近打开」子菜单里的第 `idx` 行**（鼠标点击 / 回车都走这里）。
    ///
    /// 与其它菜单动作同一套骨架：拼上下文 → 调 [`menu::action::open_recent_index`]
    /// → 把回执落到状态栏 + 打一行诊断（`[菜单] 点击 file.open_recent（…）：…`）。
    /// 真正干活的判定全在 `menu::action` 里：文件还在 ⇒ 走与「打开地图…」**完全相同**的
    /// `open_path`；文件已不存在 ⇒ 状态栏给中文原因 + 从列表里移除。
    fn open_recent_at(&mut self, idx: usize, canvas: Rect) {
        let out = {
            let mut cx = menu::action::Ctx {
                doc: &mut self.doc,
                view: &mut self.view,
                edit: &mut self.edit,
                panels: &mut self.panels,
                dark: &mut self.dark,
                grid: &mut self.grid,
                cell_hover: &mut self.cell_hover,
                doors_open: &mut self.doors_open,
                lighting: &mut self.lighting,
                canvas,
                overlay: &mut self.overlay,
                settings_path: &self.settings_path,
                recent: &mut self.recent,
                tool: self.selected_tool,
                hover: self.hover,
            };
            menu::action::open_recent_index(&mut cx, idx)
        };
        self.finish_menu(menu::model::Action::OpenRecent, out);
    }

    /// **另存为到指定路径**（模态里点「另存为」/ 回车）。
    ///
    /// 与 [`Self::open_map_path`] 同一套骨架：拼上下文 → 调
    /// [`menu::action::save_as_path`] → 把回执落到状态栏 + 打一行诊断。
    /// 真正的写盘与"改认新路径"在 [`crate::save::save_as_session`] 里，
    /// 这里一行写盘逻辑都没有。
    fn save_as_path(&mut self, path: &str, canvas: Rect) {
        let out = {
            let mut cx = menu::action::Ctx {
                doc: &mut self.doc,
                view: &mut self.view,
                edit: &mut self.edit,
                panels: &mut self.panels,
                dark: &mut self.dark,
                grid: &mut self.grid,
                cell_hover: &mut self.cell_hover,
                doors_open: &mut self.doors_open,
                lighting: &mut self.lighting,
                canvas,
                overlay: &mut self.overlay,
                settings_path: &self.settings_path,
                recent: &mut self.recent,
                tool: self.selected_tool,
                hover: self.hover,
            };
            menu::action::save_as_path(&mut cx, path)
        };
        self.finish_menu(menu::model::Action::SaveAs, out);
    }

    /// **导出选区到指定路径**（导出选区模态里点「导出」/ 回车）。
    ///
    /// 与 [`Self::save_as_path`] 同一套骨架：拼上下文 → 调
    /// [`menu::action::export_selection_path`] → 把回执落到状态栏 + 打一行诊断。
    /// 渲染/分块/PNG 编码/原子写全在 [`crate::export`]，这里一行都没有。
    fn export_selection_path(&mut self, path: &str, canvas: Rect) {
        let out = {
            let mut cx = menu::action::Ctx {
                doc: &mut self.doc,
                view: &mut self.view,
                edit: &mut self.edit,
                panels: &mut self.panels,
                dark: &mut self.dark,
                grid: &mut self.grid,
                cell_hover: &mut self.cell_hover,
                doors_open: &mut self.doors_open,
                lighting: &mut self.lighting,
                canvas,
                overlay: &mut self.overlay,
                settings_path: &self.settings_path,
                recent: &mut self.recent,
                tool: self.selected_tool,
                hover: self.hover,
            };
            menu::action::export_selection_path(&mut cx, path)
        };
        self.finish_menu(menu::model::Action::ExportSelection, out);
    }

    /// **导出小地图到指定路径**（导出小地图模态里点「导出」/ 回车）。
    ///
    /// `size` = 模态里校验过的尺寸口径（默认 `Preset(Small)`，手改宽高 ⇒ `Explicit`）；
    /// 尺寸校验在模态里已经做过，这里只把它透传给 [`menu::action::export_minimap_path`]。
    fn export_minimap_path(
        &mut self,
        path: &str,
        size: crate::export::MinimapSize,
        canvas: Rect,
    ) {
        let out = {
            let mut cx = menu::action::Ctx {
                doc: &mut self.doc,
                view: &mut self.view,
                edit: &mut self.edit,
                panels: &mut self.panels,
                dark: &mut self.dark,
                grid: &mut self.grid,
                cell_hover: &mut self.cell_hover,
                doors_open: &mut self.doors_open,
                lighting: &mut self.lighting,
                canvas,
                overlay: &mut self.overlay,
                settings_path: &self.settings_path,
                recent: &mut self.recent,
                tool: self.selected_tool,
                hover: self.hover,
            };
            menu::action::export_minimap_path(&mut cx, path, size)
        };
        self.finish_menu(menu::model::Action::ExportMinimap, out);
    }

    /// 处理浮层的输入（打开地图模态 / 另存为模态 / 快捷键浮层）；返回是否**吃掉了这一帧的交互**。
    fn handle_overlay(&mut self, ui: &mut egui::Ui, l: &Layout) -> bool {
        if !self.overlay.is_some() {
            return false;
        }
        let mut act = menu::dialog::DialogAction::None;
        if matches!(self.overlay, Overlay::OpenMap { .. }) {
            act = menu::dialog::interact_open_map(ui, l, &mut self.overlay);
        } else if matches!(self.overlay, Overlay::SaveAs { .. }) {
            // 另存为模态：与打开模态**同一套**输入/校验/回车提交（见 `menu::dialog`）
            act = menu::dialog::interact_save_as(ui, l, &mut self.overlay);
        } else if matches!(self.overlay, Overlay::ExportSelection { .. }) {
            // 导出选区模态：同一套输入，提交后走 `crate::export::export_selection`
            act = menu::dialog::interact_export_selection(ui, l, &mut self.overlay);
        } else if matches!(self.overlay, Overlay::ExportMinimap { .. }) {
            // 导出小地图模态：同一套路径输入 + **尺寸输入**（预设三选一 / 自定义宽高）。
            // 地图格数每帧从文档取：选预设时按它算像素（官方 `document.Width*48/divisor`）。
            let cells = self
                .doc
                .doc
                .as_ref()
                .map(|d| (u32::from(d.map.width), u32::from(d.map.height)))
                .unwrap_or((0, 0));
            act = menu::dialog::interact_export_minimap(ui, l, &mut self.overlay, cells);
        } else if matches!(self.overlay, Overlay::CellProps { .. }) {
            // 单元格属性模态（R12-F1）：三个数字框；Esc/取消关窗，回车/应用提交。
            // 提交**不在这里做**：先把动作记下来，下面统一 match 时再落到会话（与其它模态同一套）。
            match menu::cellprops::interact(ui, l, &mut self.overlay) {
                menu::cellprops::动作::None => {}
                menu::cellprops::动作::Cancel => act = menu::dialog::DialogAction::Cancel,
                menu::cellprops::动作::Apply {
                    light,
                    door,
                    offset,
                } => {
                    (self.op, self.overlay) = (None, Overlay::None);
                    let (text, warn) = self.apply_cell_props(light, door, offset);
                    self.op = Some((text, warn));
                    ui.ctx().request_repaint();
                }
            }
        } else if matches!(self.overlay, Overlay::ExpandMap { .. }) {
            // 地图扩边模态（A-015 Expand）：Esc / 取消 ⇒ 关；「地图扩边」⇒ 交给内核改尺寸。
            let cells = self
                .doc
                .doc
                .as_ref()
                .map(|d| (d.map.width, d.map.height))
                .unwrap_or((0, 0));
            match menu::expand::interact(ui, l, &mut self.overlay, cells) {
                menu::expand::动作::None => {}
                menu::expand::动作::Cancel => act = menu::dialog::DialogAction::Cancel,
                menu::expand::动作::Apply {
                    left,
                    top,
                    right,
                    bottom,
                } => {
                    let 结果 = {
                        let mut cx = menu::action::Ctx {
                            doc: &mut self.doc,
                            view: &mut self.view,
                            edit: &mut self.edit,
                            panels: &mut self.panels,
                            dark: &mut self.dark,
                            grid: &mut self.grid,
                            cell_hover: &mut self.cell_hover,
                            doors_open: &mut self.doors_open,
                            lighting: &mut self.lighting,
                            canvas: l.viewport,
                            overlay: &mut self.overlay,
                            settings_path: &self.settings_path,
                            recent: &mut self.recent,
                            tool: self.selected_tool,
                            hover: self.hover,
                        };
                        menu::action::expand_apply(&mut cx, left, top, right, bottom)
                    };
                    match 结果 {
                        Ok(out) => {
                            // 成功：关模态 + 落回执（`touched_map` ⇒ `map_gen+1`）
                            self.finish_menu(menu::model::Action::ExpandMap, out);
                            self.overlay = Overlay::None;
                        }
                        Err(why) => {
                            // 失败：**不关窗**，把原因写进模态摘要（上游 `IsEnabled=false` 同义）
                            if let Overlay::ExpandMap { error, .. } = &mut self.overlay {
                                *error = Some(why);
                            }
                        }
                    }
                    ui.ctx().request_repaint();
                }
            }
        } else if matches!(self.overlay, Overlay::CheckMap { .. }) {
            // 检查地图模态：Esc / 关闭 ⇒ 收浮层；点行 ⇒ 选中；「定位」⇒ 画布居中 +
            // 1×1 选区 + 关模态（官方 `LocateCell(issue.X, issue.Y)` + `UpdateCellInspector`）。
            match menu::check_map::interact(ui, l, &mut self.overlay, self.doc.map_gen) {
                menu::check_map::动作::None | menu::check_map::动作::Select(_) => {
                    // 选中态已写进浮层，只需重绘
                    ui.ctx().request_repaint();
                }
                menu::check_map::动作::Close => act = menu::dialog::DialogAction::Cancel,
                menu::check_map::动作::Locate { x, y } => {
                    (self.op, self.overlay) = (None, Overlay::None);
                    let text = self.locate_cell(x, y, l.viewport);
                    self.op = Some((text, false));
                    ui.ctx().request_repaint();
                }
            }
        } else if matches!(self.overlay, Overlay::Shortcuts | Overlay::About) {
            // 只读浮层（键盘快捷键 / 关于 Crystal Studio）：任意点击 / Esc 关闭。
            //
            // ⚠ 关于对话框**不走 egui::Window**（与快捷键浮层同一套自绘模态）：
            // 它是"看一眼就关"的只读面板，没有输入框、没有提交按钮 ⇒ 点击任意处即关，
            // 不阻塞主编辑（关掉立刻能继续画）。
            if ui.input(|i| i.pointer.any_click()) || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                act = menu::dialog::DialogAction::Cancel;
            }
        }
        match act {
            menu::dialog::DialogAction::None => {}
            menu::dialog::DialogAction::Cancel => {
                self.overlay = Overlay::None;
                ui.ctx().request_repaint();
            }
            menu::dialog::DialogAction::Open(path) => {
                self.open_map_path(&path, l.viewport);
                ui.ctx().request_repaint();
            }
            menu::dialog::DialogAction::SaveAs(path) => {
                self.save_as_path(&path, l.viewport);
                ui.ctx().request_repaint();
            }
            menu::dialog::DialogAction::ExportSelection(path) => {
                self.export_selection_path(&path, l.viewport);
                ui.ctx().request_repaint();
            }
            menu::dialog::DialogAction::ExportMinimap { path, size } => {
                self.export_minimap_path(&path, size, l.viewport);
                ui.ctx().request_repaint();
            }
        }
        true
    }

    /// `Ctrl+C` / `Ctrl+X` / `Ctrl+V` / `Delete` / `Ctrl+Shift+V` —— 选区与剪贴板的五个命令。
    ///
    /// 全部走 [`crate::selection_ops::apply`]（唯一实现），本方法只负责
    /// ① 把键折成 [`SelCmd`]、② 取鼠标格、③ 把回执落到状态栏 + 需要时重渲。
    ///
    /// 官方依据逐条见 [`crate::selection_ops`] 模块文档 §二（`Ctrl+Shift+V` 是本仓自定）。
    fn handle_sel_keys(&mut self, ctx: &egui::Context) {
        let (copy, cut, paste, del, fill_sel) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            (
                cmd && !i.modifiers.shift && i.key_pressed(egui::Key::C),
                cmd && !i.modifiers.shift && i.key_pressed(egui::Key::X),
                cmd && !i.modifiers.shift && i.key_pressed(egui::Key::V),
                !cmd && !i.modifiers.alt && i.key_pressed(egui::Key::Delete),
                // ⚠ **无官方依据**：官方没有给"填充选区"绑键（见 `selection_ops` §二）
                cmd && i.modifiers.shift && i.key_pressed(egui::Key::V),
            )
        });
        let cmd = if fill_sel {
            Some(SelCmd::Fill)
        } else if copy {
            Some(SelCmd::Copy)
        } else if cut {
            Some(SelCmd::Cut)
        } else if paste {
            Some(SelCmd::Paste)
        } else if del {
            Some(SelCmd::Delete)
        } else {
            None
        };
        let Some(cmd) = cmd else {
            return;
        };
        // 粘贴传的是**鼠标格**（点击点）；真正的落点由内核按上游 `RegionCopy.cs:70` 的 `Anchor`
        // 重算（地表可见时按源的奇偶往左上对齐，负锚点整块拒绝）—— GUI 不做第二次对齐，
        // 以免出现"界面预览/回执一套、落笔另一套"。
        // 这里现算而不复用 `self.hover`：`handle_keys` 跑在 `update_hover` 之前，
        // 复用会拿到**上一帧**的鼠标格 —— 移动鼠标后立刻按 `Ctrl+V` 就会贴到旧位置。
        let cursor = self.cursor_cell(ctx);
        let Some(d) = self.doc.doc.as_mut() else {
            return;
        };
        let Some(rec) = selection_ops::apply(&mut self.edit, &mut d.map, cmd, cursor) else {
            return; // 只有"粘贴时鼠标不在图上"会走到这里（什么都没做）
        };
        eprintln!(
            "{}",
            tx::fill(
                tx::DIAG_SEL_CMD,
                &[
                    ("cmd", cmd.key()),
                    ("warn", if rec.warn { "1" } else { "0" }),
                    ("n", &rec.changed.to_string()),
                    ("undo", &self.edit.undo_len().to_string()),
                    ("text", &rec.text),
                ]
            )
        );
        // D-004 拍板①：选区**填充成功**不写「上一次操作」段（`status_fill` 已经在说同一件事），
        // 顺手把它清空 —— 上一次操作的旧回执对新这次填充同样不再成立。
        // 其余命令（复制/剪切/粘贴/删除）与**被拒**的填充照旧写。
        if selection_ops::写操作回执(cmd, &rec) {
            self.op = Some((rec.text, rec.warn));
        } else {
            self.op = None;
        }
        if rec.changed > 0 {
            self.touch_map();
        }
        ctx.request_repaint();
    }

    /// `Enter` = 把"当前选中的印章"变成待放置态；`Esc` = 退出（见 [`Self::handle_keys`] 的 `esc`）。
    ///
    /// 为什么是 `Enter` 而不是"点一下就放"：选中是**浏览**动作（面板里一枚枚看过去），
    /// 放置是**破坏性**动作（写地图 + 占一步撤销）。两者必须分开，
    /// 否则用户翻页看缩略图就会在图上到处留印章。
    fn handle_arm_keys(&mut self, ctx: &egui::Context) {
        let enter = ctx.input(|i| {
            !i.modifiers.command && !i.modifiers.alt && i.key_pressed(egui::Key::Enter)
        });
        if !enter {
            return;
        }
        // 搜索框有焦点时 `Enter` 属于搜索框（不抢）
        if self.stamps.focused() {
            return;
        }
        self.arm_selected();
        ctx.request_repaint();
    }

    /// 把**当前选中的印章**置为待放置态（`Enter` 与 `--stamp-arm` 共用这一条）。
    ///
    /// 待放置印章的 `.X` 绝对路径 —— **与面板缩略图同源**（`catalog.resolve(id)`）。
    ///
    /// ⚠ 为什么不用 `stamps.selected_path()`（2026-11-07 实测踩到）：那条路只有在
    /// **面板里真的选中过一行**（`Item.path` 非空）时才有值；`--stamp-pick/--stamp-arm`
    /// 这条 CLI 取证路径上它是 `None` ⇒ `Armed.cells` 拿不到 ⇒ A-011 的预览只有轮廓、
    /// 没有半透明内容与受阻诊断（三张验收图当场拍空）。`resolve` 是按 id 现查，
    /// 与面板给缩略图取路径**完全同一条**，两条入口都能拿到。
    fn path_of_stamp(&mut self, id: &str) -> Option<std::path::PathBuf> {
        self.stamps.catalog_mut()?.resolve(id).map(|r| r.path)
    }

    /// 返回是否真的进入了待放置态（没选中任何一枚 ⇒ `false`，什么都不做）。
    fn arm_selected(&mut self) -> bool {
        let Some(a) = self.stamps.armed() else {
            return false;
        };
        eprintln!(
            "{}",
            tx::fill(
                tx::DIAG_STAMP_ARM,
                &[
                    ("id", &a.id),
                    ("name", &a.name),
                    (
                        "bounds",
                        &match a.bounds {
                            Some((x0, y0, x1, y1)) => format!("({x0},{y0},{x1},{y1})"),
                            None => tx::VAL_UNKNOWN.to_owned(),
                        }
                    ),
                ]
            )
        );
        // 待放置态的提示进 `status_op`（不是 `status_stamp`）：后者是"选中的是谁"，
        // 前者是"现在点下去会发生什么"，两件事分开说。
        self.op = Some((stamp_place::armed_text(&a.name), false));
        // A-011：进待放置态时**顺带读一次盘**把格表装进 `Armed`（Alt 预览要用它渲染
        // 半透明内容与受阻诊断）。读失败 ⇒ `cells: None` ⇒ 预览降级成只画轮廓；
        // **不在这里报错**：真正落位时 `place` 会现读盘并给出中文原因（一条路径，不两套口径）。
        let 号段 = self
            .doc
            .assets
            .号段()
            .unwrap_or(miredit_core::库号号段::Wemade);
        let cells = self
            .path_of_stamp(&a.id)
            .and_then(|p| stamp_place::read(&p, 号段).ok())
            .map(|f| f.cells);

        let mut a = a;
        a.cells = cells;
        self.armed = Some(a);
        // 进待放置态时切到「印章」工具图标，让工具列也反映当前意图
        self.selected_tool = selection_ops::TOOL_STAMP_IDX;
        self.edit.select(selection_ops::TOOL_STAMP_IDX);
        true
    }

    /// 鼠标当前所在的世界格（**现算**，不吃 `self.hover` 的上一帧值）。
    fn cursor_cell(&self, ctx: &egui::Context) -> Option<(u16, u16)> {
        if let Some(h) = self.forced_hover {
            return Some(h);
        }
        let pos = ctx.pointer_hover_pos()?;
        let (x, y) = self.view.screen_to_cell(self.last_canvas, pos)?;
        self.cell_in_map(x, y)
    }

    /// **落位**：把待放置的印章贴到 `(x, y)`。
    ///
    /// 链路（全在 [`stamp_place::place`] 里）：读 `.X` → `register_stamp` → `place_stamp_by_id`。
    /// 失败时地图**一个字节都不动**（三道闸门都在写之前），原因进状态栏。
    fn commit_place(&mut self, x: u16, y: u16) {
        let Some(armed) = self.armed.clone() else {
            return;
        };
        // 路径来自目录的 `resolve`（真实读盘结果）；未登记 / 解析失败 ⇒ 不猜路径
        let path = self.stamps.selected_path().map(|p| p.to_path_buf());
        let Some(path) = path else {
            let why = tx::fill(tx::STAMP_NO_PATH, &[("id", &armed.id)]);
            eprintln!(
                "{}",
                tx::fill(
                    tx::DIAG_STAMP_PLACE_FAIL,
                    &[("id", &armed.id), ("x", &x.to_string()), ("y", &y.to_string()), ("why", &why)]
                )
            );
            self.op = Some((stamp_place::fail_text(&why), true));
            return;
        };
        // 库号号段（A-024）：与 `read`/缩略图同源（`mapdoc::Assets`）；装载失败时兜底 `Wemade`
        let 号段 = self
            .doc
            .assets
            .号段()
            .unwrap_or(miredit_core::库号号段::Wemade);
        let Some(d) = self.doc.doc.as_mut() else {
            return;
        };
        let out = stamp_place::place(&mut self.edit, &mut d.map, &armed, &path, x, y, 号段);
        // 内核在成功时也可能留一句"库号字节被忽略"的说明。**取走它，但不拼进回执**：
        //
        // ① 那句话很长（实测 88 字：`Old12（12B/格，TMapInfo 旧版） 里 物体 的库号字节没有
        //    对应字节（writer 不写它）—— 该字节已忽略，只写图号`）。拼进回执后回执整段
        //    **放不下状态栏，被"放不下就不画"的规则整段丢掉** —— 实测截图里
        //    `放置印章「…」8 格` 直接消失，用户看不到任何"放成功了"的确认。
        // ② 它说的信息「本图没有库号字节」**已经在「落笔」段里**了
        //    （`落笔 bk_img（本图 12B 档无库号字节，只写图号）`）⇒ 不重复占用状态栏。
        // ③ 它仍然是真实数据、必须可查 ⇒ 走 stderr 的诊断行（本仓所有"细节事实"都在那儿）。
        let extra = selection_ops::take_note(&mut self.edit);
        if let Some(n) = &extra {
            eprintln!(
                "{}",
                tx::fill(tx::DIAG_STAMP_PLACE_NOTE, &[("id", &armed.id), ("note", n)])
            );
        }
        match out {
            Ok(p) => {
                eprintln!(
                    "{}",
                    tx::fill(
                        tx::DIAG_STAMP_PLACE,
                        &[
                            ("id", &armed.id),
                            ("x", &x.to_string()),
                            ("y", &y.to_string()),
                            ("n", &p.cells.to_string()),
                            ("k", &p.skipped.to_string()),
                            ("undo", &self.edit.undo_len().to_string()),
                        ]
                    )
                );
                self.op = Some((stamp_place::receipt(&armed.name, p), false));
                // 落位成功 ⇒ 退出待放置态（放第二枚要重新按 Enter，避免连点误放）
                self.armed = None;
                self.touch_map();
            }
            Err(why) => {
                eprintln!(
                    "{}",
                    tx::fill(
                        tx::DIAG_STAMP_PLACE_FAIL,
                        &[
                            ("id", &armed.id),
                            ("x", &x.to_string()),
                            ("y", &y.to_string()),
                            ("why", &why),
                        ]
                    )
                );
                self.op = Some((stamp_place::fail_text(&why), true));
                // 失败**不退**待放置态：用户多半是点到了图外，换个地方再点即可
            }
        }
    }


    /// `Ctrl+S`：**只在有修改时**才写盘，走 [`save::save_atomic`] 原子替换。
    ///
    /// 失败时把原因原样留在会话里（状态栏左段显示），且**原文件保持原样**。
    /// 真正干活的判定与写盘都在 [`save::save_session`] 里（无窗驱动器共用同一份）。
    fn do_save(&mut self) {
        let _ = save::save_session(&mut self.doc, &mut self.edit);
        // 存成功/失败都要重算文案（标题的 `*` 与状态栏左段都跟着变）
        self.text = screen_text::build(
            &self.doc,
            &self.view,
            self.hover,
            &self.lines,
            &self.edit,
        );
    }

    /// 画布上的工具落格（左键）：吸管即时取，画笔/橡皮按拖动累积成**一步**。
    ///
    /// 平移优先：调用方已判定"不是平移"才会进来（见 [`Self::interact`]）。
    fn handle_tool_pointer(&mut self, ui: &egui::Ui, l: &Layout) {
        let (down, pressed, released, pos) = ui.input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.button_released(egui::PointerButton::Primary),
                i.pointer.interact_pos(),
            )
        });
        let cell = pos
            .and_then(|p| self.view.screen_to_cell(l.viewport, p))
            .and_then(|(x, y)| self.cell_in_map(x, y));

        // ⓪ **待放置态优先**：点画布 = 落位（不进任何笔画，也不碰当前工具）
        //
        //    为什么放在最前面：待放置态是一个"待确认的破坏性动作"，
        //    此时左键的语义必须唯一 —— 落位。若还让画笔落格，一次点击会同时写两样东西。
        if pressed && self.armed.is_some() {
            if let Some((x, y)) = cell {
                self.commit_place(x, y);
            }
            ui.ctx().request_repaint();
            return;
        }

        // ⓪b **选择工具**：拖动 = 画矩形选区（`SetSelection`），松手定格
        if self.selected_tool == selection_ops::TOOL_SELECT_IDX {
            self.handle_select_drag(ui, cell);
            return;
        }

        // ⓪c **地表落笔前端拦截**（九种格式里只有 Type5 会走到）：
        //     判据与文案在 [`ground_block_reason`]（与验收用例同一份），这里只负责
        //     **在 `begin()` / `touch_footprint()` 之前**把这一下拦掉并说出来。
        //     吸管（`Tool::Pickup`）只读、`Tool::Unwired` 什么都不做 ⇒ 两者不拦。
        if pressed && matches!(self.edit.tool(), Tool::Brush | Tool::Eraser | Tool::Bucket) {
            if let Some(why) = ground_block_reason(&self.doc, self.edit.layer()) {
                self.report_ground_blocked(&why);
                ui.ctx().request_repaint();
                return;
            }
        }

        // ① 按下：吸管当场取；画笔/橡皮开一笔
        if pressed {
            if let Some((x, y)) = cell {
                match self.edit.tool() {
                    Tool::Pickup => {
                        let picked = match self.doc.doc.as_ref() {
                            Some(d) => self.edit.pick_at(&d.map, x, y),
                            None => None,
                        };
                        let picked_cell = match self.doc.doc.as_ref() {
                            Some(d) => self.edit.pick_cell(&d.map, x, y),
                            None => None,
                        };
                        let fmt = self.doc.doc.as_ref().map(|d| d.map.format);
                        match picked {
                            Some(b) => {
                                eprintln!(
                                    "{}",
                                    tx::fill(
                                        tx::DIAG_EDIT_PICK,
                                        &[
                                            ("x", &x.to_string()),
                                            ("y", &y.to_string()),
                                            ("fam", b.family_name()),
                                            ("img", &b.sprite().to_string()),
                                            ("lib", &edit::brush_lib_text(fmt, &b)),
                                        ]
                                    )
                                );
                                // 整格三族也打一行（吸管读整格，但笔刷只取当前层字段）
                                if let Some(pc) = picked_cell {
                                    eprintln!(
                                        "{}",
                                        tx::fill(
                                            tx::DIAG_EDIT_PICK_CELL,
                                            &[
                                                ("x", &x.to_string()),
                                                ("y", &y.to_string()),
                                                ("bk", &pc.bk_img.to_string()),
                                                ("mid", &pc.mid_img.to_string()),
                                                ("fr", &pc.fr_img.to_string()),
                                            ]
                                        )
                                    );
                                }
                            }
                            None => eprintln!(
                                "{}",
                                tx::fill(
                                    tx::DIAG_EDIT_PICK_NONE,
                                    &[("x", &x.to_string()), ("y", &y.to_string())]
                                )
                            ),
                        }
                    }
                    Tool::Brush | Tool::Eraser => {
                        self.edit.begin();
                        self.paint_to(x, y);
                    }
                    Tool::Bucket => {
                        // 填充：点一下填一次（**整次填充 = 一步撤销**，由 `fill` 自己记账）
                        let out = match self.doc.doc.as_mut() {
                            Some(d) => self.edit.fill(&mut d.map, x, y),
                            None => edit::FillOutcome::NoBrush,
                        };
                        match out {
                            edit::FillOutcome::Filled(n) => {
                                eprintln!(
                                    "{}",
                                    tx::fill(
                                        tx::DIAG_EDIT_FILL,
                                        &[
                                            ("x", &x.to_string()),
                                            ("y", &y.to_string()),
                                            ("n", &n.to_string()),
                                            ("layer", self.edit.layer().name()),
                                        ]
                                    )
                                );
                                self.touch_map();
                            }
                            edit::FillOutcome::AlreadyFilled => {}
                            other => {
                                // 拒绝（越界/上限/层不支持/无笔刷/族不匹配）：原因已在 `note` 里
                                eprintln!(
                                    "{}",
                                    tx::fill(
                                        tx::DIAG_EDIT_FILL_REFUSED,
                                        &[("why", self.edit.note().unwrap_or(tx::VAL_UNKNOWN))]
                                    )
                                );
                                let _ = other;
                            }
                        }
                        ui.ctx().request_repaint();
                    }
                    Tool::Unwired => {}
                }
                ui.ctx().request_repaint();
            }
        }

        // ② 拖动中：笔画没结束就继续落格（指针移出画布时暂停，回来接着画）
        if down && self.edit.stroke_active() {
            if let Some((x, y)) = cell {
                self.paint_to(x, y);
            }
        }

        // ③ 松开：结算成一步（**一次拖动 = 一步撤销**）
        if released {
            let n = self.edit.end();
            if n > 0 {
                ui.ctx().request_repaint();
            }
        }
    }

    /// 从上一格插值到 `(x, y)` 并落格；真改了东西才 `touch_map`（触发重渲）。
    ///
    /// **整条轨迹都按当前笔刷尺寸落**（N×N，居中，边缘裁剪）——见
    /// [`EditSession::touch_footprint`]；插值仍然是 Bresenham（[`edit::line_cells`]）。
    ///
    /// ⚠ 进来先过一遍 [`ground_block_reason`]：拖动途中若用户把当前层切到地表
    /// （Type5 上），后续每一格也必须**一格都不写**，而不是"前几格写进去了"。
    fn paint_to(&mut self, x: u16, y: u16) {
        if ground_block_reason(&self.doc, self.edit.layer()).is_some() {
            return;
        }
        let from = self.edit.last_cell().unwrap_or((x, y));
        let path = edit::line_cells(from, (x, y));
        let mut changed = 0;
        if let Some(d) = self.doc.doc.as_mut() {
            for (cx, cy) in path {
                changed += self.edit.touch_footprint(&mut d.map, cx, cy);
            }
        }
        if changed > 0 {
            self.touch_map();
        }
    }

    /// **选择工具**的拖动：按下记锚点、拖动每帧重设选区、松手定格。
    ///
    /// 语义（闭区间、越界裁剪）全在内核 `Selection::clamp_to` + `EditSession::set_selection`；
    /// 本方法只做三件事：
    /// ① 按下时把鼠标格记成锚点（**锚点必须在图内**，否则内核会把整块判成图外）；
    /// ② 拖动时用「锚点 → 当前格」重设选区（指针移出画布时**保持上一次**，不静默清空）；
    /// ③ 松手把拖动状态清掉（选区留在会话里，直到下次拖动 / `Esc`）。
    ///
    /// ⚠ **不置脏、不进撤销栈**：选区是 UI 状态，内核已保证（`session.rs` 的选区段）。
    fn handle_select_drag(&mut self, ui: &egui::Ui, cell: Option<(u16, u16)>) {
        let (pressed, down, released) = ui.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.primary_down(),
                i.pointer.button_released(egui::PointerButton::Primary),
            )
        });
        if pressed {
            self.sel_drag = cell.map(|c| SelDrag { anchor: c });
        }
        if down {
            if let (Some(d), Some(cur), Some(doc)) =
                (self.sel_drag, cell, self.doc.doc.as_mut())
            {
                let sel = selection_ops::selection_of(d.anchor, cur);
                if self.edit.set_selection(&mut doc.map, sel) {
                    ui.ctx().request_repaint();
                }
            }
        }
        if released && self.sel_drag.is_some() {
            self.sel_drag = None;
            ui.ctx().request_repaint();
        }
    }

    /// 「检查地图」→「定位」：把世界格 `(x, y)` 居中，并把选区设成 **1×1**。
    ///
    /// 上游 `MainWindow.MapCheck.cs:30-35`：`Viewport.LocateCell(issue.X, issue.Y)` +
    /// `UpdateCellInspector(x, y)` + `dialog.DialogResult = true`（关窗）。本仓的
    /// 「单元格信息」面板每帧按悬停/选区自动更新（`cellinfo` / `dock_right`）⇒
    /// 这里只需要「居中 + 设选区」两件事，关窗由调用方负责。
    ///
    /// 返回状态栏回执文案（定位是**只读**动作：不置脏、不进撤销栈，选区本来就不入栈）。
    fn locate_cell(&mut self, x: u16, y: u16, canvas: egui::Rect) -> String {
        self.view.center_on_cell(canvas, f32::from(x), f32::from(y));
        if let Some(d) = self.doc.doc.as_mut() {
            self.edit
                .set_selection(&mut d.map, miredit_core::Selection::new(x, y, x, y));
        }
        tx::fill(
            tx::STATUS_CHECK_MAP_LOCATE_TMPL,
            &[("x", &x.to_string()), ("y", &y.to_string())],
        )
    }

    /// 世界格是否在地图内（越界 = `None`，不静默夹到边界上）。
    fn cell_in_map(&self, x: i32, y: i32) -> Option<(u16, u16)> {
        let d = self.doc.doc.as_ref()?;
        if x < 0 || y < 0 || x >= i32::from(d.map.width) || y >= i32::from(d.map.height) {
            return None;
        }
        Some((x as u16, y as u16))
    }

    /// 系统关窗请求的拦截：有未保存修改时取消关窗、弹出模态（第二次才放行）。
    ///
    /// ⚠ **自动化取证运行例外**：带 `--shot` / `--autodrag` 时程序自己会发
    /// `ViewportCommand::Close` 收尾，若也拦下来，无人值守的截图/拖动实测就永远退不出去。
    /// 那两种模式下**不拦**（它们本来就只读磁盘、不保存）。
    fn guard_close_request(&mut self, ctx: &egui::Context) {
        if self.auto.shot.is_some()
            || self.auto.autodrag.is_some()
            || self.auto.palette_scroll.is_some()
        {
            return;
        }
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if self.edit.dirty() && !self.confirm_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
            ctx.request_repaint();
        }
        // 不脏 / 已确认 ⇒ 什么都不做，让关窗照常发生
    }

    /// 标题变了才发 `ViewportCommand::Title`（`*` 要同步到 OS 窗口标题上）。
    fn sync_window_title(&mut self, ctx: &egui::Context) {
        if self.text.title == self.shown_title {
            return;
        }
        self.shown_title = self.text.title.clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.shown_title.clone()));
    }

    /// 关闭拦截模态（简单居中提示框 + 半透明遮罩）。
    ///
    /// **不加可交互矩形**：右上角 `×` 的命中测试在 [`Self::interact`] 里按矩形算，
    /// 与绘制先后无关，所以"再点关闭"照样点得到 —— 这正是提示文案说的操作。
    fn draw_close_modal(&self, p: &egui::Painter, l: &Layout) {
        if !self.confirm_close {
            return;
        }
        pt::fill(p, l.content, t::ui().modal_scrim);
        let c = Rect::from_center_size(
            l.viewport.center(),
            Vec2::new(m::MODAL_W, m::MODAL_H),
        );
        pt::fill_round(p, c, m::MODAL_RADIUS, t::ui().panel_bg);
        pt::border(p, c, t::ui().accent_dark);
        pt::text(
            p,
            c.center(),
            Align2::CENTER_CENTER,
            tx::CLOSE_CONFIRM,
            m::MODAL_FONT,
            t::ui().text,
        );
    }

    /// 拖动实测（`--autodrag`）：按帧平移，时间到就打印报告并关窗。
    /// 走的是**与鼠标拖动完全相同的** [`CanvasView::pan_by`]，所以"每帧会不会重复
    /// `render_region`"这件事测出来的就是真实拖动时的行为。
    fn drive_autodrag(&mut self, ctx: &egui::Context) {
        let Some(secs) = self.auto.autodrag else {
            return;
        };
        let started = *self.auto.drag_started.get_or_insert_with(Instant::now);
        let elapsed = started.elapsed().as_secs_f32();
        if elapsed < secs {
            // 每帧右移 6px、下移 2px（斜向拖，两个轴都在变，才能覆盖"跨格重渲"路径）
            self.view.pan_by(Vec2::new(6.0, 2.0));
            ctx.request_repaint();
            return;
        }
        if self.auto.drag_reported {
            return;
        }
        self.auto.drag_reported = true;
        let avg_gap = if self.frames > 0 {
            self.frame_gap_ms / self.frames as f64
        } else {
            0.0
        };
        let (_, _, cw, ch) = self.view.visible_cells(self.last_canvas);
        eprintln!(
            "{}",
            tx::fill(
                tx::DIAG_AUTODRAG,
                &[
                    ("frames", &self.frames.to_string()),
                    ("calls", &self.view.render_calls.to_string()),
                    ("ms", &format!("{avg_gap:.2}")),
                    ("scale", &format!("{}", self.view.scale)),
                    ("cw", &cw.to_string()),
                    ("ch", &ch.to_string()),
                ]
            )
        );
        eprintln!(
            "        其中 update() 自耗时平均 {:.2} ms/帧；最近一次重渲 {:.1} ms；stats: {}",
            if self.frames > 0 {
                self.update_ms / self.frames as f64
            } else {
                0.0
            },
            self.view.last_render_ms,
            self.view.stats
        );
        // 有截图任务就先等截图，否则直接收尾
        if self.auto.shot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// 素材面板滚动实测（`--palette-autoscroll`）：每帧往下滚一行，时间到就打印报告并关窗。
    ///
    /// 走的是**与滚轮完全相同的** [`PaletteState::scroll_by`] 与 [`PaletteState::draw`]，
    /// 所以"每滚一屏会不会把已缓存的帧重新解码一遍"这件事测出来的就是真实行为。
    fn drive_palette_autoscroll(&mut self, ctx: &egui::Context) {
        let Some(secs) = self.auto.palette_scroll else {
            return;
        };
        let started = *self.auto.palette_started.get_or_insert_with(Instant::now);
        let elapsed = started.elapsed().as_secs_f32();
        if elapsed < secs {
            self.palette.scroll_by(1);
            ctx.request_repaint();
            return;
        }
        if self.auto.palette_reported {
            return;
        }
        self.auto.palette_reported = true;
        let avg_gap = if self.frames > 0 {
            self.frame_gap_ms / self.frames as f64
        } else {
            0.0
        };
        eprintln!(
            "{}",
            tx::fill(
                tx::DIAG_PALETTE_SCROLL,
                &[
                    ("frames", &self.frames.to_string()),
                    ("ms", &format!("{avg_gap:.2}")),
                    ("decodes", &self.palette.decodes().to_string()),
                    ("hits", &self.palette.hits().to_string()),
                    ("row", &self.palette.scroll().to_string()),
                ]
            )
        );
        self.report_palette();
        if self.auto.shot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// 把素材面板的统计打到 stderr（`--palette-report` / 滚动实测收尾时各调一次）。
    fn report_palette(&self) {        let pal = &self.palette;
        let stem = pal
            .current()
            .map(|e| e.stem.clone())
            .unwrap_or_else(|| tx::VAL_UNKNOWN.to_owned());
        let fam = pal
            .current()
            .map(|e| e.family_label())
            .unwrap_or(tx::VAL_UNKNOWN);
        let total = pal
            .current_count()
            .map(|n| n.to_string())
            .unwrap_or_else(|| tx::VAL_UNKNOWN.to_owned());
        eprintln!(
            "{}",
            tx::fill(
                tx::DIAG_PALETTE,
                &[
                    ("stem", &stem),
                    ("fam", fam),
                    ("total", &total),
                    ("slots", &pal.slots().to_string()),
                    ("decodes", &pal.decodes().to_string()),
                    ("hits", &pal.hits().to_string()),
                    ("empty", &pal.empty_count().to_string()),
                    ("row", &pal.scroll().to_string()),
                    ("rows", &pal.rows_drawn().to_string()),
                ]
            )
        );
        // 解码失败/空帧在面板上合成一个数（`空帧 N`），这里把失败原文单独说清楚
        if let Some(src) = self.doc.assets.source() {
            let fails = src.failures();
            if let Some(first) = fails.first() {
                eprintln!(
                    "{}",
                    tx::fill(
                        tx::DIAG_PALETTE_FAIL,
                        &[("n", &fails.len().to_string()), ("first", first)]
                    )
                );
            }
        }
    }

    /// 打一行"选中了哪枚印章"的诊断（**真实 id/名称/分类/格数/包围盒**，不是占位）。
    fn report_stamp_pick(&self) {
        let Some(s) = self.stamps.selected() else {
            return;
        };
        let unk = || tx::VAL_UNKNOWN.to_owned();
        let cells = s.cells.map(|n| n.to_string()).unwrap_or_else(unk);
        let b = s.bounds;
        let num = |i: usize| match b {
            Some(t) => [t.0, t.1, t.2, t.3][i].to_string(),
            None => tx::VAL_UNKNOWN.to_owned(),
        };
        eprintln!(
            "{}",
            tx::fill(
                tx::DIAG_STAMP_PICK,
                &[
                    ("id", &s.id),
                    ("name", &s.name),
                    ("kind", &s.kind_zh),
                    ("cells", &cells),
                    ("x0", &num(0)),
                    ("y0", &num(1)),
                    ("x1", &num(2)),
                    ("y1", &num(3)),
                ]
            )
        );
    }

    /// 印章面板统计（`--stamp-report`）：过滤条件 / 页 / 缓存峰值 / 渲染与命中数。
    fn report_stamps(&self) {
        eprintln!("{}", stamp_panel::report_text(&self.stamps));
    }

    /// 处理截图：到帧就请求一次；收到事件就落盘（并按需做同视点比对）。
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        // ① 收图
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            if let egui::Event::Screenshot { image, .. } = e {
                let Some(path) = self.auto.shot.clone() else {
                    continue;
                };
                if self.auto.saved {
                    continue;
                }
                match selfcheck::save_screenshot(&image, &path) {
                    Ok(()) => {
                        self.auto.saved = true;
                        self.run_gate2(&path);
                        // `--palette-report`：截图这一帧的素材面板状态（真数据，不是估计值）
                        if self.auto.palette_report {
                            self.report_palette();
                        }
                        // `--stamp-report`：同一帧的印章面板状态（缓存峰值/命中/缺帧数）
                        if self.auto.stamp_report {
                            self.report_stamps();
                        }
                    }
                    Err(err) => {
                        eprintln!("{}", tx::fill(tx::DIAG_SHOT_FAIL, &[("err", &err)]));
                        self.auto.saved = true; // 失败也算"做过了"，不再反复请求
                    }
                }
            }
        }

        // ② 请求（只发一次）
        //
        // ⚠ **取证运行必须自己推进帧**：egui 只在有输入/有请求时重绘，
        //    而 `--shot` 运行**锁死了鼠标输入**（[`Self::input_locked`]）⇒ 没人碰窗口就没人重绘，
        //    `self.frames` 会停在几十帧以内、永远到不了 `--frames`，于是"截图不落盘、窗口不关"
        //    （2026-11-05 实测：进程空转 10 分钟、CPU 累计 0.95 s、PNG 未生成）。
        //    所以在**还没发出截图请求**之前，每帧主动请求下一帧。
        if self.auto.shot.is_some() && !self.auto.requested && self.frames < self.auto.frames {
            ctx.request_repaint();
        }
        if self.auto.shot.is_some() && !self.auto.requested && self.frames >= self.auto.frames {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                egui::UserData::default(),
            ));
            self.auto.requested = true;
            ctx.request_repaint();
        }

        // ③ 收尾：截图完成且没有拖动任务 ⇒ 关窗
        if self.auto.saved && self.auto.autodrag.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// 截图拍完后，若带了 `--gate2-client`，就地做同视点分块比对。
    fn run_gate2(&self, shot_path: &std::path::Path) {
        let (Some(client), Some(at)) = (self.auto.gate2_client.clone(), self.auto.gate2_at) else {
            return;
        };
        let shot = match miredit_render::png_io::read_png(shot_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[门2] 读回截图失败：{e}");
                return;
            }
        };
        match selfcheck::gate2(
            &shot,
            self.last_canvas,
            self.last_overlay,
            &self.view,
            &client,
            at,
            shot_path,
        ) {
            Ok(g) => {
                let pass = g.rate >= 0.95;
                eprintln!(
                    "[门2] 判定：命中率 {:.2}%（要求 ≥95%）⇒ {}",
                    g.rate * 100.0,
                    if pass { "PASS" } else { "FAIL" }
                );
            }
            Err(e) => eprintln!("[门2] 比对失败：{e}"),
        }
    }
}

impl eframe::App for MireditApp {
    /// 窗口清屏色：直接用面板底色，避免任何一帧出现黑边。
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = t::ui().panel_bg;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let t0 = Instant::now();
        // 帧间隔统计（拖动实测要报"平均帧时间"）
        let now = Instant::now();
        if let Some(prev) = self.last_frame {
            self.frame_gap_ms += (now - prev).as_secs_f64() * 1000.0;
        }
        self.last_frame = Some(now);
        self.frames += 1;

        // 强制 1 逻辑像素 = 1 物理像素，保证窗口是精确的 1500×950（截图口径）。
        if (ctx.pixels_per_point() - 1.0).abs() > f32::EPSILON {
            ctx.set_pixels_per_point(1.0);
        }

        // ⓪ 快捷键（Ctrl+Z/Y/S、Esc）与关窗拦截 —— 必须在交互/布局之前，
        //    否则本帧的撤销/保存要等到下一帧才反映到画布与标题上。
        self.handle_keys(ctx);
        self.guard_close_request(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let screen = ctx.content_rect();
                let l = self.layout(screen);
                // 面板自身不留边距，painter 的裁剪盒扩到整个窗口
                let p = ui.painter().with_clip_rect(screen);
                // A-011：Alt 预览（内容纹理懒加载 ⇒ 每帧只在需要时渲染一次）
                let 预览 = self.stamp_preview(ctx);

                // ① 自动化驱动（平移 / 素材滚动）——必须在交互/布局之前
                self.drive_autodrag(ctx);
                self.drive_palette_autoscroll(ctx);
                // ② 首次对准（需要画布矩形）
                self.init_view(&l);
                // ②a **窗口尺寸变化 ⇒ 按上游 guard 重 fit**（A-012）。必须在
                //     `ensure_texture` 之前：重 fit 改的是 `scale`/`pan`，纹理按它取可见区。
                self.refit_after_resize(&l);
                self.last_canvas = l.viewport;
                self.last_overlay = l.zoom_pill;

                // ②b `--menu-click`：等首次布局算出画布矩形后再执行（缩放/适应要锚点）。
                //     走的是与鼠标点击**完全相同**的 `dispatch_menu`，不是另写一条捷径。
                //
                //     每条带一个**帧号**（缺省 1）：到点才执行，其余留到下一帧
                //     （`@40` 那种推迟执行就是靠这里 —— 见 `pending_menu_clicks` 的文档）。
                if !self.pending_menu_clicks.is_empty() {
                    let mut 到期: Vec<String> = Vec::new();
                    let mut 未到: Vec<(u64, String)> = Vec::with_capacity(self.pending_menu_clicks.len());
                    for (frame, id) in std::mem::take(&mut self.pending_menu_clicks) {
                        if self.frames >= frame {
                            到期.push(id);
                        } else {
                            未到.push((frame, id));
                        }
                    }
                    self.pending_menu_clicks = 未到;
                    for id in 到期 {
                        // 顶级项与静态子项都认（`view.light_night` 这类子项 id 靠
                        // `find_action_by_id` 才找得到，见该函数的文档）。
                        match menu::model::find_action_by_id(&id) {
                            Some(a) => self.dispatch_menu(a, l.viewport),
                            None => eprintln!("[菜单] 没有这个菜单项 id：{id}"),
                        }
                    }
                }
                // ②c `--open-map`：走「文件 → 打开地图…」模态点「打开」时的**同一个函数**
                if let Some(p) = self.pending_open_map.take() {
                    let text = p.to_string_lossy().to_string();
                    self.open_map_path(&text, l.viewport);
                }

                // ③ 交互（先算，后画，保证 hover 态与点击同帧生效）
                self.interact(ui, &l);

                // ④ 悬停格 + 单元格信息
                self.update_hover(ctx, &l);

                // ⑤ 重渲调度（只在变化时真渲）
                self.ensure_texture(ctx, &l);
                // ⑤b 素材面板帧数（锁定输入的取证运行里也要查，否则网格画不出来）
                self.palette.refresh_count(self.doc.assets.source());
                // ⑤d **地图列表面板**：推进一帧扫描（4 ms 预算 + 512 项上限）。
                //     还在扫 ⇒ 每帧请求重绘，否则面板会停在「扫描中…」等人动鼠标。
                if self.maps.poll() {
                    ctx.request_repaint();
                }
                // ⑤c 印章面板：收后台装载结果；就绪后把 `--stamp-*` 落到面板上（只做一次）。
                //    装载中要持续请求重绘，否则画面会停在「正在加载印章目录…」等用户动鼠标。
                let loaded = self.stamps.poll();
                if loaded || self.stamps.is_loading() {
                    ctx.request_repaint();
                }
                if self.stamps.is_ready() {
                    if let Some(p) = self.pending_stamp.take() {
                        apply_stamp_cli(&mut self.stamps, &mut self.palette, &p);
                        // `--stamp-arm` / `--stamp-place-at`：装载完成后**立即**走与
                        // `Enter` + 点画布完全相同的两条路径（见 `handle_arm_keys` / `commit_place`）
                        if p.arm {
                            self.arm_selected();
                        }
                        if let Some((x, y)) = p.place_at {
                            self.commit_place(x, y);
                        }
                    }
                }

                // ⑥ 装配文案
                self.text = screen_text::build(
                    &self.doc,
                    &self.view,
                    self.hover,
                    &self.lines,
                    &self.edit,
                );
                // ⑥a 状态栏追加「印章：<名称>」（选中印章后才有；没选就是 `None`，
                //     绝不写"印章：无"这种占位 —— 见 `stamp_panel::status_text`）
                self.text.status_stamp = self.stamps.status_text();
                // ⑥a2 状态栏「上一次操作回执」（印章放置 / 选区命令）：
                //     存在宿主里（`self.op`），因为 `screen_text::build` 不碰跨帧状态。
                if let Some((text, warn)) = &self.op {
                    self.text.status_op = Some(text.clone());
                    self.text.status_op_warn = *warn;
                }
                // ⑥a3 工具名要认得宿主层的两支（选择 / 印章）：内核 `Tool` 没有它们，
                //     直接取 `ed.tool().name()` 会显示成「未接线」——那是假信息。
                self.text.status_tool = tx::fill(
                    tx::STATUS_TOOL_TMPL,
                    &[("tool", edit::slot_label(self.selected_tool))],
                );

                // ⑥b 小地图的真实数据：地图格数（`.map` 头）+ 当前可见格窗口
                //     （= 真实 pan/scale/画布矩形算出来的）。**一个常量都不掺**（审计 E10）。
                let minimap = dock_right::MiniMap::new(
                    self.doc.doc.as_ref().map(|d| d.size()),
                    self.view.visible_cells(l.viewport),
                );

                // ⑦ 绘制
                pt::fill(&p, screen, t::ui().panel_bg);
                // 中列与右停靠区之间的 1px 分隔线（截图 x=1135 为 `#E7EBEF`）。
                // 必须显式画：地图纹理比视口大，只靠背景色会被顶掉。
                pt::fill(&p, l.dock_divider, t::ui().dock_divider);
                dock_left::draw(
                    &p,
                    &l,
                    self.selected_tool,
                    self.edit.layer(),
                    &self.view.mask,
                    &self.view.overlay,
                    &self.text,
                    &self.panels,
                );
                // ⑦a2 **地图列表面板**（中列左侧一列）：先让它把当前图那行滚进可见区，
                //      再画。顺序有因果：`tick` 改 `top`，`draw` 按 `top` 出图 ——
                //      反过来会先画一帧旧滚动位置再跳，截图就会拍到"当前项不在视野里"。
                let cur_path = self.doc.doc.as_ref().map(|d| d.path.clone());
                maplist::tick(&l, &mut self.maps, cur_path.as_deref());
                maplist::draw(&p, &l, &self.maps, cur_path.as_deref());
                canvas::draw(
                    &p,
                    &l,
                    &CanvasCtx {
                        tex: self.view.texture(),
                        view: &self.view,
                        text: &self.text,
                        hover: self.hover,
                        has_map: self.doc.doc.is_some(),
                        // 单元格网格（视图 → 单元格网格）：**与菜单勾选同一个字段**
                        grid: self.grid,
                        // 地图尺寸：网格按它把格线裁到地图边界（官方 `Presentation.cs:37-39`）。
                        // 无图 ⇒ 0 ⇒ `grid::lines_in` 直接返回空集（不是"不夹"档）。
                        map_w: self.doc.doc.as_ref().map(|d| d.map.width).unwrap_or(0),
                        map_h: self.doc.doc.as_ref().map(|d| d.map.height).unwrap_or(0),
                        selection: self.edit.selection(),
                        // 待放置预览（A-011）：内容 + 轮廓 + 受阻诊断。
                        // ⚠ 必须**先**算到局部变量：`stamp_preview` 要 `&mut self`（懒加载纹理），
                        //  而下面的 `CanvasCtx` 字面量同时**不可变**借用了 `self` 的其它字段。
                        // 待放置轮廓：**只有按住 Alt 才画**（任务书 §三.5 的推荐做法）——
                        // 常驻显示会盖住地图，按住 Alt 才看是"临时预览"的语义。
                        stamp_preview: 预览,
                        armed_badge: self.armed_badge(),
                    },
                );
                // ⑦b **常驻告警黄条**：压在画布纹理之上（必须在 `canvas::draw` 之后，
                //     否则会被比视口还大的纹理顶掉）。内容 = `Document::edit_warning()` 原文，
                //     不是 toast、不自动消失（见 [`crate::banner`]）。
                banner::draw(&p, &l, &self.text);
                // 库号号段（A-024）：与图源同源，都来自 `mapdoc::Assets`。
                // 素材装载失败时兜底 `Wemade` —— 那时一格图都画不出来，号段只影响字节语义。
                let 号段 = self
                    .doc
                    .assets
                    .号段()
                    .unwrap_or(miredit_core::库号号段::Wemade);
                dock_right::draw(
                    &p,
                    &l,
                    &self.text,
                    &mut self.palette,
                    &mut self.stamps,
                    self.doc.assets.source(),
                    // 库号号段与图源**同源**（都来自 `mapdoc::Assets`，A-024）：
                    // 素材装载失败时给兜底 `Wemade`（那时没有任何图能画出来，号段只影响字节含义）
                    号段,
                    &minimap,
                    &self.panels,
                );
                menubar::draw(&p, &l);
                // ⑦c **菜单弹层**：必须在菜单栏之后画（要压住面板与画布）
                //
                // `draw_with` 而不是 `draw`：子菜单的行来自**真数据**（设置文件里的
                // `recent`），与 `interact_with` / `handle_key_with` 问的是同一份
                // `SubState`（否则"画 3 行、键盘只走 1 行"这种错位就出来了）。
                let sub = self.sub_state();
                menu::draw_with(
                    &p,
                    &l,
                    &self.menu,
                    self.doc.doc.is_some(),
                    self.dark,
                    // 勾选型项的勾必须反映真状态（网格开着才画勾 / 门开着才画勾 /
                    // 当前是哪档光照就只勾哪一档 / 浮层开着才画「地砖信息悬浮」的勾）
                    self.grid,
                    self.doors_open,
                    self.cell_hover,
                    self.lighting,
                    &self.panels,
                    &sub,
                );
                // ⑦c2 首帧几何（截图比对要用：画布 / 弹层 / 缩放浮层 / 告警黄条四块矩形 + 当前主题）
                if self.frames == 1 && self.menu.open_index().is_some() {
                    if let Some(pr) = menu::popup_rect(&p, &l, &self.menu) {
                        let br = banner::rect(&p, &l, &self.text).unwrap_or(Rect::NOTHING);
                        eprintln!(
                            "{}",
                            tx::fill(
                                tx::DIAG_MENU_GEOM,
                                &[
                                    ("cx", &format!("{:.0}", l.viewport.left())),
                                    ("cy", &format!("{:.0}", l.viewport.top())),
                                    ("cw", &format!("{:.0}", l.viewport.width())),
                                    ("ch", &format!("{:.0}", l.viewport.height())),
                                    ("px", &format!("{:.0}", pr.left())),
                                    ("py", &format!("{:.0}", pr.top())),
                                    ("pw", &format!("{:.0}", pr.width())),
                                    ("ph", &format!("{:.0}", pr.height())),
                                    ("gx", &format!("{:.0}", l.zoom_pill.left())),
                                    ("gy", &format!("{:.0}", l.zoom_pill.top())),
                                    ("gw", &format!("{:.0}", l.zoom_pill.width())),
                                    ("gh", &format!("{:.0}", l.zoom_pill.height())),
                                    ("dark", if self.dark { "1" } else { "0" }),
                                    ("bx", &format!("{:.0}", br.left())),
                                    ("by", &format!("{:.0}", br.top())),
                                    ("bw", &format!("{:.0}", br.width())),
                                    ("bh", &format!("{:.0}", br.height())),
                                ]
                            )
                        );
                    }
                }
                titlebar::draw(&p, &l, &self.text);
                statusbar::draw(&p, &l, &self.text);
                // ⑦e **悬停地砖信息浮层**（视图 → 地砖信息悬浮）：
                //     官方 `CellHoverOverlay` 是 `ZIndex="100"`、覆盖整窗的叠加层 ⇒
                //     本仓在**面板/画布/菜单栏/状态栏之后**画，仍然压在两个模态（下面那段）之下。
                //     开关关着 / 没悬停 / 格在图外 ⇒ `hover_panel` 返回 `None`，一个像素都不画。
                if let Some(panel) = self.hover_panel(&l) {
                    cell_hover::draw(&p, &panel);
                }
                // 关闭拦截模态压在一切之上（只有真有未保存修改时才会画）
                self.draw_close_modal(&p, &l);
                // ⑦d **浮层**：打开地图模态 / 另存为模态 / 快捷键浮层，压在最上层
                match &self.overlay {
                    Overlay::OpenMap { path } => menu::dialog::draw_open_map(&p, &l, path),
                    Overlay::SaveAs { path } => menu::dialog::draw_save_as(&p, &l, path),
                    // 导出两个模态与另存为**同一套排版**，只是标题/提示/按钮文案不同
                    Overlay::ExportSelection { path } => {
                        menu::dialog::draw_export_selection(&p, &l, path)
                    }
                    Overlay::ExportMinimap { path, size } => {
                        // 画的时候也要地图格数：尺寸非法 ⇒「导出」按钮画灰（官方 IsEnabled）
                        let cells = self
                            .doc
                            .doc
                            .as_ref()
                            .map(|d| (u32::from(d.map.width), u32::from(d.map.height)))
                            .unwrap_or((0, 0));
                        menu::dialog::draw_export_minimap(&p, &l, path, size, cells)
                    }
                    Overlay::CellProps {
                        area,
                        文本,
                        focus,
                        error,
                        有门字节,
                        ..
                    } => {
                        // 头行要地图尺寸来算"单元格 x,y / 选区 N 格"（与落笔同一份夹紧口径）
                        let 头行 = match self.doc.doc.as_ref() {
                            Some(d) => crate::cell_props::头行(&d.map, *area),
                            None => String::new(),
                        };
                        menu::cellprops::draw(
                            &p,
                            &l,
                            &头行,
                            文本,
                            *focus,
                            error.as_deref(),
                            *有门字节,
                        )
                    }
                    // 工具 → 地图扩边（A-015 Expand）：440 宽模态，摘要实时反映四个输入。
                    Overlay::ExpandMap { 文本, focus, error } => {
                        let cells = self
                            .doc
                            .doc
                            .as_ref()
                            .map(|d| (d.map.width, d.map.height))
                            .unwrap_or((0, 0));
                        let 判定 = menu::expand::校验(文本, cells);
                        menu::expand::draw(&p, &l, 文本, *focus, &判定, error.as_deref())
                    }
                    // 工具 → 检查地图（A-015）：760×480 只读诊断面板。
                    // 陈旧判定：浮层里存的是**检查那一刻**的 `map_gen`，与当前值不同 ⇒
                    // summary 换成「请重新检查」并不再画列表（官方同样不填 ListBox）。
                    Overlay::CheckMap {
                        report,
                        selected,
                        scroll,
                        gen,
                        has_libs,
                    } => {
                        let stale = *gen != self.doc.map_gen;
                        menu::check_map::draw(
                            &p, &l, report, *selected, *scroll, stale, *has_libs,
                        )
                    }
                    Overlay::Shortcuts => menu::dialog::draw_shortcuts(&p, &l),
                    // 帮助 → 关于 Crystal Studio：只读模态（产品名/版本/上游/许可证/构建档位）
                    Overlay::About => menu::about::draw(&p, &l),
                    Overlay::None => {}
                }

                // 窗口尺寸自检：与常量表不符说明窗口没开成 1500×950
                debug_assert!(
                    (screen.width() - m::WINDOW_W).abs() < 1.0 || screen.width() >= m::WINDOW_MIN_W,
                    "窗口宽度 {} 小于最小宽度 {}",
                    screen.width(),
                    m::WINDOW_MIN_W
                );
            });

        // ⑧ 标题同步（`*` 要上 OS 窗口标题）+ 截图流程
        self.sync_window_title(ctx);
        self.handle_screenshot(ctx);

        self.update_ms += t0.elapsed().as_secs_f64() * 1000.0;
    }
}

impl MireditApp {
    /// 自动化取证运行（`--shot` / `--autodrag` / `--palette-autoscroll`）期间**忽略鼠标输入**。
    ///
    /// 取证要的是一张**可复现**的图：外部鼠标滚轮会改缩放、误点会在图上落笔，
    /// 两者都会让"同一命令两次跑出不同截图"（实测：同一命令行两次跑，缩放一次 200%、
    /// 一次 133%，因为期间收到了一次滚轮事件）。所以这两种模式下把交互整段关掉 ——
    /// 它们本来就不该由人来点。
    fn input_locked(&self) -> bool {
        self.auto.shot.is_some() || self.auto.autodrag.is_some() || self.auto.palette_scroll.is_some()
    }

    /// 这一帧要不要画**印章待放置轮廓**：`(锚点, 包围盒)`。
    ///
    /// 四条同时成立才画：① 处于待放置态；② **按住 `Alt`**；③ 鼠标在某格上；
    /// ④ 目录给得出包围盒（`resolve` 成功）。
    ///
    /// 为什么"按住 Alt 才画"而不是常驻：轮廓是一块矩形描边，常驻会一直盖在地图上；
    /// 按住 `Alt` 才看是**临时预览**的语义（任务书 §三.5 的推荐做法）。
    /// 为什么缺 ④ 就不画：没有包围盒就没有真实几何，随便框一个"大概大小"就是假数据。
    /// **Alt 落位预览**（A-011）：内容（半透明）+ 轮廓 + 受阻诊断。
    ///
    /// 上游 `MapViewport.StampRendering.cs:58-109` 的三段：`PushOpacity(Allowed ? 0.55 : 0.18)`
    /// → 内容 → 轮廓 → `blocked` 非空才画红/琥珀诊断。
    ///
    /// 本方法只做**宿主侧**三件事，判据全在内核/纯函数：
    /// ① Alt 门控（保留既有差异：上游 armed 常驻，本仓要按住 Alt）；
    /// ② 懒渲染内容纹理（按 `armed.id` 缓存；失败/无 `cells` ⇒ `tex: None` ⇒ 只画轮廓，
    ///    **绝不画假像素**）；
    /// ③ 把内核 `check_stamp_placement` 的受阻格原样带出去给绘制层。
    fn stamp_preview(&mut self, ctx: &egui::Context) -> Option<crate::canvas_overlay::PreviewData> {
        // ①b 格表**懒加载兜底**：`arm_selected` 已经试过一次；若那次没读到（路径未解析出来等），
        //    这里再按当前选中项补一次 —— 预览只是只读展示，多读一次盘不影响任何语义。
        let 缺格 = self
            .armed
            .as_ref()
            .map(|a| a.cells.is_none())
            .unwrap_or(false);
        if 缺格 {
            if let Some(id) = self.armed.as_ref().map(|a| a.id.clone()) {
                if let Some(p) = self.path_of_stamp(&id) {
                    let 号段 = self
                        .doc
                        .assets
                        .号段()
                        .unwrap_or(miredit_core::库号号段::Wemade);
                    let cells = stamp_place::read(&p, 号段).ok().map(|f| f.cells);
                    if let Some(a) = self.armed.as_mut() {
                        a.cells = cells;
                    }
                }
            }
        }
        let armed = self.armed.as_ref()?;
        // Alt 门控（保留既有差异：上游 armed 常驻）；`--preview-alt` 是取证用的伪造开关
        if !self.preview_alt && !ctx.input(|i| i.modifiers.alt) {
            return None;
        }
        let anchor = self.hover?;
        let d = self.doc.doc.as_ref()?;
        // 诊断（内核只读检查）：没有格表 ⇒ 只有轮廓、无诊断
        let 状态 = armed.cells.as_deref().and_then(|cells| {
            stamp_place::preview_state(&d.map, cells, anchor.0, anchor.1, true)
        });
        // 内容纹理：按 id 懒加载（换选失效）
        let mut tex = match &self.preview_tex {
            Some((id, t, w, h)) if id == &armed.id => Some((*t, *w, *h)),
            _ => None,
        };
        if tex.is_none() {
            if let (Some(cells), Some(src)) = (
                armed.cells.as_deref(),
                self.doc.assets.source().map(|s| s as &dyn mir2_assets::SpriteSource),
            ) {
                // 与 `stamp_panel::thumbs` 同一套搬运（掩 bit15 交给渲染库）
                let 位置格: Vec<miredit_render::stamp::PositionedCell> = cells
                    .iter()
                    .map(|c| miredit_render::stamp::PositionedCell {
                        dx: c.dx,
                        dy: c.dy,
                        bk: c.bk & 0x7FFF,
                        mid: c.mid & 0x7FFF,
                        fr: c.fr & 0x7FFF,
                        bk_lib: c.bk_lib,
                        mid_lib: c.mid_lib,
                        fr_lib: c.fr_lib,
                    })
                    .collect();
                // 上游预览是**1:1 原生像素**：scale 1、Nearest、**margin 0**
                let opts = miredit_render::stamp::StampRenderOptions {
                    box_px: None,
                    scale: 1.0,
                    filter: miredit_render::stamp::FilterClass::Nearest,
                    margin_px: 0,
                };
                if let Ok(img) = miredit_render::stamp::render_stamp(&位置格, src, &opts) {

                    if img.width > 0 && img.height > 0 && !img.rgba.is_empty() {
                        let 色 = egui::ColorImage::from_rgba_unmultiplied(
                            [img.width as usize, img.height as usize],
                            &img.rgba,
                        );
                        let t = ctx.load_texture(
                            format!("miredit-stamp-preview-{}", armed.id),
                            色,
                            egui::TextureOptions::NEAREST,
                        );
                        tex = Some((t.id(), img.width, img.height));
                        self.preview_tex = Some((armed.id.clone(), t.id(), img.width, img.height));
                    }
                }
            }
        }
        // 包围盒：目录给的优先；没有就用格表算（仍然只有轮廓）
        let bounds = armed
            .bounds
            .or_else(|| armed.cells.as_deref().map(stamp_place::格表包围盒))?;

        Some(crate::canvas_overlay::PreviewData {
            anchor,
            bounds,
            tex,
            opacity: 状态
                .as_ref()
                .map(|s| s.不透明度)
                .unwrap_or(stamp_place::不透明度允许),
            blocked: 状态.map(|s| s.检查.blocked).unwrap_or_default(),
        })
    }

    /// 这一帧要不要画**待放置徽标**（跟着鼠标走的小标签）。
    fn armed_badge(&self) -> Option<String> {
        let a = self.armed.as_ref()?;
        Some(tx::fill(tx::STAMP_ARMED_BADGE_TMPL, &[("name", &a.name)]))
    }

    /// 处理全部点击/拖拽。
    fn interact(&mut self, ui: &mut egui::Ui, l: &Layout) {
        if self.input_locked() {
            return;
        }
        // ⓪ **浮层是模态**：打开地图 / 快捷键浮层在的时候，其它一切交互让位
        if self.handle_overlay(ui, l) {
            return;
        }
        // ⓪b **菜单**：标签点击 → 展开/收起/切换；弹层点击 → 执行。
        //    展开态下其余交互整体让位（菜单是"模态式"的：这一次点击归它，
        //    否则点弹层外面既会收起弹层、又会顺手在画布上落一笔）。
        let was_open = self.menu.open_index().is_some();
        // 子菜单状态（「最近打开」的行数靠它）——与 `draw` 用**同一份**，
        // 否则画出来的行与能点的行会错位。
        let sub = self.sub_state();
        match menu::interact_with(ui, l, &mut self.menu, &sub) {
            menu::Click::Run(a) => {
                self.dispatch_menu(a, l.viewport);
                ui.ctx().request_repaint();
                return;
            }
            // 点了一条最近地图 ⇒ 打开**那一条**（与键盘回车同一条路）
            menu::Click::RunRecent(i) => {
                self.open_recent_at(i, l.viewport);
                ui.ctx().request_repaint();
                return;
            }
            menu::Click::Toggled => {
                ui.ctx().request_repaint();
                if was_open {
                    return;
                }
            }
            menu::Click::None => {}
        }
        if self.menu.open_index().is_some() {
            return;
        }
        // ① 标题栏拖拽
        titlebar::handle_drag(ui, l);

        // ② 三个窗口按钮：只有最右的 × 真正关闭窗口（最小化/最大化仍是装饰）
        //
        // ⚠ **关闭拦截**：有未保存修改时第一次点 × 只弹模态（不关窗），
        //    再点一次才真放弃 —— 与 `ui_text::CLOSE_CONFIRM` 的文案一字对应。
        if ui
            .interact(
                titlebar::close_rect(l),
                ui.id().with("caption_close"),
                Sense::click(),
            )
            .clicked()
        {
            if self.edit.dirty() && !self.confirm_close {
                self.confirm_close = true;
                ui.ctx().request_repaint();
            } else {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // ③ 菜单栏 hover 高亮：本版起由 `menu::interact`（点击 + 弹层）接管，
        //    这里不再单独跑一遍 `menubar::handle_hover`（两个命中盒会打架）。

        // ③b 素材面板：页签 / 库下拉 / 搜索 / 网格点选 / 分页
        //
        // ⚠ 「分类」页签（印章浏览器）**只借用页签行**：其余交互由 `stamp_panel` 自己处理
        //   （两套下拉/搜索/网格语义不同，不能共用同一份命中测试）。
        // ⚠ 资源面板被隐藏时**整段交互跳过**（否则看不见的控件还能被点到）。
        let src = self.doc.assets.source();
        if !self.panels.get(PanelId::Palette) {
            // 面板隐藏：不响应任何点击
        } else if self.palette.tab() == stamp_panel::TAB_INDEX {
            self.palette.interact_tabs(ui, l);
            if let StampAction::Pick = self.stamps.interact(ui, l) {
                self.report_stamp_pick();
                ui.ctx().request_repaint();
            }
        } else {
            match self.palette.interact(ui, l, src) {
                PaletteAction::SetBrush(key) => {
                    if let Some(b) = self.edit.set_brush_from_palette(key.lib, key.index) {
                        // 点选即切到画笔（任务书 §四.4）
                        self.selected_tool = edit::TOOL_BRUSH_IDX;
                        self.edit.select(edit::TOOL_BRUSH_IDX);
                        let stem = self
                            .palette
                            .current()
                            .map(|e| e.stem.clone())
                            .unwrap_or_else(|| tx::VAL_UNKNOWN.to_owned());
                        eprintln!(
                            "{}",
                            tx::fill(
                                tx::DIAG_PALETTE_PICK,
                                &[
                                    ("stem", &stem),
                                    ("fam", b.family_name()),
                                    ("img", &b.sprite().to_string()),
                                    ("lib", &edit::brush_lib_text(None, &b)),
                                ]
                            )
                        );
                        ui.ctx().request_repaint();
                    } else {
                        // 面板侧已经拦过图号上限，走到这里说明约束又变了：如实报，不假装成功
                        eprintln!("{}", tx::PALETTE_FRAME_TOO_BIG);
                    }
                }
                PaletteAction::None => {}
            }
        }

        // ③c **地图列表面板**：点一行 = 打开那张图。
        //
        // 走的是 [`Self::open_map_path`]（= 「打开地图…」模态点「打开」时的同一个函数
        // → [`menu::action::open_path`] → `Document::load`），**不另写加载旁路**。
        // 「刷新」重扫同一个根（换根由 `follow_map_dir` 在打开别的目录的图时自动做）。
        if self.panels.get(PanelId::Maps) {
            match maplist::interact(ui, l, &mut self.maps) {
                maplist::Action::Open(i) => {
                    let target = self.maps.entries().get(i).map(|e| e.path.clone());
                    if let Some(p) = target {
                        let text = p.display().to_string();
                        self.open_map_path(&text, l.viewport);
                        ui.ctx().request_repaint();
                        return;
                    }
                }
                maplist::Action::Refresh => {
                    let root = self.maps.root().map(|p| p.to_path_buf());
                    if let Some(r) = root {
                        self.maps.start(&r);
                    }
                    ui.ctx().request_repaint();
                    return;
                }
                maplist::Action::None => {}
            }
        }

        // ④ 10 个工具按钮（点哪个就切哪个；未接线的工具照样能选，只是落格无效果）
        //    ⚠ 工具面板隐藏时整段跳过（看不见的按钮不该还能点）。
        if self.panels.get(PanelId::Tools) {
            for (i, r) in l.tool_buttons.iter().enumerate() {
                if ui
                    .interact(*r, ui.id().with(("tool_btn", i)), Sense::click())
                    .clicked()
                {
                    self.selected_tool = i;
                    self.edit.select(i);
                    // 换了工具就退出待放置态：待放置态的语义是"下一次左键 = 落位"，
                    // 用户既然去点别的工具，说明不想放了（否则点下去会莫名其妙落一枚印章）。
                    if self.armed.is_some() {
                        self.armed = None;
                        self.op = None;
                    }
                }
            }
        }

        // ⑤ 5 个图层行：**两个互不影响的动作**
        //    · 点眼睛 = 切该行可见性（前三行 → `LayerMask`；后两行 → `CollisionOverlay`）
        //    · 点行内其它位置 = 把该行设为**当前编辑层**（决定落笔写哪个字段/哪个位）
        //    五行全部接线（2026-11-04 审计 P0-9）：只有越界行才是"点了没反应"。
        //    ⚠ 图层面板隐藏时整段跳过。
        for (i, r) in l.layer_rows.iter().enumerate() {
            if !self.panels.get(PanelId::Layers) {
                break;
            }
            let resp = ui.interact(*r, ui.id().with(("layer_row", i)), Sense::click());
            if !resp.clicked() {
                continue;
            }
            let eye = dock_left::eye_rect(l, i).expand(4.0);
            let on_eye = resp
                .interact_pointer_pos()
                .map(|p| eye.contains(p))
                .unwrap_or(false);
            match dock_left::dispatch_click(i, on_eye) {
                dock_left::LayerClick::ToggleVisibility(_) => {
                    // 前三行动 `LayerMask`、后两行动 `CollisionOverlay`；
                    // 两者都进画布缓存键 ⇒ 下一帧 `ensure` 必然重渲一次（不是"点了没反应"）
                    if dock_left::toggle_visibility(&mut self.view.mask, &mut self.view.overlay, i) {
                        ui.ctx().request_repaint();
                    }
                }
                dock_left::LayerClick::SetEditLayer(layer) => {
                    self.edit.set_layer(layer);
                    // 状态栏的「当前层 / 落笔字段」要立刻跟着变
                    self.text = screen_text::build(
                        &self.doc,
                        &self.view,
                        self.hover,
                        &self.lines,
                        &self.edit,
                    );
                    ui.ctx().request_repaint();
                }
                dock_left::LayerClick::Ignore => {}
            }
        }

        // ⑥ 画布：中键拖动 / 空格+左键拖动 → 平移；否则左键 = 工具落格；滚轮 → 以鼠标为中心缩放
        let resp = ui.interact(l.viewport, ui.id().with("canvas"), Sense::click_and_drag());
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        let panning = resp.dragged_by(egui::PointerButton::Middle)
            || (space && resp.dragged_by(egui::PointerButton::Primary));
        if panning {
            self.view.pan_by(resp.drag_delta());
            // 平移中途打断了一笔画：**把它当一步结算掉**（已写进地图的字节必须有对应的撤销步，
            // 否则这些字节既撤销不了、也不会让 `dirty` 变真 ⇒ 关窗时不提示保存，直接丢改动）
            if self.edit.stroke_active() {
                let n = self.edit.end();
                if n > 0 {
                    ui.ctx().request_repaint();
                }
            }
        } else {
            self.handle_tool_pointer(ui, l);
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.5 {
                if let Some(pos) = ui.ctx().pointer_hover_pos() {
                    let notches = scroll / m::SCROLL_PX_PER_NOTCH;
                    let factor = m::ZOOM_PER_NOTCH.powf(notches);
                    self.view.zoom_at(l.viewport, pos, factor);
                }
            }
        }

        // ⑦ 缩放浮层三个可点区域（放在画布之后 = 盖在画布之上，先吃到点击）
        if ui
            .interact(
                canvas::zoom_minus_rect(l),
                ui.id().with("zoom_minus"),
                Sense::click(),
            )
            .clicked()
        {
            let c = l.viewport.center();
            self.view.zoom_at(l.viewport, c, 1.0 / m::ZOOM_PER_NOTCH);
        }
        if ui
            .interact(
                canvas::zoom_plus_rect(l),
                ui.id().with("zoom_plus"),
                Sense::click(),
            )
            .clicked()
        {
            let c = l.viewport.center();
            self.view.zoom_at(l.viewport, c, m::ZOOM_PER_NOTCH);
        }
        if ui
            .interact(
                canvas::zoom_fit_rect(l),
                ui.id().with("zoom_fit"),
                Sense::click(),
            )
            .clicked()
        {
            if let Some(d) = self.doc.doc.as_ref() {
                self.view.fit_map(l.viewport, d.map.width, d.map.height);
            }
        }
        // ⑦b **「原始大小」按钮**（上游 `901dbe2f` 的 `OriginalSizeButton`，
        //     `MainWindow.axaml:22`，`Click="ResetZoomClick"`）。
        //
        //     走**同一条 action**：[`menu::model::Action::ActualSize`] ⇒ 与菜单
        //     「视图 → 实际大小」、`--menu-click view.actual_size` 完全同一条代码路径
        //     （回执文案、锚点、夹取规则都不可能出现两套）。
        if ui
            .interact(
                canvas::zoom_original_rect(l),
                ui.id().with("zoom_original"),
                Sense::click(),
            )
            .clicked()
        {
            self.dispatch_menu(menu::model::Action::ActualSize, l.viewport);
            ui.ctx().request_repaint();
        }
    }
}

/// 把 `--palette-*` 开关落到面板上（**启动时**一次性；走的都是 GUI 里的同一套方法）。
///
/// 为什么要有它：验收要的是"同一段代码路径、可复现的一次运行"。
/// 切库 / 滚到某行 / 搜图号 / 切页签 / 点选设笔刷全部由这里驱动，**不另写一套捷径**；
/// GUI（[`MireditApp::new`]）与无窗驱动器（[`crate::editrun::run`]）共用同一份。
pub fn apply_palette_cli(
    pal: &mut PaletteState,
    ed: &mut EditSession,
    cli: &Cli,
    src: Option<&mir2_assets::WilSource>,
) {
    pal.refresh_count(src);
    if let Some(stem) = cli.palette_lib.as_deref() {
        if pal.select_lib_by_stem(stem) {
            pal.refresh_count(src);
        } else {
            eprintln!(
                "{}",
                tx::fill(
                    tx::DIAG_BAD_ARG,
                    &[("arg", &format!("--palette-lib {stem}（没有这个库）"))]
                )
            );
        }
    }
    if let Some(row) = cli.palette_scroll {
        pal.set_scroll(row);
    }
    if let Some(t) = cli.palette_tab {
        pal.set_tab(t);
    }
    if let Some(q) = cli.palette_search.as_deref() {
        pal.push_text(q);
        eprintln!("{}", pal.submit_search());
    }
    if let Some(img) = cli.palette_pick {
        // 与鼠标点选**同一个** `pick`（三族都允许；族与图层的把关在落笔那一刻）
        if let PaletteAction::SetBrush(key) = pal.pick_frame(img) {
            match ed.set_brush_from_palette(key.lib, key.index) {
                Some(b) => eprintln!(
                    "{}",
                    tx::fill(
                        tx::DIAG_PALETTE_PICK,
                        &[
                            (
                                "stem",
                                pal.current()
                                    .map(|e| e.stem.as_str())
                                    .unwrap_or(tx::VAL_UNKNOWN),
                            ),
                            ("fam", b.family_name()),
                            ("img", &b.sprite().to_string()),
                            ("lib", &edit::brush_lib_text(None, &b)),
                        ]
                    )
                ),
                None => eprintln!("{}", tx::PALETTE_FRAME_TOO_BIG),
            }
        } else {
            eprintln!(
                "{}",
                tx::fill(
                    tx::DIAG_BAD_ARG,
                    &[("arg", &format!("--palette-pick {img}（该帧不能设为笔刷）"))]
                )
            );
        }
    }
    if cli.palette_combo {
        pal.set_combo_open(true);
    }
}

/// 把 `--stamp-*` 落到印章面板上（**目录装载完成后**才调，见 [`PendingStamp`]）。
///
/// 走的都是 GUI 里的同一套方法（[`StampPanel::set_filter_key`] / `set_search` /
/// `set_page` / `pick_slot`），**不另写一条捷径** —— 这样"命令行能复现的，鼠标也能做出来"。
///
/// 只要有任何一个 `--stamp-*` 开关生效，就**顺带把页签切到「分类」**：
/// 否则用户看到的是一张"改了但看不见"的图（截图验收会因此拍空）。
fn apply_stamp_cli(stamps: &mut StampPanel, pal: &mut PaletteState, p: &PendingStamp) {
    let mut any = false;
    if let Some(kind) = p.kind.as_deref() {
        any = true;
        if !stamps.set_filter_key(kind) {
            eprintln!(
                "{}",
                tx::fill(
                    tx::DIAG_BAD_ARG,
                    &[("arg", &format!("--stamp-kind {kind}（没有这个分类）"))]
                )
            );
        }
    }
    if let Some(q) = p.search.as_deref() {
        any = true;
        stamps.set_search(q);
    }
    if let Some(page) = p.page {
        any = true;
        // 命令行给的是**1 基页码**（与界面上的「第 N 页」一致），内部是 0 基
        stamps.set_page(page.saturating_sub(1));
    }
    if let Some(slot) = p.pick {
        any = true;
        if !stamps.pick_slot(slot) {
            eprintln!(
                "{}",
                tx::fill(
                    tx::DIAG_BAD_ARG,
                    &[("arg", &format!("--stamp-pick {slot}（当前页没有这一格）"))]
                )
            );
        }
    }
    if any {
        pal.set_tab(stamp_panel::TAB_INDEX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 资产根缺省值：本机真实根（**整个测试模块里只出现这一处**）。换机器用 `MIREDIT_ASSET_ROOT`。
    const 缺省资产根: &str = r"E:\GodM2";

    /// 资产根：`MIREDIT_ASSET_ROOT` → 缺省 [`缺省资产根`]。
    ///
    /// 根目录不存在（公开仓/PR 的机器上没有游戏素材）⇒ `None`，调用方打印 `[skip]` 并返回。
    /// 只做 `var_os` + `PathBuf`，不引入任何第三方库。
    fn 资产根() -> Option<PathBuf> {
        let root = std::env::var_os("MIREDIT_ASSET_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(缺省资产根));
        root.is_dir().then_some(root)
    }

    /// 用例专用的设置文件路径（**绝不许碰真实用户设置**）。
    ///
    /// # 为什么必须隔离（2026-11-05 实测踩到）
    ///
    /// 「最近打开」接线后，[`MireditApp::open_map_path`] 成功时会 `remember_recent` →
    /// **写设置文件**。不隔离的话，跑一次 `cargo test` 就会把开发机
    /// `%LOCALAPPDATA%\miredit\settings.json` 里的 `recent` 冲成测试用的地图路径
    /// —— 本轮实测发生过：一次 `LOCALAPPDATA` 指向临时目录的全量测试，
    /// 把 `10.map` 写进了那个"应该是空"的目录（正是它暴露了这个坑）。
    fn 用例设置路径(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("miredit_gui_app_settings_{tag}.json"))
    }

    fn 空应用() -> MireditApp {
        let settings = 用例设置路径("empty_app");
        // 每条用例都从"没有设置文件"开始（否则上一轮跑剩的 `recent` / `dark` 会漏进来，
        // 结果就不可复现了）
        let _ = std::fs::remove_file(&settings);
        MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            settings: Some(settings),
            ..Default::default()
        })
    }

    /// 打不开地图时：`doc` 为空、`map_error` 有原因、缩放仍在合法区间（不能是 NaN/0）。
    #[test]
    fn 地图失败时不崩且缩放合法() {
        let a = 空应用();
        assert!(a.doc.doc.is_none());
        assert!(a.doc.map_error.is_some());
        assert!(a.view.scale >= m::CANVAS_MIN_SCALE && a.view.scale <= m::CANVAS_MAX_SCALE);
        assert_eq!(a.hover, None);
        assert_eq!(a.lines.head, "—");
    }

    /// `--layers` 要真的落到 `LayerMask` 上（否则验收门 4 的证据就是假的）。
    #[test]
    fn 初始图层开关生效() {
        let a = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            layers: Some([true, true, false]),
            ..Default::default()
        });
        assert!(a.view.mask.ground);
        assert!(a.view.mask.middle);
        assert!(!a.view.mask.front, "门4 要的就是关掉物件层");
    }

    /// `--scale` 越界要夹住（不许把 0.01 传进渲染）。
    #[test]
    fn 初始缩放被夹住() {
        let a = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            scale: Some(0.01),
            ..Default::default()
        });
        assert!((a.view.scale - m::CANVAS_MIN_SCALE).abs() < 1e-6);
    }

    /// `--hover` 伪造悬停优先于真实鼠标（截图取证靠它）。
    #[test]
    fn 伪造悬停优先() {
        let a = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            hover: Some((100, 100)),
            ..Default::default()
        });
        assert_eq!(a.forced_hover, Some((100, 100)));
    }

    /// **改笔刷尺寸不重渲**（任务书 §三.1 的最后一条）：尺寸只改"下一次落笔多大一块"，
    /// 已写进地图的字节一个都不动 ⇒ `map_gen` 不许变（画布缓存键里有它，变了就重渲一次）。
    #[test]
    fn 改笔刷尺寸不重渲() {
        let mut a = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            brush_size: Some(7),
            ..Default::default()
        });
        // `--brush-size 7` 真的落到会话上，且**没碰**地图代号
        assert_eq!(a.edit.brush_size(), 7);
        assert_eq!(a.doc.map_gen, 1, "--brush-size 不许改地图代号");
        assert_eq!(a.edit.undo_len(), 0, "--brush-size 不许产生撤销步");
        assert!(!a.edit.dirty());

        // 再改几次：地图代号与撤销栈都必须纹丝不动
        let gen = a.doc.map_gen;
        for n in [3usize, 5, 15, 31, 2, 99] {
            let got = a.edit.set_brush_size(n);
            assert_eq!(got, a.edit.brush_size());
            assert_eq!(a.doc.map_gen, gen, "改尺寸到 {n} 不该重渲");
        }
        assert_eq!(a.edit.undo_len(), 0);
        // 文案里的胶囊跟着变（这是"尺寸真的接上了"的可见证据）
        a.text = screen_text::build(&a.doc, &a.view, a.hover, &a.lines, &a.edit);
        assert_eq!(a.text.pill, "31×31", "99 会被夹到 31");
    }

    /// 关闭拦截模态：底板必须整块落在画布内，且那句中文放得进底板（不留半句被裁）。
    ///
    /// 这条挡的是"只在关窗时才会出现、平时截图看不到"的控件 —— 它要是排歪了，
    /// 只有真去关窗才发现，那时用户已经以为"程序没反应"。
    #[test]
    fn 关闭拦截模态放得下文案() {
        let l = Layout::compute(Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        let c = Rect::from_center_size(l.viewport.center(), Vec2::new(m::MODAL_W, m::MODAL_H));
        assert!(
            l.viewport.contains_rect(c),
            "模态底板 {c:?} 超出了画布 {:?}",
            l.viewport
        );
        let p = crate::fonts::measure_ctx();
        let painter = egui::Painter::new(
            p,
            egui::LayerId::debug(),
            Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(m::WINDOW_W, m::WINDOW_H)),
        );
        let w = pt::text_w(&painter, tx::CLOSE_CONFIRM, m::MODAL_FONT);
        assert!(
            w <= c.width() - 40.0,
            "提示文案宽 {w:.0} 放不进底板 {:.0}（需留左右各 20 的呼吸）",
            c.width()
        );
    }

    /// 自动化取证运行（`--shot` / `--autodrag`）必须锁鼠标输入：否则外部滚轮会把缩放改掉，
    /// 同一命令两次跑出两张不同的图（实测 200% 与 133% 都出现过）。
    #[test]
    fn 取证运行锁鼠标输入() {
        let mut cli = Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            ..Default::default()
        };
        assert!(!MireditApp::new(&cli).input_locked(), "普通运行不锁");
        cli.shot = Some(PathBuf::from(r"I:\tmp\x.png"));
        assert!(MireditApp::new(&cli).input_locked(), "带 --shot 必须锁");
        cli.shot = None;
        cli.autodrag = Some(1.0);
        assert!(MireditApp::new(&cli).input_locked(), "带 --autodrag 必须锁");
    }

    // ───────────────────── 菜单系统 / 深色模式（2026-11-05） ─────────────────────

    /// 造一个"有图"的应用（用内存里的 2×2 合成图，不依赖磁盘上的 0.map）。
    ///
    /// 为什么要合成图：`0.map` 有 5.88 MB，每条用例都读一遍会把测试拖到分钟级；
    /// 而这里要验的是"菜单动作有没有改到文档/视口/面板状态"，与地图多大无关。
    fn 有图应用() -> MireditApp {
        let mut app = 空应用();
        let mut b = vec![0u8; 52 + 12 * 16];
        b[0..2].copy_from_slice(&4u16.to_le_bytes());
        b[2..4].copy_from_slice(&4u16.to_le_bytes());
        app.doc.doc = Some(
            crate::mapdoc::MapDoc::from_bytes(std::path::Path::new(r"E:\x\4.map"), b)
                .expect("4×4 合成图应能解析"),
        );
        app.doc.map_error = None;
        if let Some(d) = app.doc.doc.as_ref() {
            let _ = app.edit.bind_document(&d.path, d.orig_bytes());
        }
        app
    }

    /// 画布矩形（与 `update` 里同一份布局）。
    fn 画布() -> Rect {
        Layout::compute(Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(m::WINDOW_W, m::WINDOW_H),
        ))
        .viewport
    }

    /// 跑一帧 `handle_keys`，这一帧里按了一下 `key`（**真键盘路径**，不是直接调动作）。
    ///
    /// 为什么这样写：`handle_keys` 读的是 `ctx.input(...)`，只有把事件塞进
    /// `ctx.run(RawInput{events})` 的那一帧才读得到 —— 与 `menu::dialog` 的模态用例同一套做法。
    fn 按一次键(app: &mut MireditApp, key: egui::Key) {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.handle_keys(ctx));
    }

    /// **无图时点「需要先开图」的项**：回执是「需要先开图：…」，且文档一个字节都不动。
    #[test]
    fn 无图时菜单项被拦且不改文档() {
        let mut app = 空应用();
        let canvas = 画布();
        for a in [
            menu::model::Action::SaveMap,
            menu::model::Action::Undo,
            menu::model::Action::ZoomIn,
            menu::model::Action::FitMap,
        ] {
            app.dispatch_menu(a, canvas);
            let (text, warn) = app.op.clone().expect("必须有回执");
            assert!(warn, "{} 的拒绝回执要用警示色", a.id());
            assert!(
                text.contains("需要先开图"),
                "{} 的回执没说清原因：{text}",
                a.id()
            );
            assert!(app.doc.doc.is_none(), "{} 不该造出一张图", a.id());
        }
    }

    /// **灰显项点了要说原因**，且地图/视口/面板一个都不许变。
    #[test]
    fn 灰显项只给原因不改任何状态() {
        let mut app = 有图应用();
        let canvas = 画布();
        let scale = app.view.scale;
        let gen = app.doc.map_gen;
        let panels = app.panels;
        for a in [
            menu::model::Action::Exit,
            menu::model::Action::NewMap,
            menu::model::Action::GenTown,
            menu::model::Action::LockLayout,
        ] {
            app.dispatch_menu(a, canvas);
            let (text, warn) = app.op.clone().expect("必须有回执");
            assert!(warn, "{} 的拒绝回执要用警示色", a.id());
            assert!(text.starts_with("未实现："), "{} 的回执开头不对：{text}", a.id());
            assert!(text.len() > 8, "{} 的原因太短：{text}", a.id());
        }
        assert_eq!(app.view.scale, scale, "灰显项不该改缩放");
        assert_eq!(app.doc.map_gen, gen, "灰显项不该改地图（map_gen 是改图的唯一记号）");
        assert_eq!(app.panels, panels, "灰显项不该改面板显隐");
        assert!(app.overlay == Overlay::None, "灰显项不该弹浮层");
    }

    /// **深色模式**：翻转开关 + 写设置文件 + `theme::set_dark` 同步（三件事必须一起发生）。
    #[test]
    fn 深色模式翻转并落盘() {
        let dir = std::env::temp_dir().join("miredit_dark_test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        let mut app = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            settings: Some(path.clone()),
            ..Default::default()
        });
        assert!(!app.dark, "默认浅色");
        assert!(!t::is_dark());
        let canvas = 画布();
        app.dispatch_menu(menu::model::Action::DarkMode, canvas);
        assert!(app.dark, "菜单没翻转开关");
        assert!(t::is_dark(), "theme::set_dark 没跟上（绘制会仍是浅色）");
        let saved = std::fs::read_to_string(&path).expect("设置文件必须真写出来");
        assert!(saved.contains("\"dark\": true"), "文件内容不对：{saved}");
        // 再点一次 → 关，文件跟着变
        app.dispatch_menu(menu::model::Action::DarkMode, canvas);
        assert!(!app.dark);
        assert!(!t::is_dark());
        let saved = std::fs::read_to_string(&path).expect("再写一次");
        assert!(saved.contains("\"dark\": false"), "文件内容不对：{saved}");
        let _ = std::fs::remove_dir_all(&dir);
        // 收尾：把线程局部的主题恢复成浅色，别污染同进程的其它用例
        t::set_dark(false);
    }

    /// **单元格网格**（视图 → 单元格网格）：`--cell-grid` / `--no-cell-grid` 置初值、
    /// 菜单项翻转、回执说人话、菜单勾跟着真状态，且**不动地图一个字节**（网格是叠加层，不是编辑）。
    ///
    /// 这条盯的是"三处必须同一份真源"：`app.grid`（画布读它）、
    /// 菜单勾（`model::check` 读它）、回执文案。任何一处没跟上，
    /// 用户看到的就是"点了没反应"或"勾着却没有线"。
    #[test]
    fn 单元格网格可开可关且不动地图() {
        // ① 缺省**开**：官方 `MainWindow.axaml:5` 的 `GridToggle` 是
        //    `ToggleType="CheckBox" IsChecked="True"`。
        //    ⚠ 2026-11-05 订正：旧版这里断言"默认必须是关"（当时的理由写在
        //    `grid` 模块文档 §五），审查（内部报告《审查-菜单 39 项》，已移出公开仓
        //    缺陷 4）判定那是**形态偏离官方** ⇒ 改成默认开，**不是放宽断言**。
        let mut app = 有图应用();
        assert!(app.grid, "官方 IsChecked=True ⇒ 默认必须开");
        let canvas = 画布();
        let gen = app.doc.map_gen;

        // ② 点一次 → 关；回执说清是关
        app.dispatch_menu(menu::model::Action::CellGrid, canvas);
        assert!(!app.grid, "菜单项没翻转开关");
        let (text, warn) = app.op.clone().expect("必须有回执");
        assert!(!warn, "关网格不是拒绝");
        assert!(text.contains('关'), "回执没说清开还是关：{text}");
        assert_eq!(
            app.doc.map_gen, gen,
            "网格是叠加层，不许改地图（map_gen 不许变）"
        );

        // ③ 菜单勾必须跟着真状态（勾着却无线 / 有线却没勾，都是假信息）
        assert_eq!(
            menu::model::check(
                menu::model::Action::CellGrid,
                app.dark,
                app.grid,
                &app.panels
            ),
            Some(false),
            "网格关着时不许画勾"
        );

        // ④ 再点一次 → 开（回执要写出真实格尺寸）
        app.dispatch_menu(menu::model::Action::CellGrid, canvas);
        assert!(app.grid, "再点没打开");
        let (text, _) = app.op.clone().expect("回执");
        assert!(text.contains('开'), "{text}");
        assert!(text.contains("48×32"), "开网格的回执要写出真实格尺寸：{text}");
        assert_eq!(app.doc.map_gen, gen);
        assert_eq!(
            menu::model::check(
                menu::model::Action::CellGrid,
                app.dark,
                app.grid,
                &app.panels
            ),
            Some(true),
            "网格开着时菜单必须画勾"
        );

        // ⑤ 两个开关都置初值（与菜单翻转的是**同一个字段**）
        //    `--cell-grid` = 显式开（默认已开 ⇒ 幂等）；`--no-cell-grid` 才是关的手段
        let on = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            cell_grid: true,
            ..Default::default()
        });
        assert!(on.grid, "--cell-grid 没生效");
        assert_eq!(on.doc.map_gen, 1, "--cell-grid 不许改地图代号");
        let off = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            no_cell_grid: true,
            ..Default::default()
        });
        assert!(!off.grid, "--no-cell-grid 必须关掉网格");
        // 两个都给时 `--no-cell-grid` 优先（关是"更明确"的那个诉求）
        let both = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            cell_grid: true,
            no_cell_grid: true,
            ..Default::default()
        });
        assert!(!both.grid, "--no-cell-grid 优先级最高");
    }

    /// **视图 → 地砖信息悬浮**（A-008）：菜单翻转 / 回执说人话 / 勾跟真状态 /
    /// 不动地图 / **一次都不写设置文件** / 浮层真的排得出来。
    ///
    /// 钉的是"三处同一份真源"：`app.cell_hover`（本帧画不画浮层）、
    /// 菜单勾（`menu::model::check_with_hover` 的 `hover` 参数）、回执文案。
    /// 另外单独钉**不持久化**：官方 `CellHoverToggle` 只是运行期勾选态，
    /// 所以本开关连设置文件都不许碰（判据 = 文件根本没被创建）。
    #[test]
    fn 地砖信息悬浮可开可关且不落盘() {
        let dir = std::env::temp_dir().join("miredit_cell_hover_test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        let mut app = 有图应用();
        app.settings_path = path.clone();
        let canvas = 画布();
        let l = Layout::compute(Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(m::WINDOW_W, m::WINDOW_H),
        ));

        // ① 缺省关（官方 `CellHoverToggle` 没写 `IsChecked`）
        assert!(!app.cell_hover, "官方没写 IsChecked ⇒ 默认必须关");
        assert!(app.hover_panel(&l).is_none(), "关着时一个像素都不许画");

        // ② 点一次 → 开；回执说清是开；地图一个字节不动
        let gen = app.doc.map_gen;
        app.dispatch_menu(menu::model::Action::CellHover, canvas);
        assert!(app.cell_hover, "菜单项没翻转开关");
        let (text, warn) = app.op.clone().expect("必须有回执");
        assert!(!warn, "开浮层不是拒绝");
        assert!(text.contains('开'), "回执没说清开还是关：{text}");
        assert_eq!(app.doc.map_gen, gen, "浮层是只读叠加，不许改地图");
        assert!(
            !path.exists(),
            "地砖信息悬浮**不持久化** ⇒ 不许写设置文件（`{}` 不该存在）",
            path.display()
        );

        // ③ 菜单勾跟着真状态
        assert_eq!(
            menu::model::check_with_hover(
                menu::model::Action::CellHover,
                app.dark,
                app.grid,
                app.doors_open,
                app.lighting,
                app.cell_hover,
                &app.panels,
                &app.sub_state()
            ),
            Some(true),
            "浮层开着时菜单必须画勾"
        );

        // ④ 浮层真的排得出来，且内容来自**真实取数**（坐标行 + 六条明细）
        app.hover = Some((1, 1));
        app.refresh_cell_lines();
        let panel = app.hover_panel(&l).expect("开着 + 有悬停格 ⇒ 必须排版");
        assert_eq!(panel.rows[0].1, "单元格 1,1", "坐标行必须来自真实格号");
        assert_eq!(panel.rows.len(), 8, "2 行头 + 6 条明细");
        assert!(
            panel.rect.left() >= l.content.left()
                && panel.rect.top() >= l.content.top()
                && panel.rect.right() <= l.content.right()
                && panel.rect.bottom() <= l.content.bottom(),
            "浮层必须整个落在窗口内：{:?} ⊄ {:?}",
            panel.rect,
            l.content
        );

        // ⑤ 没有悬停格 ⇒ 开着也不画（官方 `CellHover.cs:31-42` 的"不画空框"）
        app.hover = None;
        assert!(app.hover_panel(&l).is_none(), "没有悬停格时不许画空框");
        app.hover = Some((1, 1));
        assert!(app.hover_panel(&l).is_some(), "恢复悬停后必须重新画");

        // ⑥ 再点一次 → 关；回执说清；地图仍不动
        app.dispatch_menu(menu::model::Action::CellHover, canvas);
        assert!(!app.cell_hover, "再点没关掉");
        let (text, _) = app.op.clone().expect("回执");
        assert!(text.contains('关'), "{text}");
        assert_eq!(app.doc.map_gen, gen);
        assert!(app.hover_panel(&l).is_none(), "关掉后必须立刻不画");
        assert!(!path.exists(), "关也不许写设置文件");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **模态浮层打开时悬停框必须让位**（A-008 P3）。
    ///
    /// 上游的模态是独立窗口（`MainWindow.axaml` 的对话框），悬停框与它**不可能同屏**；
    /// 本仓模态是同一张画布上的浮层 ⇒ 不拦就会从对话框边缘露出半截面板。
    /// 判据刻意用 `Overlay::is_some()`（而不是逐个变体列举）：新增模态自动被拦。
    #[test]
    fn 模态浮层打开时不画悬停框() {
        let mut app = 有图应用();
        let l = Layout::compute(Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        // 前提：开关开着 + 有悬停格 ⇒ 正常状态下**必须**排得出来（否则本用例是空转）
        app.cell_hover = true;
        app.hover = Some((1, 1));
        app.refresh_cell_lines();
        app.overlay = menu::Overlay::None;
        let 基线 = app.hover_panel(&l);
        assert!(基线.is_some(), "开着 + 有悬停格 ⇒ 必须排版（否则本用例测不到东西）");

        // 逐个模态（含空变体与带载荷的）都必须让位
        let 模态们 = [
            menu::Overlay::OpenMap {
                path: "E:\\x.map".to_string(),
            },
            menu::Overlay::SaveAs {
                path: "E:\\y.map".to_string(),
            },
            menu::Overlay::Shortcuts,
            menu::Overlay::About,
        ];
        for 模态 in 模态们 {
            let 名 = format!("{模态:?}");
            app.overlay = 模态;
            assert!(
                app.overlay.is_some(),
                "{名} 应当算「模态打开」（判据是 `Overlay::is_some`）"
            );
            assert!(
                app.hover_panel(&l).is_none(),
                "{名} 打开时不许画悬停框（对话框边缘会露出来）"
            );
        }

        // 关掉模态 ⇒ 立刻恢复（不残留、不需要重新悬停）
        app.overlay = menu::Overlay::None;
        assert!(
            app.hover_panel(&l).is_some(),
            "模态关掉后悬停框必须恢复（不许把状态改坏）"
        );

        // 开关本身仍然生效：模态关掉 + 开关关掉 ⇒ 依旧是 `None`
        app.cell_hover = false;
        assert!(app.hover_panel(&l).is_none(), "开关关着时与模态无关");
    }

    /// **开启门**（视图 → 开启门，2026-11-05 接线）：`--open-doors` 置初值、菜单项翻转、
    /// 回执说人话、菜单勾跟着真状态，且**不动地图一个字节**（门态是渲染开关，不是编辑）。
    ///
    /// 这条盯的是"三处必须同一份真源"：`app.doors_open`（画布渲染读它，
    /// 见 `CanvasView::ensure` 的缓存键 + `RenderOptions`）、菜单勾
    /// （`model::check_with` 的 `doors` 参数）、`--open-doors` 的初值。
    /// 任何一处没跟上，用户看到的就是"点了没反应"或"勾着却是闭门画面"。
    #[test]
    fn 开启门可开可关且不动地图() {
        let mut app = 有图应用();
        assert!(!app.doors_open, "缺省必须闭门（RenderOptions::default()）");
        let canvas = 画布();
        let gen = app.doc.map_gen;
        let cell = app.doc.doc.as_ref().and_then(|d| d.map.cell(0, 0).copied());

        // ① 点一次 → 开
        app.dispatch_menu(menu::model::Action::OpenDoors, canvas);
        assert!(app.doors_open, "菜单项没翻转开关");
        let (text, warn) = app.op.clone().expect("必须有回执");
        assert!(!warn, "开门不是拒绝");
        assert_eq!(text, tx::MENU_OPEN_DOORS_ON);
        assert_eq!(app.doc.map_gen, gen, "门态是渲染开关，不许改地图代号");
        assert_eq!(
            app.doc.doc.as_ref().and_then(|d| d.map.cell(0, 0).copied()),
            cell,
            "门态是渲染开关，不许改格数据"
        );

        // ② 菜单勾必须跟着真状态（勾着却是闭门画面 = 假信息）
        assert_eq!(
            menu::model::check_with(
                menu::model::Action::OpenDoors,
                app.dark,
                app.grid,
                app.doors_open,
                app.lighting,
                &app.panels,
                &app.sub_state()
            ),
            Some(true),
            "门开着时菜单必须画勾"
        );

        // ③ 再点一次 → 关
        app.dispatch_menu(menu::model::Action::OpenDoors, canvas);
        assert!(!app.doors_open, "再点没关上");
        let (text, _) = app.op.clone().expect("回执");
        assert_eq!(text, tx::MENU_OPEN_DOORS_OFF);
        assert_eq!(app.doc.map_gen, gen);
        assert_eq!(
            menu::model::check_with(
                menu::model::Action::OpenDoors,
                app.dark,
                app.grid,
                app.doors_open,
                app.lighting,
                &app.panels,
                &app.sub_state()
            ),
            Some(false),
            "门关着时不许画勾"
        );

        // ④ `--open-doors` 置的是**同一个字段**的初值
        let on = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            open_doors: true,
            ..Default::default()
        });
        assert!(on.doors_open, "--open-doors 没生效");
        assert_eq!(on.doc.map_gen, 1, "--open-doors 不许改地图代号");
        // 缺省（不给开关）仍是闭门
        let off = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            ..Default::default()
        });
        assert!(!off.doors_open, "不给 --open-doors 时必须闭门");
    }

    /// **地图光照三档**（2026-11-05 接线）：菜单三子项切档、回执中文、勾选态读真状态、
    /// `--lighting` 置初值、三档互斥，且**不动地图一个字节**（光照是渲染开关，不是编辑）。
    ///
    /// 与 `开启门可开可关且不动地图` 同一套口径，钉的是"三处必须同一份真源"：
    /// `app.lighting`（画布渲染读它，见 `CanvasView::ensure` 的缓存键 + `RenderOptions`）、
    /// 菜单勾（`model::check_with` 的 `lighting` 参数）、`--lighting` 的初值。
    #[test]
    fn 地图光照三档可切且不动地图() {
        use menu::model::{Action, MapLight};
        let mut app = 有图应用();
        assert_eq!(app.lighting, MapLight::Day, "缺省必须白天（RenderOptions::default()）");
        let canvas = 画布();
        let gen = app.doc.map_gen;
        let cell = app.doc.doc.as_ref().and_then(|d| d.map.cell(0, 0).copied());

        let 勾 = |a: Action, app: &MireditApp| {
            menu::model::check_with(
                a,
                app.dark,
                app.grid,
                app.doors_open,
                app.lighting,
                &app.panels,
                &app.sub_state(),
            )
        };
        // ① 白天 ⇒ 只勾白天
        assert_eq!(勾(Action::LightDay, &app), Some(true));
        assert_eq!(勾(Action::LightDawnDusk, &app), Some(false));
        assert_eq!(勾(Action::LightNight, &app), Some(false));

        // ② 三个子项逐一切档：回执是中文、勾选态互斥、地图一格不动
        for (a, 档, 回执) in [
            (Action::LightDawnDusk, MapLight::DawnDusk, tx::MENU_LIGHT_DAWN_DUSK),
            (Action::LightNight, MapLight::Night, tx::MENU_LIGHT_NIGHT),
            (Action::LightDay, MapLight::Day, tx::MENU_LIGHT_DAY),
        ] {
            app.dispatch_menu(a, canvas);
            assert_eq!(app.lighting, 档, "{} 没切到档位", a.id());
            let (text, warn) = app.op.clone().expect("必须有回执");
            assert!(!warn, "切档不是拒绝");
            assert_eq!(text, 回执, "{} 回执文案不对", a.id());
            assert_eq!(app.doc.map_gen, gen, "光照是渲染开关，不许改地图代号");
            assert_eq!(
                app.doc.doc.as_ref().and_then(|d| d.map.cell(0, 0).copied()),
                cell,
                "光照是渲染开关，不许改格数据"
            );
            let 命中 = [Action::LightDawnDusk, Action::LightDay, Action::LightNight]
                .iter()
                .filter(|x| 勾(**x, &app) == Some(true))
                .count();
            assert_eq!(命中, 1, "{档:?} 档位下必须正好一个勾");
            assert_eq!(勾(a, &app), Some(true), "{档:?} 档位点中的那一项必须画勾");
        }

        // ③ 父项是容器：直接派发不切档，只给一句"去哪选"的回执
        app.dispatch_menu(Action::MapLighting, canvas);
        assert_eq!(app.lighting, MapLight::Day, "容器项不许切档");
        assert_eq!(app.op.clone().expect("回执").0, tx::STATUS_LIGHT_CONTAINER);

        // ④ `--lighting` 置的是**同一个字段**的初值
        for (档, want) in [
            ("dawn_dusk", MapLight::DawnDusk),
            ("day", MapLight::Day),
            ("night", MapLight::Night),
        ] {
            let cli = crate::cli::Cli::parse([
                "--map".to_owned(),
                r"E:\__no_such_map__.map".to_owned(),
                "--lighting".to_owned(),
                档.to_owned(),
            ]);
            assert!(cli.errors.is_empty(), "{档} 应被识别：{:?}", cli.errors);
            let a = MireditApp::new(&cli);
            assert_eq!(a.lighting, want, "--lighting {档} 没生效");
            assert_eq!(a.doc.map_gen, 1, "--lighting 不许改地图代号");
            assert!(!a.doors_open, "--lighting 不许把门态弄开");
        }
        assert_eq!(
            MireditApp::new(&crate::cli::Cli::default()).lighting,
            MapLight::Day,
            "不给 --lighting 时必须是白天"
        );
    }

    /// **`--menu-hover` 只在取证档（`--shot` / `--menu-open`）生效**。
    ///
    /// 为什么钉这条：它是给截图取证开的洞（子菜单只在某项成为高亮项时才展开，
    /// 而 `--shot` 锁死输入 ⇒ 没有 hover）。正常交互下它**不该**动状态，
    /// 否则"命令行能改高亮"就成了一条影子入口。
    #[test]
    fn 菜单高亮项只在取证档生效() {
        let p = || PathBuf::from(r"E:\__no_such_map__.map");
        // ① 单独给 ⇒ 忽略（菜单也没展开）
        let alone = MireditApp::new(&Cli {
            map: Some(p()),
            menu_hover: Some(2),
            ..Default::default()
        });
        assert!(
            alone.menu.active().is_none(),
            "没有 --shot / --menu-open 时不许设高亮项"
        );
        // ② 与 `--menu-open` 同给 ⇒ 生效
        let with_open = MireditApp::new(&Cli {
            map: Some(p()),
            menu_open: Some(2),
            menu_hover: Some(2),
            ..Default::default()
        });
        assert_eq!(
            with_open.menu.active(),
            Some(2),
            "--menu-open + --menu-hover 必须设上高亮项（否则子面板拍不到）"
        );
        // ③ 与 `--shot` 同给 ⇒ 也生效
        let with_shot = MireditApp::new(&Cli {
            map: Some(p()),
            menu_open: Some(0),
            menu_hover: Some(2),
            shot: Some(std::env::temp_dir().join("miredit-shot-probe.png")),
            ..Default::default()
        });
        assert_eq!(with_shot.menu.active(), Some(2));
    }

    /// **8 个面板开关**：逐个翻转、回执带面板名、状态真的变。
    ///
    /// ⚠ 地图列表（`PanelId::Maps`）**默认关**（唯一例外，理由见该枚举的文档），
    /// 所以它的第一下是"打开"、回执是「已显示面板」；其余 7 个默认开、第一下是"隐藏"。
    /// 这条用例按"默认值"分支，而不是硬写"默认全开"—— 否则加第 9 个面板时又要改。
    #[test]
    fn 面板开关逐项生效() {
        let mut app = 有图应用();
        let canvas = 画布();
        for id in PanelId::ALL {
            let before = app.panels.get(id);
            assert_eq!(
                before,
                id != PanelId::Maps,
                "{:?} 的默认显隐不对（只有地图列表默认关）",
                id
            );
            app.dispatch_menu(menu::model::Action::Panel(id), canvas);
            assert_eq!(app.panels.get(id), !before, "{:?} 点了没翻转", id);
            let (text, warn) = app.op.clone().expect("必须有回执");
            assert!(!warn, "面板开关不是拒绝");
            assert!(
                text.contains(if before { "已隐藏面板" } else { "已显示面板" }),
                "回执文案不对：{text}"
            );
            assert!(text.contains(id.title()), "回执里要有面板名：{text}");
            app.dispatch_menu(menu::model::Action::Panel(id), canvas);
            assert_eq!(app.panels.get(id), before, "{:?} 再点没恢复", id);
        }
    }

    /// **缩放三项 + 适应地图**：缩放百分比真的变（不是"点了没反应"）。
    #[test]
    fn 缩放三项真的改视口() {
        let mut app = 有图应用();
        let canvas = 画布();
        app.view.scale = 1.0;
        app.dispatch_menu(menu::model::Action::ZoomIn, canvas);
        let up = app.view.scale;
        assert!(up > 1.0, "放大没生效：{up}");
        app.dispatch_menu(menu::model::Action::ZoomOut, canvas);
        let down = app.view.scale;
        assert!(down < up, "缩小没生效：{down}");
        app.dispatch_menu(menu::model::Action::ActualSize, canvas);
        assert!(
            (app.view.scale - 1.0).abs() < 1e-6,
            "实际大小没回到 1.0：{}",
            app.view.scale
        );
        app.dispatch_menu(menu::model::Action::FitMap, canvas);
        let fit = app.view.scale;
        // 4×4 的图在 987×815 的画布里只需 ~2.5×（放大而不是缩小），所以这里只断言
        // 「落在合法区间内」+「与 100% 不同」—— 具体倍数由 `CanvasView::fit_map` 决定。
        assert!(
            (m::CANVAS_MIN_SCALE..=m::CANVAS_MAX_SCALE).contains(&fit),
            "适应地图后的缩放越界：{fit}"
        );
        assert!(
            (fit - 1.0).abs() > 1e-6,
            "4×4 的图适应窗口后不该正好是 100%：{fit}"
        );
        // 回执里必须有真实百分比
        let (text, _) = app.op.clone().expect("回执");
        assert!(text.contains('%'), "回执里要有百分比：{text}");
    }

    /// **打开地图**：`open_path` 成功换图（文档换掉、map_gen+1、撤销栈清空），
    /// 失败**保留原图**且回执带原因。
    #[test]
    fn 打开地图成功换图失败保留原图() {
        let mut app = 有图应用();
        let canvas = 画布();
        let gen = app.doc.map_gen;
        // 失败：文件不存在 → 原图不动，map_error 有原因
        app.open_map_path(r"E:\__no_such_map__.map", canvas);
        assert!(app.doc.doc.is_some(), "打开失败不该丢掉手上的图");
        assert!(app.doc.map_error.is_some(), "失败原因要落到 map_error");
        let (text, warn) = app.op.clone().expect("回执");
        assert!(warn);
        assert!(text.contains("打开失败"), "{text}");
        assert_eq!(app.doc.map_gen, gen, "失败不该换图");
        // 成功：真读一张磁盘上的图（10.map 只有 177 KB，够小）
        // 路径相对资产根（`MIREDIT_ASSET_ROOT`，缺省本机真实根）；缺文件就只验失败分支。
        match 资产根() {
            Some(root) => {
                let p = root.join("GodotClient").join("maps").join("10.map");
                if !p.is_file() {
                    eprintln!("[skip] 缺文件 {}", p.display());
                } else {
                    app.open_map_path(&p.display().to_string(), canvas);
                    let d = app.doc.doc.as_ref().expect("应已换图");
                    assert_eq!(d.file_name, "10.map");
                    assert!(app.doc.map_gen > gen, "换图要 +1（画布缓存靠它失效）");
                    assert_eq!(app.edit.undo_len(), 0, "换图必须清撤销栈（旧增量不能作用到新图）");
                    let (text, warn) = app.op.clone().expect("回执");
                    assert!(!warn, "成功不该用警示色");
                    assert!(text.contains("已打开地图：10.map"), "{text}");
                }
            }
            None => eprintln!("[skip] 无资产根 MIREDIT_ASSET_ROOT"),
        }
    }


    // ───────────────────── 地图列表（2026-11-05，上游 901dbe2f） ─────────────────────

    /// **点地图列表里的一行 ⇒ 地图真的打开**（走 `open_map_path`，与「打开地图…」同一条路）。
    ///
    /// 四件事一起钉：
    /// ① 列表扫出 2 张、按**自然序**排（`n2.map` 在 `n10.map` 前）；
    /// ② 当前打开的那张 `index_of` 命中（面板据此画金框高亮）；
    /// ③ 打开第 2 行 ⇒ `doc.path` 真的换了 + 回执说「已打开地图：n10.map」；
    /// ④ 高亮跟着换成第 2 行（`index_of` 从 0 变 1）。
    #[test]
    fn 地图列表点击换图且高亮跟随() {
        let dir = std::env::temp_dir().join("miredit_gui_app_maplist_click");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let mut made = Vec::new();
        for (name, w) in [("n10.map", 5u16), ("n2.map", 4u16)] {
            let mut b = vec![0u8; 52 + 12 * (w as usize * w as usize)];
            b[0..2].copy_from_slice(&w.to_le_bytes());
            b[2..4].copy_from_slice(&w.to_le_bytes());
            let p = dir.join(name);
            std::fs::write(&p, &b).expect("写合成地图");
            made.push(p);
        }
        let settings = 用例设置路径("maplist_click");
        let _ = std::fs::remove_file(&settings);
        let mut app = MireditApp::new(&Cli {
            map: Some(made[1].clone()), // n2.map
            settings: Some(settings),
            map_list_root: Some(dir.clone()),
            show_maps: true,
            ..Default::default()
        });
        // 分帧扫描在 `update` 里推进；用例里手动推完
        while app.maps.poll() {}
        assert_eq!(app.maps.len(), 2, "两张图都要进列表");
        assert_eq!(app.maps.entries()[0].rel, "n2.map", "自然序：n2 在 n10 前");
        assert_eq!(app.maps.entries()[1].rel, "n10.map");
        // ② 当前项 = 第 0 行
        let cur = app.doc.doc.as_ref().map(|d| d.path.clone());
        assert_eq!(app.maps.index_of(cur.as_deref()), Some(0));

        // ③ 打开第 1 行（= 列表里点它）
        let canvas = 画布();
        let target = app.maps.entries()[1].path.clone();
        app.open_map_path(&target.display().to_string(), canvas);
        let (text, warn) = app.op.clone().expect("要有回执");
        assert!(!warn, "打开成功不该是警示色：{text}");
        assert!(text.contains("已打开地图：n10.map"), "回执文案不对：{text}");
        assert_eq!(
            app.doc.doc.as_ref().map(|d| d.path.clone()),
            Some(target),
            "文档必须换成点的那张"
        );
        // ④ 高亮跟着换
        let cur = app.doc.doc.as_ref().map(|d| d.path.clone());
        assert_eq!(
            app.maps.index_of(cur.as_deref()),
            Some(1),
            "当前项高亮必须跟着换到第 1 行"
        );
        // 面板开着时布局真的让出一列（不是"开关点了没反应"）
        let l = app.layout(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(m::WINDOW_W, m::WINDOW_H),
        ));
        assert!(l.p_maps.whole.width() > 1.0, "面板开着却没占位");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **列表根跟着当前地图换目录**（`--map-list-root` 没给时）。
    ///
    /// 判据：从 `dir_a` 的图切到 `dir_b` 的图 ⇒ 列表根变成 `dir_b`、条目换成 dir_b 的。
    #[test]
    fn 列表根跟着当前地图换目录() {
        let base = std::env::temp_dir().join("miredit_gui_app_maplist_follow");
        let (a, b) = (base.join("a"), base.join("b"));
        let _ = std::fs::remove_dir_all(&base);
        for d in [&a, &b] {
            std::fs::create_dir_all(d).expect("建目录");
        }
        let 造 = |dir: &std::path::Path, name: &str| -> PathBuf {
            let mut bytes = vec![0u8; 52 + 12 * 16];
            bytes[0..2].copy_from_slice(&4u16.to_le_bytes());
            bytes[2..4].copy_from_slice(&4u16.to_le_bytes());
            let p = dir.join(name);
            std::fs::write(&p, &bytes).expect("写图");
            p
        };
        let ma = 造(&a, "only_a.map");
        let mb = 造(&b, "only_b.map");
        let settings = 用例设置路径("maplist_follow");
        let _ = std::fs::remove_file(&settings);
        let mut app = MireditApp::new(&Cli {
            map: Some(ma.clone()),
            settings: Some(settings),
            ..Default::default()
        });
        while app.maps.poll() {}
        assert_eq!(app.maps.len(), 1);
        assert_eq!(app.maps.entries()[0].rel, "only_a.map");
        // 换到 b 目录的图 ⇒ 根跟着换
        app.open_map_path(&mb.display().to_string(), 画布());
        while app.maps.poll() {}
        assert_eq!(
            app.maps.root().map(|p| p.to_path_buf()),
            Some(b.clone()),
            "根必须跟着换到 b"
        );
        assert_eq!(app.maps.entries()[0].rel, "only_b.map");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// **`--map-list-root` 钉住根**：从别的目录开图也不换根（验收要能对固定目录复跑）。
    #[test]
    fn 钉住的列表根不跟着换() {
        let base = std::env::temp_dir().join("miredit_gui_app_maplist_pin");
        let (a, b) = (base.join("a"), base.join("b"));
        let _ = std::fs::remove_dir_all(&base);
        for d in [&a, &b] {
            std::fs::create_dir_all(d).expect("建目录");
        }
        let mut bytes = vec![0u8; 52 + 12 * 16];
        bytes[0..2].copy_from_slice(&4u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&4u16.to_le_bytes());
        let ma = a.join("a.map");
        let mb = b.join("b.map");
        std::fs::write(&ma, &bytes).expect("写图 a");
        std::fs::write(&mb, &bytes).expect("写图 b");
        let settings = 用例设置路径("maplist_pin");
        let _ = std::fs::remove_file(&settings);
        let mut app = MireditApp::new(&Cli {
            map: Some(ma),
            settings: Some(settings),
            map_list_root: Some(a.clone()),
            ..Default::default()
        });
        while app.maps.poll() {}
        app.open_map_path(&mb.display().to_string(), 画布());
        while app.maps.poll() {}
        assert_eq!(
            app.maps.root().map(|p| p.to_path_buf()),
            Some(a),
            "给了 --map-list-root 就该钉住"
        );
        assert_eq!(app.maps.entries()[0].rel, "a.map");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// **「原始大小」按钮**：先放大到 200%，点一下回到 100%（走 `Action::ActualSize`）。
    #[test]
    fn 原始大小按钮回到百分之百() {
        let mut app = 有图应用();
        let canvas = 画布();
        // 先放大两档（走同一个 `zoom_at`，与滚轮一致）
        for _ in 0..2 {
            app.view.zoom_at(canvas, canvas.center(), m::ZOOM_PER_NOTCH);
        }
        assert!(app.view.scale > 1.0, "先要真的放大：{}", app.view.scale);
        // 点按钮 = 派发同一条 action
        app.dispatch_menu(menu::model::Action::ActualSize, canvas);
        assert!(
            (app.view.scale - 1.0).abs() < 1e-4,
            "点了「原始大小」缩放必须回到 100%，实际 {}",
            app.view.scale
        );
        let (text, warn) = app.op.clone().expect("要有回执");
        assert!(!warn, "回到 100% 不是拒绝：{text}");
        assert_eq!(text, tx::MENU_ACTUAL_SIZE_OK);
        assert_eq!(app.view.zoom_percent(), 100);
    }

    /// 快捷键浮层：菜单项把它打开、`Overlay::is_some` 为真；再按 Esc 关掉。
    ///
    /// 2026-11-06 全类清理：与「关于」同一套口径 —— 打开浮层**不写状态栏回执**
    /// （浮层标题「键盘快捷键」已在屏上），所以这里追加 `app.op.is_none()`。
    #[test]
    fn 快捷键浮层可开可关() {
        let mut app = 有图应用();
        let canvas = 画布();
        assert!(app.overlay == Overlay::None);
        app.dispatch_menu(menu::model::Action::KeyboardShortcuts, canvas);
        assert!(app.overlay == Overlay::Shortcuts, "菜单项没打开浮层");
        assert!(app.overlay.is_some());
        assert!(
            app.op.is_none(),
            "快捷键浮层不该写状态栏回执（标题已写同一句）：{:?}",
            app.op
        );
        // 按 `update` 的同一套装配重建文案：状态栏「上一次操作」那一段必须是空的
        app.text = screen_text::build(&app.doc, &app.view, app.hover, &app.lines, &app.edit);
        assert!(
            app.text.status_op.is_none(),
            "状态栏出现了回执：{:?}",
            app.text.status_op
        );
    }

    /// **帮助 → 关于**：菜单项打开只读模态，且**地图一字节不动**（`map_gen` 不变）。
    ///
    /// 为什么单列一条（不并进快捷键那条）：这两条走的是同一个 `dispatch_menu`，
    /// 但 About 是 2026-11-06 新增的**本仓功能**，它的"版本号现算 + 不碰文档"是硬要求，
    /// 得有一条用例在宿主层（app）钉住，而不是只在 `menu::action` 的夹具里绿。
    ///
    /// 2026-11-06 追加（用户截图红框标出）：**状态栏不许再出现「关于 Crystal Studio」**。
    /// 浮层标题已经写着同一句，回执再写一遍就是同屏重复 —— 所以这里钉 `app.op.is_none()`
    /// （它是 `status_op` 的唯一来源：`update` 只在 `op` 为 `Some` 时才往状态栏落字），
    /// 并顺手按同一份 `screen_text::build` 重建一次文案，确认状态栏那一段确实是空的。
    #[test]
    fn 关于浮层打开且不改地图() {
        let mut app = 有图应用();
        let canvas = 画布();
        let gen = app.doc.map_gen;
        assert!(app.overlay == Overlay::None);
        app.dispatch_menu(menu::model::Action::About, canvas);
        assert!(app.overlay == Overlay::About, "菜单项没打开关于模态");
        assert!(app.overlay.is_some());
        assert_eq!(app.doc.map_gen, gen, "打开关于不许改地图/触发重渲");
        assert!(
            app.op.is_none(),
            "关于不该写状态栏回执（否则标题与状态栏重复）：{:?}",
            app.op
        );
        // 按 `update` 的同一套装配重建文案：`build` 先清空 `status_op`，而 `op` 是 `None`
        // ⇒ 这一帧状态栏的「上一次操作」那一段必须是空的。
        app.text = screen_text::build(&app.doc, &app.view, app.hover, &app.lines, &app.edit);
        assert!(
            app.text.status_op.is_none(),
            "状态栏出现了回执：{:?}",
            app.text.status_op
        );
        // 版本号在宿主这一层也是同一份真源（`--version` / 关于 / 网页 health）
        assert_eq!(menu::about::VERSION, env!("CARGO_PKG_VERSION"));
        // 浮层是"只读模态"：`is_some()` ⇒ `handle_keys` 整段早退，编辑快捷键不会落进地图
        assert!(app.overlay.is_some(), "关于打开期间编辑快捷键必须被挡住");
    }

    /// **回归（2026-11-06 用户截图红框）**：打开「关于」后，状态栏不许出现「关于 Crystal Studio」。
    ///
    /// 与上一条的分工：上一条从**空状态栏**出发钉 `op.is_none()`；这一条先制造一条别的回执，
    /// 证明"关于"**不会把自己的标题写进去**（不是靠清空状态栏掩盖重复），并直接断言
    /// 文案装配里那一段不含标题 —— 这正是用户截图里红框圈出的同屏重复。
    #[test]
    fn 关于打开后状态栏不出现重复标题() {
        let mut app = 有图应用();
        let canvas = 画布();
        // 先留下一条真回执：要让"重复"有机会发生，状态栏就不能是空的。
        app.dispatch_menu(menu::model::Action::CellGrid, canvas);
        assert!(app.op.is_some(), "前置：单元格网格开关本来就会写一条回执");
        app.dispatch_menu(menu::model::Action::About, canvas);
        assert_eq!(app.overlay, Overlay::About, "关于必须真的打开");
        assert_ne!(
            app.op.as_ref().map(|(t, _)| t.as_str()),
            Some(tx::ABOUT_TITLE),
            "关于不许把浮层标题当回执写进状态栏（同屏会重复两遍）"
        );
        // `update` 只把 `op` 落进 `status_op`，而 `screen_text::build` 先把它清空
        // ⇒ 只要 `op` 不是那句标题，状态栏就不会出现重复标题。
        app.text = screen_text::build(&app.doc, &app.view, app.hover, &app.lines, &app.edit);
        assert!(
            !app.text
                .status_op
                .as_deref()
                .unwrap_or("")
                .contains(tx::ABOUT_TITLE),
            "状态栏「上一次操作」段混进了标题：{:?}",
            app.text.status_op
        );
    }

    /// **全类回归（2026-11-06）**：打开任何一个模态/浮层，状态栏都不许出现该浮层的标题。
    ///
    /// 用户截图红框标出的是「关于 Crystal Studio」，但同一个毛病在 6 个"打开浮层"的动作上
    /// 都存在过（打开地图 / 另存为 / 导出选区 / 导出小地图 / 键盘快捷键 / 关于）——
    /// 它们以前都 `Outcome::ok(<对话框标题>)`：标题在模态上写一遍、状态栏再写一遍。
    /// 现在统一走 `Outcome::opened_overlay()`（`receipt: None`），本用例逐条钉住：
    /// ① **空状态栏出发** ⇒ `app.op.is_none()`，重建文案后 `status_op` 为空；
    /// ② **先留一条别人的真回执** ⇒ 打开浮层后仍是那条回执（不是靠清空状态栏掩盖重复），
    ///    且重建后的状态栏**不含该浮层标题**；
    /// ③ 地图一字节不动（`map_gen` 不变）、浮层真的开着（`overlay.is_some()`）；
    /// ④ **快捷键早退**：浮层开着时按 `G`（无浮层时会翻单元格网格）一个字段都不许变 ——
    ///    依据是 `handle_keys` ⓪ 的 `if self.overlay.is_some() { return; }`。
    #[test]
    fn 打开浮层不写标题回执_表驱动() {
        let canvas = 画布();
        let 用例: [(menu::model::Action, &str); 6] = [
            (menu::model::Action::OpenMap, tx::OPEN_DLG_TITLE),
            (menu::model::Action::SaveAs, tx::SAVE_AS_DLG_TITLE),
            (menu::model::Action::ExportSelection, tx::EXPORT_SEL_DLG_TITLE),
            (menu::model::Action::ExportMinimap, tx::EXPORT_MINI_DLG_TITLE),
            (menu::model::Action::KeyboardShortcuts, tx::SHORTCUTS_TITLE),
            (menu::model::Action::About, tx::ABOUT_TITLE),
        ];
        // ⓪ 反假绿：同一个按键助手在**没有浮层**时必须真的翻动网格，
        //    否则下面「G 被挡住」可能只是"事件根本没送到"。
        let mut 对照 = 有图应用();
        let 对照前 = 对照.grid;
        按一次键(&mut 对照, egui::Key::G);
        assert_ne!(
            对照.grid, 对照前,
            "对照组：无浮层时按 G 必须真的翻单元格网格（否则早退断言是假绿）"
        );

        for (action, title) in 用例 {
            // ① 空状态栏出发
            let mut app = 有图应用();
            let gen = app.doc.map_gen;
            assert!(app.op.is_none(), "前置：{} 起点应是空状态栏", action.id());
            app.dispatch_menu(action, canvas);
            assert!(app.overlay.is_some(), "{} 必须真的打开浮层", action.id());
            assert!(
                app.op.is_none(),
                "{} 把浮层标题当回执写进了状态栏：{:?}",
                action.id(),
                app.op
            );
            assert_eq!(app.doc.map_gen, gen, "{} 不许改地图/触发重渲", action.id());
            app.text = screen_text::build(&app.doc, &app.view, app.hover, &app.lines, &app.edit);
            assert!(
                app.text.status_op.is_none(),
                "{} 之后状态栏「上一次操作」段必须是空的：{:?}",
                action.id(),
                app.text.status_op
            );

            // ④ 快捷键早退：浮层开着时 G 必须被挡住
            let grid_before = app.grid;
            按一次键(&mut app, egui::Key::G);
            assert_eq!(
                app.grid,
                grid_before,
                "{} 打开期间按 G 不该翻网格（编辑快捷键要整段早退）",
                action.id()
            );

            // ② 已有回执时也不许被标题覆盖 / 也不许被清空
            app.overlay = Overlay::None;
            app.dispatch_menu(menu::model::Action::CellGrid, canvas);
            let 前一条 = app.op.clone().expect("前置：单元格网格本来就会写一条回执");
            assert_ne!(前一条.0, title, "前置回执不许恰好是同名标题");
            let gen = app.doc.map_gen;
            app.dispatch_menu(action, canvas);
            assert_eq!(
                app.op.clone().expect("旧回执要留着"),
                前一条,
                "{} 覆盖了别人的回执（或把状态栏清空了）",
                action.id()
            );
            assert_eq!(app.doc.map_gen, gen, "{} 第二次打开也不许改地图", action.id());
            app.text = screen_text::build(&app.doc, &app.view, app.hover, &app.lines, &app.edit);
            let 状态栏 = app.text.status_op.clone().unwrap_or_default();
            assert!(
                !状态栏.contains(title),
                "{} 的标题混进了状态栏：{状态栏:?}",
                action.id()
            );
        }
    }

    /// **`--menu-click file.open_recent`**：容器项**不执行任何动作**，只回一条"去哪选"的话。
    ///
    /// 这条挡的是"脚本/命令行直接派发容器项"时误跑父项动作 ——
    /// 与键盘回车、鼠标点击是同一件事的第三条入口（审查缺陷 3 的三条路要一致）。
    #[test]
    fn 直接派发最近打开只给指路回执() {
        let mut app = 有图应用();
        let canvas = 画布();
        let gen = app.doc.map_gen;
        app.dispatch_menu(menu::model::Action::OpenRecent, canvas);
        let (text, warn) = app.op.clone().expect("必须有回执");
        assert!(!warn, "这不是错误（不用警示色）：{text}");
        assert_eq!(text, menu::model::STATUS_RECENT_CONTAINER);
        assert_eq!(app.doc.map_gen, gen, "容器项不许改地图");
        assert!(app.overlay == Overlay::None, "容器项不许弹浮层");
    }

    /// **「最近打开」端到端**（2026-11-05 接线）：真开两张图 ⇒ 设置文件里的 `recent`
    /// 最近优先；再开第一张 ⇒ 提升到第一位且**不重复**；删掉文件再点它
    /// ⇒ 状态栏给中文原因 + 该条从列表与设置文件里一起移除。
    ///
    /// ⚠ 用的是 `GodotClient/maps/{10,41}.map` 的**临时副本**：真文件是工程资产，
    /// 本用例要"删一个文件再点它"，绝不许动到真文件（删了就是事故）。
    #[test]
    fn 最近打开端到端记入提升与移除() {
        // 路径相对资产根（`MIREDIT_ASSET_ROOT`，缺省本机真实根）——
        // 公开仓/PR 的机器上没有游戏素材 ⇒ 打印 `[skip]` 并返回（不 panic）。
        let Some(root) = 资产根() else {
            eprintln!("[skip] 无资产根 MIREDIT_ASSET_ROOT");
            return;
        };
        let 语料 = root.join("GodotClient").join("maps");
        let src10 = 语料.join("10.map");
        let src41 = 语料.join("41.map");
        if !src10.is_file() || !src41.is_file() {
            // 样本不在 ⇒ 如实跳过（不编假路径，也不假装验过）
            eprintln!(
                "[跳过] 最近打开端到端：缺样本 {} / {}",
                src10.display(),
                src41.display()
            );
            return;
        }
        let dir = std::env::temp_dir().join("miredit_recent_e2e");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let m10 = dir.join("10.map");
        let m41 = dir.join("41.map");
        std::fs::copy(&src10, &m10).expect("拷 10.map");
        std::fs::copy(&src41, &m41).expect("拷 41.map");
        let settings = dir.join("settings.json");
        let mut app = MireditApp::new(&Cli {
            map: Some(PathBuf::from(r"E:\__no_such_map__.map")),
            settings: Some(settings.clone()),
            ..Default::default()
        });
        assert!(app.recent.is_empty(), "首次运行（无设置文件）⇒ 没有历史");
        let canvas = 画布();

        // ⓪ **打开失败不记入历史**（单子：只记成功打开的）
        app.open_map_path(r"E:\__no_such_map__.map", canvas);
        assert!(
            app.recent.is_empty(),
            "打不开的路径不许进最近列表：{:?}",
            app.recent
        );
        assert!(!settings.exists(), "失败时一个字节都不许写设置文件");

        // ① 依次打开 10.map、41.map ⇒ 最近优先
        app.open_map_path(&m10.display().to_string(), canvas);
        app.open_map_path(&m41.display().to_string(), canvas);
        let on_disk = menu::settings::load_quiet(&settings).expect("设置文件必须真写出来");
        // 证据：把真写出来的设置文件原样打到测试日志（`--nocapture` 可读）
        eprintln!(
            "[证据] 打开 10.map → 41.map 之后，{} 的内容：\n{}",
            settings.display(),
            std::fs::read_to_string(&settings).unwrap_or_default()
        );
        assert_eq!(
            on_disk.recent,
            vec![m41.display().to_string(), m10.display().to_string()],
            "最近打开的那张必须排第一"
        );
        assert_eq!(app.recent, on_disk.recent, "内存与磁盘必须是同一份");

        // ② 再打开 10.map ⇒ 提升到第一位、且**不重复**
        app.open_map_path(&m10.display().to_string(), canvas);
        let on_disk = menu::settings::load_quiet(&settings).expect("再读设置");
        eprintln!(
            "[证据] 再打开 10.map 之后（提升、不重复）：\n{}",
            std::fs::read_to_string(&settings).unwrap_or_default()
        );
        assert_eq!(
            on_disk.recent,
            vec![m10.display().to_string(), m41.display().to_string()],
            "重新打开要把它提升到第一位"
        );
        assert_eq!(on_disk.recent.len(), 2, "不许出现两份 10.map：{on_disk:?}");
        assert_eq!(app.recent, on_disk.recent);

        // ③ 子菜单的行就是这份列表（文件名 + 都能点）
        let rows = app.sub_state().recent_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "10.map", "官方取 Path.GetFileName");
        assert_eq!(rows[1].0, "41.map");
        assert!(rows.iter().all(|(_, on)| *on), "两个文件都在 ⇒ 都可点");

        // ④ 删掉 41.map 再"点"它（第 2 行）⇒ 状态栏给中文原因 + 从列表移除
        std::fs::remove_file(&m41).expect("删掉副本 41.map");
        app.open_recent_at(1, canvas);
        let (text, warn) = app.op.clone().expect("必须有回执");
        assert!(warn, "打不开是一条拒绝回执");
        assert!(
            text.contains("最近打开的文件已不存在："),
            "回执要说明原因：{text}"
        );
        assert!(text.contains("41.map"), "回执里要有那条路径：{text}");
        eprintln!("[证据] 点已不存在的 41.map ⇒ 状态栏回执：{text}（warn={warn}）");
        assert_eq!(
            app.recent,
            vec![m10.display().to_string()],
            "点了已不存在的历史 ⇒ 该条必须从列表里移除"
        );
        let on_disk = menu::settings::load_quiet(&settings).expect("三读设置");
        eprintln!(
            "[证据] 移除之后 settings.json：\n{}",
            std::fs::read_to_string(&settings).unwrap_or_default()
        );
        assert_eq!(
            on_disk.recent,
            vec![m10.display().to_string()],
            "移除必须落盘（否则下次启动又冒出来）"
        );
        // ⑤ 当前地图**不受影响**（打开失败不许丢掉手上的图）
        assert_eq!(
            app.doc.doc.as_ref().map(|d| d.file_name.clone()),
            Some("10.map".to_owned()),
            "打开历史失败不许换掉当前文档"
        );

        // ⑥ 清空列表 ⇒ 子菜单只剩一行官方空态行（`--menu-dump` 同一条判据）
        app.recent.clear();
        let rows = app.sub_state().recent_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, menu::model::SUB_RECENT_EMPTY);
        assert!(!rows[0].1, "空态行灰显");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ───────────────── R12-F1：单元格属性模态（open → 初值 → 提交 → 字节/撤销） ─────────────────

    /// **「编辑 → 单元格属性…」的完整链路**：无目标拒绝 → 打开时初值 = 选区共同值
    /// （有差异留空）→ 提交只写改过的字段 → 字节只动 `off6/off7/off11` → 整批一步撤销 →
    /// 改图后 `map_gen` 前进。
    ///
    /// 夹具：`有图应用()` 是一张 4×4 的 **12 B/格** 合成图，格值全 0
    /// （`light=0` / `door_index=0` / `door_offset=0`），所以"初值 = 0"与"混合值 = 空"
    /// 这两种情形都能在这张图上造出来。
    #[test]
    fn 单元格属性模态初值与提交() {
        let mut app = 有图应用();
        let canvas = 画布();

        // ① 既没选区也没悬停格 ⇒ **拒绝且不打开空模态**（上游 `Contains(x, y)` 早退的等价物）
        app.hover = None;
        app.edit.clear_selection();
        app.dispatch_menu(menu::model::Action::CellProperties, canvas);
        assert!(
            matches!(app.overlay, Overlay::None),
            "没有目标格时不该打开模态"
        );
        let (text, warn) = app.op.clone().expect("拒绝必须有回执");
        assert!(warn, "拒绝要用警示色：{text}");
        assert!(text.contains("先把鼠标"), "{text}");

        // ② 悬停一格 ⇒ 打开模态，初值 = 该格真值，输入框已填好
        app.hover = Some((1, 1));
        app.dispatch_menu(menu::model::Action::CellProperties, canvas);
        let Overlay::CellProps {
            area,
            初值,
            文本,
            focus: _,
            有门字节,
            ..
        } = app.overlay.clone()
        else {
            panic!("应打开单元格属性模态：{:?}", app.overlay);
        };
        assert_eq!(area, (1, 1, 1, 1), "没有选区 ⇒ 悬停的那一格");
        assert_eq!(
            初值,
            ["0".to_string(), "0".to_string(), "0".to_string()],
            "夹具格值全 0 ⇒ 三个字段初值都是 \"0\""
        );
        assert_eq!(文本, 初值, "打开时输入框就是初值（没改 ⇒ 不写）");
        assert!(有门字节, "Old12 有门字节 ⇒ 三个框都可编辑");

        // ③ 只给 light ⇒ 只写 light；字节只动 off11；一步撤销；map_gen 前进
        let 原 = app.doc.doc.as_ref().expect("有文档").map.to_bytes();
        let gen = app.doc.map_gen;
        let (text, warn) = app.apply_cell_props(Some(200), None, None);
        assert!(!warn, "成功不该是警示色：{text}");
        assert_eq!(text, "改属性 1 格");
        let 新 = app.doc.doc.as_ref().expect("有文档").map.to_bytes();
        let 基 = 52 + (4 + 1) * 12; // (1,1)：列优先 index = x*h + y = 1*4 + 1
        let mut 变: Vec<usize> = (0..原.len()).filter(|i| 原[*i] != 新[*i]).collect();
        变.sort_unstable();
        assert_eq!(变, vec![基 + 11], "只给 light ⇒ 只许动 off11");
        assert_eq!((新[基 + 6], 新[基 + 7], 新[基 + 11]), (0, 0, 200));
        assert_eq!(app.edit.undo_len(), 1, "整批 = 一步撤销");
        assert_eq!(
            app.doc.map_gen,
            gen.wrapping_add(1),
            "改了图必须换代号（画布缓存键靠它重渲）"
        );
        app.edit.undo(&mut app.doc.doc.as_mut().expect("有文档").map);
        assert_eq!(
            app.doc.doc.as_ref().expect("有文档").map.to_bytes(),
            原,
            "撤销一步必须逐字节回原样"
        );

        // ④ 混合值：选区两格的 light 不同 ⇒ 初值留空；只改门索引 ⇒ 两格光照各自保留
        {
            let d = app.doc.doc.as_mut().expect("有文档");
            // (1,0)：index = 1*4 + 0 = 4
            d.map.cells[4].light = 7;
            assert!(
                app.edit
                    .set_selection(&mut d.map, miredit_core::Selection::new(1, 0, 1, 1)),
                "设置选区应成功"
            );
        }
        app.dispatch_menu(menu::model::Action::CellProperties, canvas);
        let Overlay::CellProps { 初值, .. } = app.overlay.clone() else {
            panic!("应打开单元格属性模态");
        };
        assert_eq!(初值[0], "", "两格 light 不同 ⇒ 混合值留空（Watermark）");
        assert_eq!(初值[1], "0", "门索引相同 ⇒ 显示共同值");
        let (text, _) = app.apply_cell_props(None, Some(3), None);
        assert_eq!(text, "改属性 2 格", "两格的门索引都从 0 变 3");
        let map = &app.doc.doc.as_ref().expect("有文档").map;
        assert_eq!(
            map.cell(1, 0).unwrap().light,
            7,
            "没给 light ⇒ 混合值不许被抹平"
        );
        assert_eq!(map.cell(1, 1).unwrap().light, 0, "另一格也保持原值");
        assert_eq!(map.cell(1, 0).unwrap().door_index, 3);
        assert_eq!(map.cell(1, 1).unwrap().door_index, 3);
        assert_eq!(map.cell(1, 0).unwrap().door_offset, 0, "门偏移没给 ⇒ 原值");

        // ⑤ 三个都空（混合值也没填）⇒ 一格不改、不开撤销步
        let undo_before = app.edit.undo_len();
        let 原2 = app.doc.doc.as_ref().expect("有文档").map.to_bytes();
        let (text, warn) = app.apply_cell_props(None, None, None);
        assert_eq!(text, "值没有变化，一格未改");
        assert!(warn, "没改动要按「没做事」提示");
        assert_eq!(app.edit.undo_len(), undo_before, "不许开撤销步");
        assert_eq!(app.doc.doc.as_ref().expect("有文档").map.to_bytes(), 原2);
    }

    // ─────────────────────── A-012：fit 边距与窗口尺寸变化 ───────────────────────

    /// 造一张 `n×n` 的 12 B/格合成图当应用（每格地表图号 1）。
    fn 有图应用n(n: u16) -> MireditApp {
        let mut app = 空应用();
        let 格数 = n as usize * n as usize;
        let mut b = vec![0u8; 52 + 12 * 格数];
        b[0..2].copy_from_slice(&n.to_le_bytes());
        b[2..4].copy_from_slice(&n.to_le_bytes());
        for i in 0..格数 {
            let base = 52 + i * 12;
            b[base..base + 2].copy_from_slice(&1u16.to_le_bytes());
        }
        app.doc.doc = Some(
            crate::mapdoc::MapDoc::from_bytes(std::path::Path::new(r"E:\x\fit.map"), b)
                .expect("合成图应能解析"),
        );
        app.doc.map_error = None;
        if let Some(d) = app.doc.doc.as_ref() {
            let _ = app.edit.bind_document(&d.path, d.orig_bytes());
        }
        app
    }

    /// 窗口 `(w,h)` 下的布局。
    fn 布局(w: f32, h: f32) -> Layout {
        Layout::compute(Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(w, h),
        ))
    }

    /// **「适应地图」只有一条算法**：菜单动作算出的 `scale`/`pan` 必须与
    /// [`CanvasView::fit_scale`] + 居中公式逐位一致，且**重复点结果不变**（幂等）。
    ///
    /// 三条入口（`menu/action.rs:511` 的适应地图、`app.rs:2824` 的缩放条按钮、
    /// `open_map_path` 开图时的首次 fit）都调同一个 [`CanvasView::fit_map`] ——
    /// 本用例钉住菜单那条，另两条由"只存在一份实现"结构保证（回执里给 grep 证据）。
    #[test]
    fn 适应地图三条入口同一条fit() {
        let mut app = 有图应用n(40);
        let l = 布局(m::WINDOW_W, m::WINDOW_H);
        let canvas = l.viewport;

        app.dispatch_menu(menu::model::Action::FitMap, canvas);
        let 期望 = CanvasView::fit_scale(canvas, 40, 40);
        assert!(
            (app.view.scale - 期望).abs() < 1e-6,
            "菜单「适应地图」必须用 fit_scale：{} vs {期望}",
            app.view.scale
        );
        // 居中：pan = (图 - 画布/倍数)/2（与上游 `Fit()` 同一套）
        let (map_w, map_h) = (40.0 * 48.0, 40.0 * 32.0);
        let (pan_x, pan_y) = (
            (map_w - canvas.width() / 期望) * 0.5,
            (map_h - canvas.height() / 期望) * 0.5,
        );
        assert!((app.view.pan_x - pan_x).abs() <= 0.51, "pan_x 必须居中");
        assert!((app.view.pan_y - pan_y).abs() <= 0.51, "pan_y 必须居中");

        // 幂等：手动改一通再点一次，仍然回到同一个 fit
        app.view.zoom_at(canvas, canvas.center(), 1.7);
        app.view.pan_by(egui::vec2(37.0, -21.0));
        app.dispatch_menu(menu::model::Action::FitMap, canvas);
        assert!((app.view.scale - 期望).abs() < 1e-6, "重复适应必须同值");
        assert!((app.view.pan_x - pan_x).abs() <= 0.51);
        assert!((app.view.pan_y - pan_y).abs() <= 0.51);
    }

    /// **窗口变大 ⇒ 重新适应到新的 fit，且世界中心不动**（上游 `OnSizeChanged` 的等价物）。
    #[test]
    fn 窗口尺寸变化后重适应() {
        let mut app = 有图应用n(40);
        let 小 = 布局(m::WINDOW_W, m::WINDOW_H);
        let 大 = 布局(m::WINDOW_W + 500.0, m::WINDOW_H + 350.0);
        let 旧fit = CanvasView::fit_scale(小.viewport, 40, 40);
        let 新fit = CanvasView::fit_scale(大.viewport, 40, 40);
        assert!(
            新fit > 旧fit + 1e-3,
            "用例前提：窗口变大后 fit 必须变大（{旧fit} → {新fit}）"
        );

        // 首次布局：只记住尺寸，不动缩放（缩放由开图时的 fit_map 负责）
        app.view.fit_map(小.viewport, 40, 40);
        app.refit_after_resize(&小);
        assert!((app.view.scale - 旧fit).abs() < 1e-6, "首次布局不许改 fit");

        let 旧中心 = (
            app.view.pan_x + 小.viewport.width() / (2.0 * app.view.scale),
            app.view.pan_y + 小.viewport.height() / (2.0 * app.view.scale),
        );
        app.refit_after_resize(&大);
        assert!(
            (app.view.scale - 新fit).abs() < 1e-4,
            "窗口变大必须抬到新 fit：{} vs {新fit}",
            app.view.scale
        );
        let 新中心 = (
            app.view.pan_x + 大.viewport.width() / (2.0 * app.view.scale),
            app.view.pan_y + 大.viewport.height() / (2.0 * app.view.scale),
        );
        // `snap()` 把 pan 吸附到**屏幕**整数像素 ⇒ 世界坐标容差是 1/新fit（这里 ≈1.36 px）
        let 容差 = 0.5 / 新fit + 0.01;
        assert!(
            (新中心.0 - 旧中心.0).abs() < 容差 && (新中心.1 - 旧中心.1).abs() < 容差,
            "重 fit 后世界中心必须不动（容差 {容差}）：{旧中心:?} vs {新中心:?}"
        );
    }

    /// **不每帧抢用户的手动缩放/平移**（这是最容易写错的一条）。
    ///
    /// 两个角度都要钉：
    /// ① 尺寸没变 ⇒ `refit_after_resize` 什么都不做（哪怕当前倍数比 fit 小 ——
    ///    那种情况只该由"真的 resize 了"来纠正，否则用户永远缩不小）；
    /// ② 尺寸变了但当前倍数已经 ≥ 新 fit（用户手动放大过）⇒ 也不动。
    #[test]
    fn 不每帧抢用户缩放与平移() {
        let mut app = 有图应用n(40);
        let l = 布局(m::WINDOW_W, m::WINDOW_H);
        app.view.fit_map(l.viewport, 40, 40);
        app.refit_after_resize(&l); // 记住尺寸

        // ① 手动缩到 fit 以下（0.3× < fit 0.47×），同一尺寸再走一遍 ⇒ 不许被拉回
        app.view.zoom_at(l.viewport, l.viewport.center(), 0.3 / app.view.scale);
        app.view.pan_by(egui::vec2(19.0, 11.0));
        let (缩放前, 平移前) = (app.view.scale, (app.view.pan_x, app.view.pan_y));
        app.refit_after_resize(&l);
        assert_eq!(app.view.scale, 缩放前, "尺寸没变时不许改缩放");
        assert_eq!(
            (app.view.pan_x, app.view.pan_y),
            平移前,
            "尺寸没变时不许改平移"
        );

        // ② 手动放大到 2×（> 任何 fit），再"变大窗口"也没关系 —— 新 fit 仍 ≤ 2×
        app.view.zoom_at(l.viewport, l.viewport.center(), 2.0 / app.view.scale);
        let 大 = 布局(m::WINDOW_W + 200.0, m::WINDOW_H + 150.0);
        let (缩放前, 平移前) = (app.view.scale, (app.view.pan_x, app.view.pan_y));
        app.refit_after_resize(&大);
        assert_eq!(app.view.scale, 缩放前, "手动放大过的倍数不许被覆盖");
        assert_eq!(
            (app.view.pan_x, app.view.pan_y),
            平移前,
            "手动平移过的位置不许被覆盖"
        );
    }

    // ─────────────────── A-011：arm 之后格表必须到位（预览的数据前提） ───────────────────

    /// 印章调色板路径：`MIR2_STAMP_PALETTE` → 缺省 `_upstream/catalogue/palette.json`。
    fn 印章调色板() -> Option<PathBuf> {
        let p = std::env::var_os("MIR2_STAMP_PALETTE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"I:\miredit\_upstream\catalogue\palette.json"));
        p.is_file().then_some(p)
    }

    /// **arm 之后必须拿到格表**（`Armed.cells`），且 `preview_state` 带出内容/受阻诊断。
    ///
    /// 钉住 2026-11-07 实测踩到的坑：`--stamp-pick/--stamp-arm` 这条入口上
    /// `stamps.selected_path()` 是 `None` ⇒ `Armed.cells` 为空 ⇒ 半透明内容与红/琥珀诊断
    /// 一个像素都画不出来（三张验收图当场拍空）。现在路径改走 `catalog.resolve(id)`
    /// （与面板缩略图同源），本用例守这条链：arm ⇒ 有格表 ⇒ 预览有状态。
    #[test]
    fn arm之后拿到格表且预览有诊断() {
        let Some(pal) = 印章调色板() else {
            eprintln!("[跳过] 印章调色板不在（MIR2_STAMP_PALETTE / _upstream/catalogue/palette.json）");
            return;
        };
        let mut app = 有图应用n(8);
        app.stamps = crate::stamp_panel::StampPanel::with_path(pal);
        let t0 = std::time::Instant::now();
        while !app.stamps.is_ready() && t0.elapsed() < std::time::Duration::from_secs(5) {
            // 后台线程把结果送进 channel，`poll()` 才把它换成 `Load::Ready`（宿主每帧做同一件事）
            app.stamps.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(app.stamps.is_ready(), "印章目录必须装载成功");
        assert!(app.stamps.pick_slot(0), "第 1 格应能选中");
        assert!(app.arm_selected(), "应进入待放置态");

        let cells = app
            .armed
            .as_ref()
            .and_then(|a| a.cells.clone())
            .expect("arm 之后必须有格表（否则预览只能画轮廓）");
        assert!(!cells.is_empty(), "格表不许为空");
        // 抽一格**非负偏移**的格当"占用"目标（保证落在锚点右下方、图内）
        let 偏移 = cells
            .iter()
            .find(|c| c.dx >= 0 && c.dy >= 0)
            .map(|c| (c.dx, c.dy))
            .expect("印章至少有一格非负偏移");

        // ① 空位 ⇒ 允许 + 0.55，无受阻
        {
            let map = &app.doc.doc.as_ref().expect("有图").map;
            let s = stamp_place::preview_state(map, &cells, 2, 2, true).expect("Alt 按住");
            assert!(s.检查.error.is_none(), "空位不该有 error");
            assert!(s.检查.blocked.is_empty(), "空位不该有受阻格");
            assert!(
                (s.不透明度 - stamp_place::不透明度允许).abs() < 1e-6,
                "允许 ⇒ 0.55"
            );
        }
        // ② 占掉目标格 ⇒ 受阻（OccupiedObject）+ 0.18
        {
            let d = app.doc.doc.as_mut().expect("有图");
            let (tx, ty) = (
                2u16 + u16::try_from(偏移.0).unwrap(),
                2u16 + u16::try_from(偏移.1).unwrap(),
            );
            let i = miredit_core::index_of(&d.map, tx, ty).expect("图内格");
            d.map.cells[i].fr_img = 7;
            let s = stamp_place::preview_state(&d.map, &cells, 2, 2, true).expect("Alt 按住");
            assert_eq!(s.检查.blocked.len(), 1, "只该有那一格被占");
            assert_eq!(
                s.检查.blocked[0].reason,
                miredit_core::StampBlockReason::OccupiedObject
            );
            assert!(
                (s.不透明度 - stamp_place::不透明度受阻).abs() < 1e-6,
                "受阻 ⇒ 0.18"
            );
        }
        // ③ 贴边（锚点 0,0）⇒ 出界的格报 MapBoundary（琥珀）
        {
            let map = &app.doc.doc.as_ref().expect("有图").map;
            let s = stamp_place::preview_state(map, &cells, 0, 0, true).expect("Alt 按住");
            assert!(
                s.检查
                    .blocked
                    .iter()
                    .any(|b| b.reason == miredit_core::StampBlockReason::MapBoundary),
                "贴左上角必须有出界格：{:?}",
                s.检查.blocked
            );
        }
    }
    // ───────────────────── 工具 → 裁剪到选区（A-015 Crop / R36） ─────────────────────

    /// 造一个 `w×h` 的 Old12 应用：格的 `bk_img` = 格序 + 1（列优先，便于断言搬格）。
    fn 尺寸应用(w: u16, h: u16) -> MireditApp {
        let mut app = 空应用();
        let mut b = vec![0u8; 52 + 12 * (w as usize * h as usize)];
        b[0..2].copy_from_slice(&w.to_le_bytes());
        b[2..4].copy_from_slice(&h.to_le_bytes());
        for i in 0..(w as usize * h as usize) {
            let base = 52 + i * 12;
            b[base..base + 2].copy_from_slice(&((i as u16) + 1).to_le_bytes());
        }
        app.doc.doc = Some(
            crate::mapdoc::MapDoc::from_bytes(std::path::Path::new(r"E:\x\crop.map"), b)
                .expect("合成图应能解析"),
        );
        app.doc.map_error = None;
        app.doc.assets = crate::mapdoc::Assets::Failed("测试：不装载图库".to_owned());
        if let Some(d) = app.doc.doc.as_ref() {
            let _ = app.edit.bind_document(&d.path, d.orig_bytes());
        }
        app
    }

    /// 裁剪：尺寸变小、状态栏回执、`map_gen` +1、选区清空、视口重新 fit、**保存字节能读回新尺寸**。
    #[test]
    fn 裁剪到选区改尺寸并重拟合() {
        let mut app = 尺寸应用(8, 8);
        let 画布矩形 = 画布();
        assert!(app.edit.set_selection(
            &mut app.doc.doc.as_mut().expect("有图").map,
            miredit_core::Selection::new(2, 2, 5, 5)
        ));
        let gen = app.doc.map_gen;
        app.dispatch_menu(menu::model::Action::CropSelection, 画布矩形);
        let d = app.doc.doc.as_ref().expect("有图");
        assert_eq!((d.map.width, d.map.height), (4, 4), "8×8 裁 2..5 ⇒ 4×4");
        let (text, warn) = app.op.as_ref().expect("必须有回执");
        assert_eq!(text, &tx::fill(tx::STATUS_CROP_OK_TMPL, &[("w", "4"), ("h", "4")]));
        assert!(!*warn, "成功回执不是警示色");
        assert!(app.doc.map_gen > gen, "改尺寸必须让地图代号 +1（画布缓存要失效）");
        assert!(app.edit.selection().is_none(), "裁剪后选区必须清掉");
        // 视口重新 fit：缩放应等于「整张新图塞进画布」的 fit 比例
        let want = crate::canvas_view::CanvasView::fit_scale(画布矩形, 4, 4);
        assert!((app.view.scale - want).abs() < 1e-4, "裁剪后要重新 fit（{} vs {want}）", app.view.scale);
        // 保存字节：能按 Old12 读回 4×4，且格按 (left, top) 平移过
        let bytes = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        // 读回走内核的同一套解析（GUI 不依赖 mir2-format）
        let 回读 = miredit_core::Document::from_bytes(
            std::path::Path::new(r"E:\x\crop-readback.map"),
            bytes,
        )
        .expect("读回应成功");
        assert_eq!(回读.size(), (4, 4));
        for x in 0..4u16 {
            for y in 0..4u16 {
                let idx = x as usize * 4 + y as usize;
                let want = ((2 + x) as usize * 8 + (2 + y) as usize) as u32 + 1;
                assert_eq!(回读.raw().cells[idx].bk_img, want, "回读 ({x},{y})");
            }
        }
    }

    /// 没有选区 ⇒ 明确拒绝、地图一个字节不动、`map_gen` 不变。
    #[test]
    fn 裁剪无选区时拒绝() {
        let mut app = 尺寸应用(8, 8);
        let 前 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        let gen = app.doc.map_gen;
        app.dispatch_menu(menu::model::Action::CropSelection, 画布());
        let (text, warn) = app.op.as_ref().expect("必须有回执");
        assert_eq!(text, tx::STATUS_CROP_NEED_SELECTION);
        assert!(*warn, "拒绝回执要警示色");
        assert_eq!(app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节"), 前);
        assert_eq!(app.doc.map_gen, gen, "没改图 ⇒ 代号不变");
        assert_eq!(app.overlay, Overlay::None, "不该弹任何浮层");
    }

    /// 选区已覆盖整图 ⇒ 什么都不做（上游 `CropSelection` 返回 false 的那条）。
    #[test]
    fn 整图裁剪不动作() {
        let mut app = 尺寸应用(4, 4);
        assert!(app.edit.set_selection(
            &mut app.doc.doc.as_mut().expect("有图").map,
            miredit_core::Selection::new(0, 0, 3, 3)
        ));
        let gen = app.doc.map_gen;
        app.dispatch_menu(menu::model::Action::CropSelection, 画布());
        let (text, warn) = app.op.as_ref().expect("必须有回执");
        assert_eq!(text, tx::STATUS_CROP_NOOP);
        assert!(*warn);
        assert_eq!(app.doc.map_gen, gen, "整图裁剪不许改地图代号");
        assert_eq!(app.edit.undo_len(), 0, "整图裁剪不许入撤销栈");
        assert!(app.edit.selection().is_some(), "没改动就不该清选区");
    }

    /// 裁剪可撤销、可重做（撤销后保存字节逐字节回原）。
    #[test]
    fn 裁剪可撤销可重做() {
        let mut app = 尺寸应用(8, 8);
        let 前 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        assert!(app.edit.set_selection(
            &mut app.doc.doc.as_mut().expect("有图").map,
            miredit_core::Selection::new(2, 2, 5, 5)
        ));
        app.dispatch_menu(menu::model::Action::CropSelection, 画布());
        assert_eq!(app.doc.doc.as_ref().expect("有图").map.width, 4);
        // 撤销 ⇒ 回 8×8，且保存源也回旧尺寸（否则保存会因形状不符被拒）
        app.dispatch_menu(menu::model::Action::Undo, 画布());
        let d = app.doc.doc.as_mut().expect("有图");
        assert_eq!((d.map.width, d.map.height), (8, 8), "撤销回原尺寸");
        assert_eq!(d.to_bytes().expect("出字节"), 前, "撤销后保存字节必须逐字节回原");
        // 重做 ⇒ 又回 4×4
        app.dispatch_menu(menu::model::Action::Redo, 画布());
        let d = app.doc.doc.as_mut().expect("有图");
        assert_eq!((d.map.width, d.map.height), (4, 4), "重做回新尺寸");
        let bytes = d.to_bytes().expect("出字节");
        let 回读 = miredit_core::Document::from_bytes(
            std::path::Path::new(r"E:\x\crop-redo.map"),
            bytes,
        )
        .expect("读回");
        assert_eq!(回读.size().0, 4, "重做后保存字节读回 4 宽");
    }

    /// 按一次**带修饰键**的键（`Ctrl+Z` 那种；`按一次键` 只发裸键）。
    fn 按一次带修饰键(app: &mut MireditApp, key: egui::Key, modifiers: egui::Modifiers) {
        let ctx = egui::Context::default();
        // ⚠ 修饰键要放在 `RawInput::modifiers`（"这一帧开头按着谁"），
        // 只塞进 `Event::Key{modifiers}` 是不够的 —— egui 0.33 的 `InputState::modifiers`
        // 取的是 `RawInput::modifiers`，事件里那份只描述"这个按键事件当时的修饰键"。
        let input = egui::RawInput {
            modifiers,
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.handle_keys(ctx));
    }

    /// **键盘撤销/重做跨过裁剪**（R37 P2：这条入口原先绕过了保存源同步）。
    ///
    /// 判据全在**保存字节**上：`Ctrl+Z` 之后存出来的必须与裁剪前逐字节相同（原尺寸），
    /// `Ctrl+Y` 之后存出来的必须能读回新尺寸。
    #[test]
    fn 键盘撤销重做跨越裁剪() {
        let mut app = 尺寸应用(8, 8);
        let 原 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        assert!(app.edit.set_selection(
            &mut app.doc.doc.as_mut().expect("有图").map,
            miredit_core::Selection::new(2, 2, 5, 5)
        ));
        app.dispatch_menu(menu::model::Action::CropSelection, 画布());
        assert_eq!(app.doc.doc.as_ref().expect("有图").map.width, 4);

        // Ctrl+Z（键盘）⇒ 回 8×8，且**保存字节逐字节回原**
        按一次带修饰键(&mut app, egui::Key::Z, egui::Modifiers::COMMAND);
        let d = app.doc.doc.as_mut().expect("有图");
        assert_eq!((d.map.width, d.map.height), (8, 8), "Ctrl+Z 回原尺寸");
        assert_eq!(d.to_bytes().expect("出字节"), 原, "Ctrl+Z 后保存字节必须逐字节回原");

        // Ctrl+Y（键盘）⇒ 回 4×4，保存字节读回新尺寸
        按一次带修饰键(&mut app, egui::Key::Y, egui::Modifiers::COMMAND);
        let d = app.doc.doc.as_mut().expect("有图");
        assert_eq!((d.map.width, d.map.height), (4, 4), "Ctrl+Y 回新尺寸");
        let bytes = d.to_bytes().expect("出字节");
        let 回读 = miredit_core::Document::from_bytes(
            std::path::Path::new(r"E:\x\crop-kbd.map"),
            bytes,
        )
        .expect("读回");
        assert_eq!(回读.size(), (4, 4), "Ctrl+Y 后保存字节读回 4×4");
    }

    /// **`Command::Undo/Redo` 跨过裁剪**（`--edit-undo` 与网页层走的就是这条）。
    ///
    /// 与键盘用例同判据（只看保存字节），因为 R37 P2 的漏洞正是"这条入口没同步保存源"。
    #[test]
    fn 命令撤销重做跨越裁剪() {
        let mut app = 尺寸应用(8, 8);
        let 原 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        assert!(app.edit.set_selection(
            &mut app.doc.doc.as_mut().expect("有图").map,
            miredit_core::Selection::new(2, 2, 5, 5)
        ));
        app.dispatch_menu(menu::model::Action::CropSelection, 画布());
        assert_eq!(app.doc.doc.as_ref().expect("有图").map.width, 4);

        // `Command::Undo`
        let out = app.edit.apply_cmd_in(&mut app.doc, miredit_core::Command::Undo);
        assert!(out.changed > 0, "撤销要有回执");
        let d = app.doc.doc.as_mut().expect("有图");
        assert_eq!((d.map.width, d.map.height), (8, 8), "Command::Undo 回原尺寸");
        assert_eq!(d.to_bytes().expect("出字节"), 原, "Command::Undo 后保存字节回原");

        // `Command::Redo`
        let out = app.edit.apply_cmd_in(&mut app.doc, miredit_core::Command::Redo);
        assert!(out.changed > 0);
        let d = app.doc.doc.as_mut().expect("有图");
        assert_eq!((d.map.width, d.map.height), (4, 4), "Command::Redo 回新尺寸");
        let bytes = d.to_bytes().expect("出字节");
        let 回读 = miredit_core::Document::from_bytes(
            std::path::Path::new(r"E:\x\crop-cmd.map"),
            bytes,
        )
        .expect("读回");
        assert_eq!(回读.size(), (4, 4));
    }

    // ───────────────────── 工具 → 地图扩边（A-015 Expand / R36） ─────────────────────

    /// 经**动作层**做一次扩边（= 模态提交那条路；`Ok` = 成功回执，`Err` = 中文原因）。
    fn 扩边(app: &mut MireditApp, 四向: [i32; 4]) -> Result<menu::action::Outcome, String> {
        let mut cx = menu::action::Ctx {
            doc: &mut app.doc,
            view: &mut app.view,
            edit: &mut app.edit,
            panels: &mut app.panels,
            dark: &mut app.dark,
            grid: &mut app.grid,
            cell_hover: &mut app.cell_hover,
            doors_open: &mut app.doors_open,
            lighting: &mut app.lighting,
            canvas: 画布(),
            overlay: &mut app.overlay,
            settings_path: &app.settings_path,
            recent: &mut app.recent,
            tool: app.selected_tool,
            hover: app.hover,
        };
        let r = menu::action::expand_apply(&mut cx, 四向[0], 四向[1], 四向[2], 四向[3]);
        if let Ok(out) = &r {
            app.finish_menu(menu::model::Action::ExpandMap, out.clone());
        }
        r
    }

    /// 菜单项打开 440 宽模态：四个输入初值 `"0"`、焦点在第一个、没有错误。
    #[test]
    fn 扩边菜单打开模态() {
        let mut app = 尺寸应用(8, 8);
        app.dispatch_menu(menu::model::Action::ExpandMap, 画布());
        match &app.overlay {
            Overlay::ExpandMap { 文本, focus, error } => {
                assert_eq!(文本, &["0".to_string(), "0".into(), "0".into(), "0".into()]);
                assert_eq!(*focus, 0);
                assert!(error.is_none());
                assert_eq!(menu::model::Action::ExpandMap.id(), "tools.expand_map");
            }
            other => panic!("应打开扩边模态：{other:?}"),
        }
        // 模态开着时不该改图
        assert_eq!(app.doc.doc.as_ref().expect("有图").map.width, 8);
    }

    /// 扩边成功：尺寸按四向变大、回执、`map_gen` +1、重 fit、保存字节读回新尺寸。
    #[test]
    fn 扩边改变尺寸并重拟合() {
        let mut app = 尺寸应用(8, 8);
        let gen = app.doc.map_gen;
        let out = 扩边(&mut app, [2, 4, 6, 8]).expect("合法四向应成功");
        let d = app.doc.doc.as_ref().expect("有图");
        assert_eq!((d.map.width, d.map.height), (16, 20), "8+2+6 × 8+4+8");
        let (text, warn) = out.receipt.expect("要有回执");
        assert_eq!(text, tx::fill(tx::STATUS_EXPAND_OK_TMPL, &[("w", "16"), ("h", "20")]));
        assert!(!warn);
        assert!(out.touched_map, "改尺寸要标 touched_map");
        assert!(app.doc.map_gen > gen, "地图代号要 +1");
        let want = crate::canvas_view::CanvasView::fit_scale(画布(), 16, 20);
        assert!((app.view.scale - want).abs() < 1e-4, "扩边后要重 fit（{} vs {want}）", app.view.scale);
        // 保存字节：读回 16×20；旧 (0,0) 在 (2,4)，边框空白
        let bytes = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        let 回读 = miredit_core::Document::from_bytes(
            std::path::Path::new(r"E:\x\expand.map"),
            bytes,
        )
        .expect("读回");
        assert_eq!(回读.size(), (16, 20));
        assert_eq!(回读.raw().cell(2, 4).expect("图内").bk_img, 1, "旧 (0,0) 在 (2,4)");
        assert_eq!(回读.raw().cell(0, 0).expect("图内").bk_img, 0, "左上角是空白格");
    }

    /// 非法四向（奇数 / 全 0）⇒ 明确原因、地图一字节不动、不入撤销栈。
    #[test]
    fn 扩边非法报原因不改图() {
        let mut app = 尺寸应用(8, 8);
        let 前 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        let gen = app.doc.map_gen;
        let e = 扩边(&mut app, [1, 0, 0, 0]).expect_err("奇数必须拒");
        assert!(e.contains("偶数"), "{e}");
        let e = 扩边(&mut app, [0, 0, 0, 0]).expect_err("全 0 必须拒");
        assert!(e.contains("不能全为 0"), "{e}");
        assert_eq!(app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节"), 前);
        assert_eq!(app.doc.map_gen, gen);
        assert_eq!(app.edit.undo_len(), 0, "失败不许入撤销栈");
    }

    /// 扩边后 **Ctrl+Z** ⇒ 保存字节逐字节回原（与裁剪同一条整档撤销）。
    #[test]
    fn 扩边后键盘撤销回原字节() {
        let mut app = 尺寸应用(8, 8);
        let 原 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        扩边(&mut app, [2, 2, 2, 2]).expect("应成功");
        assert_eq!(app.doc.doc.as_ref().expect("有图").map.width, 12);
        按一次带修饰键(&mut app, egui::Key::Z, egui::Modifiers::COMMAND);
        let d = app.doc.doc.as_mut().expect("有图");
        assert_eq!((d.map.width, d.map.height), (8, 8), "撤销回原尺寸");
        assert_eq!(d.to_bytes().expect("出字节"), 原, "撤销后保存字节必须逐字节回原");
    }

    // ───────────────────── 编辑 → 填充选区：同屏不并排同义（D-004） ─────────────────────

    /// **D-004 拍板①：选区填充成功后不再并排出现两句同义文案**。
    ///
    /// 判据（同屏可验）：
    /// ① 成功填充 ⇒ `app.op`（= 状态栏「上一次操作」段）**为空**，
    ///    而内核 `last_fill` 在（⇒ 状态栏「填充」段照旧显示 `填充 N 格`）；
    /// ② 被拒的填充（没有选区）⇒ 回执照写且是警示色（那时没有「填充」段，原因只能靠它说）。
    #[test]
    fn 填充选区不再并排两句同义() {
        let mut app = 尺寸应用(8, 8);
        // 取一格做笔刷（地表层默认；格 (0,0) 的 bk_img = 1）
        let brush = {
            let d = app.doc.doc.as_ref().expect("有图");
            app.edit.pick_at(&d.map, 0, 0)
        };
        assert!(brush.is_some(), "格 (0,0) 应有地表图号可吸");
        assert!(app.edit.set_selection(
            &mut app.doc.doc.as_mut().expect("有图").map,
            miredit_core::Selection::new(0, 0, 3, 3)
        ));
        app.dispatch_menu(menu::model::Action::FillSelection, 画布());
        // ① 成功：只有「填充」段，没有「上一次操作」段
        assert!(
            app.edit.last_fill().is_some(),
            "填充成功要留下 last_fill（状态栏「填充」段的数据源）"
        );
        assert!(app.op.is_none(), "填充成功不许再写「上一次操作」段：{:?}", app.op);
        let (fill_n, _) = (
            app.edit.last_fill().map(|r| r.cells).unwrap_or(0),
            app.edit.last_fill().map(|r| r.layer),
        );
        assert!(fill_n > 0, "地表 step=2 ⇒ 4×4 选区应真填到格");
        assert_eq!(app.edit.undo_len(), 1, "整次填充 = 一步撤销");
        // ② 被拒：清掉选区再填 ⇒ 回执必须写（且警示色）
        app.edit.clear_selection();
        app.dispatch_menu(menu::model::Action::FillSelection, 画布());
        let (text, warn) = app.op.as_ref().expect("被拒必须有回执");
        assert!(*warn, "拒绝回执要警示色");
        assert!(!text.is_empty());
    }

    /// **D-004 键盘入口（`Ctrl+Shift+V`）与菜单同判据**：成功不写 `op`，拒绝照写。
    ///
    /// 与 `填充选区不再并排两句同义` 同一组断言，只是走**真键盘路径**（三条入口共用
    /// `selection_ops::写操作回执`，这条用例钉住键盘那条也接了同一个函数）。
    #[test]
    fn 键盘填充选区同判据() {
        let mut app = 尺寸应用(8, 8);
        let brush = {
            let d = app.doc.doc.as_ref().expect("有图");
            app.edit.pick_at(&d.map, 0, 0)
        };
        assert!(brush.is_some(), "格 (0,0) 应有地表图号可吸");
        assert!(app.edit.set_selection(
            &mut app.doc.doc.as_mut().expect("有图").map,
            miredit_core::Selection::new(0, 0, 3, 3)
        ));
        // 真键盘：Ctrl+Shift+V（修饰键要放 `RawInput::modifiers`，事件里那份 egui 不读）
        按一次带修饰键(
            &mut app,
            egui::Key::V,
            egui::Modifiers {
                command: true,
                shift: true,
                ..Default::default()
            },
        );
        // ① 成功：`op` 空，`status_fill` 有值（同一件事只在一处说）
        assert!(app.op.is_none(), "键盘填充成功也不许写「上一次操作」段：{:?}", app.op);
        let 文案 = crate::screen_text::build(
            &app.doc,
            &app.view,
            app.hover,
            &app.lines,
            &app.edit,
        );
        let fill = 文案.status_fill.expect("状态栏「填充」段必须有值");
        assert!(fill.contains("填充"), "「填充」段文案：{fill}");
        assert_eq!(app.edit.undo_len(), 1, "整次填充 = 一步撤销");
        // ② 拒绝（清掉选区再按一次）：`op` 必须有值且是警示色
        app.edit.clear_selection();
        按一次带修饰键(
            &mut app,
            egui::Key::V,
            egui::Modifiers {
                command: true,
                shift: true,
                ..Default::default()
            },
        );
        let (text, warn) = app.op.as_ref().expect("被拒的键盘填充必须给回执");
        assert!(*warn, "拒绝回执要警示色");
        assert!(!text.is_empty());
    }

    // ───────────────────── 键位 R9-F2：工具键 / 阻挡开关 / 平移 ─────────────────────

    /// **工具键 `B/P/E/V/K` 各选自己的槽位**（上游 `MainWindow.axaml.cs:989`）。
    ///
    /// 槽位下标 = `icons::TOOLS` 的顺序（与官方 axaml 的按钮顺序一致）：
    /// `B`=0 画笔 / `P`=1 铅笔 / `E`=3 橡皮 / `V`=6 吸管 / `K`=7 克隆。
    ///
    /// ⚠ `P` / `K` 对应的**工具**本仓尚未接线 ⇒ `edit.tool()` 是 `Tool::Unwired`（如实报未接线），
    /// 但槽位必须真的切过去（否则"按了什么都没发生"才是假功能）。
    #[test]
    fn 工具键各选自己槽位() {
        let mut app = 尺寸应用(8, 8);
        use miredit_core::Tool;
        for (key, slot, expect) in [
            (egui::Key::B, edit::TOOL_BRUSH_IDX, Tool::Brush),
            (egui::Key::P, edit::TOOL_PENCIL_IDX, Tool::Unwired),
            (egui::Key::E, miredit_core::TOOL_ERASER_IDX, Tool::Eraser),
            (egui::Key::V, miredit_core::TOOL_PICKUP_IDX, Tool::Pickup),
            (egui::Key::K, edit::TOOL_CLONE_IDX, Tool::Unwired),
        ] {
            按一次键(&mut app, key);
            assert_eq!(app.selected_tool, slot, "{key:?} 应选到槽位 {slot}");
            assert_eq!(app.edit.tool(), expect, "{key:?} 选中的工具");
        }
        // 对照：`F`（既有的填充键）仍选槽位 2，别被新键挤掉
        按一次键(&mut app, egui::Key::F);
        assert_eq!(app.selected_tool, miredit_core::TOOL_BUCKET_IDX);
        assert_eq!(app.edit.tool(), Tool::Bucket);
    }

    /// **`P` / `K` 选中的工具尚未接线 ⇒ 落笔一格都不改**（R40：只切槽、画布无动作）。
    ///
    /// 判据四条：格内容不变、字节逐字节不变、不置脏、不进撤销栈。
    /// 这条挡住"为了凑键位偷偷接一个半成品工具"的路 —— 没实现就老实不动。
    #[test]
    fn 铅笔与克隆落笔不改图() {
        for (key, slot) in [
            (egui::Key::P, edit::TOOL_PENCIL_IDX),
            (egui::Key::K, edit::TOOL_CLONE_IDX),
        ] {
            let mut app = 尺寸应用(8, 8);
            // 先吸一格当笔刷，确保"不改图"不是因为没笔刷
            let brush = {
                let d = app.doc.doc.as_ref().expect("有图");
                app.edit.pick_at(&d.map, 0, 0)
            };
            assert!(brush.is_some(), "{key:?} 前置：应吸到笔刷");
            let 前 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
            按一次键(&mut app, key);
            assert_eq!(app.selected_tool, slot);
            assert_eq!(app.edit.tool(), miredit_core::Tool::Unwired, "{key:?} 工具未接线");
            // 落笔一次（走与画布相同的入口）
            let n = {
                let d = app.doc.doc.as_mut().expect("有图");
                app.edit.paint_stroke_once(&mut d.map, 1, 1)
            };
            assert_eq!(n, 0, "{key:?} 未接线工具不许改任何格");
            assert!(!app.edit.dirty(), "{key:?} 不许置脏");
            assert_eq!(app.edit.undo_len(), 0, "{key:?} 不许入撤销栈");
            let d = app.doc.doc.as_mut().expect("有图");
            assert_eq!(d.to_bytes().expect("出字节"), 前, "{key:?} 字节必须逐字节不变");
        }
    }

    /// **`C` 只切低位/高位阻挡标记，不动当前工具**（上游 `MainWindow.axaml.cs:997` +
    /// `MapViewport.cs:82-85`：getter = 低或高，setter 同写两边）。
    #[test]
    fn c键只切阻挡不动工具() {
        let mut app = 尺寸应用(8, 8);
        // 先把工具切成填充，验证 C 不会把它改回画笔
        按一次键(&mut app, egui::Key::F);
        assert_eq!(app.edit.tool(), miredit_core::Tool::Bucket);
        assert!(!app.view.overlay.low && !app.view.overlay.high, "初始两通道都关");
        // R40：切阻挡**不脏、不入栈、不动图**（它只是显示开关）
        let 前 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");

        // 第一次 C ⇒ 两个通道**同时**打开
        按一次键(&mut app, egui::Key::C);
        assert!(app.view.overlay.low && app.view.overlay.high, "两个通道必须同时开");
        assert_eq!(app.edit.tool(), miredit_core::Tool::Bucket, "C 不许改当前工具");
        let (text, warn) = app.op.as_ref().expect("C 要写回执");
        assert_eq!(text, tx::MENU_COLLISION_ON);
        assert!(!*warn, "这不是拒绝，别染警示色");

        // 再按一次 ⇒ 两个通道同时关
        按一次键(&mut app, egui::Key::C);
        assert!(!app.view.overlay.low && !app.view.overlay.high, "再按一次两通道同关");
        assert_eq!(app.op.as_ref().expect("C 要写回执").0, tx::MENU_COLLISION_OFF);

        // R40 的另外两个组合：**半开 ⇒ 全关**（getter = 低或高，setter 同写两边）
        for (low, high) in [(true, false), (false, true)] {
            app.view.overlay.low = low;
            app.view.overlay.high = high;
            按一次键(&mut app, egui::Key::C);
            assert!(
                !app.view.overlay.low && !app.view.overlay.high,
                "半开（low={low}, high={high}）按 C 必须变全关"
            );
        }
        // 四组合总表（再走一遍全开 / 全关两个角）
        for (before_low, before_high, want) in [
            (false, false, true),
            (true, true, false),
            (true, false, false),
            (false, true, false),
        ] {
            app.view.overlay.low = before_low;
            app.view.overlay.high = before_high;
            按一次键(&mut app, egui::Key::C);
            assert_eq!(
                app.view.overlay.low, want,
                "组合 (low={before_low}, high={before_high})：low 应为 {want}"
            );
            assert_eq!(app.view.overlay.high, want, "两个通道必须同值");
        }

        // `Ctrl+C` 是复制，**不许**碰阻挡通道
        app.view.overlay.low = false;
        app.view.overlay.high = false;
        按一次带修饰键(&mut app, egui::Key::C, egui::Modifiers::COMMAND);
        assert!(!app.view.overlay.low && !app.view.overlay.high, "Ctrl+C 不许切阻挡");

        // R40：整套切下来地图一字节不动、不脏、不进撤销栈
        assert_eq!(app.edit.undo_len(), 0, "切阻挡不许入撤销栈");
        assert!(!app.edit.dirty(), "切阻挡不许置脏");
        assert_eq!(
            app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节"),
            前,
            "切阻挡不许改地图字节"
        );
    }

    /// **`Ctrl+C` / `Ctrl+V` / `Ctrl+S` 的原功能不被新裸键吃掉**（R40 守卫的 `!command` 分流）。
    ///
    /// 三个反例各自对应一个新裸键：`C`(阻挡) / `V`(吸管) / `S`(平移) —— 带 Ctrl 时一个都不许触发。
    #[test]
    fn ctrl组合不被裸键吃掉() {
        let mut app = 尺寸应用(8, 8);
        app.view.scale = 1.0;
        app.view.pan_x = 0.0;
        app.view.pan_y = 0.0;
        app.view.overlay.low = false;
        app.view.overlay.high = false;
        let 前工具 = app.selected_tool;

        // `Ctrl+V`：不许变成吸管（裸 V 才是吸管）
        按一次带修饰键(&mut app, egui::Key::V, egui::Modifiers::COMMAND);
        assert_eq!(app.selected_tool, 前工具, "Ctrl+V 不许切工具（它是粘贴）");

        // `Ctrl+S`：不许平移（裸 S 才平移）
        按一次带修饰键(&mut app, egui::Key::S, egui::Modifiers::COMMAND);
        assert_eq!(
            (app.view.pan_x, app.view.pan_y),
            (0.0, 0.0),
            "Ctrl+S 不许平移（它是保存）"
        );

        // `Ctrl+C`：不许切阻挡（裸 C 才切）；它真做的事是复制 —— 先框一个选区再验
        assert!(app.edit.set_selection(
            &mut app.doc.doc.as_mut().expect("有图").map,
            miredit_core::Selection::new(0, 0, 1, 1)
        ));
        按一次带修饰键(&mut app, egui::Key::C, egui::Modifiers::COMMAND);
        assert!(!app.view.overlay.low && !app.view.overlay.high, "Ctrl+C 不许切阻挡");
        assert!(app.edit.clipboard().is_some(), "Ctrl+C 必须真的复制了选区");
    }

    /// **`W/A/S/D` 每次平移 48 屏幕像素**（上游 `PanBy`，`MapViewport.Minimap.cs:11-22`）。
    ///
    /// 判据刻意用 `pan * scale`（= 屏幕像素）而不是 `pan`：缩放 200% 时一次 W 仍是 48 屏幕像素，
    /// 也就是世界坐标只走 24 —— 这正是"按屏幕像素、不按格"的区别。
    #[test]
    fn wasd按屏幕像素平移() {
        let mut app = 尺寸应用(8, 8);
        for scale in [1.0_f32, 2.0, 0.5] {
            app.view.scale = scale;
            app.view.pan_x = 0.0;
            app.view.pan_y = 0.0;
            for (key, dx, dy) in [
                (egui::Key::W, 0.0_f32, -1.0_f32),
                (egui::Key::S, 0.0, 1.0),
                (egui::Key::A, -1.0, 0.0),
                (egui::Key::D, 1.0, 0.0),
            ] {
                按一次键(&mut app, key);
                let 屏幕dx = (app.view.pan_x - 0.0) * scale;
                let 屏幕dy = (app.view.pan_y - 0.0) * scale;
                assert!(
                    (屏幕dx - dx * crate::metrics::PAN_STEP_PX).abs() < 0.01,
                    "scale={scale} {key:?}：屏幕水平位移 {屏幕dx}（期望 {}）",
                    dx * crate::metrics::PAN_STEP_PX
                );
                assert!(
                    (屏幕dy - dy * crate::metrics::PAN_STEP_PX).abs() < 0.01,
                    "scale={scale} {key:?}：屏幕垂直位移 {屏幕dy}（期望 {}）",
                    dy * crate::metrics::PAN_STEP_PX
                );
                // 复位，下一次按键单独量
                app.view.pan_x = 0.0;
                app.view.pan_y = 0.0;
            }
        }
    }

    /// **两个搜索框有焦点时，裸键归键盘输入、不归画布**（R40 守卫）。
    ///
    /// 否则在「资源库」搜索框里敲 `b` 会顺手换工具、敲 `w` 会把地图推走。
    #[test]
    fn 搜索框有焦点时裸键不生效() {
        for 是印章面板 in [false, true] {
            let mut app = 尺寸应用(8, 8);
            app.view.scale = 1.0;
            app.view.pan_x = 0.0;
            app.view.pan_y = 0.0;
            app.view.overlay.low = false;
            app.view.overlay.high = false;
            if 是印章面板 {
                app.stamps.set_focus(true);
            } else {
                app.palette.set_focus(true);
            }
            for key in [
                egui::Key::W,
                egui::Key::A,
                egui::Key::C,
                egui::Key::B,
                egui::Key::V,
            ] {
                按一次键(&mut app, key);
            }
            assert_eq!(
                (app.view.pan_x, app.view.pan_y),
                (0.0, 0.0),
                "搜索框有焦点（印章={是印章面板}）时不许平移"
            );
            assert!(
                !app.view.overlay.low && !app.view.overlay.high,
                "搜索框有焦点时不许切阻挡"
            );
            assert_eq!(
                app.selected_tool,
                edit::TOOL_BRUSH_IDX,
                "搜索框有焦点时不许换工具（默认应是画笔）"
            );
            // 清掉焦点 ⇒ 同一个键立刻恢复生效（证明确实是"焦点"在拦，不是键没绑）
            if 是印章面板 {
                app.stamps.set_focus(false);
            } else {
                app.palette.set_focus(false);
            }
            按一次键(&mut app, egui::Key::C);
            assert!(
                app.view.overlay.low && app.view.overlay.high,
                "焦点清掉后 `C` 必须恢复生效"
            );
        }
    }

    /// **模态打开时 `C` 与 `W/A/S/D` 都不生效**（`handle_keys` 开头对浮层让位）。
    #[test]
    fn 模态打开时键位让位() {
        let mut app = 尺寸应用(8, 8);
        app.overlay = Overlay::Shortcuts;
        app.view.scale = 1.0;
        app.view.pan_x = 0.0;
        app.view.pan_y = 0.0;
        app.view.overlay.low = false;
        app.view.overlay.high = false;
        for key in [egui::Key::W, egui::Key::A, egui::Key::C, egui::Key::B] {
            按一次键(&mut app, key);
        }
        assert_eq!((app.view.pan_x, app.view.pan_y), (0.0, 0.0), "浮层开着不许平移");
        assert!(!app.view.overlay.low && !app.view.overlay.high, "浮层开着不许切阻挡");
    }

    // ───────────────────── 工具 → 检查地图（A-015 / R32-R33） ─────────────────────

    /// 造一个「有图且带 2 条门警告」的应用：4×4，全图不可走，`(1, 2)` 是门格。
    ///
    /// 判据来自 `miredit_core::check_map`：整图不可走 ⇒ 门格 3×3 里没有可走格（1 条），
    /// 门格没有前景图（`fr_img == 0`，再 1 条）⇒ `total == 2`。
    /// 用合成图而不是真实 `0.map`：本组用例验的是"菜单→模态→定位"的接线，与地图多大无关。
    fn 有门图应用() -> MireditApp {
        let mut app = 空应用();
        let mut b = vec![0u8; 52 + 12 * 16];
        b[0..2].copy_from_slice(&4u16.to_le_bytes());
        b[2..4].copy_from_slice(&4u16.to_le_bytes());
        for i in 0..16usize {
            let base = 52 + i * 12;
            // 背景 bit15 = 阻挡（全图不可走）
            b[base..base + 2].copy_from_slice(&0x8000u16.to_le_bytes());
        }
        // (1, 2) 的门：列优先 `x * h + y` = 1 * 4 + 2 = 6
        b[52 + 6 * 12 + 6] = 1;
        app.doc.doc = Some(
            crate::mapdoc::MapDoc::from_bytes(std::path::Path::new(r"E:\x\door.map"), b)
                .expect("4×4 合成图应能解析"),
        );
        app.doc.map_error = None;
        // 图库置成「装载失败」：本组用例要的是 env 无关的「没有图库」分支
        app.doc.assets = crate::mapdoc::Assets::Failed("测试：不装载图库".to_owned());
        if let Some(d) = app.doc.doc.as_ref() {
            let _ = app.edit.bind_document(&d.path, d.orig_bytes());
        }
        app
    }

    /// 往应用里装一个**合成 WIL 图库**（2 帧的 `Tiles`，见 `mir2_assets` 的同名用例）。
    ///
    /// 为什么要真装一个：`Assets::source()` 只认 `WilSource`，探针是它包出来的；
    /// 不装就只能验「没有图库」那半边，验不到"图号问题真的会报出来"。
    fn 装合成图库(app: &mut MireditApp) {
        let dir = std::env::temp_dir().join(format!("miredit_app_checkmap_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let sub = dir.join("data");
        std::fs::create_dir_all(&sub).expect("建临时目录");
        // 帧 0：2×2 8bit、锚点 (0,0)、未压缩（csize = 0，裸数据 align4(2)×2 = 8 字节）
        let mut 帧0 = Vec::new();
        帧0.extend_from_slice(&3u16.to_le_bytes());
        帧0.extend_from_slice(&0u16.to_le_bytes());
        帧0.extend_from_slice(&2u16.to_le_bytes());
        帧0.extend_from_slice(&2u16.to_le_bytes());
        帧0.extend_from_slice(&0i16.to_le_bytes());
        帧0.extend_from_slice(&0i16.to_le_bytes());
        帧0.extend_from_slice(&0u32.to_le_bytes());
        帧0.extend_from_slice(&[0x22u8; 8]);
        let mut wzl = vec![0u8; 64];
        let off0 = 64u32;
        let off1 = off0 + 帧0.len() as u32;
        wzl.extend_from_slice(&帧0);
        wzl.extend_from_slice(&[0u8; 16]); // 帧 1：空帧
        let mut wzx = vec![0u8; 48];
        wzx[0x2C..0x30].copy_from_slice(&2u32.to_le_bytes());
        wzx.extend_from_slice(&off0.to_le_bytes());
        wzx.extend_from_slice(&off1.to_le_bytes());
        std::fs::write(sub.join("Tiles.wzl"), &wzl).expect("写 wzl");
        std::fs::write(sub.join("Tiles.wzx"), &wzx).expect("写 wzx");
        let paths = mir2_assets::LibPaths::discover(&dir).expect("扫描应成功");
        let src = mir2_assets::WilSource::open(&paths).expect("应能打开");
        app.doc.assets = crate::mapdoc::Assets::Ready {
            paths,
            src: Box::new(src),
            号段: miredit_core::库号号段::Wemade,
        };
    }

    /// 打开「检查地图」：模态里有报告、预选第 0 行、`gen` = 当前地图代号、状态栏有回执。
    ///
    /// **只读**判据也在这一条里：前后 `to_bytes()` 逐字节相等、撤销栈长度不变。
    #[test]
    fn 检查地图打开模态并预选首行() {
        let mut app = 有门图应用();
        let 前 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        let 撤销前 = app.edit.undo_len();
        app.dispatch_menu(menu::model::Action::CheckMap, 画布());
        match &app.overlay {
            Overlay::CheckMap {
                report,
                selected,
                scroll,
                gen,
                has_libs,
            } => {
                assert_eq!(report.total, 2, "两条门警告：{:?}", report.issues);
                assert_eq!(*selected, Some(0), "非空报告要预选第 0 行（官方 SelectedIndex = 0）");
                assert_eq!(*scroll, 0);
                assert_eq!(*gen, app.doc.map_gen, "gen 必须是检查那一刻的地图代号");
                assert!(!*has_libs, "图库装失败 ⇒ 图号一项整体没查");
                assert!(report.未检查.is_empty(), "Old12 三族都可查：{:?}", report.未检查);
            }
            other => panic!("应打开检查地图模态，实际 {other:?}"),
        }
        let 后 = app.doc.doc.as_mut().expect("有图").to_bytes().expect("出字节");
        assert_eq!(前, 后, "检查地图不许改任何字节");
        assert_eq!(app.edit.undo_len(), 撤销前, "检查地图不许进撤销栈");
        let (text, warn) = app.op.as_ref().expect("必须有状态栏回执");
        assert!(text.contains("共 2 条"), "回执要写总数：{text}");
        assert!(!*warn, "不是警示色");
    }

    /// 没有地图时点「检查地图」：**不弹模态**、给一条带原因的拒绝回执。
    #[test]
    fn 检查地图无图时拒绝() {
        let mut app = 空应用();
        app.dispatch_menu(menu::model::Action::CheckMap, 画布());
        assert_eq!(app.overlay, Overlay::None, "没图时不该弹模态");
        let (text, warn) = app.op.as_ref().expect("必须有拒绝回执");
        assert_eq!(text, tx::STATUS_CHECK_MAP_NEEDS_MAP);
        assert!(*warn, "拒绝回执要警示色（State::NeedsMap 同口径）");
    }

    /// 「定位」：**1×1 选区**（闭区间）+ 画布把该格居中（上游 `LocateCell` 的等价物）。
    #[test]
    fn 检查地图定位设一基选区并居中() {
        let mut app = 有门图应用();
        let 画布矩形 = 画布();
        app.dispatch_menu(menu::model::Action::CheckMap, 画布矩形);
        // 取报告里的第 0 条问题格（门格 = (1, 2)）
        let (x, y) = match &app.overlay {
            Overlay::CheckMap { report, .. } => {
                let i = report.issues.first().expect("应有问题");
                (i.x, i.y)
            }
            other => panic!("应先打开模态：{other:?}"),
        };
        assert_eq!((x, y), (1, 2), "门格坐标");
        let 前 = (app.view.pan_x, app.view.pan_y);
        let 回执 = app.locate_cell(x, y, 画布矩形);
        let sel = app.edit.selection().expect("必须有选区");
        assert_eq!(
            (sel.x0, sel.y0, sel.x1, sel.y1),
            (1, 2, 1, 2),
            "定位必须是 1×1 选区（闭区间）"
        );
        assert!(
            (app.view.pan_x, app.view.pan_y) != 前,
            "定位必须真的把视口挪到那一格"
        );
        assert_eq!(回执, tx::fill(tx::STATUS_CHECK_MAP_LOCATE_TMPL, &[("x", "1"), ("y", "2")]));
        // 只读：定位不动字节、不进撤销栈
        assert_eq!(app.edit.undo_len(), 0, "选区不入撤销栈（内核既有语义）");
    }

    /// 地图一改（`map_gen` +1）⇒ 浮层里的 `gen` 与当前值不等 ⇒ 结果算陈旧。
    #[test]
    fn 检查地图结果随地图变化变陈旧() {
        let mut app = 有门图应用();
        app.dispatch_menu(menu::model::Action::CheckMap, 画布());
        let 检查时 = match &app.overlay {
            Overlay::CheckMap { gen, .. } => *gen,
            other => panic!("应先打开模态：{other:?}"),
        };
        assert_eq!(检查时, app.doc.map_gen, "刚检查完不陈旧");
        app.touch_map();
        assert_ne!(app.doc.map_gen, 检查时, "改图后地图代号必须变");
        // 绘图/交互用的是同一个判据：`gen != map_gen` ⇒ 陈旧
        assert_ne!(
            match &app.overlay {
                Overlay::CheckMap { gen, .. } => *gen,
                _ => panic!("浮层丢了"),
            },
            app.doc.map_gen
        );
        // 陈旧文案由 `menu::check_map::summary_text(_, true)` 给
        assert_eq!(
            menu::check_map::summary_text(&miredit_core::MapCheckReport::default(), true),
            tx::CHECK_MAP_STALE
        );
    }

    /// 装了图库、图号越界 ⇒ 报告里出现 `Invalid image number`（端到端：菜单 → 探针 → 报告）。
    #[test]
    fn 检查地图有库时报图号问题() {
        let mut app = 有门图应用();
        装合成图库(&mut app);
        // 让 (0,0) 引用一个不存在的图号（Tiles 只有 2 帧 ⇒ 图号 9 越界）
        if let Some(d) = app.doc.doc.as_mut() {
            d.map.cells[0].bk_img = 9;
        }
        app.dispatch_menu(menu::model::Action::CheckMap, 画布());
        match &app.overlay {
            Overlay::CheckMap { report, has_libs, .. } => {
                assert!(*has_libs, "装了图库 ⇒ has_libs 必须为真（界面不该再提示去装库）");
                assert!(
                    report
                        .issues
                        .iter()
                        .any(|i| i.message.contains(miredit_core::MSG_IMAGE_RANGE)),
                    "越界图号必须报出来：{:?}",
                    report.issues
                );
                assert!(
                    report
                        .issues
                        .iter()
                        .any(|i| i.message.starts_with("Ground:") && i.message.contains("Library 0")),
                    "地面层要带上库号：{:?}",
                    report.issues
                );
            }
            other => panic!("应打开检查地图模态：{other:?}"),
        }
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!(
            "miredit_app_checkmap_{}",
            std::process::id()
        )));
    }

}
