//! 主题配色 —— **成对定义的两套色板**（浅色 [`LIGHT`] / 深色 [`DARK`]）+ 当前主题选择。
//!
//! # 一、为什么要成对定义
//!
//! 2026-11-05 起菜单「视图 → 深色模式」要能真切换，而**绘制处一律不许写死颜色**：
//! 所有绘制代码取色都走 [`ui()`]（返回当前生效的 [`Palette`]），切换主题只改一个开关
//! （[`set_dark`]），全界面（标题栏/菜单栏/面板/画布底/状态栏/弹层）随之变色。
//!
//! # 二、浅色那套从哪来（不变）
//!
//! CrystalStudio 的 `App.axaml` 里那套深色 `#14171D` 是 **XAML 声明值**，运行期
//! `ThemeService.ApplyPalette(dark=false, …)` 会按系统 accent 重算 ⇒ 上屏是浅色 + 金色强调。
//! 因此 [`LIGHT`] 的每个值**一律取自运行期截图**
//! `layout/refs/crystalstudio-main-runtime-1500x950.png`（1500×950，
//! 内容区原点 = (8, 0)，内容区尺寸 = 1484×942，见 `metrics::REF_CONTENT_ORIGIN`）。
//! 取色方法：对每个区域做**直方图**（Pillow，区域像素 Counter 取众数），
//! 每个字段的文档注释里写着 `(x,y)-(x,y)` 取样矩形与占比。
//!
//! # 三、深色那套从哪来（**不是自己发明的**）
//!
//! 官方 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:35-57` 的 `ApplyPalette(bool dark, …)`
//! 是**同一份 XAML 资源**的双向变换：XAML 里那些 `ThemeXXXXXX` 键就是**深色值**
//! （`_upstream/CrystalStudio/App.axaml:1` 的 `RequestedThemeVariant="Dark"` 即默认深色），浅色是算出来的。
//! 实测两条交叉验证（本文档作者按 `:47` 的公式手算）：
//!
//! ```text
//! Theme1C2028 → 浅色 #F5F6F7  = 本文件 LIGHT.panel_bg   ✔
//! Theme303640 → 浅色 #EAECEF  = 本文件 LIGHT.separator  ✔
//! Theme252C38 → 浅色 #EFF1F3  = 本文件 LIGHT.input_bg   ✔
//! Theme14181F → 浅色 #F9FAFA  = 本文件 LIGHT.input_search_bg ✔
//! ```
//!
//! ⇒ **深色板的每个值都直接抄官方 XAML 资源**（`App.axaml` 的 `ThemeXXXXXX` 键 /
//! 命名刷 `Panel`·`Muted`·`Accent`·`Line`，以及 `ThemeService` 里写死的几个），
//! 每个字段的注释里标 `官方: App.axaml:NN ThemeXXXXXX` 或 `官方: ThemeService.Avalonia.cs:NN`。
//! 少数「浅色有、官方深色无对应角色」的（如金色标题栏），取官方**同角色**的深色资源并在
//! 注释里写明「映射是我选的、值是官方的」。
//!
//! # 四、当前主题存在哪
//!
//! [`set_dark`] 写的是**线程局部**开关：本 GUI 单线程（`eframe::App::update` 在同一线程），
//! 而 `cargo test` 每条用例各跑一个线程 ⇒ 用例之间不会互相污染（默认都是浅色）。
//! 持久化（`%LOCALAPPDATA%\miredit\settings.json`）在 [`crate::menu::settings`]。

use std::cell::Cell;

use egui::Color32;

/// 从分量构造**预乘**半透明色（egui 的 `Color32` 内部就是预乘存储）。
const fn rgba_pm(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(r, g, b, a)
}

/// 一套完整配色。字段顺序 = 视觉层级（顶栏 → 面板 → 画布 → 状态栏 → 控件 → 强调）。
///
/// ⚠ **不要在绘制处再写颜色字面量**：任何新控件都从本结构体取色，
/// 否则深色模式下那一处会保持浅色（本单的验收项之一就是"没有地方仍用浅色常量"）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    // ── 顶栏 ──
    /// 标题栏底色。
    pub title_bar: Color32,
    /// 标题栏文字色。
    pub title_bar_text: Color32,
    /// 菜单栏底色。
    pub menu_bar: Color32,
    /// 菜单栏文字色。
    pub menu_bar_text: Color32,

    // ── 面板 ──
    /// 面板内容区底色（左/右停靠区面板体）。
    pub panel_bg: Color32,
    /// 面板标题条底色。
    pub panel_header: Color32,
    /// 视口留白底色（中列地图面之外的区域）。
    pub view_surround: Color32,
    /// 画布（地图区域）的**底层底色**：地图位图之外的区域露出的颜色。
    pub canvas_bg: Color32,
    /// 状态栏底色。
    pub status_bar: Color32,

    // ── 标签条（中列多地图标签，上游 `_upstream/CrystalStudio/EditorControls.axaml:58-66` 的 `PaletteTabItem`） ──
    /// 标签条**容器**底色（每张标签的 `Border#Chrome` 背景是 `Transparent`，透出的就是它）。
    ///
    /// 官方: `_upstream/CrystalStudio/MainWindow.MapTabs.cs:36`
    ///       `mapTabs.SetResource(TemplatedControl.BackgroundProperty, "Theme1C2028")`；
    ///       键定义 `_upstream/CrystalStudio/App.axaml:8` `Theme1C2028` = `#1C2028`。
    /// ⚠ 不复用 `menu_bar`：两者深色同值（都是 `Panel`/`Theme1C2028`），但**角色不同**
    ///   （菜单条 vs 标签条容器），本仓历史上"借错角色"已出过多次缺陷
    ///   （见 `status_bar` / `status_bar_text` / `check_box_border` 的订正记录）。
    pub tab_strip_bg: Color32,
    /// 标签**悬停**底色（上游 `:pointerover`）。
    ///
    /// 官方: `_upstream/CrystalStudio/EditorControls.axaml:63`
    ///       `^:pointerover /template/ Border#Chrome` 的 `Background = {DynamicResource Theme29313E}`；
    ///       键定义 `_upstream/CrystalStudio/App.axaml:14` `Theme29313E` = `#29313E`。
    pub tab_hover_bg: Color32,
    /// **选中**标签的底边线色（`BorderThickness="0,0,0,2"` 那 2px 的 bottom 边）。
    ///
    /// 官方: `_upstream/CrystalStudio/EditorControls.axaml:64`
    ///       `^:selected /template/ Border#Chrome` 的 `BorderBrush = {DynamicResource Theme8CA1FF}`；
    ///       键定义 `_upstream/CrystalStudio/App.axaml:47` `Theme8CA1FF` = `#8CA1FF`。
    /// ⚠ **不是 `accent_dark`**：深色下 `accent_dark` 是 `Accent` `#8498FF`
    ///   （`_upstream/CrystalStudio/ThemeService.Avalonia.cs:51` 用 accent 覆盖了 `Theme6378DF`，与 `Theme8CA1FF` 无关）；
    ///   上游这一条是**独立的键**，照抄才不丢 `#8CA1FF` 与 `#8498FF` 的色差。
    pub tab_selected_underline: Color32,
    /// **关闭符悬停底**（标签条上那个 `×` 被指针压住时的底色）。
    ///
    /// 官方: 关闭钮用的是 `QuietButton`（`_upstream/CrystalStudio/MainWindow.MapTabs.cs:58` `close.Theme =
    ///       (Style)this.FindResource("QuietButton")`），而 `QuietButton`
    ///       （`_upstream/CrystalStudio/EditorControls.axaml:17`）是 `BasedOn="{StaticResource {x:Type Button}}"`
    ///       **只覆盖 `Background` / `BorderBrush` 为 `Transparent`** ⇒ 基模板那条
    ///       `^:pointerover /template/ Border#Chrome` 的 `Background`
    ///       （`_upstream/CrystalStudio/EditorControls.axaml:12`）= `Theme394354`（`_upstream/CrystalStudio/App.axaml:26` = `#394354`）
    ///       **照样生效** ⇒ 上游**有** hover 底，不是"无 hover"。
    /// ⚠ 它**不是** `tab_hover_bg`（那个是 `PaletteTabItem` 的 `Theme29313E`，
    ///   `_upstream/CrystalStudio/EditorControls.axaml:63`）：两个键不同色（`#394354` vs `#29313E`），不许互相顶替。
    pub close_hover_bg: Color32,

    // ── 文字 ──
    /// 正文/常规文字色。
    pub text: Color32,
    /// 次要/占位/禁用文字色。
    pub text_muted: Color32,
    /// **状态栏正文色**（`Status` 那段「就绪」/「打开失败…」的文字）。
    ///
    /// ⚠ **不是 `text` 的别名**：官方给状态栏正文**单独的键** `ThemeADB8C9`
    /// （`_upstream/CrystalStudio/App.axaml:55`；消费点 `_upstream/CrystalStudio/MainWindow.axaml:84` 的 `Status` TextBlock
    /// `Foreground="{DynamicResource ThemeADB8C9}"`），深色下它比 `text`
    /// （`Window` 的 `Foreground` `#E5E9F0`）**暗一档**。借用 `text` 会让状态栏正文
    /// 比官方亮一档 —— 审计项 H A-004 就是这一条。
    ///
    /// 浅色侧两者**同值** `#434D5A`，那是 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:47` 常量支的结果
    /// （见 `LIGHT.status_bar_text` 的算式），不是抄错角色。
    pub status_bar_text: Color32,

    // ── 强调色族 ──
    /// 强调色（主按钮填充 / 选中描边 / 图标）。
    pub accent: Color32,
    /// 强调色的深一档（1px 边框、页签下划线、状态栏强调段）。
    pub accent_dark: Color32,
    /// 选中行/选中按钮的浅强调填充。
    pub accent_soft: Color32,
    /// 主按钮文字色（在 `accent` 底上的字）。
    pub accent_button_text: Color32,
    /// 画布上**鼠标所在格**的描边色：深蓝 `#00008B` @ α 0.7（预乘落 `rgba_pm(0, 0, 97, 179)`）。
    ///
    /// **上游出处（逐字）**：`fluttergo/CrystalStudio@a78affdf` 的 `MapViewport.cs:144`
    /// `private static readonly ScenePen hoverCellOutline = new(new ImmutableSolidColorBrush(Colors.DarkBlue, .7), 4);`
    /// —— 颜色 `Colors.DarkBlue`（Avalonia/System.Drawing 常量 = `#00008B`，即 RGB(0,0,139)）、
    /// 不透明度 `.7`；同版 `:305` 用它描悬停格。
    /// ⚠ 本地 `_upstream` = `f84566bc` **快照**（`MapViewport.cs` 604 行），`DarkBlue` **0 命中**；
    /// 该符号是 `a78affdf` **引入**的（属"某提交引入"的历史事实，故此处保留 `@<sha>`；
    /// 行号位置一律不声称对应某个提交）。该版 616 行，取证说明同 `metrics::CELL_HOVER_STROKE`。
    /// ⚠ `:144` / `:305` 都是 `a78affdf` **版内行号**（本地 `_upstream` 快照无 `hoverCellOutline` 符号）。**可复核**：`a78affdf` 全量副本已落地本地，路径 `I:\cargo\tmp\cs-master\CrystalStudio-a78affdfd8f912d10a8bce940b54ff7a836cbbfc\CrystalStudio\MapViewport.cs`（616 行 / 40,168 B）。**订正（2026-11-08）**：原写「离线不可复核」，该表述已过期（`a78affdf` 全量副本已落地本地），已订正为「可复核」并给出路径；本地 `f84566bc` 的
    /// `MapViewport.cs:144` 是 `public MapViewport()` 构造函数，**不是**这个符号。
    ///
    /// ⚠ **两套主题同值，因为上游是硬编码常量、不是主题刷**：
    /// 上游这一行**不经** `ThemeService.Brush(...)` —— 对比同文件 `:143`
    /// `private static ScenePen cursor => new(ThemeService.Brush("Accent"), 1.5);`（那才是主题刷），
    /// 以及 `App.axaml` 里 `ThemeXXXXXX` 那些键。`ImmutableSolidColorBrush(Colors.DarkBlue, .7)`
    /// 是**写死的** ⇒ 深浅两套必须逐字节相同，已登记进 `same_ok`，并由
    /// [`主题无关字段取官方常量`] 按官方 straight-alpha 逐通道钉住（防"两套都抄错但抄成一样"）。
    ///
    /// **字段名**：原名 `cell_hover_gold` —— 真值是深蓝，「gold」已不成立（2026-11-08 改名）。
    /// 新名与上游标识符 `hoverCellOutline` 一一对应，且避免与 `metrics::CELL_HOVER_STROKE`
    /// （线宽常量）只差大小写、让 `grep` 互相污染。
    pub cell_hover_outline: Color32,
    /// **悬停地砖信息浮层**里「不可行走」那行的字色（红）。
    ///
    /// ⚠ 官方 `_upstream/CrystalStudio/MainWindow.CellHover.cs:61`
    /// `CellHoverWalkability.Foreground = blocked ? Brushes.Red : ThemeService.Brush("ThemeE5E9F0")`：
    /// 「可行走」用主题色 `ThemeE5E9F0`（= 本板的 `text`），**「不可行走」直接写
    /// `Brushes.Red`、不经 `ThemeService`** ⇒ 主题无关常量，两套板同值（已入 `same_ok`），
    /// 官方 RGB 由 [`主题无关字段取官方常量`] 逐通道钉住。
    pub hover_blocked_text: Color32,
    /// 画布上**单元格网格线**的颜色（视图 → 单元格网格，1px 世界格边界线）。
    ///
    /// ⚠ 它压在**地图位图**上，不是压在面板底色上。官方为此**专门**定义了 `CanvasGrid`
    /// （`_upstream/CrystalStudio/ThemeService.Avalonia.cs:55`，半透明刷、**不是任何控件的边框色**），
    /// 消费点 `_upstream/CrystalStudio/MapViewport.Presentation.cs:57`
    /// `new ScenePen(ThemeService.Brush("CanvasGrid"), 1 / Zoom)` —— **笔宽 = `1/Zoom`**
    /// （世界单位，屏上恒 1px）。旧注释引的「`_upstream/CrystalStudio/MapViewport.cs:137` 固定 0.6」那行已被
    /// 上游 `901dbe2f` 删除，别再照它算。
    /// 两套板都是**预乘**半透明值（见各板里的换算算式）。
    pub grid_line: Color32,

    // ── 分隔线 / 控件 ──
    /// 全局分隔线（菜单栏下沿、状态栏上沿）。
    pub separator: Color32,
    /// 中列与两侧停靠区之间的竖向分隔线。
    pub dock_divider: Color32,
    /// 输入控件（下拉框）底色。
    pub input_bg: Color32,
    /// 搜索框底色。
    pub input_search_bg: Color32,
    /// 输入控件边框。
    pub input_border: Color32,
    /// 复选框边框。
    pub checkbox_border: Color32,
    /// 滚动条滑块色。
    pub scrollbar: Color32,
    /// Tool Options 面板的色块（当前画笔颜色）。
    pub color_swatch: Color32,
    /// 层列表里 `低位阻挡` 的标记块 / 拒绝回执的警示色。
    ///
    /// ⚠ 官方 `_upstream/CrystalStudio/CollisionMarkers.cs:6-7` 是 `static readonly`、**不经 `ThemeService`**
    /// ⇒ **主题无关常量**，两套板同值（已入 `tests::两套色板逐字段不同` 的 `same_ok`）。
    pub collision_low_mark: Color32,
    /// 层列表里 `高位阻挡` 的标记块（同上，官方主题无关常量）。
    pub collision_high_mark: Color32,
    /// 状态栏 `● 资源库 N 个` 的圆点色。
    pub status_dot: Color32,
    /// 空态图标颜色。
    pub empty_icon: Color32,
    /// 空态说明文字色（与 `text_muted` 同值但语义不同）。
    pub empty_text: Color32,

    // ── 常驻告警黄条（宿主新增，无官方截图依据） ──
    /// 告警黄条底色。
    pub warn_banner_bg: Color32,
    /// 告警黄条的边框 / 左侧竖条色。
    pub warn_banner_edge: Color32,
    /// 告警黄条正文色。
    pub warn_banner_text: Color32,

    // ── 印章 / 画布叠加层 ──
    /// 印章格子里「缺帧」占位的底色。
    pub stamp_missing_bg: Color32,
    /// 矩形选区的**虚线边框**色。
    pub selection_stroke: Color32,
    /// 矩形选区的**半透明填充**色（预乘）。
    ///
    /// ⚠ 官方 `_upstream/CrystalStudio/MapViewport.cs:303` 是写死的 `Color.FromArgb(30, 110, 150, 255)`、**不经 `ThemeService`**
    /// ⇒ **主题无关常量**，两套板同值（已入 `same_ok`）。
    pub selection_fill: Color32,
    /// 印章**待放置轮廓框**的线色。
    pub stamp_preview_stroke: Color32,
    /// 落位预览里**已有物件**的受阻格诊断色（上游 `Color.FromRgb(247,66,85)` = `#F74255`）。
    ///
    /// 两套主题**同值**：这是压在**地图像素**上的诊断色，与界面主题无关
    /// （同 `modal_scrim` 的理由）；实际绘制时填充 α95、描边不透明（上游 `:102-103`）。
    pub stamp_blocked: Color32,
    /// 落位预览里**落到地图外**的受阻格诊断色（上游 `Color.FromRgb(245,174,52)` = `#F5AE32`）。
    pub stamp_boundary: Color32,

    // ── 模态 / 弹层 ──
    /// 关闭拦截模态 / 打开地图模态的遮罩色（预乘半透明黑）。
    pub modal_scrim: Color32,
    /// 菜单下拉弹层的底色。
    pub menu_popup_bg: Color32,
    /// 菜单下拉弹层的 1px 边框色。
    pub menu_popup_border: Color32,
    /// 菜单项 hover / 键盘选中项的底色。
    pub menu_popup_hover: Color32,
}

/// **浅色板** —— 不透明值取自运行期截图直方图（见各字段的 `LIGHT:` 注释）；
/// 官方有**写死常量**的几项（网格线 / 两个碰撞标记 / 选区填充）改取官方常量并标出处。
pub const LIGHT: Palette = Palette {
    // 标题栏底色。截图 (300,6)-(1200,26) 区域直方图：`#604715` 占 100.0%。
    title_bar: Color32::from_rgb(0x60, 0x47, 0x15),
    // 标题栏文字。标题栏文字 (28,6)-(420,26) 区域最亮像素 = `#FFFFFF`。
    title_bar_text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    // 菜单栏底色。截图 (300,36)-(1100,60) 区域直方图：`#F5F6F7` 占 100.0%。
    menu_bar: Color32::from_rgb(0xF5, 0xF6, 0xF7),
    // 菜单栏文字。菜单栏文字 (20,38)-(240,60) 区域暗像素众数 = `#000000`。
    menu_bar_text: Color32::from_rgb(0x00, 0x00, 0x00),

    // 面板体。截图 (20,600)-(150,760) 区域直方图：`#F5F6F7` 占 84.1%。
    panel_bg: Color32::from_rgb(0xF5, 0xF6, 0xF7),
    // 面板标题条。截图 (60,70)-(150,86) 区域直方图：`#E3E6E9` 占 100.0%。
    panel_header: Color32::from_rgb(0xE3, 0xE6, 0xE9),
    // 视口留白。截图 (170,110)-(220,170) 区域直方图：`#F9FAFB` 占 100.0%。
    view_surround: Color32::from_rgb(0xF9, 0xFA, 0xFB),
    // 画布底层。截图 (300,200)-(600,400) 区域直方图：`#EFF2F5` 占 89.3%。
    canvas_bg: Color32::from_rgb(0xEF, 0xF2, 0xF5),
    // 状态栏。截图 (200,916)-(1100,936) 区域直方图：`#F7F8F9` 占 100.0%。
    //   官方角色：`_upstream/CrystalStudio/MainWindow.axaml:84` `<Border Grid.Row="4" Background="{DynamicResource Theme171B22}"
    //   BorderBrush="{DynamicResource Line}" BorderThickness="0,1,0,0">` —— `Grid.Row="4"` 就是**状态栏**
    //   （同一行里还有 `Status` / `Coordinates` / `LibraryDot` 三个状态控件）；
    //   键定义 `_upstream/CrystalStudio/App.axaml:5` `Theme171B22` = `#171B22`。
    //   浅色值按 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:47` 现算（非蓝键 B−R=11≤35、lum<0.4 那一支）：
    //     lum    = (23×.2126 + 27×.7152 + 34×.0722)/255 = 26.655/255 = 0.104529
    //     amount = clamp(lum×1.4 − 0.07, 0, 0.65) = clamp(0.146341 − 0.07) = 0.076341
    //     Blend(White, #A3ADB9, amount) = 255 − (255−通道)×amount
    //       R: 255 − 92×0.076341 = 247.977 → 247 = 0xF7
    //       G: 255 − 82×0.076341 = 248.740 → 248 = 0xF8
    //       B: 255 − 70×0.076341 = 249.656 → 249 = 0xF9   ⇒ **#F7F8F9**
    //   ⇒ 截图直方图与官方公式**逐字节相同**：源键无疑（同款手算已入用例 `官方浅色变换可复算`）。
    status_bar: Color32::from_rgb(0xF7, 0xF8, 0xF9),

    // 标签条容器底。官方: `_upstream/CrystalStudio/MainWindow.MapTabs.cs:36` 的 `Theme1C2028`（`_upstream/CrystalStudio/App.axaml:8` = `#1C2028`），
    //   浅色按 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:47` 现算（**非蓝键** B−R = 40−28 = 12 ≤ 35、lum < 0.4 那一支）：
    //     lum    = (28×.2126 + 32×.7152 + 40×.0722)/255 = 31.7272/255 = 0.124420
    //     amount = clamp(lum×1.4 − 0.07, 0, 0.65) = clamp(0.174188 − 0.07) = 0.104188
    //     Blend(White, #A3ADB9, amount) = 255 − (255−通道)×amount
    //       R: 255 − 92×0.104188 = 245.414 → 245 = 0xF5
    //       G: 255 − 82×0.104188 = 246.457 → 246 = 0xF6
    //       B: 255 − 70×0.104188 = 247.707 → 247 = 0xF7   ⇒ **#F5F6F7**
    //   旁证（闭环）：参考截图页签条 (700, 66..103) 直方图 = `#F5F6F7`（见 `metrics::TABSTRIP_H` 的来源）。
    //   ⚠ 与 `LIGHT.menu_bar` 同值 —— 那是**官方变换的结论**（两个键在深色同值），不是抄错角色。
    tab_strip_bg: Color32::from_rgb(0xF5, 0xF6, 0xF7),
    // 标签悬停底。官方: `_upstream/CrystalStudio/EditorControls.axaml:63` 的 `Theme29313E`（`_upstream/CrystalStudio/App.axaml:14` = `#29313E`），
    //   浅色按 `:47` 现算（**非蓝键** B−R = 62−41 = 21 ≤ 35、lum < 0.4 那一支）：
    //     lum    = (41×.2126 + 49×.7152 + 62×.0722)/255 = 48.2378/255 = 0.189168
    //     amount = clamp(0.264835 − 0.07) = 0.194835
    //       R: 255 − 92×0.194835 = 237.075 → 237 = 0xED
    //       G: 255 − 82×0.194835 = 239.024 → 239 = 0xEF
    //       B: 255 − 70×0.194835 = 241.362 → 241 = 0xF1   ⇒ **#EDEFF1**
    tab_hover_bg: Color32::from_rgb(0xED, 0xEF, 0xF1),
    // 选中标签下划线。官方: `_upstream/CrystalStudio/EditorControls.axaml:64` 的 `Theme8CA1FF`（`_upstream/CrystalStudio/App.axaml:47` = `#8CA1FF`），
    //   浅色按 `:43`/`:46` 现算（**蓝键** B−R = 255−140 = 115 > 35、B−G = 255−161 = 94 > 12；
    //   lum = (140×.2126 + 161×.7152 + 255×.0722)/255 = 163.322/255 = 0.640478 ≥ 0.4）
    //   ⇒ `:46` 后半支 `Blend(accent, dark ? White : Black, dark ? 0.3 : 0.15)`
    //   ⇒ 浅色 = **accent × 0.85**（本仓浅色 accent = 截图实测 `#CC9629`，见 `LIGHT.accent`）：
    //     R: 204×0.85 = 173.4 → 173 = 0xAD
    //     G: 150×0.85 = 127.5 → 127 = 0x7F
    //     B:  41×0.85 =  34.85 → 34 = 0x22   ⇒ **#AD7F22**
    //   旁证（闭环）：参考截图页签下划线 y=103..104 直方图 = `#AD7F22`（`metrics::TAB_W` 的来源），
    //   与本字段**逐字节相同** ⇒ 源键无疑（同 `LIGHT.accent_dark` 的算式，两者同值、语义不同）。
    tab_selected_underline: Color32::from_rgb(0xAD, 0x7F, 0x22),
    // 关闭符悬停底。官方: `_upstream/CrystalStudio/EditorControls.axaml:17` `QuietButton` 继承基模板的
    //   `:pointerover /template/ Border#Chrome` 底（`_upstream/CrystalStudio/EditorControls.axaml:12`）= `Theme394354`
    //   （`_upstream/CrystalStudio/App.axaml:26` = `#394354`），浅色按 `:47` 现算（**非蓝键** B−R = 84−57 = 27 ≤ 35、
    //   lum < 0.4 那一支）：
    //     lum    = (57×.2126 + 67×.7152 + 84×.0722)/255 = 66.1014/255 = 0.259221
    //     amount = clamp(0.362910 − 0.07) = 0.292910
    //       R: 255 − 92×0.292910 = 228.052 → 228 = 0xE4
    //       G: 255 − 82×0.292910 = 230.981 → 230 = 0xE6
    //       B: 255 − 70×0.292910 = 234.496 → 234 = 0xEA   ⇒ **#E4E6EA**
    close_hover_bg: Color32::from_rgb(0xE4, 0xE6, 0xEA),

    // 正文。左面板正文 (94,596)、层列表 (52,353)、状态栏 (104,924)、面板标题 (28,70)-(120,86) 均 `#434D5A`。
    text: Color32::from_rgb(0x43, 0x4D, 0x5A),
    // 次要文字。搜索框占位符最暗像素 (1158,406) = `#9EA4AB`。
    text_muted: Color32::from_rgb(0x9E, 0xA4, 0xAB),
    // 状态栏正文。官方: `_upstream/CrystalStudio/MainWindow.axaml:84` 的 `Status` TextBlock
    //       `Foreground="{DynamicResource ThemeADB8C9}"`，键定义 `_upstream/CrystalStudio/App.axaml:55` = `#ADB8C9`。
    //       浅色按 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:43-47` 现算（**非蓝键、lum ≥ 0.4 那一支**）：
    //         blue   = B−R = 201−173 = 28 ≤ 35 ⇒ 不是蓝键（不走 `:46`）
    //         lum    = (173×.2126 + 184×.7152 + 201×.0722)/255 = 182.889/255 = 0.71721 ≥ 0.4
    //         ⇒ `:47` 的后半支是**常量** `Color.FromRgb(67, 77, 90)` = **#434D5A**
    //           （不是 `Blend(White, #A3ADB9, …)` 那一支 —— 那一支只在 lum < 0.4 时用）
    //       旁证：截图状态栏正文 (104,924) 取样 = `#434D5A` ⇒ 与 `LIGHT.text` **同值**
    //       （同值是官方变换的结论；深色侧两者不同，见 `DARK.status_bar_text`）。
    status_bar_text: Color32::from_rgb(0x43, 0x4D, 0x5A),

    // 强调金。`Set Library Folder` 按钮 (1230,610)-(1390,625) 直方图：`#CC9629` 占 79.8%。
    accent: Color32::from_rgb(0xCC, 0x96, 0x29),
    // 强调金深一档。金按钮左右边框 (1218/1408,607..628)、选中工具描边、页签下划线均 `#AD7F22`。
    accent_dark: Color32::from_rgb(0xAD, 0x7F, 0x22),
    // 选中浅金。层列表选中行 (60,350)-(140,362) 直方图 `#F7EFDE` 占 78.8%。
    accent_soft: Color32::from_rgb(0xF7, 0xEF, 0xDE),
    // 金按钮文字。`Set Library Folder` 文字区域最暗像素 = `#000000`。
    accent_button_text: Color32::from_rgb(0x00, 0x00, 0x00),
    // 悬停格描边。官方: `a78affdf` 引入的 `MapViewport.cs` `hoverCellOutline`（本地 `_upstream` 快照无此符号）
    //       `new ImmutableSolidColorBrush(Colors.DarkBlue, .7)`（**写死常量**，不经 ThemeService）。
    //       `Colors.DarkBlue` = `#00008B`（RGB 0,0,139）、α = 0.7。
    //       预乘换算（**截断**口径，与本文件 `grid_line`/`selection_fill` 一致）：
    //         α = 0.7×255 = 178.5 → **179**
    //         R = 0×179/255 = 0、G = 0×179/255 = 0
    //         B = 139×179/255 = 24881/255 = 97.57 → **97**（截断）
    //       ⇒ `rgba_pm(0, 0, 97, 179)`。
    //       ⚠ 若改用 `Color32::from_rgba_unmultiplied(0,0,139,179)` 得 **98**
    //         （ecolor 0.33.3 的 `from_rgba_unmultiplied` 走 `fast_round` = 四舍五入），
    //         差 1/255；本文件半透明色统一走 `rgba_pm` + 截断口径 ⇒ 取 97，
    //         且与守卫用例里独立实现的 `pm`（`c*a/255` 整除）逐字节一致。
    //       ⚠ 历史引用：本字段旧值 `#D2AA59`（截图 (607..896,813..911) 金色像素众数，
    //         占该掩码 82%）——那是**截图采样**，与源码真值冲突时源码优先，已订正。
    cell_hover_outline: rgba_pm(0, 0, 97, 179),
    // 悬停浮层「不可行走」字色。官方: _upstream/CrystalStudio/MainWindow.CellHover.cs:61 `Brushes.Red`（写死常量、
    //       不经 ThemeService）⇒ 两套板同值；α=255 ⇒ 预乘与原值相同。
    hover_blocked_text: rgba_pm(255, 0, 0, 255),
    // 网格线。官方: _upstream/CrystalStudio/ThemeService.Avalonia.cs:55 `resources["CanvasGrid"] = … : Color.FromArgb(60, 73, 89, 108)`
    //       ⇒ 浅色是 **24% 半透明深灰蓝**（官方**专用**的网格键，不是任何控件的边框色；
    //       旧值 `#D3D8DD` 其实是复选框边框 `Theme586477`（_upstream/CrystalStudio/App.axaml:36）的官方浅色变换 —— 抄错了角色）。
    //       消费点 _upstream/CrystalStudio/MapViewport.Presentation.cs:57
    //       `new ScenePen(ThemeService.Brush("CanvasGrid"), 1 / Zoom)` ⇒ **笔宽 = 1/Zoom**
    //       （世界单位；屏上恒 1px）。旧注释引的「`_upstream/CrystalStudio/MapViewport.cs:137` 固定 0.6」已被上游
    //       `901dbe2f` 删除 —— 颜色键没变，只是笔宽换了口径，别再照 0.6 算。
    //       预乘换算（**截断**口径，与 `rgba_pm` 一致）：73×60/255=17.2→17、89×60/255=20.9→20、108×60/255=25.4→25。
    grid_line: rgba_pm(17, 20, 25, 60),

    // 分隔线。菜单栏下沿 y=66、状态栏上沿 y=912 均为 `#EAECEF`。
    separator: Color32::from_rgb(0xEA, 0xEC, 0xEF),
    // 停靠区竖线。截图 x=224 与 x=1135 均为 `#E7EBEF`。
    dock_divider: Color32::from_rgb(0xE7, 0xEB, 0xEF),
    // 下拉框底。截图 (1160,356)-(1460,378) 直方图：`#EFF1F3` 占 100.0%。
    input_bg: Color32::from_rgb(0xEF, 0xF1, 0xF3),
    // 搜索框底。截图 (1160,400)-(1460,418) 直方图：`#F9FAFA` 占 89.5%。
    input_search_bg: Color32::from_rgb(0xF9, 0xFA, 0xFA),
    // 输入框边框。截图 x=1146 竖线：`#616262` 占 100.0%。
    input_border: Color32::from_rgb(0x61, 0x62, 0x62),
    // 复选框边框。`Light Cells` 复选框 (23,500)-(38,516)：`#D3D8DD` 占 11.8%。
    checkbox_border: Color32::from_rgb(0xD3, 0xD8, 0xDD),
    // 滚动条滑块。左停靠区 (157,380)-(161,500)：`#7A7A7A` 占 40.0%。
    scrollbar: Color32::from_rgb(0x7A, 0x7A, 0x7A),
    // 当前画笔色块。截图 (17,270)-(28,280)：`#AD7F22` 占 63.6%。
    color_swatch: Color32::from_rgb(0xAD, 0x7F, 0x22),
    // 低位阻挡标记。官方: _upstream/CrystalStudio/CollisionMarkers.cs:6 `Color.FromArgb(90, 240, 85, 105)`（红、**35% 半透明**）。
    //       截图 (44,428)-(56,442) 的 `#F05569` = 官方 RGB **逐字节相同** ⇒ 源键无疑，只是**漏了 α90**
    //       （截图直方图取的是众数，把 α 抹成了不透明）。官方不经 `ThemeService` ⇒ 与 DARK 同值。
    //       预乘（截断）: 240×90/255=84.7→84、85×90/255=30.0→30、105×90/255=37.1→37。
    collision_low_mark: rgba_pm(84, 30, 37, 90),
    // 高位阻挡标记。官方: _upstream/CrystalStudio/CollisionMarkers.cs:7 `Color.FromArgb(90, 65, 165, 245)`（蓝、35% 半透明）。
    //       截图 (44,454)-(56,468) 的 `#41A5F5` = 官方 RGB 同值。预乘（截断）:
    //       65×90/255=22.9→22、165×90/255=58.2→58、245×90/255=86.5→86。
    collision_high_mark: rgba_pm(22, 58, 86, 90),
    // 库状态圆点。截图 (1375..1380,918..930) 非底色众数 `#434D5A`（与正文同色）。
    status_dot: Color32::from_rgb(0x43, 0x4D, 0x5A),
    // 空态图标。截图 (1308,488)-(1330,508)：`#AD7F22` 占 15.7%（空态图标是金色）。
    empty_icon: Color32::from_rgb(0xAD, 0x7F, 0x22),
    // 空态文字（与 `text_muted` 同值、语义不同）。
    empty_text: Color32::from_rgb(0x9E, 0xA4, 0xAB),

    // 告警黄条（**宿主新增，无截图依据**）：底色取 `ACCENT_SOFT` 同族的浅琥珀。
    warn_banner_bg: Color32::from_rgb(0xFF, 0xF6, 0xE0),
    // 黄条边框/竖条 = `accent` 同族。
    warn_banner_edge: Color32::from_rgb(0xCC, 0x96, 0x29),
    // 黄条正文：`#7A4E00` 在 `#FFF6E0` 上对比度 ≈ 8.6:1（WCAG AAA 正文档）。
    warn_banner_text: Color32::from_rgb(0x7A, 0x4E, 0x00),

    // 缺帧占位底 = 面板标题条的同一个实测值（语义不同，单独命名）。
    stamp_missing_bg: Color32::from_rgb(0xE3, 0xE6, 0xE9),
    // 选区虚线 = 页签下划线的实测值 `#AD7F22`。
    selection_stroke: Color32::from_rgb(0xAD, 0x7F, 0x22),
    // 选区填充。官方: _upstream/CrystalStudio/MapViewport.cs:303 `new ImmutableSolidColorBrush(Color.FromArgb(30, 110, 150, 255))`
    //       —— 写死的固定蓝、**不经 `ThemeService`** ⇒ 主题无关常量，与 DARK 同值
    //       （旧值 `rgba_pm(32,23,6,40)` 是自造的"accent 金 × α40"，与官方既不同色也不同 α30；
    //        报告 D5 只点了深色，浅色侧按同一判据补齐）。
    //       预乘（截断）: 110×30/255=12.9→12、150×30/255=17.6→17、255×30/255=30.0→30。
    selection_fill: rgba_pm(12, 17, 30, 30),
    // 印章轮廓 = `accent` 实测值 `#CC9629`（不透明实线）。
    stamp_preview_stroke: Color32::from_rgb(0xCC, 0x96, 0x29),
    // 落位诊断两色与主题无关（官方硬编码，压在游戏画面之上）
    stamp_blocked: Color32::from_rgb(0xF7, 0x42, 0x55),
    stamp_boundary: Color32::from_rgb(0xF5, 0xAE, 0x32),

    // 模态遮罩（**推导**，参考截图无此控件）：35% 黑。
    modal_scrim: rgba_pm(0, 0, 0, 90),
    // 菜单弹层底色：取浅色视口留白 `#F9FAFB`（比面板体亮一档，弹层才"浮"起来）。
    menu_popup_bg: Color32::from_rgb(0xF9, 0xFA, 0xFB),
    // 菜单弹层边框：取复选框边框实测值 `#D3D8DD`。
    menu_popup_border: Color32::from_rgb(0xD3, 0xD8, 0xDD),
    // 菜单项 hover / 键盘选中：取选中浅金 `#F7EFDE`（与层列表选中行同一档）。
    menu_popup_hover: Color32::from_rgb(0xF7, 0xEF, 0xDE),
};

/// **深色板** —— 每个值都抄自官方深色资源（`App.axaml` 的 `ThemeXXXXXX` 键 / 命名刷，
/// 或 `ThemeService.Avalonia.cs` 里写死的那几个），见各字段的 `DARK:` 注释。
pub const DARK: Palette = Palette {
    // 官方: **没有"标题栏"这个角色** —— CrystalStudio 全仓 grep `ExtendClientArea` / `SystemDecorations`
    //       零命中 ⇒ 窗口用的是 OS 原生标题栏（DWM 画），`App.axaml` 里自然没有标题栏键。
    //       本仓自绘标题栏是 `with_decorations(false)` 的副产物（`window.rs:27-28`）⇒ 这一档**必须映射**。
    //       深色取官方**窗口外框档** `Theme14171D`：`_upstream/CrystalStudio/App.axaml:83` `Selector="Window"` 的
    //       `Background="{DynamicResource Theme14171D}"`（窗口自身底色；同一个键又被
    //       `_upstream/CrystalStudio/ThemeService.Avalonia.cs:53` 用作 `CanvasBackground`，即本文件 `view_surround`）
    //       ⇒ 深色下 `title_bar` 与 `view_surround` **同键同值**，不是漏抄。
    //       浅色侧**不是**这个键的变换：`#604715` 是截图里 DWM 按系统强调色画的标题栏
    //       （(300,6)-(1200,26) 直方图 100%），官方应用资源里没有金色标题栏 ⇒ 两套本来就不对称。
    //       ⚠ 旧值 `#171B22` 是**状态栏**色（`_upstream/CrystalStudio/MainWindow.axaml:84`）⇒ 借错了角色，本次归位。
    //       旁证（浅色闭环）：`Theme14171D` 按 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:47` 变换 = `#F9FAFB`
    //       = 本文件 `LIGHT.view_surround`（算式同 `LIGHT.status_bar`，已入 `官方浅色变换可复算`）。
    //
    // ⚠ 本节的不透明色一律写 `Color32::from_rgb(r, g, b)` 三通道，**不写 `0xRRGG_BB` 那种分组**
    //   （半透明色写 `rgba_pm(...)`：`grid_line` / 两个碰撞标记 / `selection_fill`）：
    //   ① `clippy::unusual_byte_groupings` 会对"分组长度不等"（4+2）逐条报警，而本单的门是 NEW=0；
    //   ② `0x2026_32` 这种还会被当成**字面量后缀**（`_32` = i32）直接编译失败 —— 实测踩过。
    title_bar: Color32::from_rgb(0x14, 0x17, 0x1D),
    // 官方: _upstream/CrystalStudio/App.axaml:83 `Selector="Window"` 的 `Foreground` = `ThemeE5E9F0`。
    title_bar_text: Color32::from_rgb(0xE5, 0xE9, 0xF0),
    // 官方: _upstream/CrystalStudio/ThemeService.Avalonia.cs:50 `resources["Panel"] = Brush("Theme1C2028")`；
    //       同值见 _upstream/CrystalStudio/App.axaml:70 命名刷 `Panel`、_upstream/CrystalStudio/MainWindow.axaml:5 顶部条。
    menu_bar: Color32::from_rgb(0x1C, 0x20, 0x28),
    // 官方: _upstream/CrystalStudio/App.axaml:83 Window Foreground。
    menu_bar_text: Color32::from_rgb(0xE5, 0xE9, 0xF0),

    // 官方: 同 `menu_bar`（`Panel`）。
    panel_bg: Color32::from_rgb(0x1C, 0x20, 0x28),
    // 官方: _upstream/CrystalStudio/App.axaml:9 `Theme202632` = `#202632`（_upstream/CrystalStudio/EditorControls.axaml:13 按下态底色）。
    // ⚠ 映射是我选的：标题条要比面板体**可辨**（浅色里 `#E3E6E9` 比 `#F5F6F7` 暗一档；
    //   官方深色里 `Panel` 更暗 ⇒ 对应的是比它**亮一档**的 `#202632`）。
    panel_header: Color32::from_rgb(0x20, 0x26, 0x32),
    // 官方: _upstream/CrystalStudio/ThemeService.Avalonia.cs:53 `resources["CanvasBackground"] = Brush("Theme14171D")`。
    view_surround: Color32::from_rgb(0x14, 0x17, 0x1D),
    // 官方: _upstream/CrystalStudio/ThemeService.Avalonia.cs:54
    //       `resources["CanvasPaper"] = dark ? Color.FromRgb(31, 37, 43) : Color.FromRgb(239, 242, 245)`
    //       ⇒ 深色 = (31,37,43) = `#1F252B`（浅色那半正是本文件 `LIGHT.canvas_bg` 的 `#EFF2F5`，闭环可验）。
    canvas_bg: Color32::from_rgb(0x1F, 0x25, 0x2B),
    // 官方: _upstream/CrystalStudio/MainWindow.axaml:84 `<Border Grid.Row="4" Background="{DynamicResource Theme171B22}"
    //       BorderBrush="{DynamicResource Line}" BorderThickness="0,1,0,0">` —— `Grid.Row="4"` = **状态栏**
    //       （同行还有 `Status` / `Coordinates` / `LibraryDot` 三个状态控件）；键定义 `_upstream/CrystalStudio/App.axaml:5`。
    //       ⇒ 深色 = `#171B22`，**比 `Panel`（`#1C2028`）暗一档**。
    //       旧值 `#1C2028` 抄的是 `Panel`（面板体 / 工具栏 / 菜单条，`_upstream/CrystalStudio/App.axaml:8` +
    //       `_upstream/CrystalStudio/ThemeService.Avalonia.cs:50` `resources["Panel"] = Brush("Theme1C2028")`，
    //       消费点 `_upstream/CrystalStudio/MainWindow.axaml:5/6/8/27`）⇒ 深色下状态栏与面板同色，丢掉官方那条更暗的横带。
    //       浅色侧按 `:47` 现算 = `#F7F8F9`，与截图直方图逐字节相同（算式见 `LIGHT.status_bar`）。
    status_bar: Color32::from_rgb(0x17, 0x1B, 0x22),

    // 官方: `_upstream/CrystalStudio/MainWindow.MapTabs.cs:36` `mapTabs.SetResource(TemplatedControl.BackgroundProperty, "Theme1C2028")`；
    //       键定义 `_upstream/CrystalStudio/App.axaml:8` `Theme1C2028` = `#1C2028`（同 `Panel`，`_upstream/CrystalStudio/ThemeService.Avalonia.cs:50`）。
    //       浅色侧按 `:47` 现算 = `#F5F6F7`，与参考截图页签条直方图逐字节相同（算式见 `LIGHT.tab_strip_bg`）。
    tab_strip_bg: Color32::from_rgb(0x1C, 0x20, 0x28),
    // 官方: `_upstream/CrystalStudio/EditorControls.axaml:63` `^:pointerover /template/ Border#Chrome` 的 `Background` = `Theme29313E`；
    //       键定义 `_upstream/CrystalStudio/App.axaml:14` = `#29313E`。
    tab_hover_bg: Color32::from_rgb(0x29, 0x31, 0x3E),
    // 官方: `_upstream/CrystalStudio/EditorControls.axaml:64` `^:selected /template/ Border#Chrome` 的 `BorderBrush` = `Theme8CA1FF`；
    //       键定义 `_upstream/CrystalStudio/App.axaml:47` = `#8CA1FF`。
    //       ⚠ 别改成 `accent_dark`（= `#8498FF`）：两者在深色下**不同色**，见字段文档。
    tab_selected_underline: Color32::from_rgb(0x8C, 0xA1, 0xFF),
    // 官方: `_upstream/CrystalStudio/EditorControls.axaml:12` `^:pointerover /template/ Border#Chrome` 的
    //       `Background` = `Theme394354`；键定义 `_upstream/CrystalStudio/App.axaml:26` = `#394354`。
    //       `QuietButton`（`:17`）只把常态 `Background` 覆盖成 `Transparent`，
    //       `:pointerover` 那条**没被覆盖** ⇒ 关闭符悬停时就是它。
    //       浅色侧按 `:47` 现算 = `#E4E6EA`，与参考截图无关（截图里没有悬停态）。
    close_hover_bg: Color32::from_rgb(0x39, 0x43, 0x54),

    // 官方: _upstream/CrystalStudio/App.axaml:83 Window Foreground `#E5E9F0`。
    text: Color32::from_rgb(0xE5, 0xE9, 0xF0),
    // 官方: _upstream/CrystalStudio/ThemeService.Avalonia.cs:50 `resources["Muted"] = Brush("Theme929CAB")`。
    text_muted: Color32::from_rgb(0x92, 0x9C, 0xAB),
    // 官方: _upstream/CrystalStudio/App.axaml:55 `ThemeADB8C9` = `#ADB8C9`；消费点 `_upstream/CrystalStudio/MainWindow.axaml:84`
    //       `Status` TextBlock 的 `Foreground="{DynamicResource ThemeADB8C9}"`。
    //       ⚠ 状态栏那一行有**三个文字角色**，别合并：正文 = 本键（`#ADB8C9`）、
    //       `Coordinates` 与 `LibraryStatus` = `Muted`（`#929CAB`，见 `text_muted`）、
    //       圆点 = `ThemeD1AD77`（见 `status_dot`）。
    //       旧值借的是 `text`（`#E5E9F0`）⇒ 深色下状态栏正文比官方**亮一档**（本单订正）；
    //       浅色侧没有视觉变化（`:47` 的常量支恰好也是 `#434D5A`）。
    status_bar_text: Color32::from_rgb(0xAD, 0xB8, 0xC9),

    // 官方: _upstream/CrystalStudio/App.axaml:70 命名刷 `Accent` = `#8498FF`（_upstream/CrystalStudio/ThemeService.Avalonia.cs:32 的默认 accent
    //       也是 `Color.FromRgb(132,152,255)`）。
    accent: Color32::from_rgb(0x84, 0x98, 0xFF),
    // 官方: _upstream/CrystalStudio/ThemeService.Avalonia.cs:51 `resources["Theme6378DF"] = new ImmutableSolidColorBrush(accent);`
    //       —— 这行在变换循环（`:39-49`）**之后**执行，连蓝键公式算出的 `#A8B6FF` 也一并覆盖
    //       ⇒ 深色下 `Theme6378DF` **恒等于 `accent`**（默认 `#8498FF`）。
    //       `_upstream/CrystalStudio/App.axaml:40` 的 `#6378DF` 只是 XAML **声明值**（上屏前就被覆盖）；旧值抄的正是这个声明值。
    accent_dark: Color32::from_rgb(0x84, 0x98, 0xFF),
    // 官方: 键 `Theme2B355A`（_upstream/CrystalStudio/App.axaml:18；_upstream/CrystalStudio/EditorControls.axaml:30 选中态 `Border#Chrome` 底）是**蓝键**：
    //       B-R=90-43=47>35、B-G=90-53=37>12（判据 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:43`），
    //       亮度 =(43×.2126+53×.7152+90×.0722)/255 = 0.210 < 0.4 ⇒ 走 `:46` 第一支
    //       `Blend(#191D23, accent, 0.28)` = (25×.72+132×.28, 29×.72+152×.28, 35×.72+255×.28)
    //       = (54.96, 63.44, 96.60) 截断 ⇒ `#363F60`。旧值抄的是 XAML 声明值 `#2B355A`（上屏前被重算）。
    accent_soft: Color32::from_rgb(0x36, 0x3F, 0x60),
    // 官方: _upstream/CrystalStudio/ThemeService.Avalonia.cs:52 `OnAccent` = 亮度 > 145 ? 黑 : 白。
    //       accent `#8498FF` 亮度 = 132×.2126 + 152×.7152 + 255×.0722 = 155.2 > 145 ⇒ 黑。
    accent_button_text: Color32::from_rgb(0x00, 0x00, 0x00),
    // 悬停格描边（**与浅色板同值**）：官方 `a78affdf` 引入的 `hoverCellOutline` 写死
    //       `new ImmutableSolidColorBrush(Colors.DarkBlue, .7)`，不经 ThemeService ⇒ 深色板不另算。
    //       预乘换算（**截断**口径）：α=0.7×255=178.5→179；R=G=0；
    //       B=139×179/255=97.57→97 ⇒ `rgba_pm(0, 0, 97, 179)`（算式与浅色板同一条，见上）。
    //       ⚠ 历史引用：深色板旧值 `#A8B6FF` 来自
    //         `_upstream/CrystalStudio/ThemeService.Avalonia.cs:46` 的 `Blend(accent, White, 0.30)` 公式 ——
    //         那是"强调色描边"角色（`ThemeXXXXXX` 类主题刷）的推导，**不是**悬停框的真值来源；
    //         悬停框上游是硬编码常量，故深浅同值。该推导已作废，保留仅为追溯。
    cell_hover_outline: rgba_pm(0, 0, 97, 179),
    // 同上：官方写死的 `Brushes.Red`，深色板同值。
    hover_blocked_text: rgba_pm(255, 0, 0, 255),
    // 官方: _upstream/CrystalStudio/ThemeService.Avalonia.cs:55 `resources["CanvasGrid"] = … : Color.FromArgb(38, 160, 177, 199)`
    //       ⇒ 深色是 **15% 半透明浅蓝灰**（官方**专用**的网格键，不是任何控件的边框色；
    //       旧值 `#353E4C` 是 `Theme353E4C`（_upstream/CrystalStudio/App.axaml:22）—— 那是 TextBox 的 BorderBrush，抄错了角色）。
    //       消费点 _upstream/CrystalStudio/MapViewport.Presentation.cs:57
    //       `new ScenePen(ThemeService.Brush("CanvasGrid"), 1 / Zoom)` ⇒ **笔宽 = 1/Zoom**
    //       （世界单位；屏上恒 1px）。旧注释引的「`_upstream/CrystalStudio/MapViewport.cs:137` 固定 0.6」已被上游
    //       `901dbe2f` 删除 —— 颜色键没变，只是笔宽换了口径，别再照 0.6 算。
    //       预乘换算（**截断**口径，与 `rgba_pm` 一致）：160×38/255=23.8→23、177×38/255=26.4→26、199×38/255=29.7→29。
    grid_line: rgba_pm(23, 26, 29, 38),

    // 官方: _upstream/CrystalStudio/ThemeService.Avalonia.cs:50 `resources["Line"] = Brush("Theme303640")`。
    separator: Color32::from_rgb(0x30, 0x36, 0x40),
    // 官方: _upstream/CrystalStudio/App.axaml:22 `Theme353E4C` = `#353E4C`（_upstream/CrystalStudio/App.axaml:84 TextBox 的 BorderBrush）。
    // ⚠ 映射是我选的：浅色里停靠区竖线 `#E7EBEF` 与全局分隔线 `#EAECEF` 是**两个不同的值**，
    //   深色也取两个相邻的官方值（Line / Theme353E4C），不合并成一个。
    dock_divider: Color32::from_rgb(0x35, 0x3E, 0x4C),
    // 官方: _upstream/CrystalStudio/App.axaml:13 `Theme252C38` = `#252C38`（_upstream/CrystalStudio/App.axaml:85 ComboBox 的 Background）。
    input_bg: Color32::from_rgb(0x25, 0x2C, 0x38),
    // 官方: _upstream/CrystalStudio/App.axaml:3 `Theme14181F` = `#14181F`（_upstream/CrystalStudio/App.axaml:84 TextBox 的 Background）。
    input_search_bg: Color32::from_rgb(0x14, 0x18, 0x1F),
    // 官方: _upstream/CrystalStudio/App.axaml:84 TextBox `BorderBrush` = `Theme353E4C`。
    input_border: Color32::from_rgb(0x35, 0x3E, 0x4C),
    // 官方: _upstream/CrystalStudio/EditorControls.axaml:37 复选框 `Border#Box` 的 `BorderBrush` = `Theme586477`。
    checkbox_border: Color32::from_rgb(0x58, 0x64, 0x77),
    // 官方: _upstream/CrystalStudio/EditorControls.axaml:94 滑杆轨道 = `Theme394455`。
    scrollbar: Color32::from_rgb(0x39, 0x44, 0x55),
    // 官方: 同 `accent`（`#8498FF`）。
    color_swatch: Color32::from_rgb(0x84, 0x98, 0xFF),
    // 官方: _upstream/CrystalStudio/CollisionMarkers.cs:6 `Color.FromArgb(90, 240, 85, 105)` —— `static readonly`、**不经 ThemeService**
    //       ⇒ 主题无关常量，与 LIGHT 同值（旧值 `#D69D92` 是自造的"警示色映射"，报告 D4 已否）。
    //       预乘（截断）: 240×90/255=84.7→84、85×90/255=30.0→30、105×90/255=37.1→37。
    collision_low_mark: rgba_pm(84, 30, 37, 90),
    // 官方: _upstream/CrystalStudio/CollisionMarkers.cs:7 `Color.FromArgb(90, 65, 165, 245)` —— 同上，主题无关常量，与 LIGHT 同值。
    //       预乘（截断）: 65×90/255=22.9→22、165×90/255=58.2→58、245×90/255=86.5→86。
    collision_high_mark: rgba_pm(22, 58, 86, 90),
    // 官方: _upstream/CrystalStudio/MainWindow.axaml:84 `<Ellipse x:Name="LibraryDot" … Fill="{DynamicResource ThemeD1AD77}"/>`
    //       （键定义 _upstream/CrystalStudio/App.axaml:65）⇒ 状态圆点是**金/琥珀档** `#D1AD77`。
    //       旧值 `#E5E9F0` 抄的是 Window Foreground 文字色 ⇒ 圆点与旁边的 `Muted` 文字同色、
    //       失去"状态指示"的视觉区分（浅色侧 `#434D5A` 正是 `ThemeD1AD77` 的官方浅色变换，闭环可验）。
    status_dot: Color32::from_rgb(0xD1, 0xAD, 0x77),
    // 官方: 同 `accent`。
    empty_icon: Color32::from_rgb(0x84, 0x98, 0xFF),
    // 官方: 同 `text_muted`（`Muted` `#929CAB`）。
    empty_text: Color32::from_rgb(0x92, 0x9C, 0xAB),

    // 官方: _upstream/CrystalStudio/App.axaml:9 `Theme202632`（深色里"比 Panel 亮一档"的底色）。
    warn_banner_bg: Color32::from_rgb(0x20, 0x26, 0x32),
    // 官方: _upstream/CrystalStudio/App.axaml:65 `ThemeD1AD77` = `#D1AD77`（深色板的金/琥珀档；同键的另一处用途见 `status_dot`）。
    warn_banner_edge: Color32::from_rgb(0xD1, 0xAD, 0x77),
    // 官方: _upstream/CrystalStudio/App.axaml:83 Window Foreground（`#E5E9F0` 在 `#202632` 上对比度 ≈ 11.6:1）。
    warn_banner_text: Color32::from_rgb(0xE5, 0xE9, 0xF0),

    // 官方: 同 `panel_header`（`#202632`）。
    stamp_missing_bg: Color32::from_rgb(0x20, 0x26, 0x32),
    // 官方: 同 `accent`。
    selection_stroke: Color32::from_rgb(0x84, 0x98, 0xFF),
    // 官方: _upstream/CrystalStudio/MapViewport.cs:303 `new ImmutableSolidColorBrush(Color.FromArgb(30, 110, 150, 255))`
    //       —— 写死、**不经 `ThemeService`** ⇒ 主题无关常量，与 LIGHT 同值
    //       （旧值 `rgba_pm(20,23,40,40)` 是自造的"accent × α40"：既非官方色，也非官方 α30）。
    //       预乘（截断）: 110×30/255=12.9→12、150×30/255=17.6→17、255×30/255=30.0→30。
    selection_fill: rgba_pm(12, 17, 30, 30),
    // 官方: 同 `accent`。
    stamp_preview_stroke: Color32::from_rgb(0x84, 0x98, 0xFF),
    // 落位诊断两色与主题无关（同浅色侧）
    stamp_blocked: Color32::from_rgb(0xF7, 0x42, 0x55),
    stamp_boundary: Color32::from_rgb(0xF5, 0xAE, 0x32),

    // 遮罩两套同值：它压在**地图**上，与主题无关（浅色下也不该变）。
    modal_scrim: rgba_pm(0, 0, 0, 90),
    // 官方: 同 `panel_bg`（`Panel` `#1C2028`）。
    menu_popup_bg: Color32::from_rgb(0x1C, 0x20, 0x28),
    // 官方: 同 `separator`（`Line` `#303640`）。
    menu_popup_border: Color32::from_rgb(0x30, 0x36, 0x40),
    // 官方: 同 `accent_soft`（蓝键 `Theme2B355A` 经 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:46` 重算 ⇒ `#363F60`）。
    menu_popup_hover: Color32::from_rgb(0x36, 0x3F, 0x60),
};
thread_local! {
    /// 当前是否深色（**线程局部**：GUI 单线程，`cargo test` 每条用例独立线程 ⇒ 互不污染）。
    static DARK_MODE: Cell<bool> = const { Cell::new(false) };
}

/// 按开关取色板（**纯函数**，不读全局；验收用例用它逐字段核对两套值）。
pub const fn palette_for(dark: bool) -> &'static Palette {
    if dark {
        &DARK
    } else {
        &LIGHT
    }
}

/// 当前是否深色主题。
pub fn is_dark() -> bool {
    DARK_MODE.with(Cell::get)
}

/// 切换主题（只改开关，不动任何绘制代码）。
pub fn set_dark(on: bool) {
    DARK_MODE.with(|c| c.set(on));
}

/// **当前生效的色板** —— 所有绘制代码取色的唯一入口。
pub fn ui() -> &'static Palette {
    palette_for(is_dark())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 两套色板必须**逐字段**都不相同（除了官方"与主题无关"的那几项，见 `same_ok`）。
    ///
    /// 这条挡的是"深色板漏抄一个字段，于是那一处仍然浅色"——那正是本单要防的回归。
    ///
    /// ⚠ `same_ok` **不是放宽**：进这张表的每个字段都有官方依据证明它**本就与主题无关**
    /// （遮罩压在**地图**上；`OnAccent` 算出来两套都是黑；`_upstream/CrystalStudio/CollisionMarkers.cs:6-7`、
    /// `_upstream/CrystalStudio/MapViewport.cs:303` 与 `a78affdf` 引入的 `hoverCellOutline`
    /// （本地快照无）都是写死常量、不经 `ThemeService`）。
    /// 反向纪律：凡是"官方有主题相关来源"的字段**一律不许**进这张表 —— 那才是真的把回归放过去。
    /// 这张表里的每一项另由 [`主题无关字段取官方常量`] 逐字节钉住官方值，
    /// 防止"两套都抄错但抄成一样"。
    ///
    /// 📌 **2026-11-08 变动**：`cell_hover_outline`（原 `cell_hover_gold`）由 `pairs` **移入**本表 ——
    /// 上游 `a78affdf` 引入的 `hoverCellOutline` 是 `new ImmutableSolidColorBrush(Colors.DarkBlue, .7)`
    /// 硬编码常量，不是 `ThemeXXXXXX` 主题刷 ⇒ 深浅必须同值。总数仍 49（一进一出，见断言）。
    #[test]
    fn 两套色板逐字段不同() {
        // 与主题无关的那几项（逐条依据见上）。
        let same_ok = [
            "modal_scrim",
            "accent_button_text",
            "collision_low_mark",
            "collision_high_mark",
            "selection_fill",
            "hover_blocked_text",
            // A-011：落位诊断两色与主题无关（压在游戏画面上的硬编码诊断色）
            "stamp_blocked",
            "stamp_boundary",
            // 2026-11-08：悬停格描边。上游 `a78affdf` 引入的 `hoverCellOutline` 写死
            // `ImmutableSolidColorBrush(Colors.DarkBlue, .7)`（不经 `ThemeService`）
            // ⇒ 深浅同值；官方值由 [`主题无关字段取官方常量`] 逐通道钉住。
            "cell_hover_outline",
        ];
        let pairs = [
            ("title_bar", LIGHT.title_bar, DARK.title_bar),
            ("title_bar_text", LIGHT.title_bar_text, DARK.title_bar_text),
            ("menu_bar", LIGHT.menu_bar, DARK.menu_bar),
            ("menu_bar_text", LIGHT.menu_bar_text, DARK.menu_bar_text),
            ("panel_bg", LIGHT.panel_bg, DARK.panel_bg),
            ("panel_header", LIGHT.panel_header, DARK.panel_header),
            ("view_surround", LIGHT.view_surround, DARK.view_surround),
            ("canvas_bg", LIGHT.canvas_bg, DARK.canvas_bg),
            ("status_bar", LIGHT.status_bar, DARK.status_bar),
            ("tab_strip_bg", LIGHT.tab_strip_bg, DARK.tab_strip_bg),
            ("tab_hover_bg", LIGHT.tab_hover_bg, DARK.tab_hover_bg),
            (
                "tab_selected_underline",
                LIGHT.tab_selected_underline,
                DARK.tab_selected_underline,
            ),
            // B2 新增字段：关闭符悬停底（`QuietButton` 继承的 `:pointerover` 底，
            // `_upstream/CrystalStudio/EditorControls.axaml:12` 的 `Theme394354`）。
            ("close_hover_bg", LIGHT.close_hover_bg, DARK.close_hover_bg),
            ("text", LIGHT.text, DARK.text),
            ("text_muted", LIGHT.text_muted, DARK.text_muted),
            ("status_bar_text", LIGHT.status_bar_text, DARK.status_bar_text),
            ("accent", LIGHT.accent, DARK.accent),
            ("accent_dark", LIGHT.accent_dark, DARK.accent_dark),
            ("accent_soft", LIGHT.accent_soft, DARK.accent_soft),
            // `cell_hover_outline` 不在此列：上游是硬编码常量 ⇒ 已移入 `same_ok`（见上）。
            ("grid_line", LIGHT.grid_line, DARK.grid_line),
            ("separator", LIGHT.separator, DARK.separator),
            ("dock_divider", LIGHT.dock_divider, DARK.dock_divider),
            ("input_bg", LIGHT.input_bg, DARK.input_bg),
            ("input_search_bg", LIGHT.input_search_bg, DARK.input_search_bg),
            ("input_border", LIGHT.input_border, DARK.input_border),
            ("checkbox_border", LIGHT.checkbox_border, DARK.checkbox_border),
            ("scrollbar", LIGHT.scrollbar, DARK.scrollbar),
            ("color_swatch", LIGHT.color_swatch, DARK.color_swatch),
            ("status_dot", LIGHT.status_dot, DARK.status_dot),
            ("empty_icon", LIGHT.empty_icon, DARK.empty_icon),
            ("empty_text", LIGHT.empty_text, DARK.empty_text),
            ("warn_banner_bg", LIGHT.warn_banner_bg, DARK.warn_banner_bg),
            ("warn_banner_edge", LIGHT.warn_banner_edge, DARK.warn_banner_edge),
            ("warn_banner_text", LIGHT.warn_banner_text, DARK.warn_banner_text),
            ("stamp_missing_bg", LIGHT.stamp_missing_bg, DARK.stamp_missing_bg),
            ("selection_stroke", LIGHT.selection_stroke, DARK.selection_stroke),
            (
                "stamp_preview_stroke",
                LIGHT.stamp_preview_stroke,
                DARK.stamp_preview_stroke,
            ),

            ("menu_popup_bg", LIGHT.menu_popup_bg, DARK.menu_popup_bg),
            ("menu_popup_border", LIGHT.menu_popup_border, DARK.menu_popup_border),
            ("menu_popup_hover", LIGHT.menu_popup_hover, DARK.menu_popup_hover),
        ];
        assert_eq!(
            pairs.len() + same_ok.len(),
            49,
            "字段清单要覆盖全部 49 项（A-011 加了两条诊断色；B1-FIX 加了标签条三色：容器底/悬停底/选中下划线；B2 加了关闭符悬停底 close_hover_bg；2026-11-08 悬停格描边由 pairs 移入 same_ok，一进一出、总数不变）"
        );
        for (name, l, d) in pairs {
            assert_ne!(l, d, "字段 {name} 两套色板相同（深色模式会漏变）");
        }
    }

    /// **主题无关字段取官方常量**（与 `same_ok` 配对，防止"两套都抄错但抄成一样"）。
    ///
    /// 这几项官方都是**写死常量、不经 `ThemeService`** ⇒ 深浅两套必须**逐字节等于官方预乘值**：
    /// - 低/高位阻挡：`_upstream/CrystalStudio/CollisionMarkers.cs:6-7` `Color.FromArgb(90, 240, 85, 105)` / `(90, 65, 165, 245)`
    /// - 选区填充：`_upstream/CrystalStudio/MapViewport.cs:303` `Color.FromArgb(30, 110, 150, 255)`
    /// - 悬停格描边：`a78affdf` 引入的 `hoverCellOutline`（本地 `_upstream` 快照无此符号）
    ///   `new ImmutableSolidColorBrush(Colors.DarkBlue, .7)` ⇒ `#00008B` @ α0.7
    ///
    /// ⚠ 为什么必须有这条：`same_ok` 让它们退出"两套必须不同"的检查，**只退出不补就真是放宽了**。
    /// 这条把"退出逐字段不同"换成"必须等于官方"，强度只增不减。
    ///
    /// ⚠ **`want` 必须从官方 straight-alpha 现算，不许复用板里的字面量**（审查第 7 轮缺陷 1）：
    /// 旧版 `want` 写的是与色板**同一串** `rgba_pm(84, 30, 37, 90)` ⇒ 预乘算错一位、两处一起改
    /// 仍然全绿，等于只证了"两套板一致"。现在：官方值只出现在本用例的常量表里，
    /// 预乘用本用例**独立实现**的 `pm`（不碰生产代码的 `rgba_pm`），断言比**逐通道字节**
    /// （`Color32::r/g/b` 返回的正是预乘后的字节，见 `ecolor` 的 `color32.rs:211-233`）。
    #[test]
    fn 主题无关字段取官方常量() {
        /// 官方 straight-alpha → 预乘（**截断**口径，`c × a / 255` 整数除法）。
        /// **测试内独立实现**：生产代码那个 `rgba_pm` 只做
        /// `from_rgba_premultiplied`、不含除法，两者没有共同实现可复用。
        ///
        /// ⚠ 2026-11-08 显式订正：这里原写「与 `egui` 的 `from_rgba_unmultiplied`
        /// 同一个整数除法」，**该说法是错的**。实读 `ecolor 0.33.3`
        /// （`src/color32.rs`，`Cargo.lock` 钉的版本）的 `from_rgba_unmultiplied`：
        /// `fast_round(value as f32 * linear_f32_from_linear_u8(alpha))` ⇒ **四舍五入**，
        /// 与本用例的**截断**在边界上会差 1（例：`139×179/255 = 97.57`，
        /// 截断 97 / 四舍五入 98）。本仓半透明色统一按**截断**落盘，故本用例保留截断，
        /// 并请勿据此注释去改用 `from_rgba_unmultiplied`（会与既有字段口径不一致）。
        fn pm(c: u8, a: u8) -> u8 {
            (u16::from(c) * u16::from(a) / 255) as u8
        }
        // (字段名, 官方 (α, R, G, B), 浅色板, 深色板)
        let cases = [
            (
                "collision_low_mark",
                (90u8, 240u8, 85u8, 105u8),
                LIGHT.collision_low_mark,
                DARK.collision_low_mark,
            ),
            (
                "collision_high_mark",
                (90, 65, 165, 245),
                LIGHT.collision_high_mark,
                DARK.collision_high_mark,
            ),
            (
                "selection_fill",
                (30, 110, 150, 255),
                LIGHT.selection_fill,
                DARK.selection_fill,
            ),
            (
                // 官方 `_upstream/CrystalStudio/MainWindow.CellHover.cs:61` 的 `Brushes.Red` = `#FFFF0000`（不透明）
                "hover_blocked_text",
                (255, 255, 0, 0),
                LIGHT.hover_blocked_text,
                DARK.hover_blocked_text,
            ),
            (
                // 官方 `a78affdf` 引入的 `hoverCellOutline`
                // `new ImmutableSolidColorBrush(Colors.DarkBlue, .7)`
                // ⇒ `Colors.DarkBlue` = `#00008B`（RGB 0,0,139）、α = 0.7×255 = 178.5 → 179。
                // 期望预乘：R=0、G=0、B=139×179/255=97.57→**97**（截断）、α=179。
                "cell_hover_outline",
                (179, 0, 0, 139),
                LIGHT.cell_hover_outline,
                DARK.cell_hover_outline,
            ),
        ];
        for (name, (a, r, g, b), l, d) in cases {
            // 期望值：逐通道由官方 straight-alpha 现算（240×90/255=84.7→84 这种）
            let want = (pm(r, a), pm(g, a), pm(b, a), a);
            let got_l = (l.r(), l.g(), l.b(), l.a());
            let got_d = (d.r(), d.g(), d.b(), d.a());
            assert_eq!(
                got_l, want,
                "{name} 浅色板不是官方常量（官方 straight alpha = ({a}, {r}, {g}, {b})）"
            );
            assert_eq!(
                got_d, want,
                "{name} 深色板不是官方常量（官方与主题无关 ⇒ 两套必须同值）"
            );
        }
    }

    /// **官方浅色变换可复算**（交叉验证"XAML 资源就是深色值"这条结论）。
    ///
    /// 照 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:47` 的公式手算下面每个键，必须逐字节等于本文件浅色板的值：
    /// `Blend(White, #A3ADB9, clamp(L*1.4-0.07, 0, 0.65))`。
    ///
    /// 2026-11-05 增补两个键（都是本轮订正过的角色，用**公式**而不是注释钉住）：
    /// - `Theme171B22` → `LIGHT.status_bar`：证明状态栏源键是 `_upstream/CrystalStudio/MainWindow.axaml:84` 那一行；
    /// - `Theme14171D` → `LIGHT.view_surround`：证明 `DARK.title_bar` 取的窗口外框键真实存在
    ///   （`_upstream/CrystalStudio/App.axaml:83` `Selector="Window"` 的 `Background`）。
    #[test]
    fn 官方浅色变换可复算() {
        let blend = |a: [u8; 3], b: [u8; 3], amount: f64| -> [u8; 3] {
            let f = |x: u8, y: u8| {
                (f64::from(x) * (1.0 - amount) + f64::from(y) * amount) as u8
            };
            [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]
        };
        let light_of = |dark: [u8; 3]| -> [u8; 3] {
            let lum = (f64::from(dark[0]) * 0.2126
                + f64::from(dark[1]) * 0.7152
                + f64::from(dark[2]) * 0.0722)
                / 255.0;
            let blue = i32::from(dark[2]) - i32::from(dark[0]) > 35
                && i32::from(dark[2]) - i32::from(dark[1]) > 12;
            assert!(!blue, "本用例只覆盖非蓝键");
            blend([255, 255, 255], [0xA3, 0xAD, 0xB9], (lum * 1.4 - 0.07).clamp(0.0, 0.65))
        };
        let c = |p: Color32| [p.r(), p.g(), p.b()];
        for (dark, light, name) in [
            ([0x1C, 0x20, 0x28], LIGHT.panel_bg, "panel_bg"),
            ([0x30, 0x36, 0x40], LIGHT.separator, "separator"),
            ([0x25, 0x2C, 0x38], LIGHT.input_bg, "input_bg"),
            ([0x14, 0x18, 0x1F], LIGHT.input_search_bg, "input_search_bg"),
            // 状态栏：`_upstream/CrystalStudio/MainWindow.axaml:84` 的 `Theme171B22`（浅色由公式算出，与截图直方图逐字节相同）。
            ([0x17, 0x1B, 0x22], LIGHT.status_bar, "status_bar"),
            // 窗口外框：`_upstream/CrystalStudio/App.axaml:83` `Selector="Window"` 的 `Theme14171D`（= `DARK.title_bar` 的源键）。
            ([0x14, 0x17, 0x1D], LIGHT.view_surround, "view_surround"),
            // 标签条容器底：`_upstream/CrystalStudio/MainWindow.MapTabs.cs:36` 的 `Theme1C2028`（B1-FIX 新增字段）。
            ([0x1C, 0x20, 0x28], LIGHT.tab_strip_bg, "tab_strip_bg"),
            // 标签悬停底：`_upstream/CrystalStudio/EditorControls.axaml:63` 的 `Theme29313E`（B1-FIX 新增字段）。
            ([0x29, 0x31, 0x3E], LIGHT.tab_hover_bg, "tab_hover_bg"),
        ] {
            assert_eq!(light_of(dark), c(light), "{name} 与官方公式算出的浅色不一致");
        }
    }

    /// **状态栏正文色回源**（审计项 H A-004）：深色 = 官方键 `#ADB8C9`，
    /// 浅色 = `_upstream/CrystalStudio/ThemeService.Avalonia.cs:47` 常量支 `#434D5A`。
    ///
    /// 钉的是「状态栏正文是**独立角色**」这条结论：若有人把它改回 `text`
    /// （深色 `#E5E9F0`）或 `text_muted`（`#929CAB`），深色侧立刻不等 ⇒ 本用例变红。
    ///
    /// `want` 一律**从官方键现算**（复刻 `:43` 判蓝 + `:47` 两支），不复用板里的字面量
    /// —— 否则"色板和断言一起抄错"仍然全绿，那等于只证了自洽。
    #[test]
    fn 状态栏正文取官方键与其浅色变换() {
        // 官方源键：`_upstream/CrystalStudio/App.axaml:55` `ThemeADB8C9` = `#ADB8C9`
        // （消费点 `_upstream/CrystalStudio/MainWindow.axaml:84` 的 `Status` TextBlock）。
        let src = [0xAD_u8, 0xB8, 0xC9];
        let got_dark = (
            DARK.status_bar_text.r(),
            DARK.status_bar_text.g(),
            DARK.status_bar_text.b(),
        );
        assert_eq!(
            got_dark,
            (src[0], src[1], src[2]),
            "深色状态栏正文必须是官方键 ThemeADB8C9 的 #ADB8C9（旧值借 text 的 #E5E9F0 ⇒ 亮一档）"
        );

        // 浅色：复刻 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:43` 判蓝 + `:47` 非蓝键的两支。
        let lum = (f64::from(src[0]) * 0.2126
            + f64::from(src[1]) * 0.7152
            + f64::from(src[2]) * 0.0722)
            / 255.0;
        let blue = i32::from(src[2]) - i32::from(src[0]) > 35
            && i32::from(src[2]) - i32::from(src[1]) > 12;
        assert!(!blue, "ThemeADB8C9 不是蓝键（B−R=28 ≤ 35）⇒ 走 `:47` 非蓝分支");
        assert!(lum >= 0.4, "#ADB8C9 的 lum 应 ≥ 0.4 才走常量支，实测 {lum}");
        // `:47` 后半支是**常量** `Color.FromRgb(67, 77, 90)`，与源键无关。
        let want_light = (67_u8, 77_u8, 90_u8);
        let got_light = (
            LIGHT.status_bar_text.r(),
            LIGHT.status_bar_text.g(),
            LIGHT.status_bar_text.b(),
        );
        assert_eq!(
            got_light, want_light,
            "浅色状态栏正文必须是 `:47` 常量支的 #434D5A"
        );

        // 与通用正文色的关系：浅色同值（官方常量支的结论）、深色不同（差一档）。
        assert_eq!(
            LIGHT.status_bar_text, LIGHT.text,
            "浅色同值是 `:47` 常量支的结果，不是抄错角色"
        );
        assert_ne!(
            DARK.status_bar_text, DARK.text,
            "深色下状态栏正文必须比 text 暗一档"
        );
    }

    /// **标签条三色 = 上游三个键**（B1-FIX，2026-11-08）：深色逐字节等于 XAML 声明值，
    /// 浅色逐字节等于 `_upstream/CrystalStudio/ThemeService.Avalonia.cs:43-47` 现算值。
    ///
    /// 上游出处（逐条，照抄不推导）：
    /// - 容器底 `_upstream/CrystalStudio/MainWindow.MapTabs.cs:36` `SetResource(TemplatedControl.BackgroundProperty, "Theme1C2028")`
    ///   ⇒ `_upstream/CrystalStudio/App.axaml:8` `#1C2028`；
    /// - 悬停底 `_upstream/CrystalStudio/EditorControls.axaml:63` `^:pointerover /template/ Border#Chrome` `Background`
    ///   = `Theme29313E` ⇒ `_upstream/CrystalStudio/App.axaml:14` `#29313E`；
    /// - 选中下划线 `_upstream/CrystalStudio/EditorControls.axaml:64` `^:selected /template/ Border#Chrome` `BorderBrush`
    ///   = `Theme8CA1FF` ⇒ `_upstream/CrystalStudio/App.axaml:47` `#8CA1FF`。
    ///
    /// ⚠ 为什么这条不能只靠 `两套色板逐字段不同`：那条只证"深浅不同"，**证不了抄对**。
    /// 这里 `want` 一律**从官方键现算**（复刻 `:43` 判蓝 + `:46` 蓝键 / `:47` 非蓝键两支），
    /// 不复用板里的字面量 —— 否则"色板和断言一起抄错"仍然全绿。
    ///
    /// ⚠ 第三条（下划线）在**蓝键**那一支：浅色 = `Blend(accent, Black, 0.15)` = accent×0.85，
    /// 用的是本仓浅色 `accent`（截图实测 `#CC9629`）。它与 `accent_dark` 同值但**不是同一个键**，
    /// 深色侧两者**不同色**（`#8CA1FF` vs `#8498FF`）—— 这条断言就是防"图省事改成 accent_dark"。
    #[test]
    fn 标签条三色取上游键与其浅色变换() {
        let blend = |a: [u8; 3], b: [u8; 3], amount: f64| -> [u8; 3] {
            let f = |x: u8, y: u8| (f64::from(x) * (1.0 - amount) + f64::from(y) * amount) as u8;
            [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]
        };
        let lum_of = |c: [u8; 3]| -> f64 {
            (f64::from(c[0]) * 0.2126 + f64::from(c[1]) * 0.7152 + f64::from(c[2]) * 0.0722)
                / 255.0
        };
        let blue_of = |c: [u8; 3]| -> bool {
            i32::from(c[2]) - i32::from(c[0]) > 35 && i32::from(c[2]) - i32::from(c[1]) > 12
        };
        let c = |p: Color32| [p.r(), p.g(), p.b()];

        // ── ①② 非蓝键：浅色 = Blend(White, #A3ADB9, clamp(lum×1.4 − 0.07, 0, 0.65)) ──
        let accent = c(LIGHT.accent);
        assert_eq!(accent, [0xCC, 0x96, 0x29], "浅色 accent 必须是截图实测 #CC9629");
        for (src, got_dark, got_light, name) in [
            (
                [0x1C_u8, 0x20, 0x28],
                DARK.tab_strip_bg,
                LIGHT.tab_strip_bg,
                "tab_strip_bg",
            ),
            (
                [0x29_u8, 0x31, 0x3E],
                DARK.tab_hover_bg,
                LIGHT.tab_hover_bg,
                "tab_hover_bg",
            ),
            // B2 新增：关闭符悬停底 = `QuietButton` 继承的基模板 `:pointerover` 底
            // （`_upstream/CrystalStudio/EditorControls.axaml:12` 的 `Theme394354`，`_upstream/CrystalStudio/App.axaml:26`）。
            (
                [0x39_u8, 0x43, 0x54],
                DARK.close_hover_bg,
                LIGHT.close_hover_bg,
                "close_hover_bg",
            ),
        ] {
            assert_eq!(c(got_dark), src, "{name} 深色必须逐字节等于官方键");
            assert!(!blue_of(src), "{name} 的源键不是蓝键 ⇒ 走 `:47` 非蓝分支");
            let lum = lum_of(src);
            assert!(lum < 0.4, "{name} 的 lum 应 < 0.4，实测 {lum}");
            let want = blend(
                [255, 255, 255],
                [0xA3, 0xAD, 0xB9],
                (lum * 1.4 - 0.07).clamp(0.0, 0.65),
            );
            assert_eq!(
                c(got_light),
                want,
                "{name} 浅色必须等于 `:47` 非蓝分支现算值（官方源键 {src:?}）"
            );
        }

        // ── ③ 蓝键：浅色 = Blend(accent, Black, 0.15) = accent×0.85 ──
        let src = [0x8C_u8, 0xA1, 0xFF];
        assert_eq!(
            c(DARK.tab_selected_underline),
            src,
            "选中下划线深色必须逐字节等于 Theme8CA1FF（App.axaml:47）"
        );
        assert!(
            blue_of(src),
            "Theme8CA1FF 是蓝键（B−R=115>35、B−G=94>12）⇒ 走 `:46` 蓝键分支"
        );
        assert!(
            lum_of(src) >= 0.4,
            "Theme8CA1FF 的 lum 应 ≥ 0.4 才走 `Blend(accent, Black, 0.15)` 那一支"
        );
        assert_eq!(
            c(LIGHT.tab_selected_underline),
            blend(accent, [0, 0, 0], 0.15),
            "选中下划线浅色必须等于 accent×0.85（`ThemeService.Avalonia.cs:46` 蓝键后半支）"
        );
        // 与 `accent_dark` 的关系：深色**不同色**（这是"独立键"的可执行证据），浅色同值。
        assert_ne!(
            DARK.tab_selected_underline, DARK.accent_dark,
            "深色下 Theme8CA1FF(#8CA1FF) 与 accent_dark(#8498FF) 不同色 ⇒ 不许互相替代"
        );
        assert_eq!(
            LIGHT.tab_selected_underline, LIGHT.accent_dark,
            "浅色同值是 accent×0.85 的结果（官方变换的结论），不是抄错角色"
        );
    }

    /// 全局开关：默认浅色；`set_dark(true)` 后 [`ui`] 换板；恢复后逐字段回到浅色。
    #[test]
    fn 全局开关切换色板() {
        assert!(!is_dark(), "用例线程默认必须是浅色（否则会污染同进程的其它用例）");
        assert_eq!(ui().panel_bg, LIGHT.panel_bg);
        set_dark(true);
        assert!(is_dark());
        assert_eq!(ui().panel_bg, DARK.panel_bg);
        set_dark(false);
        assert_eq!(ui().panel_bg, LIGHT.panel_bg);
    }
}
