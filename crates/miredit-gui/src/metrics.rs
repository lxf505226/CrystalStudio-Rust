//! 几何常量表（**所有尺寸的唯一来源**，代码里禁止再写"看起来差不多"的数字）。
//!
//! 计量对象：运行期截图 `layout/refs/crystalstudio-main-runtime-1500x950.png`（1500×950）。
//!
//! ## 截图内容区原点（关键，先读这段）
//! 该 PNG 里 **应用窗口不是从 (0,0) 铺满**：
//! - x=7 与 x=1492 是 1px 金色窗口边框（`#604715`，整列/整行恒定，实测 col x=7 连续 943 行同色）；
//! - x=0..6、x=1493..1499 是**身后的另一个窗口**（`#171717`/`#131315`），y=943..949 同理。
//!
//! ⇒ 应用**内容区原点 = (8, 0)**，内容区尺寸 = **1484 × 942**（x 8..1491，y 0..941）。
//!
//! 因此本文件所有 `REF_*` 常量都写成「相对内容区原点」的坐标，
//! 我们自己的窗口没有那条 1px 金边（无边框窗口），内容区原点就是 (0,0)，尺寸 1500×950。
//! 验收脚本 §七-4 也是按「各自内容区原点」去量的。
//!
//! 全部数值均由 Pillow 逐像素扫描得到（扫描脚本见交付汇报 §四）。

/// 参考截图内容区原点（应用窗口客户区左上角在 PNG 中的位置）。
pub const REF_CONTENT_ORIGIN: (f32, f32) = (8.0, 0.0);
/// 参考截图内容区尺寸（= 窗口客户区）。
pub const REF_CONTENT_SIZE: (f32, f32) = (1484.0, 942.0);

// ───────────────────────── 窗口 ─────────────────────────

/// 窗口客户区尺寸 1500×950。
/// 来源：`layout/crystalstudio-ui.json` → `windows[0].width/height`（= 1500/950），
/// 与参考 PNG 的 1500×950 一致（任务书 §三「窗口」）。
pub const WINDOW_W: f32 = 1500.0;
pub const WINDOW_H: f32 = 950.0;

/// 窗口最小尺寸 1100×720。来源：`crystalstudio-ui.json` → `windows[0].min_width/min_height`。
pub const WINDOW_MIN_W: f32 = 1100.0;
pub const WINDOW_MIN_H: f32 = 720.0;

// ───────────────────────── 顶/底条 ─────────────────────────

/// 标题栏高 31。来源：截图列 x=700 / x=1200，`y0..30` 均为 `#604715`，`y31` 起为 `#F5F6F7`。
pub const TITLE_BAR_H: f32 = 31.0;

/// 菜单栏高 35。来源：截图列 x=700、x=1200，`y31..65` 均为 `#F5F6F7`，`y66` 为分隔线 `#EAECEF`。
pub const MENU_BAR_H: f32 = 35.0;

/// 菜单栏下方 1px 分隔线。来源：截图 y=66 整行为 `#EAECEF`（x=50/152/180/1200 取样一致）。
pub const SEPARATOR_H: f32 = 1.0;

/// 状态栏总高 30 = 上沿 1px 分隔线(`#EAECEF`, y=912) + 29px 内容(`#F7F8F9`, y=913..941)。
pub const STATUS_BAR_H: f32 = 30.0;

/// 状态栏左右内边距 16。来源：状态栏左文本首像素 x=24 ⇒ 24 - 内容区原点 8 = 16；
/// 右侧 `Libraries not loaded` 末像素 x=1475 ⇒ 1491 - 1475 = 16。
/// 与 `crystalstudio-ui.json` → `theme.metrics.statusbar_margin_x` = 16 一致。
pub const STATUS_PAD_X: f32 = 16.0;

// ───────────────────────── 三列停靠 ─────────────────────────

/// 左停靠区宽 156。来源：截图 row y=600，`x8..163` 为 `#F5F6F7`（左面板底），`x164` 起进入中列。
pub const LEFT_DOCK_W: f32 = 156.0;

/// 右停靠区宽 356。来源：截图 row y=620，`x1136..1491` 为 `#F5F6F7`，`x1135` 为分隔线 `#E7EBEF`。
pub const RIGHT_DOCK_W: f32 = 356.0;

/// 停靠区与中列之间的 1px 竖分隔线（`#E7EBEF`）。来源：截图 x=224 与 x=1135。
pub const DOCK_DIVIDER_W: f32 = 1.0;

// ───────────────────────── 面板通用骨架 ─────────────────────────

/// 面板标题条高 23。来源：截图列 x=152，`y67..89`、`y228..250`、`y315..337`、`y554..576`
/// 四段均为 `#E3E6E9`，每段 23 行。（右停靠区同：`y67..89`、`y262..284`、`y748..770`。）
pub const PANEL_HEADER_H: f32 = 23.0;

/// 面板内容区内边距 5。来源：第一个工具按钮左边 x=15，左停靠区左边 x=8，
/// 且按钮自身还有 2px margin（见 `TOOL_MARGIN`）⇒ 15 - 8 - 2 = 5。
pub const PANEL_PAD: f32 = 5.0;

/// 面板标题文字左边距 7（相对面板左边）。来源：`Tools` 的 ⣿ 装饰符起于 x=15，左停靠区左边 x=8 ⇒ 7。
pub const PANEL_TITLE_PAD_L: f32 = 7.0;

/// 面板标题文字相对面板左边的偏移 21 = `PANEL_TITLE_PAD_L` 7 + 握把 6 + 间距 8。
pub const PANEL_TITLE_GAP: f32 = 8.0;

/// 面板标题右侧折叠箭头距右边缘 12。
pub const PANEL_CHEVRON_INSET: f32 = 12.0;

/// 面板标题字号 11。来源：`crystalstudio-ui.json` → `theme.metrics.glyph_mark_font_size` = 11
/// （与截图标题条 23px 高、字形约 8px 高相符）。
pub const PANEL_TITLE_FONT: f32 = 11.0;

/// 标题条左侧 ⣿ 装饰符尺寸：2 列 × 3 行小方点，整体 6 宽 × 10 高。
/// 来源：截图 (15,75)-(20,84) 的暗像素簇（`Tools` 标题左侧）。
pub const GRIP_W: f32 = 6.0;
pub const GRIP_H: f32 = 10.0;

// ───────────────────────── 左停靠区各面板高度 ─────────────────────────

/// Tools 面板总高 161（标题条 23 + 内容 138）。
/// 来源：截图列 x=152，标题条 `y67..89`，内容 `y90..227`（138 行）。
pub const PANEL_TOOLS_H: f32 = 161.0;

/// Tool Options 面板总高 87（标题条 23 + 内容 64）。
/// 来源：截图列 x=152，标题条 `y228..250`，内容 `y251..314`（64 行）。
pub const PANEL_TOOL_OPTIONS_H: f32 = 87.0;

/// Layers 面板总高 239（标题条 23 + 内容 216）。
/// 来源：截图列 x=152，标题条 `y315..337`，内容 `y338..553`（216 行）。
pub const PANEL_LAYERS_H: f32 = 239.0;

/// 右停靠区 Minimap 面板总高 195（标题条 23 + 内容 172）。
/// 来源：截图列 x=1480，标题条 `y67..89`，内容 `y90..261`。
pub const PANEL_MINIMAP_H: f32 = 195.0;

/// 右停靠区 Palette 面板总高 495（标题条 23 + 内容 472）。
/// 来源：截图列 x=1480，标题条 `y262..284`，内容 `y285..747`（463 行）；
/// 本单窗口高 950 比参考内容区 942 多 8px，多出的 9px 全部并入 Palette 内容区
/// ⇒ 463 + 9 = 472。Cell Inspector / Active Asset 两个「末位面板」吸收剩余高度。
pub const PANEL_PALETTE_H: f32 = 495.0;

// ───────────────────────── Tools 面板：工具按钮网格 ─────────────────────────

/// 工具按钮边长 32。来源：截图首个（选中）按钮的金色描边框 (15,97)-(46,128) = 32×32。
/// 与 `crystalstudio-ui.json` → `theme.metrics.tool_button` = 32 一致。
pub const TOOL_BTN: f32 = 32.0;

/// 工具按钮外边距 2。来源：`theme.metrics.tool_button_margin` = 2；
/// 截图校验：面板内容左边 8+5=13，按钮左边 15 ⇒ 差 2。
pub const TOOL_MARGIN: f32 = 2.0;

/// 工具按钮格距 36 = 32 + 2×2。来源：`theme.metrics.tool_item_min_width` = 36；
/// 截图校验：第 2 个按钮图标簇 (60,75) 相对第 1 个 (23,38) 偏移 37（含图标自身偏心 ±1）。
pub const TOOL_ITEM: f32 = 36.0;

/// 工具按钮网格列数 **4**。
/// ⚠ **与任务书 §三「2×5 图标网格」不符，以截图为准**：
/// 左停靠区可用内容宽 = 156 - 2×5 = 146，4×36 = 144 放得下，5×36 = 180 放不下；
/// 截图实测 10 个按钮排成 **4 / 4 / 2** 三行（行 y=97/133/169），列 x=15/51/87/123。
pub const TOOL_COLS: usize = 4;

/// Tools 内容区首个按钮的行偏移 7（内容区顶 y=90 → 按钮顶 y=97）。
/// = 面板内边距 5 + 按钮 margin 2。
pub const TOOLS_FIRST_ROW_OFF: f32 = 7.0;

// ───────────────────────── Tool Options 面板 ─────────────────────────

/// 色块 / 滑杆滑块 12 宽 × 16 高。来源：截图 `#AD7F22` 像素簇 (14,271)-(25,288) ⇒ 12×18，
/// 去掉上下各 1px 抗锯齿后取 12×16。
pub const SWATCH_W: f32 = 12.0;
pub const SWATCH_H: f32 = 16.0;

/// 滑杆轨道：起点相对面板左边 18（截图 x=26），长 64（到 x=90），高 3。
/// 来源：截图 y=280 行 `x26..90` 为 `#E3E6EA`；高度取自 `theme.metrics.slider_track_height` = 3。
pub const SLIDER_X: f32 = 18.0;
pub const SLIDER_W: f32 = 64.0;
pub const SLIDER_TRACK_H: f32 = 3.0;

/// `3×3` 胶囊标签的**左边位置**：截图 x=92（相对面板 84）。
/// 来源：截图 y=280 行 `x92..136` 为 `#ECEEF0`。
///
/// ⚠ **只是布局偏移，不参与上游算式**：上游 `MainWindow.axaml:6` 的胶囊宽度由内容撑出，
/// 根本没有坐标常量。本单（A17）**不动它** —— 把它和尺寸混在一起，正是上一版
/// 「45×22 当成常量」的同类错误。
pub const PILL_X: f32 = 84.0;

// ── 胶囊尺寸：**上游是算式，不是固定值**（A17，2026-11-08） ──
//
// 真值：`CrystalStudio/MainWindow.axaml:6`（逐字）
// ```xml
// <Border Background="{DynamicResource Theme293242}" CornerRadius="4" Padding="5,2"
//         Margin="0,4,0,0" HorizontalAlignment="Center">
//     <TextBlock x:Name="BrushSizeLabel" Text="3×3" MinWidth="35"
//                TextAlignment="Center" FontWeight="SemiBold" />
// </Border>
// ```
//
// 推导（Avalonia 布局：`MinWidth` 在**内层 TextBlock** 上，`Padding` 在**外层 Border** 上）：
// - TextBlock 宽 = `max(MinWidth 35, 文字 advance)`；
// - Border 宽 = TextBlock 宽 + `Padding.Left 5` + `Padding.Right 5` = **`max(35, 文字宽) + 10`**；
// - Border 高 = TextBlock 行高 + `Padding.Top 2` + `Padding.Bottom 2` = **`文字高 + 4`**；
// - 圆角 = `CornerRadius="4"`。
//
// ⇒ **原为截图实测 45×22（`x92..136` 宽 44~45），2026-11-08 按上游算式改为动态。**
//   截图那个 45 正好 = `max(35, 文字宽("3×3")) + 10 = 35 + 10`（实测 `3×3` 的 advance < 35），
//   与算式自洽 —— 这同时是「45 是内容撑出来的、不是常量」的反证：
//   若按 `max(35, 文字宽 + 10)` 算，「3×3」只有 35 宽，**对不上截图的 45**。
//
// ⚠ **字形宽 / 行高上游不可得**（Avalonia + Segoe UI 的字体度量本仓拿不到）：
//   按任务书口径取**本仓实际上屏字体**的 `layout_no_wrap` 实测值
//   （与 [`crate::tab_strip`] 的 `TAB_CLOSE_GLYPH_W` / `TAB_CLOSE_GLYPH_H` 同一做法）。
pub const PILL_MIN_W: f32 = 35.0;
pub const PILL_PAD_X: f32 = 5.0;
pub const PILL_PAD_Y: f32 = 2.0;
/// 圆角 4（上游 `CornerRadius="4"`）。改前 `draw_tool_options` 里就是字面量 `4.0`，值不变，只是给了名字。
pub const PILL_RADIUS: f32 = 4.0;

// ───────────────────────── 笔刷尺寸（3..=31 奇数） ─────────────────────────

/// 笔刷尺寸的下限 / 上限 / 步进 —— **与官方滑块逐字一致**。
///
/// 真值：`CrystalStudio/MainWindow.axaml:6`
/// `<Slider x:Name="BrushSize" Minimum="3" Maximum="31" Value="3"
///   TickFrequency="2" IsSnapToTickEnabled="True" …/>`
/// ⇒ 3 / 5 / … / 31 共 15 档，**只有奇数**（`TickFrequency=2` + 吸附）。
///
/// 键盘 `[` / `]` 各走 **2**（保持奇数），并夹在 3..31：
/// `MainWindow.axaml.cs:871`
/// `BrushSize.Value = Math.Clamp(BrushSize.Value + (e.Key == Key.OemOpenBrackets ? -2 : 2), 3, 31);`
///
/// 视图层的同一口径：`MapViewport.cs:27`
/// `public int BrushSize { get => brushSize; set => brushSize = Math.Clamp(value | 1, 3, 31); }`
/// —— `value | 1` 就是"偶数一律抬成奇数"，与本文件的 `clamp_brush_size` 同一判据。
pub const BRUSH_MIN: usize = 3;
pub const BRUSH_MAX: usize = 31;
pub const BRUSH_STEP: usize = 2;
/// 缺省尺寸 3（`MainWindow.axaml:6` 的 `Value="3"`，也是胶囊初始文案 `3×3` 的来历）。
pub const BRUSH_DEFAULT: usize = 3;

/// 把任意尺寸夹进「3..=31 的奇数」—— **唯一的规范化入口**。
///
/// 先夹范围再抬奇数（`| 1`），于是 `0/1/2` → 3、`32/40` → 31、`4` → 5、`8` → 9。
/// 与 `MapViewport.cs:27` 的 `Math.Clamp(value | 1, 3, 31)` 等价
/// （差别只在"先夹后抬"与"先抬后夹"在边界上的次序，两处都夹到 3..31 的奇数集合内）。
pub const fn clamp_brush_size(n: usize) -> usize {
    let v = if n < BRUSH_MIN { BRUSH_MIN } else { n };
    let v = if v > BRUSH_MAX { BRUSH_MAX } else { v };
    // 抬成奇数：偶数 +1（31 是奇数，+1 只会发生在被上面夹住的偶数上，故不会越界）
    if v % 2 == 0 {
        if v + 1 > BRUSH_MAX {
            BRUSH_MAX
        } else {
            v + 1
        }
    } else {
        v
    }
}

// ───────────────────────── 填充（Bucket） ─────────────────────────

/// 单次填充的**格数上限 200,000**（防呆：不让一次误点把整张图吞进撤销栈）。
///
/// 来源：任务书 §三.2 指定的硬上限（官方 C# 无此保护 —— `MapViewport.cs:485-539`
/// 的 `BucketFill` 直接 BFS 到底，`visited` 只防重复不防规模）。
/// 超过即**报错且一格不写**（见 `edit::EditSession::fill`）。
///
/// 量级参照：700×700 = 490,000 格 ⇒ 一张"全图同图号"的极端图必被拦住；
/// 一格 `Change` 约 76 B，200,000 格 ≈ 15 MB 撤销步，仍在可接受范围。
pub const FILL_MAX_CELLS: usize = 200_000;

// ───────────────────────── Layers 面板 ─────────────────────────

/// 层列表行高 26。来源：截图选中行 `Ground Tiles` 的金色框 (17,342)-(155,367) = 26 行；
/// 相邻行眼睛图标 y 中心间隔亦为 26。与 `theme.metrics.layer_row_min_height` = 26 一致。
pub const LAYER_ROW_H: f32 = 26.0;

/// 层列表行左边缩进 9（左停靠区左边 x=8 → 行左边 x=17）。
pub const LAYER_ROW_INSET_L: f32 = 9.0;
/// 层列表行右边缩进 8（左停靠区右边 x=163 → 行右边 x=155）。
pub const LAYER_ROW_INSET_R: f32 = 8.0;

/// 层列表第一行顶部偏移：内容区顶 y=338 → 行顶 y=342，偏移 4。
pub const LAYER_FIRST_ROW_OFF: f32 = 4.0;

/// 眼睛按钮宽 26。来源：`crystalstudio-ui.json` → `GroundVisible` 节点 `rect.w` = 26。
pub const LAYER_EYE_BTN_W: f32 = 26.0;

/// 眼睛图标 16×16。来源：截图 (26,348)-(45,364) 图标簇 ≈ 16；
/// 与 `theme.metrics.icon_eye` = 16 一致。
pub const ICON_EYE: f32 = 16.0;

/// 子组 `Cell Highlights` 前的分隔线之上留白 6：
/// 截图第 5 行（High Collisions）占 y=446..471，分隔线在 y=478 ⇒ 472..477 共 6 行留白。
pub const LAYER_GROUP_GAP: f32 = 6.0;

/// `Cell Highlights` 小标题高 22：截图分隔线 y=478，`Light Cells` 行自 y=500 起 ⇒ 478..499 共 22 行。
pub const LAYER_GROUP_LABEL_H: f32 = 22.0;

/// 复选框边长 14。来源：截图 Light Cells 复选框 (23,500)-(37,514)；
/// 与 `theme.metrics.checkbox_box` = 14 一致。
pub const CHECKBOX: f32 = 14.0;

// ───────────────────────── Layers 面板内部排版（原为 dock_left.rs 字面量，本轮上收） ─────────────────────────

/// 眼睛按钮盒子相对行左边的内缩 4。
pub const LAYER_EYE_INSET: f32 = 4.0;

/// 眼睛盒子右边 → 图层名文字左边的间距 2。
pub const LAYER_TEXT_GAP: f32 = 2.0;

/// 碰撞标记色块 7×7（截图 Low/High Collisions 行里的小方块）。
pub const LAYER_MARK_W: f32 = 7.0;

/// 标记块左边缘 → 文字左边缘的偏移 13（块 7 宽 + 6 间距）。
pub const LAYER_MARK_TEXT_OFF: f32 = 13.0;

/// 子组标题距分隔线底边 4（截图分隔线 y=478、标题文字 y=485 起）。
pub const LAYER_GROUP_TITLE_OFF: f32 = 4.0;

/// 子组标题字号 11（截图 `Cell Highlights` 比图层名小一档）。
pub const LAYER_GROUP_TITLE_FONT: f32 = 11.0;

/// 复选框相对行左边的内缩 6。
pub const LAYER_CB_INSET: f32 = 6.0;

/// 复选框 → 标签文字的左偏移 27（相对行左边）。
pub const LAYER_CB_TEXT_X: f32 = 27.0;

/// 「当前编辑层」行左侧金条宽 3px（贴在 `ACCENT_DARK` 的 1px 金框内沿）。
///
/// **取值依据**：参考截图里选中行 (17,342)-(155,367) 只有 1px 金框 + 浅金底
/// （`#AD7F22` / `#F7EFDE`，见 [`crate::theme`]）。1px 在 1500×950 的截图里与"眼睛开关"
/// 混在一起不好分辨，故本版把"当前编辑层"再加一根 **3px 竖条**（同一支金色 `ACCENT_DARK`，
/// 不引入新颜色）。3 是 26px 行高下肉眼可辨的最小宽度（2px 在 100% 缩放下会与边框糊成一片）。
pub const LAYER_EDIT_BAR_W: f32 = 3.0;

/// 金条相对行内沿的偏移 1（正好压在 1px 金框的内侧，不与边框重叠）。
pub const LAYER_EDIT_BAR_INSET: f32 = 1.0;

// ───────────────────────── Tool Options 面板内部 ─────────────────────────

/// 色块相对内容区左边的内缩 6。
pub const SWATCH_INSET: f32 = 6.0;

// ───────────────────────── Cell Inspector 面板内部排版 ─────────────────────────
//
// 这些值原先是 `dock_left.rs` 里的字面量（9 / 75 / 15 / 11 / 4）。本轮为中文排版核对
// 上收到常量表：验收要逐条算"文字右边界 vs 裁剪盒右边界"，散在绘制代码里没法程序化核对。

/// 信息头距内容区顶 10。
pub const INSPECTOR_HEAD_OFF: f32 = 10.0;

/// 键列左边相对面板内容左边 9。来源：截图键文本起 x=17，面板左边 x=8。
pub const INSPECTOR_KEY_X: f32 = 9.0;

/// 值列左边相对面板内容左边 75。
///
/// 截图值文本起 x=79（相对 71），这里取 75 是为了给键列留出比英文更宽松的裁剪余量：
/// 中文键最长 `图块动画` 4 字 ≈ 44px，键列直到 `INSPECTOR_VAL_X - INSPECTOR_KEY_CLIP_PAD` = 71
/// 都放得下。
/// **中文实测调整：无**（值列可用宽 = `LEFT_DOCK_W - 75` = 81px，各条中文值均已换行适配，
/// 见 `ui_text::VAL_METADATA` 的说明）。
pub const INSPECTOR_VAL_X: f32 = 75.0;

/// 键列裁剪盒右边界再往左 4（键文本最多画到值列左边 -4）。
pub const INSPECTOR_KEY_CLIP_PAD: f32 = 4.0;

/// 行高 15（截图相邻行文字基线差 15）。
pub const INSPECTOR_LINE_H: f32 = 15.0;

/// 字号 11（截图 Inspector 文字明显小于正文 12）。
pub const INSPECTOR_FONT: f32 = 11.0;

/// 每条 `(键, 值)` 之后的行间隔 4。
pub const INSPECTOR_ROW_GAP: f32 = 4.0;

// ───────────────────────── 中列：页签条 / 画布 / 网格 ─────────────────────────

/// 页签条高 38。来源：截图列 x=700，`y66..103` 为 `#F5F6F7`（页签条底），`y104` 起为 `#F9FAFB`（视口底）。
///
/// ⚠ **上游为 `GridLength.Auto`**（`MainWindow.MapTabs.cs:28` 给画布 `Grid` 加的第一行
/// `new RowDefinition { Height = GridLength.Auto }`，高度由标签项内容撑开，没有固定值）；
/// 本仓固定 38 是**既有截图基线**（`crystalstudio-main-runtime-1500x950.png` 的
/// `y66..103` = 38px），不改 —— 改了 `Layout::viewport` 上边界就会动，`--shot` 基线全废。
pub const TABSTRIP_H: f32 = 38.0;

/// 页签高 30，宽 98。来源：截图页签 `Untitled 1 ×` 占 `x164..262`（98 宽）、`y66..102`。
/// 下划线（选中指示）2px，取自截图 y=103..104 的 `#AD7F22`。
///
/// 宽度口径（B1-FIX 订正）：上游标签项是**内容自适应**（`MainWindow.MapTabs.cs:32` 的
/// `StackPanel Horizontal` 里每个 `TabItem` 宽度由 `Padding="8,7"`
/// （`EditorControls.axaml:59`）+ 标题实测宽 + `Label.Margin="4,2,8,2"`
/// （`MainWindow.MapTabs.cs:54`）+ 关闭钮 `Padding="4,0,4,0"`（`:57`）算出）。
/// `TAB_W = 98` 就是**参考标题 `Untitled 1 ×` 的那一份实测自然宽**（截图 x164..262）。
/// 本仓不逐标题重量文本宽度：单标签态由 `canvas::draw` 按 `Layout::tab_rect()` 画，
/// 那条几何是 `--shot` 基线与 `单标签矩形与旧版逐值相同` 用例共同钉死的（98×30），
/// 逐标题自适应会把它撑开 ⇒ 本单不动（见回执「仍与上游不一致」第 2 条）。
pub const TAB_W: f32 = 98.0;
pub const TAB_H: f32 = 30.0;
/// 选中下划线厚 2 = 上游 `BorderThickness="0,0,0,2"` 的 bottom 边
/// （`EditorControls.axaml:60` 模板 / `:64` 选中态把它的 `BorderBrush` 换成 `Theme8CA1FF`）。
pub const TAB_UNDERLINE_H: f32 = 2.0;

// 已删除（B1-FIX，2026-11-08，**别再加回来**）：
//
// * `TAB_GAP = 2.0` —— 原注释自承「量不到、取 2」。上游**没有标签间距**：
//   相邻 `Border#Chrome` 背景 `Transparent`、`BorderThickness="0,0,0,2"`
//   （`EditorControls.axaml:60`）、`StackPanel Orientation="Horizontal"` 里没有 Margin
//   （`MainWindow.MapTabs.cs:32`）⇒ 两个标签**边界相接**，视觉间隔全部由
//   `Padding="8,7"`（`:59`）与 `Label.Margin="4,2,8,2"`（`MapTabs.cs:54`）在**标签内部**撑出来。
// * `TAB_MIN_W = 40.0` —— 原注释自承「推导…留 8px」。上游没有最小宽：标签宽度就是内容宽，
//   放不下时由 `ScrollViewer HorizontalScrollBarVisibility="Auto"`（`MapTabs.cs:34`）横向滚动。
//   横向滚动不在本单范围（见回执），本仓溢出时退化为等分收窄且**不设下限**。

/// 标签标题裁剪盒的**右边界内缩 = 上游 `Label.Margin` 的右边距 8**。
///
/// 上游: `MainWindow.MapTabs.cs:54` `map.Label.Margin = new Thickness(4, 2, 8, 2);`
/// ⇒ 左 4 / 上 2 / **右 8** / 下 2。这里取的就是那个 **8**（不是"量出来的"）。
///
/// 旧名 `TAB_TITLE_CLIP_R`（语义是"自定的裁剪右边距"，值恰好也是 8）—— B1-FIX 归位为
/// 上游语义并改名；值不变，所以绘制结果一个像素都没动。
pub const TAB_LABEL_MARGIN_R: f32 = 8.0;

/// 世界格尺寸 48 宽 × 32 高（**像素**）。
///
/// 来源（三重一致，本单只依赖这一条）：
/// 1. 任务书 §三「48×32 网格线」；
/// 2. 截图校验：水平网格线 y=175/207/239/271…（步长 32），竖向步长 48；
/// 3. 客户端真值 `miredit_render::CELL_W/CELL_H`（= `Grobal.h:1980-1981` UNITX/UNITY，
///    Godot 侧 `map_view.gd:9-10`）—— 单元测试 `世界格与渲染库一致` 会断言三者数值相等。
///
/// ⚠ 本单起这两个常量不再是"网格线间距"，而是**真实地图的格像素**：
/// 视口里贴的是 `miredit_render::render_region` 出的 1:1 位图，缩放只作用于贴图矩形。
pub const CELL_W: f32 = 48.0;
pub const CELL_H: f32 = 32.0;

// ───────────────────── 单元格网格（视图 → 单元格网格，2026-11-05 新增） ─────────────────────
//
// 网格本身画在世界格边界上（间距就是上面的 `CELL_W` / `CELL_H`，**不另立一套间距**），
// 这里只放"怎么画"的常量：**线多粗**。

// 网格**没有任何"多小就不画"的缩放门槛**：上游 `901dbe2f`（2026-11-05 07:17）删掉了
// `if (GridVisible && Zoom >= .4)`，改成「永远画 + 缩小时按 `step` 抽稀」
// （`MapViewport.Presentation.cs:51-61@901dbe2f`，判据 `while (step * 32 * Zoom < 8) step *= 2`）。
// ⇒ 抽稀步长的唯一出口是 `crate::grid::step_for`。本文件**不许**再加缩放阈值
//   （原 `GRID_MIN_SCALE = 0.5` 与后来的 `grid::MIN_SCALE = 0.4` **都已删除**）。

/// 网格线宽 **1 屏幕像素**（恒 1，**不随缩放变**）。
///
/// 官方口径是**世界单位**笔宽 `1 / Zoom`（`MapViewport.Presentation.cs:57@901dbe2f`
/// `new ScenePen(ThemeService.Brush("CanvasGrid"), 1 / Zoom)`；旧版固定 0.6 那行已随
/// `901dbe2f` 删除）—— 乘回 `Zoom` 后**屏幕恒 1px**，与本常量一致。
///
/// 为什么不像地图那样按 `scale` 放大：网格是**读图辅助线**，不是地图内容。
/// 2× 时若线也变 2px，一屏的线面积就翻 4 倍，压图更重；而且"1px 网格"是编辑器界面的
/// 通行观感（Aseprite / Tiled 的格线都是 1px）。取整到整像素的理由见 `crate::grid` 模块文档。
pub const GRID_LINE_W: f32 = 1.0;

// ───────────────────────── 中列：真实地图画布（本单新增） ─────────────────────────
//
// 这一组常量的来源分三类，逐条注明：
// - 「任务书」：任务书 §三.4 直接给定的数值（缩放区间）—— ⚠ 上游**无此口径**，属本仓自定，
//   与上游 `LimitZoom` 的偏离已登记在 `CANVAS_MIN_SCALE` / `CANVAS_MAX_SCALE` 的文档里；
// - 「上游」：上游 C# 的硬编码字面量/公式（如滚轮倍率 `1.15`，见 `ZOOM_PER_NOTCH`）；
// - 「推导」：由既有几何常量推出来、截图量不到的新量（拖动余量、重渲上限）。

/// 画布最小缩放 0.25（= 25%）。来源：任务书 §三.4「滚轮以鼠标位置为中心缩放（0.25×–4×，1×=100%）」。
///
/// ⚠ **与上游的偏离（上游无此口径 ⇒ 本仓自定，已登记待拍板：台账 §8.68 #1）**：
/// 上游**没有** 0.25 这个下限 —— `MapViewport.cs:168` 的 `LimitZoom` 是
/// `Math.Clamp(double.IsFinite(zoom) && zoom > 0 ? zoom : Zoom, FitZoom, 1)`，
/// 下限是**随图/窗口变的 `FitZoom`**（`MapViewport.cs:166`；700×700 图约 0.027，
/// 见 [`FIT_MARGIN`] 那条注释的实测），上限是 **1×**。
/// 本仓 `[0.25, 4.0]` 来自**任务书 §三.4**，不是从上游抄来的。
pub const CANVAS_MIN_SCALE: f32 = 0.25;

/// 画布最大缩放 4.0（= 400%）。来源：任务书 §三.4（同 [`CANVAS_MIN_SCALE`]）。
///
/// ⚠ **与上游的偏离（上游无此口径 ⇒ 本仓自定，已登记待拍板：台账 §8.68 #1）**：
/// 上游上限是 **1×** —— `MapViewport.cs:168` 的 `Math.Clamp(…, FitZoom, 1)` **封死了放大**，
/// 上游根本没有 400% 这一档。本仓 4.0 来自**任务书 §三.4**。
/// 严格对齐上游 1× 等于**删掉放大功能**（`+` 与滚轮放大都成空操作，`原始大小` 与 100% 重合）
/// ⇒ 保留与否属**产品裁定**；**未拍板前不得**为了"对齐"把这里改成 1.0。
pub const CANVAS_MAX_SCALE: f32 = 4.0;

/// **「适应地图」的边距**（每轴共 80 px，不是每边 80 px）。
///
/// 来源：上游 `MapViewport.cs:166@901dbe2f` 的
/// `Math.Min(1, Math.Min(Math.Max(1, ActualWidth - 80) / (W * 48.0), Math.Max(1, ActualHeight - 80) / (H * 32.0)))`
/// —— 两条边**平分**这 80 px（每边约 40 px），所以 `usable = (画布边长 - 80).max(1)`。
///
/// ⚠ 只有 `fit ∈ (0.25, 4)` 时才看得出来：`0.map`（700×700）的 fit 算出来约 0.027，
/// 被 [`CANVAS_MIN_SCALE`] 夹到 0.25 ⇒ 边距在那张图上**不可见**（别拿它验收边距）。
pub const FIT_MARGIN: f32 = 80.0;

/// 滚轮每一「格」的缩放倍率 1.15。
///
/// **上游出处**：`MapViewport.cs:471`（`_upstream/CrystalStudio/`，逐字）——
/// `Zoom = LimitZoom(Zoom * (e.Delta.Y > 0 ? 1.15 : 1 / 1.15));`
/// 其中 `1.15` 是上游**硬编码字面量**（不是可配置参数、也不是从主题读的）；
/// 同值另见 `MapMinimap.Avalonia.cs:91`、`ModuleConnectionCanvas.cs:229`，三处一致。
///
/// 以下推导是对该值的**独立复核**（推导不是错的，只是当年缺出处）：
/// 25%→400% 共 16 档，`1.15^16 ≈ 9.36` 略大于 16 倍，实际约 20 档，
/// 手感与常见编辑器一致；不用 1.25（25%→400% 只需 7 档，滚一格跳太多）。
pub const ZOOM_PER_NOTCH: f32 = 1.15;

/// 滚轮「一格」对应的滚动像素 50（egui `smooth_scroll_delta` 的单位是像素，
/// 鼠标滚轮一格约 50px）。来源：推导 + egui 默认行高约定。
pub const SCROLL_PX_PER_NOTCH: f32 = 50.0;

/// 鼠标所在格的高亮描边宽 **4**（屏幕像素，不随缩放变）。
///
/// **上游出处（逐字）**：GitHub `fluttergo/CrystalStudio@a78affdf` 的
/// `CrystalStudio/MapViewport.cs:144` ——
/// `private static readonly ScenePen hoverCellOutline = new(new ImmutableSolidColorBrush(Colors.DarkBlue, .7), 4);`
/// 末位实参 `4` 即笔画宽。绘制点 `:305`
/// `dc.DrawRectangle(null, hoverCellOutline, new(origin.X + cell.X * 48 * Zoom, …));`，
/// 调用链 `:350 DrawHoverCell(dc);`（定义 `:298`）。
/// 上游 `:304` 注释明写 `// Screen coordinates keep the stroke four pixels wide at every zoom.`
/// ⇒ 4 是**屏幕像素**、不随 `Zoom` 缩放，与本仓 `border_w` 直接吃 f32 宽（不乘 scale）一致。
///
/// ⚠ **真值来源特殊：别指望在本地 `_upstream` 复核**。
/// 本地 `_upstream/CrystalStudio` 是 `901dbe2f` 的**旧快照**（`MapViewport.cs` 604 行，
/// SHA256 `097A244E…6520A`），`hoverCellOutline` / `DarkBlue` / `DrawHoverCell` 三符号
/// **各 0 命中** —— 本符号是 `a78affdf`（617 行 / 40,168 B）才有的。
/// 真值取自 GitHub `a78affdf` 的 `MapViewport.cs:144`（Lead 2026-11-08 用 GitHub API 取证；
/// 修复代理 2026-11-08 用 `api.github.com/…/contents/CrystalStudio/MapViewport.cs?ref=a78affdf`
/// 独立复核，解码后 617 行、四处行号逐字一致）。
///
/// **历史引用（保留，勿据此回改）**：本值原为 **2.0**，出处是**截图实测**「金色选框描边 2px」
/// （`x=607..608` 连续两列金色像素），当时只是把装饰框的宽度挪用到悬停格边框。
/// 现在 2 与 4 冲突：**2 是截图采样、4 是源码真值**，冲突时源码优先 ⇒ 取 4。
///
/// ⚠ **已知位置差（本单未改，待产品裁定）**：上游 `Pen` 是**居中**笔画
/// （宽 4 ⇒ 边外 2 + 边内 2），本仓 `crate::paint::border_w` 只能**内贴**（4 全在边内）。
/// 要完全对齐居中语义需在绘制处 `r.expand(2.0)`（本仓已有同款先例：`dock_right.rs:665`
/// 小地图视口框 `Pen(Black,4)` 就是 `expand(2)` + 宽 4）。本单只按真值三要素
/// （色 `#00008B` / α 0.7 / 宽 4）落地，**未擅自改几何**，差异已在交付说明登记。
pub const CELL_HOVER_STROKE: f32 = 4.0;

/// 重渲时为平移/大物件预留的余量 **10 格**（960×640 世界像素）。
///
/// 来源：**实测推导，不是拍的**（2026-11-04，`0.map` + `--at=380,310` 客户端同视点比对）：
///
/// | 余量 | 本机渲出的窗口 | 与客户端 16×16 分块命中率 |
/// |---|---|---|
/// | 3 格 | 28×33 | **79.77%** ← 城门楼大片缺失 |
/// | 6 格 | 34×39 | 98.12% |
/// | 10 格 | 42×47 | **99.96%** |
/// | 14 / 20 格 | 50×55 / 62×67 | 99.96%（饱和，再多没用）|
///
/// 为什么需要这么大：`miredit-render` 只画**窗口内**的格子，而表层（物件）是
/// **底边钉格底**贴整帧的，一帧城门楼宽达 300~500 px（>6 格）—— 锚点落在窗口外
/// 但帧体压进可视区的物件，如果不在窗口里就整块不画。3 格（144 px）远远不够。
/// 10 格 = 480 px 已覆盖实测最大帧体；14/20 格不再提升，说明 10 格是真实阈值。
pub const CANVAS_RENDER_MARGIN_CELLS: u32 = 10;

/// 一次性重渲的格数上限 16000（≈ 16000×48×32×4 B ≈ 98 MB RGBA）。
///
/// 来源：推导（防御性护栏，不是语义限制）。正常 1500×950 窗口：
/// 1× 时可见约 21×26 格、加余量 41×46 = 1886 格；0.25× 时可见约 83×102 格、
/// 加余量 103×122 = 12566 格 —— 都在上限内。只有把窗口拉到 4K 再缩到 0.25×
/// 才可能触碰，触到会**如实报错并保持上一帧纹理**，不会去分配几百 MB 把界面卡死。
pub const CANVAS_RENDER_MAX_CELLS: u32 = 16_000;

/// 打开程序时的默认画布缩放 1.0（= 100%，1 像素 = 1 世界像素）。
pub const CANVAS_DEFAULT_SCALE: f32 = 1.0;

/// 缩放浮层（`−  100%  +  |  Fit Map  原始大小`）尺寸 **292**×40，距视口左/下各 18。
///
/// 来源分两段：
/// * **228×40** 是 `901dbe2f` 之前的截图量值：外框 (182,854)-(409,893)；
///   视口左 x=164 ⇒ 182-164 = 18；视口底 y=911 ⇒ 911-893 = 18；
/// * **+64** 来自上游 `901dbe2f` 在浮层尾部新增的 `OriginalSizeButton`
///   （`MainWindow.axaml:22`，`Click="ResetZoomClick"`）。按上游同一套
///   `Padding="10,4"` 反推：4 字中文 ≈ 48 + 左右内边距 20 ≈ 56~58 宽，
///   与前一个按钮之间留 8 ⇒ 228 + 8 + 56 = 292。**没有截图可量**（上游该版截图未随仓提供），
///   故这一段的依据是"上游控件顺序 + 同一条内边距规则"，已在模块文档 §五 记为推导值。
pub const ZOOM_PILL_W: f32 = 292.0;
pub const ZOOM_PILL_H: f32 = 40.0;
pub const ZOOM_PILL_OFF: f32 = 18.0;

/// 缩放浮层内部各元素的横向位置（相对浮层左边）。
/// 来源：截图浮层左 x=182，`−` 中心 204.5、`100%` 中心 255.5、`+` 中心 306.5、
/// 竖线 x=332、`Fit Map` 起 x=351 ⇒ 相对左边 22.5 / 73.5 / 124.5 / 150 / 169。
pub const ZOOM_MINUS_X: f32 = 22.5;
pub const ZOOM_PERCENT_X: f32 = 73.5;
pub const ZOOM_PLUS_X: f32 = 124.5;
pub const ZOOM_DIVIDER_X: f32 = 150.0;
pub const ZOOM_FIT_X: f32 = 169.0;

/// 「适应地图」按钮的可点带右边界（相对浮层左边）= 169 + 55。
///
/// 55 是截图里 `Fit Map` 文字宽度（5 字英文 ≈ 45）加两侧留白得到的实测值；
/// **必须显式写出来**：加「原始大小」之前，这条带是"一直延伸到浮层右边缘"的
/// （`ZOOM_PILL_W - 4`），加了新按钮后若还那么算，两个按钮的命中盒会重叠。
pub const ZOOM_FIT_R: f32 = 224.0;

/// 「原始大小」按钮的起点（相对浮层左边）= `ZOOM_FIT_R` 224 + 按钮间距 8。
///
/// 上游顺序：`−  100%  +  |  适应地图  原始大小`（`MainWindow.axaml:22` 的
/// `FitMapButton` 之后紧跟 `OriginalSizeButton`）。
pub const ZOOM_ORIGINAL_X: f32 = 232.0;

/// 「原始大小」按钮的可点带右边界（相对浮层左边）= 232 + 56（4 字中文 + 两侧留白）。
/// 浮层右边缘 = 292 - 4 = 288，正好给这条带留 4px 右内边距。
pub const ZOOM_ORIGINAL_R: f32 = 288.0;

/// `−` / `+` 字形字号 13（截图两符号明显大于 `100%` 那行正文）。
pub const ZOOM_SIGN_FONT: f32 = 13.0;

/// 页签**标题**的左内缩 **12** = `Padding` 左 8 + `Label.Margin` 左 4。
///
/// # 上游推导
///
/// * `EditorControls.axaml:59`（`PaletteTabItem`）：`<Setter Property="Padding" Value="8,7"/>`
///   ⇒ 左 **8**（`:60` 的 `Border#Chrome Padding="{TemplateBinding Padding}"` 把它透到内容盒）；
/// * `MainWindow.MapTabs.cs:54`：`map.Label.Margin = new Thickness(4, 2, 8, 2);` ⇒ 左 **4**；
///   ⇒ 标题文字左边缘 = 标签左沿 + 8 + 4 = **+12**。
///
/// ⚠ **只给标题用**。`×` 的右内缩是 **15**，见 [`TAB_CLOSE_INSET`] ——
/// 两者**不是同一个数，别合并**：`×` 外侧还多一道 Button 继承来的 `Margin=3`。
///
/// 消费点（全部是"标题左锚 / 标题裁剪盒"）：`canvas::dyn_labels` 的 `canvas.tab_title`、
/// `tab_strip::draw` 的标题裁剪盒。
///
/// # 右侧那 16（`Label.Margin` 右 8 + `Padding` 右 8）为什么没有折进这个常量
///
/// 上游标题的排版右界**不是**「标签右沿 −16」：`Label` 后面还跟着关闭钮，
/// 标题右界 = 关闭钮左沿 − `Margin` 右 8（`MapTabs.cs:54`），
/// 而关闭钮左沿 = 标签右沿 − 8（`Padding` 右）− 3（按钮 `Margin`）− 4（按钮 `Padding` 右）
/// − 字形 advance − 4（按钮 `Padding` 左）。
///
/// 本仓不逐标题量宽（`TAB_W` 固定 98），标题裁剪盒取
/// `r.right() - TAB_TITLE_INSET - TAB_LABEL_MARGIN_R` ⇒ 只扣「`Padding` 右 8 + 按钮
/// `Padding` 右 4 = 12」与「`Margin` 右 8」，**没有**再扣按钮 `Margin` 3、按钮左内边距 4
/// 与字形 advance（≈7.34，见 [`TAB_CLOSE_GLYPH_W`]）⇒ 比上游宽约 14.34px。
/// 这是**已知且偏宽松**的差异：标题左对齐 + `TAB_W` 固定，参考标题 `Untitled 1`（≈50px）
/// 在 66px 可用宽里触不到裁剪 ⇒ 当前不产生像素差。严格照抄要先做"逐标题量宽 + 内容自适应"，
/// 那会撑开 `Layout::tab_rect` 的 98×30 ⇒ 属另一步（见回执「未做/待拍板」）。
///
/// ⚠ **旧值 11 是截图量值**（`Untitled 1` 文字起 x=175、页签左 x=164 ⇒ 11），不是上游推导值；
/// A12-a 单按上游真值订正为 **12**（1px 的差来自像素量读的取整/字侧距）。
pub const TAB_TITLE_INSET: f32 = 12.0;

/// 关闭符 `×` 的**右内缩 15** = `Padding` 右 8 + Button `Margin` 3 + Button `Padding` 右 4。
///
/// # 上游推导（三段，漏掉中间那段就差 3px）
///
/// 1. `EditorControls.axaml:59`：`Padding="8,7"`（`:60` `Border#Chrome Padding="{TemplateBinding Padding}"`）
///    ⇒ 标签内容盒右沿 = 标签右沿 − **8**；
/// 2. `EditorControls.axaml:7`（`{x:Type Button}` 基主题）：`<Setter Property="Margin" Value="3"/>`
///    —— `MainWindow.MapTabs.cs:57` 的关闭钮**只覆盖** `Padding` / `Background` / `BorderThickness`，
///    **没有覆盖 `Margin`**；`:58` 换的 `QuietButton`（`EditorControls.axaml:17`）也是
///    `BasedOn="{StaticResource {x:Type Button}}"` 且只覆盖 `Background`/`BorderBrush`
///    ⇒ `Margin = 3` **照样生效** ⇒ 按钮盒右沿 = 内容盒右沿 − **3**；
/// 3. `MainWindow.MapTabs.cs:57`：`Padding = new Thickness(4, 0, 4, 0)`
///    ⇒ 字形右沿 = 按钮盒右沿 − **4**；
///
/// ⇒ `×` 字形右边缘 = 标签右沿 − 8 − 3 − 4 = **−15**。
///
/// # 为什么不能复用 [`TAB_TITLE_INSET`]（12）
///
/// 标题侧是 `8 + 4`（`Label.Margin` 左 4），`×` 侧是 `8 + 3 + 4`（Button `Margin` 3 + `Padding` 右 4）
/// —— **多了那道 3**。A12-a 单曾把两者当同一个数（用 12 锚 `×`），`×` 因此**右偏 3px**。
///
/// # 参考截图佐证（`layout/refs/crystalstudio-main-runtime-1500x950.png`）
///
/// 实测：标签框 `x164..262`（选中下划线 `x164..261` / `y102..103`）；
/// `×` 墨迹 `x240..244`（中心 242.0）。按 15 推得字形右沿 247、扣字侧距后落在 `x245` 附近
/// ⇒ 差 ≈1px（跨字体/跨渲染器量读的正常残差）；按 12 推则要落在 `x248` 附近 ⇒ 差 ≈4px。
///
/// 消费点（全部是"`×` 右锚 / 命中盒 / 悬停盒"）：`canvas::labels` 的 `canvas.tab_close`、
/// `tab_strip::{close_rect, close_button_rect, draw}`。
pub const TAB_CLOSE_INSET: f32 = 15.0;

/// 关闭符命中盒的**左右内边距 4** —— 上游 `MainWindow.MapTabs.cs:57`
/// `new Button { Content = "\u00d7", Padding = new Thickness(4, 0, 4, 0), … }`
/// ⇒ 左 4 / 上 0 / **右 4** / 下 0（B2 单，2026-11-08）。
///
/// 上下为 0 ⇒ 上游按钮的**高度 = 字形行高**（垂直方向靠默认模板的
/// `VerticalContentAlignment=Center`，`EditorControls.axaml:8`，在 30px 标签内居中）。
pub const TAB_CLOSE_PAD: f32 = 4.0;

/// 关闭符 `×` 在 [`TAB_FONT`]（11）下的**排版 advance = 7.34375**。
///
/// ⚠ **上游不可得**：上游是 Avalonia + `Segoe UI`（`App.axaml:83` 的 `FontFamily`），
/// 字形宽由那套字体决定，仓里没有它的度量表 ⇒ 无法逐字抄。
/// 本值取**本仓实际上屏字体**（`fonts::FONT_CANDIDATES[0]` = 等线 `Deng.ttf`）
/// 在 11px 下的 `layout_no_wrap("×")` 实测值（2026-11-08 实测输出 `GLYPH_W=7.34375`；
/// 用例 `关闭符命中盒宽度按字形advance加上游内边距` 把它钉死在 0.01 容差内，
/// 换字体或改 [`TAB_FONT`] 会立刻变红）。
///
/// 7.34375 = 7 + 11/32，二进制可精确表示，不是截断值。
pub const TAB_CLOSE_GLYPH_W: f32 = 7.343_75;

/// 关闭符 `×` 在 [`TAB_FONT`]（11）下的**字形行高 = 11**（= 字号本身）。
///
/// 与 [`TAB_CLOSE_GLYPH_W`] 同一次实测（`layout_no_wrap("×")` 的 `size().y = 11.0`）。
/// 上游 `Padding=(4,0,4,0)` 的上下是 0 ⇒ 上游那个 `Button` 的**高度就是它**（11），
/// 再靠默认模板的 `VerticalContentAlignment=Center`（`EditorControls.axaml:8`）
/// 在 30px 标签内垂直居中 ⇒ 悬停底的绘制盒 = 15.34375 × 11。
pub const TAB_CLOSE_GLYPH_H: f32 = 11.0;

/// 关闭符悬停底的圆角 **5** —— 上游 `EditorControls.axaml:9` 的 `Border#Chrome`
/// `CornerRadius="5"`（`QuietButton` 经 `BasedOn` 继承同一个模板，见 `:17`）。
pub const TAB_CLOSE_RADIUS: f32 = 5.0;

// ───────────────────────── 中列：地图列表面板（2026-11-05，上游 901dbe2f 移植） ─────────
//
// 为什么这一组不在截图里：`crystalstudio-main-runtime-1500x950.png` 是 `901dbe2f` **之前**
// 的运行截图，那时上游还没有地图列表面板。上游把它加在**左停靠区第一个**
// （`MainWindow.axaml.cs:343` 的 `docking.Add("Maps", …, Dock.Left, 2)`），
// 而本仓左停靠区是**固定几何、156 宽、四块面板已排满**（实测单元格检查器只剩 96px 余量，
// 见 `layout` 的 `左停靠区余量` 测试）—— 硬塞第五块面板会把检查器最后两行挤出面板。
// 所以本仓把这块面板做成**中列左侧的可开合列**：开时把画布往右推（不遮挡地图）、
// 关时画布矩形与旧版**逐像素相同**（默认态不变，见 `Layout::compute` 与 `compute_with`）。
//
// 尺寸依据：
// * 宽 268 —— 推导：一行要放下「文件名 + 大小」两列。文件名按 `n0104.map` 这类
//   9 字符 @11px ≈ 54px；大小按 `697.2 MB` 8 字符 @10px ≈ 48px；加左右内边距
//   (6+6) 与两列间距 8 ⇒ 122 是下限。取 268 是为了**长文件名**（如
//   `n0104_副本_2026.map`）也能整行显示，且不超过中列的 1/3（987/3 = 329）。
// * 行高 22 —— 与上游 `MapFileList.ItemContainerTheme` 的
//   `HeightProperty = 22.0`（`MainWindow.MapBrowser.cs:32-34`）逐值相同。
// * 字号 11 / 10 —— 上游列表项 `FontSize = 11`（`MapBrowser.cs:37`），
//   状态行 `FontSize = 10`（`MapBrowser.cs:12`）。

/// 地图列表面板宽度 268。
pub const MAPS_W: f32 = 268.0;

/// 面板内容左右内边距 6。来源：上游 `CreateMapBrowser` 的 `Grid { Margin = new Thickness(6) }`。
pub const MAPS_PAD: f32 = 6.0;

/// 面板内工具条高度 26（放「刷新」按钮；上游同样有一行 `mapFolderButton / mapRefreshButton`）。
pub const MAPS_TOOLBAR_H: f32 = 26.0;

/// 「刷新」按钮尺寸 58×20（文字 2 字 + 内边距）。
pub const MAPS_BTN_W: f32 = 58.0;
pub const MAPS_BTN_H: f32 = 20.0;

/// 列表行高 22。来源：上游 `MapFileList` 的 `ItemContainerTheme`
/// `Layoutable.HeightProperty = 22.0` / `MinHeightProperty = 22.0`（`MapBrowser.cs:32-34`）。
pub const MAPS_ROW_H: f32 = 22.0;

/// 行内文件名字号 11（上游 `MapBrowser.cs:37` 的 `FontSize = 11`）。
pub const MAPS_ROW_FONT: f32 = 11.0;

/// 行内大小字号 10（比文件名小一档：它是辅助信息，不该抢文件名）。
pub const MAPS_SIZE_FONT: f32 = 10.0;

/// 行内左右内边距 4（上游 `TextBlock { Margin = new Thickness(3, 0) }`）。
pub const MAPS_ROW_PAD: f32 = 4.0;

/// 文件名列与大小列之间的间距 8（两列不贴在一起，长的文件名才不会被数字糊住）。
pub const MAPS_COL_GAP: f32 = 8.0;

/// 底部状态行高 18（上游 `mapFileStatus` 是最后一行 `Auto`，`FontSize = 10`）。
pub const MAPS_STATUS_H: f32 = 18.0;

/// 底部状态行字号 10（上游 `MapBrowser.cs:12`）。
pub const MAPS_STATUS_FONT: f32 = 10.0;

/// 当前地图行左侧的金条宽 3（与图层行的「当前编辑层」金条同一视觉语言）。
pub const MAPS_CURRENT_BAR_W: f32 = 3.0;

/// 列表滚动条宽 6（与资源面板的滚动条同宽）。
pub const MAPS_SCROLLBAR_W: f32 = 6.0;

/// 每帧扫描预算 4 ms（**分帧扫描**的上限；见 `crate::maplist` 模块文档 §三）。
///
/// 为什么是 4 ms：60 fps 的帧预算 16.7 ms，扫描最多占 1/4，输入与绘制都不受影响；
/// 实测 1,097 张图的**整段**扫描约 10~14 ms（见 `maplist::tests::实测扫描耗时`），
/// 4 ms/帧 ⇒ 3~4 帧扫完，用户看不到停顿。
pub const MAPS_SCAN_BUDGET_MS: u64 = 4;

/// 每帧最多处理的目录项数 512（**第二道**限时闸门：目录项在缓存里时
/// `metadata()` 只要几微秒，纯按时间限会让单帧处理量不可预测）。
pub const MAPS_SCAN_MAX_ITEMS: usize = 512;

// ───────────────────────── 右停靠区 ─────────────────────────

/// 右停靠区内容左右内边距 10。来源：截图组合框左边 x=1146，右停靠区左边 x=1136 ⇒ 10；
/// 组合框右边 x=1482，右停靠区右边 x=1491 ⇒ 9（取 10）。
pub const RIGHT_PAD_X: f32 = 10.0;

/// TextBox / ComboBox 高 **34**。
///
/// 上游出处：`App.axaml:85` 的 `<Style Selector="ComboBox">` ⇒ `MinHeight = 34`
/// （同款 `Padding = 10,8,28,8`）。
///
/// 历史（原以截图为准，A13 订正）：本值原为 **32**，依据是参考截图量出的组合框行高
/// (1146,352)-(1482,383) = 32 行；当时注释记「与 `theme.metrics.combo_min_height` = 34
/// 相差 2，**以截图 32 为准**」。按用户「对齐上游」令改为 34 —— 且那个 `theme.metrics`
/// 字段在当前 `theme.rs` 里**已不存在**（全仓 grep `combo_min_height` 0 命中），
/// 上游 `App.axaml:85` 是唯一可回源的真值。
pub const COMBO_H: f32 = 34.0;

/// 搜索框高 **32**。
///
/// 上游出处：`EditorControls.axaml:49` 的 `ControlTheme x:Key="PaletteSearchBox"`
/// ⇒ `Height = 32`（同款 `Padding = 8,4,28,4`）。
///
/// 历史（原以截图为准，A14 订正）：本值原为 **29**，依据是参考截图
/// (1146,396)-(1482,424) = 29 行。
pub const SEARCH_H: f32 = 32.0;

/// 页签（Library / Favourites / Recently Used / Categories）行高 30。
/// 来源：截图 Palette 页签行 y=285..315（`#F5F6F7`），下划线在 y=312..315。
pub const PALETTE_TABS_H: f32 = 30.0;

/// Palette 页签字号 11 = `theme.metrics.tab_item_font_size`。
pub const TAB_FONT: f32 = 11.0;

/// 主按钮（Set Library Folder）尺寸 191×29。
/// 来源：截图金色像素 (1218,604)-(1408,632) ⇒ 191×29。
pub const PRIMARY_BTN_W: f32 = 191.0;
pub const PRIMARY_BTN_H: f32 = 29.0;

/// Minimap 内部地图块尺寸 200×138。
/// 来源：截图 row y=150，`#EFF2F5` 区段 `x1214..1413`（200 宽）；
/// 纵向 `y105..242`（138 高，由列 x=1480 的 `#F9FAFB` 区段 y100..246 内缩得到）。
pub const MINIMAP_MAP_W: f32 = 200.0;
pub const MINIMAP_MAP_H: f32 = 138.0;

/// Minimap 浅底内框（`#F9FAFB`）相对面板内容的左右/上下内缩 10。
/// 来源：截图 row y=150，内框 `x1146..1481`；面板内容左右边 = 1136/1491 ⇒ 各内缩 10。
pub const MINIMAP_INNER_INSET: f32 = 10.0;

/// Minimap 地图区在地图块内的**每边内缩** 6（块 200×138 ⇒ 图最多可用 188×126）。
///
/// 来源：上游 `MapMinimap.Avalonia.cs:19-21`（`_upstream/CrystalStudio/`，
/// 文件 sha256 前 8 位 `11f92126`）的 `MapBounds`：
///
/// ```csharp
/// double scale = Math.Min((ActualWidth - 12) / width, (ActualHeight - 12) / height);
/// return new((ActualWidth - width * scale) / 2, (ActualHeight - height * scale) / 2, width * scale, height * scale);
/// ```
///
/// `- 12` 是**两边合计**（可用区 = 控件每边内缩 6），居中由 `(ActualWidth - w*scale) / 2`
/// 承担 ⇒ 等价于「把世界像素 fit 进 `块.shrink(6)` 再居中」。
/// 本仓那个「控件」就是 [`MINIMAP_MAP_W`]×[`MINIMAP_MAP_H`] 的地图块：**块本身尺寸不变**
/// （布局与人眼基线不动），变的只是图在块内的可用区。
pub const MINIMAP_FIT_INSET: f32 = 6.0;

// ⚠ 已删除 `MINIMAP_VIEWBOX = 14.0`（原「视口指示小方框 14×14」）。
//
// 删除理由（审计 E10，2026-11-04）：那个方块的位置恒为 `(块左上+2, 块左上+2)`、
// 尺寸恒 14×14，与 `pan` / `scale` / 地图尺寸**全都无关** —— 它用几何形状宣称
// "你的视口在整图左上角"，属于假数据。现行几何由 `dock_right::minimap_rects` 按
// 真实地图格数 + `CanvasView::visible_cells` 现算，**不再需要任何尺寸常量**。
//
// 这段用 `//` 而不是 `///`：它讲的是**已经不存在**的常量，不是下面那个常量的文档。
// （原先是 `///`，紧跟一个空行再接 `MINIMAP_VIEWBOX_MIN_PX` 的文档 ⇒ clippy
//  `empty_line_after_doc_comments` 报"这段文档注释后面跟了空行"。）

/// 小地图视口框的**最小可见边长** 1px（纯栅格化下限，不是"编的尺寸"）。
///
/// 为什么需要：视口缩到极小（例如 4× 放大时可见区只有几格）时，按真实比例换算出的框
/// 边长会 **< 1px**，`Rect` 退化后连边框都画不出来 —— 那等于"看不见"，
/// 同样是失真。抬到 1px 只是承认"屏幕上最近的相邻像素就是 1px"，位置仍严格按真实值算。
/// 视口框**外层**黑边宽度（上游 `MapMinimap.Avalonia.cs:66` 的 `Pen(Black, 4)`）。
///
/// ⚠ 与上游的差异：上游笔画**居中**在矩形线上，本仓 [`crate::paint::border_w`] **内贴** ⇒
/// 环宽一致、位置差 1px（已如实登记在 `dock_right::draw_minimap`）。
pub const MINIMAP_VIEWBOX_OUTER_W: f32 = 4.0;
/// 视口框**内层**白边宽度（`MapMinimap.Avalonia.cs:66` 的 `Pen(White, 2)`）。
pub const MINIMAP_VIEWBOX_INNER_W: f32 = 2.0;

pub const MINIMAP_VIEWBOX_MIN_PX: f32 = 1.0;

/// 状态栏 `Cell 11, 22` 文本右端到 `● Libraries not loaded` 圆点左端的间距 52。
/// 来源：截图坐标文本末像素 x=1323、圆点起 x=1375 ⇒ 52。
pub const STATUS_COORD_GAP: f32 = 52.0;

/// 状态栏圆点直径 6，与右侧文字间距 8。
/// 来源：截图圆点 (1375..1380) = 6 宽；文字起 x=1388 ⇒ 间距 8。
/// `theme.metrics` 亦有 `LibraryDot` `rect.w` = 6。
pub const STATUS_DOT_D: f32 = 6.0;
pub const STATUS_DOT_GAP: f32 = 8.0;

/// 状态栏「左段 → 工具 → 笔刷」两段之间的间距 16。
///
/// 来源：**推导**（本单新增段，参考截图里没有"工具/笔刷"这两段可比对）。
/// 取 16 = [`STATUS_PAD_X`]（状态栏左右内边距），与整条状态栏的呼吸一致；
/// 实测可用宽度：左段 `0.map · 700×700 格` 约 130px，右端坐标段左边界约 1250px，
/// 两段合计最坏 **285.3px**（`工具：橡皮擦` 72.0 + `笔刷：地表图层 图号 65535 · 资源库 255` 213.3；
/// 2026-11-04 用 `paint::text_w` 实测，见状态栏改名「橡皮擦」后的复测），余量充足
/// （典型态落笔段仍余 572.5px）。
pub const STATUS_SEG_GAP: f32 = 16.0;

// ───────────────────────── 关闭拦截模态（本单新增控件） ─────────────────────────

/// 关闭拦截模态的底板宽 **520**。
///
/// 来源：**上游真值** —— 上游这个模态就是系统 `MessageBox`：
/// `EditorDialogs.cs:79` `var dialog = new EditorWindow { Title = …, Width = 520,
/// SizeToContent = SizeToContent.Height, CanResize = false, … }`。
/// 本仓没有 `MessageBox`（自绘），所以取它的 `Width = 520` 作底板宽。
///
/// ⚠ 旧值 560 是**推导**（"文案 13px 下约 340px 宽，留左右各 110 呼吸"），
/// 上游取证后订正为 520；旧的 560 已作废，不要再引用。
pub const MODAL_W: f32 = 520.0;

/// 关闭拦截模态的底板高 **92**。
///
/// ⚠ 上游是 `SizeToContent = SizeToContent.Height`（**高随内容**），没有定值；
/// 92 是本仓自绘模态的**推导**值（见下方 [`MSG_BOX_PAD`] 的说明与 `app::close_modal_rect`）。
pub const MODAL_H: f32 = 92.0;

/// 关闭拦截模态（= 上游 `MessageBox`）的内容内边距 **22**。
///
/// 来源：**上游真值** `EditorDialogs.cs:81`
/// `var content = new StackPanel { Margin = new Thickness(22), Spacing = 18 };`。
///
/// ⚠ **必须与 [`OPEN_DLG_PAD`]（24）分开写**：上游这四个自定义对话框是
/// `Margin = new(24)`，`MessageBox` 是 `22` —— **本来就是两个数**。
/// 共用 24 会把这个差异悄悄抹掉（那样"对齐上游"就只是对齐了个大概）。
///
/// 本仓自绘的关闭模态把文案**居中**摆放（上游是系统提供的按钮行，见
/// `app::close_modal_choice_rects` 的「上游无对应物」登记），
/// 所以这 22 的等价物是"文案必须放得进 `底板宽 - 2×22`"，由
/// `app::tests::关闭拦截模态放得下文案` 钉住。
pub const MSG_BOX_PAD: f32 = 22.0;

/// 模态文字字号 13（与标题栏同档；它是一句需要看清的警告，不用 11px 小字）。
pub const MODAL_FONT: f32 = 13.0;
/// 模态底板圆角 6（与缩放浮层同档，见 [`crate::canvas`] 的 `fill_round(.., 6.0, ..)`）。
pub const MODAL_RADIUS: f32 = 6.0;

// ───────────────────────── Active Asset 面板卡片 ─────────────────────────

/// Active Asset 内容卡片：334 宽 × 109 高，相对面板内容左上偏移 (11, 16)。
/// 来源：截图 `#EFF1F3` 卡片 `x1147..1480`、`y787..895`；
/// 面板内容左边 x=1136、上边 y=771 ⇒ 偏移 (11, 16)。
pub const AA_CARD_OFF: (f32, f32) = (11.0, 16.0);
pub const AA_CARD_W: f32 = 334.0;
pub const AA_CARD_H: f32 = 109.0;

/// 卡片内预览框：188 宽 × 88 高，相对卡片左上偏移 (10, 10)。
/// 来源：截图 `#F7F7F8` 区段 `x1157..1344`（188 宽）。
pub const AA_PREVIEW_OFF: f32 = 10.0;
pub const AA_PREVIEW_W: f32 = 188.0;
pub const AA_PREVIEW_H: f32 = 88.0;

/// Palette 分页按钮 `←` / `→` 尺寸 31×22，分页行高 22。
/// 来源：截图 row y=703，`#F2F3F5` 区段 `x1150..1180` 与 `x1447..1477`（各 31 宽）。
pub const PAGER_BTN_W: f32 = 31.0;
pub const PAGER_ROW_H: f32 = 22.0;

/// 分页按钮相对 Palette 内容区左右的内缩 4。
pub const PAGER_BTN_INSET: f32 = 4.0;

/// 分页按钮里箭头字形的字号 12（`←` / `→` 用真字形时的显示字号）。
pub const PAGER_ARROW_FONT: f32 = 12.0;

/// Palette 页签行左内缩 6（截图第一个页签文字起 x=1152，内容区左右内边距右界 x0=1146）。
pub const PALETTE_TAB_INSET_L: f32 = 6.0;

/// Palette 页签下划线高 3（截图 y=312..315 的金色）。
pub const PALETTE_UNDERLINE_H: f32 = 3.0;

/// Palette 页签下划线左右各外扩 6（宽 = 文字宽 + 12）。
pub const PALETTE_UNDERLINE_PAD: f32 = 6.0;

/// Palette 顶部 `资源库` / `帧 N` 标签字号 11。
pub const PALETTE_LABEL_FONT: f32 = 11.0;

/// 搜索框文字与右侧清除符的左右内缩 12（截图占位文字起 x=1158，输入框左 x=1146）。
pub const SEARCH_TEXT_INSET: f32 = 12.0;

/// Active Asset 预览框右边 → 右侧三行文字左边的间距 8。
pub const AA_TEXT_GAP: f32 = 8.0;

/// Active Asset 右侧三行文字字号 10.5：
/// 卡片右半区可用宽 = 卡片宽 334 - 预览内缩 10 - 预览宽 188 - 间距 8 = 128px，
/// 参考截图里 `Choose a Terrain Pattern` 也就压到约 110px 宽。
pub const AA_LINE_FONT: f32 = 10.5;

/// 右侧三行文字相对卡片顶的纵向位置 22 / 44 / 68。
pub const AA_LINE_CAPTION_Y: f32 = 22.0;
pub const AA_LINE_LABEL_Y: f32 = 44.0;
pub const AA_LINE_PIN_Y: f32 = 68.0;

/// `收藏` 行星形半径 4.5；星心 → 文字左边缘 12。
pub const AA_STAR_R: f32 = 4.5;
pub const AA_STAR_TEXT_OFF: f32 = 12.0;

// ───────────────────────── 标题栏 / 菜单栏 ─────────────────────────

/// 标题栏应用图标：边长 15，距内容区左边 8（截图 x=16..30，内容区原点 8）。
pub const TITLE_ICON_X: f32 = 8.0;
pub const TITLE_ICON_SIZE: f32 = 15.0;

/// 标题栏文字起点距内容区左边 29（截图标题文字起 x=37，37 - 8 = 29）。
pub const TITLE_TEXT_X: f32 = 29.0;

/// 标题栏字号 13：截图标题文字高约 10px（y=8..18），按 13px 比例字号还原。
/// （JSON `theme.font_size` = 12 是全局正文；标题栏未单独给出，取与菜单栏同档 13。）
pub const TITLE_FONT: f32 = 13.0;

/// 标题栏右侧 3 个窗口按钮：每个 46 宽 × 31 高（= 标题栏高），紧贴右边缘排布。
/// 来源：截图字形中心 x=1377.5 / 1422.5 / 1468.5（间距 45~46），
/// 最右按钮右边 = 内容区右边 1491 ⇒ 1491 - 46 = 1445，依次左推。
pub const CAPTION_BTN_W: f32 = 46.0;

/// 三个窗口按钮字形（`—` / `□` / `×`）的字号 13。
pub const CAPTION_FONT: f32 = 13.0;

/// 菜单栏左内边距 18。来源：截图首个菜单项 `File` 文字起 x=38，
/// 项内 padding 12 ⇒ 项左边 26 ⇒ 26 - 内容区原点 8 = 18；
/// 与 JSON `theme.metrics.titlebar_margin_x` = 18 一致。
pub const MENU_PAD_X: f32 = 18.0;

/// 菜单项内左右 padding 12。来源：截图相邻菜单项文字间距恒为 25~26（`File` 末 55 → `Edit` 起 80）。
pub const MENU_ITEM_PAD_X: f32 = 12.0;

/// 菜单栏字号 13。来源：截图菜单文字高约 9~10px（y=46..55），按 13px 比例字号还原。
pub const MENU_FONT: f32 = 13.0;

/// 正文/标签字号 12。来源：JSON `theme.font_size` = 12；
/// 截图层列表行文字高约 9px、行高 26，12px 字号与之相符。
pub const BODY_FONT: f32 = 12.0;

/// 状态栏字号 12（与正文同档；截图状态栏文字高约 9px）。
pub const STATUS_FONT: f32 = 12.0;

/// 文字裁剪盒的呼吸余量 2。
///
/// 有些标签的裁剪盒是**按测量出的文字宽**给出的（Palette 页签、状态栏右对齐文字），
/// 盒子宽度恰好等于排版宽度 —— 一旦字形右边缘带抗锯齿溢出半像素，
/// 就会被裁掉一刀，"恰好放下"和"被切掉"在像素层面分不清。
/// 统一外扩 2px：既不影响布局（页签间距 10、状态栏右侧还有 52px 空隙），
/// 又让"文字像素右边界 vs 裁剪盒右边界"的核对有可判读的余量。
pub const TEXT_CLIP_SLACK: f32 = 2.0;

// ───────────────────────── Palette 面板内部纵向偏移 ─────────────────────────
//
// 全部「相对 Palette 内容区顶边」计量。参考截图里 Palette 内容区顶边 = y285。

/// 页签行：偏移 0，高 `PALETTE_TABS_H`(30)。截图页签文字 y=294..306，行区 y=285..315。
pub const PALETTE_TABS_OFF: f32 = 0.0;

/// 页签之间水平间距 10。来源：截图 `Library` 末 x=1190 → `Favourites` 起 x=1200。
pub const PALETTE_TAB_GAP: f32 = 10.0;

/// `Terrain Patterns` / `0 sets` 标签行：偏移 45。来源：截图文字 y=330..340（285 + 45 = 330）。
pub const PALETTE_LABEL_OFF: f32 = 45.0;

/// 组合框：偏移 67。来源：截图组合框 (1146,352)-(1482,383)（285 + 67 = 352）。
pub const PALETTE_COMBO_OFF: f32 = 67.0;

/// 搜索框：偏移 111。来源：截图搜索框 (1146,396)-(1482,424)（285 + 111 = 396）。
pub const PALETTE_SEARCH_OFF: f32 = 111.0;

/// 空态网格图标：偏移 203、尺寸 20×20。来源：截图图标区 (1308,488)-(1330,508)。
pub const PALETTE_EMPTY_ICON_OFF: f32 = 203.0;
pub const EMPTY_ICON_SIZE: f32 = 22.0;

/// 空态标题 `Your Assets Live Here`：偏移 248。来源：截图文字 y=533..543。
pub const PALETTE_EMPTY_TITLE_OFF: f32 = 248.0;

/// 空态说明两行：偏移 268。来源：截图 `Choose your Crystal library folder once.` y≈553。
pub const PALETTE_EMPTY_DESC_OFF: f32 = 268.0;

/// 空态主按钮 `Set Library Folder`：偏移 319。来源：截图金色按钮 y=604..632（285 + 319 = 604）。
pub const PALETTE_BTN_OFF: f32 = 319.0;

/// 分页行距面板内容区底边的内边距 10。来源：截图分页行 y=695..717，面板内容区底 y=747。
pub const PALETTE_BOTTOM_PAD: f32 = 10.0;

/// 空态说明文字的行高 16（两行 553/569）。
pub const EMPTY_DESC_LINE_H: f32 = 16.0;

// ───────────────────────── Palette 面板：图块网格（本单新增） ─────────────────────────
//
// 这一段全部是**推导值**（参考截图的资源面板是空态，没有网格可量），
// 每条都写明由哪个既有常量推出来 —— 没有一条是"看着差不多"。

/// 网格顶部相对 Palette 内容区上边 **150**。
///
/// 推导（A14 之前的原式）：搜索框底 = `PALETTE_SEARCH_OFF`(111) + `SEARCH_H` = 140，
/// 再留 10 的呼吸 ⇒ 150。
///
/// ⚠ **A14 把 `SEARCH_H` 由 29 改成 32 后，原推导不再成立**：搜索框底变成 143，
/// 到本值 150 的实际间隙只剩 **7**（不再是推导出的 10）。本值**未擅自跟着改成 153** ——
/// 那会再挪动整个网格与分页行 3px，属产品裁定，已列入回执待裁。
/// 现值安全：143 < 150，搜索框与网格**不重叠**。
pub const PALETTE_GRID_OFF: f32 = 150.0;

/// 每行 **8** 格。
///
/// 推导：内容区可用宽 = `RIGHT_DOCK_W(356) - 2 × RIGHT_PAD_X(10)` = **336**，
/// 336 / 8 = **42** 整除 ⇒ 8 列刚好铺满、不留半格。
pub const PALETTE_GRID_COLS: usize = 8;

/// 单格边长 **42**（= 336 / 8，见上一条）。格子是正方形，缩略图等比缩进格内。
pub const PALETTE_CELL: f32 = 42.0;

/// 缩略图相对格子四周的内缩 **2**：留出 1px 格线 + 1px 呼吸，
/// 于是缩略图可用区 = 42 - 2×2 = 38×38（96×64 的地表帧按 1.5:1 缩成 38×25）。
pub const PALETTE_THUMB_PAD: f32 = 2.0;

/// 网格底与分页行之间留 **6**（分页行本身 22 高、再往下 `PALETTE_BOTTOM_PAD` 10）。
///
/// 由此得可见高度 = 465（内容高）- 150 - 22 - 10 - 6 = **277**，
/// 可见行数 = 277 / 42 = **6 行**（252 px，余 25 px 不铺第 7 行——半行会看起来像渲染错位）。
pub const PALETTE_GRID_GAP: f32 = 6.0;

/// 库下拉弹层每行高 **20**（比格子里的小字行高 16 略宽，便于点）。
pub const PALETTE_COMBO_ROW_H: f32 = 20.0;

/// 库下拉弹层最多显示 **8** 行（= 160 px，向下不越过分页行）；更多的库靠滚轮翻。
pub const PALETTE_COMBO_ROWS: usize = 8;

/// 素材面板里"选中格"的金色描边宽 **2**。
///
/// ⚠ 这里原写作「与画布悬停格 `CELL_HOVER_STROKE` 同档」，**该说法 2026-11-08 已过期**：
/// 悬停框按上游 `MapViewport.cs:144`（`a78affdf`）订正为 **4**，本值**不动**——
/// 它属素材面板选中态，上游没有对应真值，不该跟着悬停框变粗（4 会让 84×… 的缩略图格子发糊）。
pub const PALETTE_SELECT_STROKE: f32 = 2.0;

/// 网格与分页行之间那条**提示行**的高度 **16**。
///
/// 它的用途：搜索失败/拒绝设笔刷/空帧计数这些"必须说出来"的话要有地方显示，
/// 又不能压在缩略图上。16px 是 `PALETTE_LABEL_FONT(11)` 的一行高度（含 3px 呼吸）。
pub const PALETTE_NOTE_H: f32 = 16.0;

// ───────────────────────── Palette 面板：「分类」页签 = 印章网格（本单新增） ─────────────────────────
//
// 纵向三段的偏移（页签 / 标签行 / 下拉 / 搜索 / 网格顶 / 提示行 / 分页行）**完全复用**
// 上面 Palette 的那一套（`PALETTE_*_OFF`）：两页签共用同一个面板骨架，只有"格子怎么排"不同。
// 下面只定义**格子本身**与**每页条数**，每条都写明由哪个既有常量推出来。

/// 印章网格每行 **4** 枚。
///
/// 推导：内容区可用宽 = `RIGHT_DOCK_W(356) - 2 × RIGHT_PAD_X(10)` = **336**，336 / 4 = **84** 整除
/// ⇒ 4 列刚好铺满。取 4 而不是资源库页的 8：印章缩略图是 **46×32**（任务书 §三.2 指定）
/// 再加下方一行名称，8 列时每列只剩 42 px、名称会挤成两三个字，失去辨识度。
pub const STAMP_GRID_COLS: usize = 4;

/// 单格宽 **84**（= 336 / 4，见上一条）。
pub const STAMP_CELL_W: f32 = 84.0;

/// 缩略图宽 **46**（任务书 §三.2 指定；`miredit_render::StampRenderOptions::thumbnail(46, 32)`）。
pub const STAMP_THUMB_W: f32 = 46.0;

/// 缩略图高 **32**（同上）。46×32 与一格地表帧 48×32 同比例，缩下来不糊。
pub const STAMP_THUMB_H: f32 = 32.0;

/// 缩略图相对格子四周的内缩 **3**：留 1px 格线 + 2px 呼吸（与 `PALETTE_THUMB_PAD` 同思路，
/// 但格子更矮，2 会显得贴边）。
pub const STAMP_CELL_PAD: f32 = 3.0;

/// 名称行与缩略图之间的间隙 **2**。
pub const STAMP_NAME_GAP: f32 = 2.0;

/// 名称行字号 **11**（= `PALETTE_LABEL_FONT`，与资源库页的小字同档，不另立字号）。
pub const STAMP_NAME_FONT: f32 = 11.0;

/// 名称行行高 **13**（11px 字号 + 2px 呼吸；比 `PALETTE_NOTE_H(16)` 紧，因为格子里只有一行）。
pub const STAMP_NAME_H: f32 = 13.0;

/// 单格高 **53** = 上内缩 3 + 缩略图 32 + 间隙 2 + 名称行 13 + 下内缩 3。
///
/// 由此得可见行数 = 网格高 277 / 53 = **5 行**（265 px，余 12 px 不铺第 6 行 ——
/// 与资源库页"半行看起来像渲染错位"同一条纪律，见 `PALETTE_GRID_GAP` 的注释）。
pub const STAMP_CELL_H: f32 = 53.0;

/// 印章页签**每页 96 枚**。
///
/// 推导：`STAMP_GRID_COLS(4) × 24 行 = 96` —— 整行整列，一页正好 24 行。
/// 一屏只能看到 5 行（20 枚），所以一页要滚 4.8 屏才看完；取 96 而不是"翻一屏"的理由：
/// ① 印章目录 9,159 枚，按屏翻会翻出 458 页，"第 N/M 页"失去定位意义；
/// ② 96 与资源库页签的"每屏 48 格"成整倍数关系（2 屏），两个页签的翻页手感一致。
///
/// ⚠ 与资源库页的**分页按钮语义不同**：那边是"上下翻一屏"，这边是"上/下一页"（整页 96）。
pub const STAMP_PAGE_SIZE: usize = 96;

/// 印章缩略图纹理缓存上限 **256** 张。
///
/// 取值依据：一张 46×32 RGBA = 5.9 KB ⇒ 256 张 ≈ 1.5 MB 显存 + 纹理对象开销。
/// 一页 96 枚、一屏 20 枚 ⇒ 256 覆盖 **2.6 页**的滚动历史；
/// 而全库 9,159 枚若全上传要 54 MB（本单明确禁止，见任务书 §三.3）。
pub const STAMP_THUMB_CACHE_CAP: usize = 256;

/// 印章分类下拉弹层最多显示 **8** 行（= 160 px，与 `PALETTE_COMBO_ROWS` 同档；
/// 21 个选项靠滚轮翻）。
pub const STAMP_COMBO_ROWS: usize = 8;

/// 印章信息两行（「当前资源」面板）的字号 **10.5**（= `AA_LINE_FONT`，不另立字号）。
///
/// ⚠ 该面板文字区只有 **128 px** 宽（`AA_CARD_W(334) - AA_PREVIEW_OFF(10) - AA_PREVIEW_W(188)
/// - AA_TEXT_GAP(8)`），而印章信息必须容下 `25 格 · 包围盒 (-2,-1,3,4)` 这一串；
/// 因此**拆成两行**（名称+分类 / 格数+包围盒），并由
/// `stamp_panel_tests::印章信息两行都放得进文字区` 用真实字体量宽把关。
pub const STAMP_AA_FONT: f32 = 10.5;

// ───────────────────── 画布叠加层：矩形选区框 / 印章轮廓（2026-11-05 新增） ─────────────────────
//
// ⚠ 这一组**没有截图真值**：官方参考截图里既没有选区也没有印章轮廓
// （`crystalstudio-main-runtime-1500x950.png` 是空画布态）。所以**线宽 / 虚线节拍**这些数
// 都是**推导值**，推导依据写在每一条上；将来若拿到带选区/轮廓的官方截图，按截图改这里即可，
// **不许**在绘制代码里就地写字面量。
// 唯一例外是**填充色**：它有官方源码真值（见下方 `selection_fill` 那条），不是推导值。

/// 选区虚线的线宽 **1.5**。
///
/// 推导：比悬停框**细一档** —— 悬停框是"当前格"（要显眼；其值 2026-11-08 已按上游
/// `MapViewport.cs:144` 由 2.0 订正为 4.0，这里比较的是"粗细档位"而不是具体数值），
/// 选区框是"一块区域"（要能看清框内的地图），1.5 在 100% 缩放下正好落在 1~2 物理像素之间，
/// 既不会像 1px 那样被抗锯齿吃掉，也不会像 2px 那样盖住格子边。
pub const SEL_STROKE_W: f32 = 1.5;

/// 选区虚线的**实线段长** **6**。
///
/// 推导：一格是 48×32（`CELL_W`/`CELL_H`）。6 + 4 = 10 一段 ⇒ 横向一格约 4.8 段、
/// 纵向约 3.2 段，肉眼一眼能数出"这是虚线"；取 8 以上在 32px 高的格子上只剩 2 段，
/// 看着更像实线。
pub const SEL_DASH_LEN: f32 = 6.0;

/// 选区虚线的**空档长** **4**（理由同上：6 + 4 = 10 一段）。
pub const SEL_GAP_LEN: f32 = 4.0;

// 选区半透明填充的颜色**不在本文件**：真值在 `theme::Palette::selection_fill`。
//
// 官方 `CrystalStudio/MapViewport.cs:295` 用的是**固定蓝**
// `Color.FromArgb(30, 110, 150, 255)`（α = 30），本仓按预乘落成 `rgba_pm(12, 17, 30, 30)`
// —— 与主题（浅色 / 深色）**无关**。此处原 `SEL_FILL_ALPHA = 40` 及其
// "颜色本体取强调金 `#CC9629`" 的注释都是误记（金 × α40 是自造值，`theme.rs` 已订正为官方蓝），
// 故整条删除，避免有人再照这个 α 去乘一次。

/// 印章轮廓框的线宽 **2.0**。
///
/// ⚠ 这里原写作「= `CELL_HOVER_STROKE`」，**该等式 2026-11-08 已不成立**：
/// 后者按上游 `MapViewport.cs:144` 订正为 4.0，本值**不变**——印章轮廓是"待放置预览框"，
/// 上游没有它的真值，不该跟着悬停框变粗。
///
/// 推导：轮廓是**只有边框、不画任何像素**的预览（任务书 §三.5 明令"不许画假像素"），
/// 所以它必须比选区虚线更粗一点，才能在"没有填充色"的情况下被认出来。
pub const STAMP_PREVIEW_STROKE_W: f32 = 2.0;

/// 画布左上角「待放置」徽标的高度 **20**。
///
/// 推导：徽标里是 `STATUS_FONT(12)` 一行字，上下各留 4 px 呼吸 ⇒ 12 + 8 = 20。
/// 取 20 而不是更高的 24：它跟着鼠标走，太高会盖住落点附近的两行格。
pub const ARMED_BADGE_H: f32 = 20.0;

/// 徽标的左右内边距 **6**（= `PALETTE_TAB_INSET_L`，与面板小标签同档）。
pub const ARMED_BADGE_PAD_X: f32 = 6.0;

/// 徽标的圆角半径 **4**（与 `PALETTE_SELECT_STROKE` 同一档视觉重量）。
pub const ARMED_BADGE_RADIUS: f32 = 4.0;

// ───────────────────────── 告警黄条（2026-11-05 折行改造） ─────────────────────────
//
// ⚠ 黄条本体是宿主新增的控件（无官方截图依据，见 `banner` 模块文档 §三）。
// 上一版是**单行 34px 固定高**；2026-11-05 起 `Shanda2012` 的 `editable_caveat` 约 250 字
// ⇒ 必须折行、高度自适应，否则不是横向溢出就是把关键词截掉。

/// 告警黄条的**单行**高度 **18**（12px 字 + 上下各 3 呼吸）。
///
/// 与上一版的 `BANNER_H = 34` 的关系：`34 = 2 × 8（上下内边距）+ 18 × 1 行`
/// ⇒ 单行文案的几何与上一版**逐像素相同**（既有截图与用例都不受影响）。
pub const BANNER_LINE_H: f32 = 18.0;

/// 告警黄条的上下内边距 **8**。
pub const BANNER_PAD_Y: f32 = 8.0;

/// 告警黄条允许占用的最大画布高度比例 **0.45**。
///
/// 为什么是 45%：告警要"看得见但不至于把地图全遮住"。
/// 实测 250 字的 caveat 在 1500×950 下折 4 行 = 88px（占画布 815 的 10.8%），
/// 在 1100×720 下折 6 行 = 124px（占 600 的 20.7%）⇒ **正常情况远不到上限**，
/// 只有"画布被压成一条"时才会触发（那时才允许在最后一行加省略号）。
pub const BANNER_MAX_VIEWPORT_RATIO: f32 = 0.45;

// ───────────────────────── 悬停地砖信息浮层（视图 → 地砖信息悬浮） ─────────────────────────
//
// 这一组**几乎全是官方真值**（不是推导）：上游 `MainWindow.axaml:86-91` 把浮层写成
// `<Border x:Name="CellHoverInfo" Width="350" Padding="12" CornerRadius="6" …>`，
// 位置算式在 `MainWindow.CellHover.cs:47-53`（+16 右 / +8 下、越界翻左、四边各夹 8），
// 最大高在 `:45`（`Bounds.Height - 16`）。逐条注在下面。

/// 浮层宽度 **350**（官方 `MainWindow.axaml:87` `Width="350"`）。
///
/// 屏幕比它窄时由 [`crate::cell_hover`] 收到「屏宽 − 2×8」为止 —— 官方那侧有
/// `ClipToBounds="True"` 的 Canvas 兜着，本仓没有裁剪盒，只能自己收窄。
pub const HOVER_W: f32 = 350.0;

/// 浮层内边距 **12**（官方 `Padding="12"`）。
pub const HOVER_PAD: f32 = 12.0;

/// 浮层圆角 **6**（官方 `CornerRadius="6"`）。
pub const HOVER_RADIUS: f32 = 6.0;

/// 光标 → 浮层左边缘的水平偏移 **16**（官方 `CellHover.cs:49` `left = position.X + 16`）。
pub const HOVER_OFFSET_X: f32 = 16.0;

/// 光标 → 浮层上边缘的垂直偏移 **8**（官方 `CellHover.cs:53` `position.Y + 8`）。
pub const HOVER_OFFSET_Y: f32 = 8.0;

/// 浮层与窗口四边的最小距离 **8**（官方 `:51-53` 的 `Math.Clamp(…, 8, …)`）。
pub const HOVER_EDGE_PAD: f32 = 8.0;

/// 浮层可用最大高 = 窗口高 − **16**（官方 `:45` `MaxHeight = Bounds.Height - 16`）。
pub const HOVER_MAX_H_MARGIN: f32 = 16.0;

/// 坐标 / 可行走两行（官方 `FontWeight="SemiBold"`、字号取正文档）的字号 **12**。
pub const HOVER_HEAD_FONT: f32 = 12.0;

/// 明细行字号 **11**（官方 `MainWindow.axaml:91` `FontSize="11"`）。
pub const HOVER_DETAIL_FONT: f32 = 11.0;

/// 坐标 / 可行走两行的行高 **18**（12px 字 + 上下 3 呼吸，与 [`BANNER_LINE_H`] 同档）。
pub const HOVER_HEAD_LINE_H: f32 = 18.0;

/// 明细行行高 **16**（官方 `MainWindow.axaml:91` `LineHeight="16"`）。
pub const HOVER_DETAIL_LINE_H: f32 = 16.0;

// ───────────────────────── 菜单下拉弹层（2026-11-05 新增） ─────────────────────────
//
// ⚠ **这一组没有官方截图真值**：参考截图（`crystalstudio-main-runtime-1500x950.png` /
// `crystalstudio-new-zh.png`）都是**菜单收起**态，官方也没给过展开态的图。
// 所以数值一律**按官方 Avalonia Fluent 菜单的度量推导**，推导链写在这里（不是"看着差不多"）：
//
// * 行高：Fluent 的 `MenuItem` 最小高是 32（字号 14）⇒ 本仓菜单字号 13（`MENU_FONT`，
//   截图实测）⇒ 等比 32 × 13/14 ≈ 29.7，再按"13px 字 + 上下各 5.5 呼吸"取 **24**；
//   取 24 而不是 30 的理由：视图菜单有 **18 项**，30 会让弹层高 540px（超出 950 窗口的可用高度）。
// * 左右内边距 12 = `MENU_ITEM_PAD_X`（与菜单栏同一档，弹层文字与菜单栏文字左对齐观感一致）。
// * 分隔线占 **9px**（1px 线 + 上下各 4px），Fluent 的 `Separator` 同款做法。
// * 勾选列 **18px**：没有勾选的项也占这一列，保证所有项文字左对齐（不因有无勾选而参差）。

/// 菜单弹层的行高 **24**（推导见本节注释）。
pub const MENU_POPUP_ROW_H: f32 = 24.0;

/// 菜单弹层的上下内边距 **4**。
pub const MENU_POPUP_PAD_Y: f32 = 4.0;

/// 菜单弹层的左右内边距 **12**（= `MENU_ITEM_PAD_X`）。
pub const MENU_POPUP_PAD_X: f32 = 12.0;

/// 菜单弹层里一条分隔线占的高度 **9**（1px 线 + 上下各 4）。
pub const MENU_POPUP_SEP_H: f32 = 9.0;

/// 菜单弹层的最小宽度 **168**（放得下最长的单字项「实际大小（100%）(A)」之外的余量由测量决定）。
pub const MENU_POPUP_MIN_W: f32 = 168.0;

/// 菜单项文字与右侧快捷键之间的**最小**间距 **28**（不足则弹层加宽）。
pub const MENU_POPUP_SHORTCUT_GAP: f32 = 28.0;

/// 勾选列宽 **18**（无勾选也占位，保证文字左对齐）。
pub const MENU_POPUP_CHECK_W: f32 = 18.0;

/// 菜单弹层的 1px 边框宽。
pub const MENU_POPUP_BORDER: f32 = 1.0;

/// 菜单弹层字号 **12**（= `BODY_FONT`；比菜单栏 13 小一档，弹层才不显笨）。
pub const MENU_POPUP_FONT: f32 = 12.0;

/// 菜单弹层圆角 **4**（与 `ARMED_BADGE_RADIUS` 同档）。
pub const MENU_POPUP_RADIUS: f32 = 4.0;

// ───────────────────── 键盘快捷键浮层 / 打开地图模态（2026-11-05 新增） ─────────────────────

/// 快捷键浮层宽 **460**。
///
/// 上游 `MainWindow.axaml.cs:1233`
/// `new EditorWindow { Title = LocalText.T("InputState Shortcuts"), Width = 460, Height = 680, … }`。
///
/// ⚠ 2026-11-05 本仓自定 560（无上游依据）；2026-11-08 回源订正为上游 **460**。
/// 460 装得下（两条都由 `menu::shortcuts::tests::每行两栏都放得下` 用真排版宽现算守）：
/// - 键列 170 ≥ 实测最宽键 **106.56**（`Ctrl+Shift+Z / Ctrl+Y`）；
/// - 说明列 = 460 − 2×28 − 170 = **234** ≥ 实测最宽说明 **190.00**
///   （`取消选区 / 退出待放置态 / 关闭弹层`）。
pub const SHORTCUTS_W: f32 = 460.0;

/// 快捷键浮层高 **901**。
///
/// # ⚠ 这是与上游的**显式偏离**（不是抄错，也不是"看着差不多"）
///
/// 上游 `MainWindow.axaml.cs:1233` 是 `Height = 680`。本仓取 **901**，原因只有一个：
/// **内容量不同** —— 上游那张表是 **19 行单列**（`MainWindow.axaml.cs:1242` 的
/// `new[] { … }` 逐条数得 19 项；`:1246` `DockPanel` + `:1248`
/// `DockPanel.SetDock(shortcut, Dock.Right)` ⇒ **无组名、无说明行、键靠右**）；
/// 本仓是 **28 行 = 6 个组名 + 22 条绑定**，另有 1 行说明。
///
/// 上游 680 是「19 行单列」这一内容量下的**结果**，不是规格（它 `CanResize = false`，
/// 若它也有 28 行同样会加高）。所以本仓**保上游的排版参数、让高度随内容走**。
///
/// # 算式（两侧都是实测值 / 上游字面量，可复算）
///
/// ```text
/// 标题行高   =  41 = 实测标题行高 23（本仓字体 @ 上游 FontSize 22）+ 上游 Margin bottom 18（:1240）
/// 说明行高   =  18 = 本仓独有的一行（上游无对应物，见 SHORTCUTS_NOTE_H）
/// 分隔呼吸   =   6 = 本仓独有（标题下 1px 分隔线 + 6px 呼吸，见 menu::shortcuts::SEP_GAP）
/// 表体       = 756 = 28 行 × 行高 27（实测正文行高 13 + 上游 Margin bottom 14，:1246）
/// 组间       =  24 = 6 组 × 4（menu::shortcuts::GROUP_GAP）
/// 上下内边距 =  56 = 2 × 28（上游 :1238 `Margin = new(28)`）
/// ─────────────────────────────────────────────
/// 合计       = 41 + 18 + 6 + 756 + 24 + 56 = **901**
/// ```
///
/// ⚠ 标题按上游 **FontSize 22** 的行高（23）算，**不是**本仓当前上屏的
/// `PANEL_TITLE_FONT`（11，行高 11）。这样"把标题改成上游 22"只需改字号、
/// **不必再改高度** —— 免得"按 11 算完再发现 22 装不下"。
/// 反过来：**若按标题 11 算，高度是 889**（41→29 少 12px），本仓**没有**取 889。
///
/// # 历史（本仓自定值，均已作废）
///
/// 560 → 600 → 620 → 820，每一档都写着"跟着表算的"；820 的来源是「R40 要求 ≥800」——
/// 那是**派单书里的一个数字**，不是布局约束（已废）。
///
/// ⚠ 若产品要求严格回到上游 680，唯一的路是**去掉 6 个组名并把表压到 19 行**
/// （信息架构改动，另案）；**不许**用"压缩行距"冒充对齐 —— 行距 14 是上游明确参数。
pub const SHORTCUTS_H: f32 = 901.0;

/// 快捷键浮层的内边距 **28**（四边）。
///
/// 上游 `MainWindow.axaml.cs:1238`：`var panel = new StackPanel { Margin = new(28) };`
/// —— Avalonia `Thickness` 单参 = 四边各 28。
///
/// ⚠ 2026-11-05 本仓自定 18（无上游依据）；2026-11-08 回源订正为 28。
pub const SHORTCUTS_PAD: f32 = 28.0;

/// 快捷键浮层行高 **27** = 实测正文行高 **13** + 上游 `Margin bottom` **14**。
///
/// 上游 `MainWindow.axaml.cs:1246`：`var row = new DockPanel { Margin = new(0, 0, 0, 14) };`
///
/// 行高本身**不估算**：用本仓上屏字体 `layout_no_wrap` 实测（Deng.ttf @ `BODY_FONT` = 12
/// ⇒ 行高 **13.0**；28 行逐条量过，去重集合只有 `{13.0}`），由
/// `menu::shortcuts::tests::标题与行高由实测行高加上游行距推出` 复算守。
///
/// ⚠ 2026-11-05 本仓自定 18（无上游依据，注释写"字号 12 + 上下各 3 呼吸"）。
pub const SHORTCUTS_ROW_H: f32 = 27.0;

/// 快捷键浮层的标题行高 **41** = 实测标题行高 **23** + 上游 `Margin bottom` **18**。
///
/// 上游 `MainWindow.axaml.cs:1240`
/// `new TextBlock { Text = LocalText.T("Work Faster"), FontSize = 22, FontWeight = SemiBold, Margin = new(0, 0, 0, 18) }`
/// ⇒ 行距 18；行高由 Avalonia 按字体度量现算，本仓拿不到那套度量
/// ⇒ 用**上屏字体**在**上游字号 22** 下实测（Deng.ttf @ 22 ⇒ **23.0**）。
///
/// ⚠ 字号本身本单**不动**：本仓标题仍按 `PANEL_TITLE_FONT`（11）上屏 ——
/// 本仓整套字号是从 1500×950 参考截图逐像素量出来的另一套刻度（见本文件头部），
/// 改成 22 属独立的视觉改动，已登记待拍板。这里只把**高度**按 22 预留。
///
/// ⚠ 2026-11-05 本仓自定 26（无上游依据）。
pub const SHORTCUTS_TITLE_H: f32 = 41.0;

/// 快捷键浮层的说明行高 **18**（**本仓独有的一行**，上游没有对应物）。
///
/// 实测正文行高 13 ⇒ 上下各 2.5px 呼吸。它不参与上游对齐，是
/// `crate::ui_text::SHORTCUTS_NOTE`（"只列当前真实生效的按键"）的载体。
pub const SHORTCUTS_NOTE_H: f32 = 18.0;

/// 快捷键浮层的「键」列宽 **170**（右侧留给说明文字）。
///
/// ⚠ 上游是「说明在左、键 `Dock.Right` **靠右**」（`MainWindow.axaml.cs:1248`），
/// 本仓是「键左列固定 170 + 说明在右」—— **排布与上游不同**，属既有差异，
/// 本单不动，已登记。170 的实测依据：最宽键 `Ctrl+Shift+Z / Ctrl+Y` 排版宽 **106.56**
/// ⇒ 余 63px。
pub const SHORTCUTS_KEY_COL_W: f32 = 170.0;

/// `W/A/S/D` 每次平移的**屏幕像素**（上游 `MainWindow.axaml.cs:980-984` 的
/// `PanBy(new Vector(0, 48))` 等四条 —— 单位是屏幕像素，与缩放无关）。
pub const PAN_STEP_PX: f32 = 48.0;

/// **定位到格时的最小缩放**（上游 `MapViewport.Navigation.cs:9`：`Zoom = Math.Max(Zoom, .5)`）。
///
/// 含义：定位只在缩放**低于** 50% 时抬到 50%；本来就放大着的（100%/200%）**不缩回去**，
/// 免得"定位一下把细节看丢了"。
pub const GOTO_MIN_SCALE: f32 = 0.5;

// ───────────────────── 关于对话框（帮助 → 关于 Crystal Studio，2026-11-06 新增） ─────────────────────

/// 关于对话框宽 **640**：最长一行是「本仓 github.com/lxf505226/CrystalStudio-Rust」（38 字符）
/// + 标签列 96 + 左右内边距 36。
pub const ABOUT_W: f32 = 640.0;

/// 关于对话框高 **264**。
///
/// 跟着内容算（[`crate::menu::about::content_height`]，与那一条用例同一算式）：
/// 内边距 18×2 + 标题 28 + 分隔线 1 + 间距 8 + 7 行 × 22 + 底部提示 22 = **249**，留 15px 余量。
/// 改行数/行高却忘了改这里 ⇒ `menu::about::tests::面板放得下内容` 当场红。
pub const ABOUT_H: f32 = 264.0;

/// 关于对话框的内边距 **18**（与快捷键浮层同一档）。
pub const ABOUT_PAD: f32 = 18.0;

/// 关于对话框标题行高 **28**（比正文行 22 高一档，标题才立得住）。
pub const ABOUT_TITLE_H: f32 = 28.0;

/// 标题下分隔线与首行之间的间距 **8**。
pub const ABOUT_TITLE_GAP: f32 = 8.0;

/// 关于对话框的行高 **22**（字号 12 + 上下各 5 呼吸，比快捷键浮层的 18 略松）。
pub const ABOUT_ROW_H: f32 = 22.0;

/// 关于对话框的「标签」列宽 **96**（"产品/版本/实现/上游/本仓/许可证/构建" 六个汉字放得下）。
pub const ABOUT_LABEL_COL_W: f32 = 96.0;

/// 关于对话框底部关闭提示行高 **22**。
pub const ABOUT_HINT_H: f32 = 22.0;

/// 路径输入模态（打开 / 另存为 / 导出选区）的宽 **640**（路径输入框要放得下 80 个字符的路径）。
///
/// ⚠ **不含「导出小地图」**：那个模态单独取 [`MINIMAP_DLG_W`]（上游 420）。
/// 上游这三个用途弹的是**系统文件选择器**（`EditorDialogs.cs` 的 `OpenFileDialog` /
/// `SaveFileDialog` 走 `StorageProvider.OpenFilePickerAsync`），宽高由系统给，本仓自绘
/// ⇒ 640 是本仓的推导值；而「导出小地图」上游是**真窗口** `Width = 420` ⇒ 必须独立。
pub const OPEN_DLG_W: f32 = 640.0;

// 路径输入模态的高 **176** —— **不再是常量**（2026-11-08 A10 订正）。
//
// 由 `crate::menu::dialog::路径模态内容高()` 现算；上游三个用途都是系统文件选择器
// （高由系统给），本仓自绘 ⇒ 高度用内容算式是等价物。
// ⚠ 旧常量 `OPEN_DLG_H = 160` 已删除：它是手写定值，改 `OPEN_DLG_PAD` 时**不跟随**，
// 正是"内边距 16→24 后提示行底到按钮顶只剩 4px"那个断裂的根因。
// 现在改内边距，高度自动跟随（见 `dialog::tests::改内边距时路径模态高度跟着变`）。

/// 打开地图模态的内边距 **24**（上游四个自定义对话框 `Margin = new(24)`）。
///
/// 来源：**上游真值**（四处逐字一致）：
/// - `MinimapExportWindow.cs:18` `var panel = new StackPanel { Margin = new(24), Spacing = 10 };`
/// - `MapExpansionWindow.cs:22` `var panel = new StackPanel { Margin = new(24), Spacing = 10 };`
/// - `MainWindow.axaml.cs:441`（新建地图）`var panel = new StackPanel { Margin = new(24) };`
/// - `MainWindow.axaml.cs:747`（单元格属性）`var panel = new StackPanel { Margin = new(24) };`
///
/// ⚠ 与关闭拦截模态的 [`MSG_BOX_PAD`]（22）**不是**同一个数，见那里的说明。
/// ⚠ 旧值 16 已作废（当时无上游真值，按手感取的）。
pub const OPEN_DLG_PAD: f32 = 24.0;

/// 打开地图模态里路径输入框的高 **28**。
pub const OPEN_DLG_FIELD_H: f32 = 28.0;

/// 打开地图模态的按钮宽 **88** / 高 **26**。
pub const OPEN_DLG_BTN_W: f32 = 88.0;
pub const OPEN_DLG_BTN_H: f32 = 26.0;

// ───────────── 「检查地图」模态（A-015 / R32-R33；官方 760×480） ─────────────
//
// 为什么单独一组而不是复用 `OPEN_DLG_*`：官方这个对话框是**大列表**（760×480，
// 最多 1000 行），路径模态那套 160 高的几何完全不适配；组内常量保持与 `OPEN_DLG_*`
// 同值的地方（内边距 / 按钮尺寸）**各写一份**，免得将来只调其中一个时牵连另一个。

/// 检查结果模态宽（官方 `Width = 760`）。
pub const CHECK_MAP_W: f32 = 760.0;

/// 检查结果模态高（官方 `Height = 480`）。
pub const CHECK_MAP_H: f32 = 480.0;

/// 面板内边距 **16**（与 [`OPEN_DLG_PAD`] 同值，语义独立）。
pub const CHECK_MAP_PAD: f32 = 16.0;

/// 标题行高 **26**。
pub const CHECK_MAP_TITLE_H: f32 = 26.0;

/// 标题与汇总之间的留白 **8**。
pub const CHECK_MAP_TITLE_GAP: f32 = 8.0;

/// 汇总区高 **44**（两行：汇总行 + 可选的「未检查 / 无图库 / 已陈旧」提示行）。
pub const CHECK_MAP_SUMMARY_H: f32 = 44.0;

/// 列表行高 **20**（`(x, y)  说明` 一行 12px 字 + 上下留白）。
pub const CHECK_MAP_ROW_H: f32 = 20.0;

/// 列表与底部按钮之间的留白 **10**。
pub const CHECK_MAP_LIST_GAP: f32 = 10.0;

// ───────────── 「导出小地图」模态（2026-11-06，N F-R14-1：尺寸输入） ─────────────
//
// 它比通用路径模态多摆三行（预设三选一 / 宽高 / 错误提示），所以**单独**给宽高：
// 宽取上游真值 420，高由内容算式现算（见 `dialog::小地图模态内容高`）。

/// 导出小地图模态的宽 **420**。
///
/// 来源：**上游真值** `MinimapExportWindow.cs:15`
/// `Title = LocalText.T("Export Minimap"); Width = 420;`
/// （同行 `:16` `SizeToContent = SizeToContent.Height; CanResize = false;`）。
///
/// ⚠ **不许改 [`OPEN_DLG_W`] 来"顺手对齐"**：上游这两者本来就是**两个独立窗口**
/// —— 导出小地图是真 `Window`（420 定宽），而打开/另存为/导出选区上游走的是
/// **系统文件选择器**（`EditorDialogs.cs` 的 `StorageProvider.OpenFilePickerAsync`，
/// 宽高由系统给）。把 640 改成 420 会让那三个模态一起变窄，属于**无上游依据**的连带改动。
pub const MINIMAP_DLG_W: f32 = 420.0;

// 导出小地图模态的高 —— **不再是常量**（2026-11-08 A10 订正）。
//
// 由 `crate::menu::dialog::小地图模态内容高()` 现算 = **250**（2026-11-08 删提示行后；
// 上一版是 270，那 20px 差额 = 已删提示带的 `12 + 18` 换成了一行 `MINIMAP_DLG_GAP`）。
// 上游是 `SizeToContent = SizeToContent.Height`（`MinimapExportWindow.cs:16`，高随内容），
// 本仓 egui 无自动测高 ⇒ 内容算式是它的等价物；改 `OPEN_DLG_PAD` 时高度**自动跟随**。
//
// ## 订正记录（2026-11-08，删提示行）：270 → 250
//
// 上游 `MinimapExportWindow.cs:20-30` 的子项里**没有**提示/说明行（实读顺序：`尺寸预设`标签
// → ComboBox → `宽度(像素)`标签 → WidthInput → `高度(像素)`标签 → HeightInput → error → 按钮行）
// ⇒ 本仓自加的那行说明（`EXPORT_MINI_DLG_HINT`）已删除，算式里 `12 + 18` 那一项换成一行
// `MINIMAP_DLG_GAP`。⚠ **不是 240**：240 只在预设行紧贴路径框底（0 间距）时成立。
//
// ## 订正记录（2026-11-08，A10）：旧注释里那个多余的 `+10`
//
// **原文（保留作历史引用，已作废）**：
// > 算式（面板顶为 0）：`16(PAD) + 24(标题) + 28(路径框) + 12+18(提示带) + 10 + 26(预设行)
// > + 10 + 26(宽高行) + 10 + 32(错误行) + 10 + 26(按钮) + 16(PAD)` = `254`
//
// **两处错，终值 254 只是碰巧对**：
// 1. 那个算式逐项相加 = **264**，不是 254 —— 与它自己声明的终值自相矛盾；
// 2. 多出来的正好是 `12+18(提示带)` 后面那个 **`+ 10`**：当时代码里是
//    `preset_top = base.field.bottom() + 12.0 + 18.0`（`dialog::minimap_rects`；
//    本单删提示行后已改成 `+ MINIMAP_DLG_GAP`），提示带之后**直接**就是预设行，没有这段间距。
//
// ⇒ 正确算式见 `dialog::小地图模态内容高`（`PAD = 24` 时 = 250；`PAD = 16` 时 = 234）。

/// 导出小地图模态里「预设行 / 宽高行」的行高 **26**（只比路径框矮 2，手感统一）。
pub const MINIMAP_DLG_ROW_H: f32 = 26.0;

/// 导出小地图模态里行与行的间距 **10**。
pub const MINIMAP_DLG_GAP: f32 = 10.0;

/// 导出小地图模态里错误提示带的高 **32**（两行 12px 中文，包住最长那条原因的前半段）。
pub const MINIMAP_DLG_ERR_H: f32 = 32.0;

/// 导出小地图模态里单个宽/高输入框的宽 **132**（放得下 10 位数字）。
pub const MINIMAP_DLG_FIELD_W: f32 = 132.0;

// 导出小地图模态里「单个预设按钮的宽」**已不再是常量**（2026-11-08 A10 订正）。
//
// 旧值 `MINIMAP_DLG_PRESET_W = 150` 已删除：3×150 + 标签列 56 + 间距 16 = 530，
// 而上游 `Width = 420`（`MinimapExportWindow.cs:15`）时面板内宽只有 420 − 2×24 = **372**
// ⇒ 第三个按钮必然越出面板（A10 实测断言红：`第 2 个预设按钮越出面板`）。
// 上游该控件是 `ComboBox { HorizontalAlignment = HorizontalAlignment.Stretch }`
// （`MinimapExportWindow.cs:5`）⇒ 这一行本来就该**填满内宽**；
// 按钮宽改由 `dialog::minimap_rects` 按剩余宽度均分（不写定值，改宽时自动跟随）。

/// 导出小地图模态里预设行左侧标签列宽 **56**（`尺寸预设` 四个字 48 + 8 呼吸）。
pub const MINIMAP_DLG_PRESET_COL_W: f32 = 56.0;

/// 导出小地图模态里「宽」/「高」两个标签列宽 **24**（两个字刚好）。
pub const MINIMAP_DLG_SIZE_LABEL_W: f32 = 24.0;

/// 导出小地图模态里宽高框后面 `px` 单位的占位宽 **24**（含与下一个标签之间的呼吸）。
pub const MINIMAP_DLG_UNIT_W: f32 = 24.0;

/// 导出小地图模态里「标签 → 控件」/「控件 → 下一个标签」的间距 **8**。
pub const MINIMAP_DLG_LABEL_GAP: f32 = 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// 规范化只产出 3..=31 的奇数（与官方 `Clamp(value | 1, 3, 31)` 同集合）。
    #[test]
    fn 笔刷尺寸只产出三到三十一的奇数() {
        // 15 档：3/5/…/31
        let all: Vec<usize> = (BRUSH_MIN..=BRUSH_MAX).filter(|n| n % 2 == 1).collect();
        assert_eq!(all.len(), 15, "3..=31 的奇数正好 15 档");
        assert_eq!(all.first(), Some(&3));
        assert_eq!(all.last(), Some(&31));
        for n in 0..=64 {
            let v = clamp_brush_size(n);
            assert!((BRUSH_MIN..=BRUSH_MAX).contains(&v), "{n} → {v} 越界");
            assert_eq!(v % 2, 1, "{n} → {v} 不是奇数");
        }
        // 边界：0/1/2 → 3；32/40/99 → 31；偶数抬成下一个奇数
        for n in [0, 1, 2] {
            assert_eq!(clamp_brush_size(n), 3, "{n} 应被夹到下限 3");
        }
        for n in [32, 33, 40, 99, usize::MAX] {
            assert_eq!(clamp_brush_size(n), 31, "{n} 应被夹到上限 31");
        }
        for n in [4, 6, 8, 30] {
            assert_eq!(clamp_brush_size(n), n + 1, "偶数 {n} 应抬成奇数");
        }
        for n in [3, 5, 7, 15, 29, 31] {
            assert_eq!(clamp_brush_size(n), n, "奇数 {n} 原样");
        }
    }

    /// 步进 2：`[` / `]` 一次走两格，且不会停在偶数上（官方 `TickFrequency=2`）。
    #[test]
    fn 步进为二() {
        assert_eq!(BRUSH_STEP, 2);
        assert_eq!(BRUSH_DEFAULT, 3);
        let mut n = BRUSH_DEFAULT;
        let mut seen = vec![n];
        while n < BRUSH_MAX {
            n = clamp_brush_size(n + BRUSH_STEP);
            seen.push(n);
        }
        assert_eq!(seen, vec![3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31]);
    }

    /// 填充上限就是任务书那个数（改它必须改测试）。
    #[test]
    fn 填充上限二十万格() {
        assert_eq!(FILL_MAX_CELLS, 200_000);
        // 700×700 的整图（490,000 格）必须被这个上限拦住。
        //
        // 两边都是常量 ⇒ 写成 `const { … }` 块，让它在**编译期**就断掉
        // （clippy `assertions_on_constants`：运行期断言常量是死代码，
        //  一旦有人把 `FILL_MAX_CELLS` 调到 490,000 以上，本 crate 直接编不过，
        //  比"跑测试才发现红"更早拦住）。
        const { assert!(700 * 700 > FILL_MAX_CELLS) };
    }

    /// 印章网格的几何必须自洽：4 列整除可用宽、单格高由内缩/缩略图/名称行拼出来、
    /// 一页正好是整行（96 = 4 × 24）。
    ///
    /// 这条挡的是"改一个数忘了改另一个"——例如把缩略图高从 32 调到 40 却忘了 `STAMP_CELL_H`，
    /// 名称就会被压到下一格上（界面上看起来像串行）。
    ///
    /// ⚠ 全部比较都先落到局部变量：直接 `assert!(常量表达式)` 会被 clippy 判成
    /// `assertions_on_constants`（"断言恒真"），而这里要的正是"常量之间必须恒真"。
    #[test]
    fn 印章网格几何自洽() {
        // 4 列整除 336（= RIGHT_DOCK_W - 2 × RIGHT_PAD_X）
        let usable = RIGHT_DOCK_W - 2.0 * RIGHT_PAD_X;
        assert_eq!(usable, 336.0, "可用宽变了，STAMP_GRID_COLS/STAMP_CELL_W 要重算");
        let grid_w = STAMP_CELL_W * STAMP_GRID_COLS as f32;
        assert_eq!(grid_w, usable);
        // 单格高 = 3 + 32 + 2 + 13 + 3
        let parts_h = STAMP_CELL_PAD * 2.0 + STAMP_THUMB_H + STAMP_NAME_GAP + STAMP_NAME_H;
        assert_eq!(STAMP_CELL_H, parts_h);
        // 缩略图必须放得进格子（宽高都要留出内缩）
        let fits_w = STAMP_THUMB_W + STAMP_CELL_PAD * 2.0 <= STAMP_CELL_W;
        let fits_h = STAMP_THUMB_H + STAMP_CELL_PAD * 2.0 <= STAMP_CELL_H;
        assert!(fits_w, "缩略图宽放不进格子");
        assert!(fits_h, "缩略图高放不进格子");
        // 一页 = 整行
        let rem = STAMP_PAGE_SIZE % STAMP_GRID_COLS;
        assert_eq!(rem, 0, "每页必须是整行数");
        let rows = STAMP_PAGE_SIZE / STAMP_GRID_COLS;
        assert_eq!(rows, 24, "一页 24 行");
        // 缓存上限必须能装下一整页（否则翻页时同一页内部就会自我淘汰）
        let cap_enough = STAMP_THUMB_CACHE_CAP >= STAMP_PAGE_SIZE;
        assert!(
            cap_enough,
            "缓存上限 {STAMP_THUMB_CACHE_CAP} 装不下一页 {STAMP_PAGE_SIZE}"
        );
    }
}
