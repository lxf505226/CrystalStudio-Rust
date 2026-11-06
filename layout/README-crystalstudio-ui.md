# CrystalStudio 界面真值（Avalonia 11 / .NET 10）— 抽取口径与未解项

> 供 `tools/miredit` 用 Rust 仿制 CrystalStudio 地图编辑器的界面。
> **源工程全程只读**：`E:\GodM2\CrystalStudio\` 下未改动一字、未编译、未运行（本机也没有 .NET SDK）。

| 项 | 值 |
|---|---|
| 产物 | `crystalstudio-ui.json` |
| 大小 / 编码 | 514,877 字节 / UTF-8 / 缩进 2 / 末尾换行 |
| SHA256 | `98CC05D0FD1CA39DDD2447FC83C44056FAE7D5AF2BD71C49D2B01C61540BB0CF` |
| 统计行 | `windows=7, total_nodes=499, theme_colors=71, code_built=52, unknowns=18` |
| 抽取日 | 2026-11-04 |
| 可重复性 | 同一输入重复运行输出字节完全一致（已连跑两次比 SHA256） |

⚠ **与派单口径有 7 处不同，逐条列在 §4，没有一处被静默改掉。**

---

## 1 数据源（全部只读）

| 文件 | 用途 | 行数 |
|---|---|---|
| `App.axaml` | 应用资源字典（71 个色刷）+ Window/TextBox/ComboBox/Slider 样式 | 94 |
| `EditorControls.axaml` | 控件主题（Button/QuietButton/PrimaryButton/ToolButton/CheckBox/SectionTitle/Palette* /LayerRow/LayerVisibility/LayerHighlight/Slider/StampCategoryItem） | 116 |
| `MainWindow.axaml` | 主窗骨架 | 73 |
| `ProvinceGeneratorWindow.axaml` | 省域生成对话框 | 47 |
| `TempleGeneratorWindow.axaml` | 地牢生成对话框 | 44 |
| `TerrainGeneratorWindow.axaml` | 地形生成对话框 | 61 |
| `TownStampWindow.axaml` | 城镇戳记对话框 | 36 |
| `SpecialBrushWindow.axaml` | 特殊笔刷（树/灌木/岩石）对话框 | 61 |
| `WallBrushWindow.axaml` | 墙体笔刷对话框 | 16 |
| `DockWorkspace.cs` | 停靠工作区（代码建控件 + 全部面板几何） | 332 |
| `SettingsWindow.cs` | 设置窗（代码建控件） | 46 |
| `StampCategoryPicker.cs` | 多选分类下拉（代码建控件） | 32 |
| `ThemeService.Avalonia.cs` | 运行期调色（把静态色板重算） | 58 |
| `ResponsivePanel.cs` | 自适应换列容器（`ResponsivePanel` 的算法） | 43 |
| `EditorWindow.cs` | 所有窗口基类（`Opened` 时改写宽高） | 143 |
| `DefaultWorkspace.json` | 内嵌默认面板布局（运行期真实停靠数据） | 100 |
| `CrystalStudio.csproj` | Avalonia 11.3.4 / net10.0（版本真值） | 44 |

---

## 2 JSON 口径

### 2.1 顶层键

| 键 | 含义 |
|---|---|
| `source` / `extracted_at` | 来源与抽取日 |
| `xaml_files` | **9 个 axaml 全在**，每个带 `role`：`application` / `control-theme` / `window` |
| `theme` | 字体、51 条度量、4 张颜色表 |
| `windows` | **7 个真正的窗口**（见 §4 冲突 1） |
| `code_built` | 52 条写死的几何常量，全部 `文件:行号` |
| `unknowns` | 18 条静态不可解项 |

### 2.2 `kind` 取值表（12 类，无未分类节点）

| kind | 含义 | 元素 | 节点数 |
|---|---|---|---|
| `label` | 文本 | TextBlock, Run | 138 |
| `panel` | 容器 | Border, Grid, StackPanel, DockPanel, WrapPanel, ScrollViewer, Expander, ResponsivePanel | 129 |
| `button` | 可点 | Button, RadioButton, CheckBox, ToggleButton, Menu, MenuItem | 107 |
| `combo` | 下拉 | ComboBox, ComboBoxItem | 45 |
| `canvas` | 画布/图元 | Canvas, Viewbox, Path, Image, Ellipse, MapViewport | 24 |
| `slider` | 滑杆 | Slider | 20 |
| `tree` | 列表/树 | ListBox, ListBoxItem | 9 |
| `separator` | 分隔线 | Separator | 8 |
| `input` | 输入框 | TextBox | 7 |
| `tab` | 页签 | TabControl, TabItem | 3 |
| `splitter` | 分隔条 | GridSplitter | 1 |
| `status` | 进度/状态 | ProgressBar | 1 |

### 2.3 节点字段

| 字段 | 说明 |
|---|---|
| `kind` / `tag` | 语义类别 / XAML 元素原名（如 `RadioButton`） |
| `name` | `x:Name`，**192 个节点带名，全部保留** |
| `text` | `Text`/`Content`/`Header`，无则由 `Run` 拼接（绑定表达式按原文保留） |
| `rect` | `{x,y,w,h}`；**算不出来整体为 `null`**，部分未知则该分量为 `null` |
| `layout` | `row:/col:/span:/dock:/orient:/halign:/valign:`，都没有则为 `fill` |
| `style_ref` | `Theme="{StaticResource X}"` 的 `X` |
| `icon` | 仅 `button` 类节点带：`path`（含每条 Path 的 `d/fill/stroke/stroke_thickness` + Viewbox 尺寸 + Canvas 设计栅格）或 `glyph`（MDL2 字体 + 字号 + 字符 + 码位） |
| `grid_rows`/`grid_columns` | 仅 `Grid` 节点带：该网格自己的轨道声明 |
| `margin`/`padding`/`font_size` | 原文字符串/数值，未写为 `null` |
| `visible`/`enabled` | `IsVisible`/`IsEnabled`，未写为 `null` |
| `tag_value` | Avalonia 的 `Tag`（工具按钮靠它区分笔刷，如 `Brush`/`Wall`） |
| `tooltip`/`automation` | 悬停提示 / `AutomationProperties.Name`（图标按钮的语义名） |
| `notes` | 由代码生成 / 自适应 / 默认隐藏禁用 等，**只写有依据的** |
| `depth`/`children` | 层级与子节点 |

### 2.4 `rect` 的推导规则（这是唯一有"算"的地方，规则写清以便复核）

能算：
1. 元素自身 `Width`/`Height`（如 `Width="78"`）；
2. 父 `Grid` 的**固定**行/列尺寸（如 `Width="72"` 的列 → 该列子节点 `w=72`）；
3. 父 `Grid` 无列/行声明时视为**单隐式轨道**，子节点撑满父格 → `w`/`h` 取父格尺寸；
4. 绝对 `x/y` **只在**"从窗口根到该节点整条祖先链偏移都已知"时给出（即祖先全是固定轨道的 Grid）。`MainWindow` 实测：标题栏 `y=0,h=36`、工具条 `y=36,h=48`、工作区 `y=84`、左侧工具列 `x=0,w=72`、分隔条 `w=5`、右栏 `w=360`。

算不出（写 `null`，不猜）：
1. 星号 `*` 与 `Auto` 轨道的尺寸；2. 星号轨道**之后**任何轨道的起点（如状态栏 `y`）；3. `StackPanel`/`WrapPanel`/`DockPanel` 内子元素的偏移（依赖文字度量）；4. 跨行/跨列（`RowSpan/ColumnSpan > 1`）时的尺寸。

492 个树节点里：**420 个 `rect` 完全为 `null`**、24 个只有 `w/h` 而 `x/y` 未知、**只有 45 个能给出绝对 `x/y`**——这就是"XAML 看不出像素"的规模，如实反映。

### 2.5 颜色：四张表

| 表 | 条数 | 内容 |
|---|---|---|
| `theme.colors` | **71** | `App.axaml` 里 `SolidColorBrush` 的**资源键名 → 色值**原文（`Theme14171D`→`#14171D` … `ThemeED1C2028`→`#ED1C2028`，另有 `Panel`/`Muted`/`Accent`/`Line`） |
| `theme.semantic` | **55** | 语义角色 → `{resource, hex, source}`，**每条带 `文件:行号`**（如 `window_bg` → `Theme14171D` `#14171D` @ `App.axaml:85`） |
| `theme.inline_colors` | **6** | 没走资源的写死颜色（低/高碰撞色块 `#F05569`/`#41A5F5`、地牢预览底 `#20272D`、设置窗错误红 `#E49E97`、停靠落点 `#418498FF`/`#8498FF`） |
| `theme.runtime_computed_colors` | **5** | 运行期才算得出的键（`OnAccent`/`CanvasBackground`/`CanvasPaper`/`CanvasGrid`/`Accent`），给公式与行号 |

### 2.6 `metrics`（51 条，每条带 `metrics_source`）

派单点名的 8 个：`border_thickness=1`、`radius_sm=2`、`radius_md=3`、`radius_lg=5`、`icon_sm=14`、`icon_md=16`、`icon_lg=24`（+ 实存 `radius_row=4`、`radius_xl=6`）。
**派单写的 `icon_sm=12 / icon_lg=20` 在工程里没有出处**，见 §4 冲突 3。
另有 `tool_button=32`、`tool_button_margin=2`、`tool_item_min_width=36`、`slider_thumb_width=12`、`slider_thumb_height=18`、`slider_track_height=3`、`checkbox_box=14`、`checkbox_gap=7`、`combo_min_height=34`、`tab_item_font_size=11`、`section_title_font_size=10`、`scrollbar_min=9` 等。

---

## 3 主窗骨架实测

### 3.1 窗口

| 项 | 值 | 出处 |
|---|---|---|
| Width×Height | **1500×950** | `MainWindow.axaml:1` |
| MinWidth×MinHeight | **1100×720** | `MainWindow.axaml:1` |
| FontFamily | **Segoe UI**（单字体） | `MainWindow.axaml:1` |
| 样式回退链 | `Segoe UI, Microsoft YaHei, sans-serif` | `App.axaml:85` |
| FontSize | **12** | `MainWindow.axaml:1` + `App.axaml:85` |
| ThemeVariant | **Dark** | `App.axaml:1` |
| 背景/前景 | `Theme14171D` / `ThemeE5E9F0` | `MainWindow.axaml:1` |
| UseLayoutRounding | True | `MainWindow.axaml:1` |

### 3.2 根 Grid（`MainWindow.axaml:3-4`）

行 = `36 / 0 / 48 / * / 30`（**5 行**）；**根 Grid 没有列定义**。

| 行 | 高 | 内容 | 实测 rect |
|---|---|---|---|
| 0 | 36 | 标题栏 Border（`DockPanel Margin="18,0"` + 菜单 + 右侧 "Crystal Studio"） | `Y=0 H=36` |
| 1 | 0 | 空保留行 | `H=0` |
| 2 | 48 | 工具选项条 Border（笔刷尺寸滑杆 `BrushSize` + `3×3` 徽标 + 城镇/墙体/拣选四组 WrapPanel） | `Y=36 H=48` |
| 3 | * | 工作区 Grid（见 3.3） | `Y=84` |
| 4 | 30 | 状态栏 Border（`Status` / `Coordinates` / `LibraryDot` / `LibraryStatus` / `Performance`） | `H=30, Y=null`（星号行后起点不可知） |

### 3.3 嵌套工作区 Grid（`MainWindow.axaml:7`）

列 = `72 / * / 5 / 360`（**4 列**），第 4 列 `MinWidth=300 MaxWidth=480`。

| 列 | 宽 | 内容 | 实测 rect |
|---|---|---|---|
| 0 | 72 | Border → `ResponsivePanel#ToolPanel`（`ItemMinWidth="36"`，10 个工具按钮） | `X=0 W=72` |
| 1 | * | `Grid` → `MapViewport#Viewport`（自绘画布）+ 缩放浮层 | `X=72 W=null` |
| 2 | 5 | `GridSplitter`（`ResizeBehavior=PreviousAndNext`） | `W=5` |
| 3 | 360 | Border → 右栏 Grid（`Margin="16,14"`，6 行：Layers / 资产头 / 资产区 / 分页 / 当前资产 / 检查器） | `W=360` |

### 3.4 10 个工具按钮（`MainWindow.axaml:9-19`）

`ToolButton` 主题：`32×32`、`Margin=2`、`GroupName=EditorTools`、圆角 5；图标 Viewbox `16×16`，内部 Canvas `24×24`（缩放比 0.667）。
**墙体工具是唯一例外：Viewbox `18×18`。**

| Tag | 图标类型 | 细节 |
|---|---|---|
| `Brush` | path | 2 条 Path（填充 + 描边 1.5）16×16 |
| `Pencil` | glyph | MDL2 `U+E70F` @16 |
| `Bucket` | path | 2 条 Path（描边 1.6 + 填充）16×16 |
| `Eraser` | glyph | MDL2 `U+E75C` @16 |
| `Select` | glyph | MDL2 `U+EF20` @16 |
| `Stamp` | glyph | MDL2 `U+E8B9` @16 |
| `Pickup` | glyph | MDL2 `U+E7C2` @16 |
| `Clone` | glyph | MDL2 `U+E8C8` @16 |
| `Wall` | path | 1 条多段 Path（描边 1.5）**18×18** |
| `SpecialBrush` | path | 1 条 Path（填充）16×16 |

### 3.5 各窗口节点数

| 窗口 | 尺寸 | 最小 | 节点 | 根 Grid 行 / 列 |
|---|---|---|---|---|
| MainWindow | 1500×950 | 1100×720 | 194 | `[36,0,48,*,30]` / 无列（嵌套 4 列） |
| ProvinceGeneratorWindow | 1100×780 | 850×650 | 53 | `[*,Auto,Auto]` / `[265,20,*]` |
| TempleGeneratorWindow | 1060×760 | 830×630 | 39 | `[*,Auto,Auto]` / `[260,20,*]` |
| TerrainGeneratorWindow | 1060×760 | 830×630 | 60 | `[*,Auto,Auto]` / `[280,20,*]` |
| TownStampWindow | 1060×760 | 850×610 | 64 | `[*,Auto,Auto]` / `[260,20,*]` |
| SpecialBrushWindow | 1140×820 | 1000×680 | 72 | `[Auto,*,Auto,Auto]` / 无列 |
| WallBrushWindow | 700×610 | 530×500 | 17 | `[Auto,Auto,Auto,*,Auto,Auto]` / 无列 |

---

## 4 与派单数字的冲突（7 条，逐条摆出，未静默改）

| # | 派单写的 | 实测 | 处置 |
|---|---|---|---|
| 1 | "解析出的窗口数 = **9**" | 9 个 axaml 里只有 **7 个窗口**：`App.axaml` 根是 `Application`、`EditorControls.axaml` 根是 `ResourceDictionary`，**都不是窗口** | `windows` 只有 7 条；9 个文件全在 `xaml_files` 里按 role 标注；验收门按 7 断言并**显式打印派单口径 9**，不硬凑 |
| 2 | "主窗列 `72/*/5/360`（**列 4**）" | 根 Grid 只声明 5 行、**无列定义**；这 4 列属于 `MainWindow.axaml:7` 的**嵌套工作区 Grid** | `window.grid_columns=[]`；4 列记在该嵌套 Grid 节点的 `grid_columns` 上；验收门拆成"根行 5"+"嵌套列 4"两条断言 |
| 3 | `icon_sm=12`、`icon_md=16`、`icon_lg=20` | 工程里**没有 12 也没有 20 的图标尺寸**。实测：工具图标 16×16、墙体图标 18×18、复选框方块 14×14、图层眼睛 16×16、滑块拇指 12（宽）×18（高）、Canvas 设计栅格 24×24 | `icon_sm=14`、`icon_md=16`、`icon_lg=24`，另加 `icon_wall=18`、`icon_eye=16`；12 归到 `slider_thumb_width` |
| 4 | "FontFamily Segoe UI / FontSize 12" | 对，但**两套来源**：MainWindow 显式写 `Segoe UI`（单字体）；`App.axaml:85` 的 Window 样式是回退链 `Segoe UI, Microsoft YaHei, sans-serif`。6 个对话框里有 4 个写了 Background/Foreground 但没写字体，全部走样式 | `theme.font_family` 与 `font_family_fallback` 分开记，不合并 |
| 5 | "代码生成清单：3 个文件" | 3 个文件按要求列全（`DockWorkspace.cs` **20** 条 / `SettingsWindow.cs` **5** 条 / `StampCategoryPicker.cs` **6** 条 = 31），**另加 `MainWindow.axaml.cs` 21 条**，合计 52 | 因为运行期布局真值在那里（`InitializeDocking()` 拆 XAML 骨架 + 各面板停靠边/权重）。不列这段，Rust 侧会照着 XAML 复刻出一个实际不存在的界面 |
| 6 | "行 `36/0/48/*/30`"（当作静态骨架） | 行值对，但**第 2 行 48px 的工具选项条在停靠化后被置 0**（`MainWindow.axaml.cs:319`），内容搬进 Tool Options 面板；`BusyProgress` 从 Row 2 挪到 Row 0（`:321`） | `grid_rows` 记 XAML 声明值（属实），运行期变化写进 `code_built` + `unknowns` |
| 7 | 统计行格式 `windows=9, ...` | 实际 `windows=7, total_nodes=499, theme_colors=71, code_built=52, unknowns=18` | 原样打印见 §6，格式照抄、数字按实测 |

另有两处无实质影响的小偏差：派单写 `App.axaml` 93 行（实际 **94**）、`MainWindow.axaml` 72 行（实际 **73**），差额是文件末尾换行。

---

## 5 未解项（18 条，与 JSON `unknowns` 同源同序）

1. **Viewbox/Path 矢量图标**：只能拿到路径数据与视口尺寸（16×16 内含 24×24 Canvas，缩放比 0.667），最终栅格像素取决于 Avalonia `Stretch=Uniform` 实现与抗锯齿；按 16/24 换算，1.5px 描边落在约 1.0px。
2. **Segoe MDL2 Assets 字形**：只给码位（U+E70F 等），字形轮廓/字宽/基线依赖系统字体；缺字体即方框。Rust 侧须自备等效矢量图标（建议按 §3.4 的码位一一换）。
3. **Fluent 主题 11.3.4 的隐式模板**：Menu / MenuItem / ComboBox / ScrollViewer / Expander / TabControl / ListBox 在本工程**没有** ControlTheme，其内边距/项高/下拉层尺寸来自 Fluent 包，随版本变化。
4. **Menu/MenuItem 项高与内边距**：主窗菜单完全依赖 Fluent 默认模板，XAML 里拿不到像素。
5. **ResponsivePanel 自适应**：算法已读全（列数 `max(1,min(n,floor(width/ItemMinWidth)))`，行高取该行 `DesiredSize` 最大值），但 `DesiredSize` 依赖字体度量，最终列宽行高必须运行期算；`ItemMinWidth` 取值：ToolPanel=36、LayerList=160、Cell Highlights=160、InspectorDetails=245。
6. **EditorWindow.Opened 改写宽高**（`EditorWindow.cs:34-38`）：把 Width/Height 减去"外框−客户区"，使外框最终等于 1500×950，**客户区比它略小**；XAML 数字是外框目标值，不是控件可用面积。
7. **PopupHeight.Fit**：窗口 `Opened` 时重新拟合弹层高度（`PopupHeight.Avalonia.cs`），未逐行读完，弹层最终高度不可静态确定。
8. **运行期主窗布局被整体替换**：`MainWindow.axaml.cs:267-391` 的 `InitializeDocking()` 拆除 XAML 的 `72/*/5/360` 工作区 Grid，改由 `DockWorkspace` 接管；真实尺寸来自内嵌 `DefaultWorkspace.json`（左区 DockSize=156：Tools 174 / Tool Options 92 / Layers 262 / Cell Inspector 400；右区 DockSize=356：Minimap 220 / Palette 556 / Active Asset 189），以及用户 `profile/workspace.json` 的覆盖。注意 Layers、Cell Inspector 在代码里分别以 `Dock.Right`/`Dock.Bottom` 加入，却被默认布局改到 `Left(0)`。
9. **主题色运行期被重算**（`ThemeService.Avalonia.cs:35-57`）：所有 `Theme*` 键按亮度/蓝色偏移与系统 accent 混合，`Accent` 取系统强调色（缺省 RGB(132,152,255)），并动态生成 `OnAccent`/`CanvasBackground`/`CanvasPaper`/`CanvasGrid`；`App.axaml` 里的静态值只是"深色基线"。
10. **`OnAccent` 设计时不存在**（只在 `ThemeService:52` 运行期创建），但 `EditorControls.axaml:18` 的 `PrimaryButton.Foreground` 静态引用它 ⇒ 该前景色在 XAML 视图里不可知。
11. **字体度量**：所有 Auto/星号尺寸与文本宽度依赖 Segoe UI 12px 的度量与 DPI 缩放，无法静态换算像素；中文标签回退 Microsoft YaHei，宽度与拉丁字不同。
12. **MapViewport 的画布内容**（地图/网格/选择框）全部自绘（`Rendering/*.cs`），不产生 XAML 节点，不在本 JSON 覆盖范围。
13. **窗口非客户区**（标题栏/边框）高度由 OS 决定，不是 XAML 数字。
14. **对话框 `CenterOwner` 的屏幕坐标**取决于 Owner 位置。
15. **StackPanel/WrapPanel 内子元素绝对 y** = 累计 `DesiredSize` 的结果，静态不可知（这些节点的 `x/y` 为 `null`，最多只给 `w/h`）。
16. **AssetList 列数**：每项宽度写死 78（`MainWindow.axaml:66`），但列数由外层 WrapPanel 按面板宽度换行决定。
17. **ComboBox 下拉层 / ToolTip / ContextMenu** 的尺寸由 Fluent 模板与内容决定，不在 XAML 里。
18. **Grid 星号余量分配**：含 Auto 轨道时星号行/列的余量分配属 Avalonia 布局算法内部，本 JSON 只给轨道声明，不给最终像素。

---

## 6 验收门（真跑，原样输出）

脚本逻辑：用 `xml.etree` **独立重解析** 9 个 axaml，再与产出 JSON 交叉校验（不是读自己写过的中间结果）。

```python
# -*- coding: utf-8 -*-
"""验收门：独立重解析 9 个 axaml，与产出 JSON 交叉校验（只用标准库）。"""
import json, os, sys, xml.etree.ElementTree as ET

SRC = r"E:\GodM2\CrystalStudio\CrystalStudio"
OUT = r"E:\GodM2\tools\miredit\layout\crystalstudio-ui.json"
AXAML = ["App.axaml", "EditorControls.axaml", "MainWindow.axaml",
         "ProvinceGeneratorWindow.axaml", "TempleGeneratorWindow.axaml",
         "TerrainGeneratorWindow.axaml", "TownStampWindow.axaml",
         "SpecialBrushWindow.axaml", "WallBrushWindow.axaml"]
SKIP_SUFFIX = {"RowDefinitions", "ColumnDefinitions", "Styles", "Resources"}
TRANSPARENT = {"ItemsPanelTemplate", "DataTemplate", "ControlTemplate", "HierarchicalDataTemplate"}
X_NS = "{http://schemas.microsoft.com/winfx/2006/xaml}"
fails = []

def check(cond, msg):
    print(("  [PASS] " if cond else "  [FAIL] ") + msg)
    if not cond: fails.append(msg)

def local(t): return t.split("}", 1)[1] if t.startswith("{") else t
def suffix(t):
    nm = local(t); return nm.split(".", 1)[1] if "." in nm else nm
def attr(el, n, d=None):
    v = el.get(X_NS + n); return v if v is not None else el.get(n, d)

def visible_children(el):
    for ch in el:
        nm = local(ch.tag)
        if "." in nm and suffix(ch.tag) in SKIP_SUFFIX: continue
        if "." in nm: yield from visible_children(ch); continue
        if nm in TRANSPARENT: yield from visible_children(ch); continue
        yield ch

def count_tree(el):
    n = 0
    for ch in visible_children(el): n += 1 + count_tree(ch)
    return n

def first_grid(el):
    for ch in el:
        if local(ch.tag) == "Grid": return ch
    return None

def tracks(grid, which):
    if grid is None: return []
    for ch in grid:
        if suffix(ch.tag) == which:
            out = []
            for d in ch:
                raw = attr(d, "Height") if which == "RowDefinitions" else attr(d, "Width")
                if raw is None: out.append("*")
                elif raw.strip() == "Auto": out.append("Auto")
                elif raw.strip().endswith("*"):
                    head = raw.strip()[:-1].strip()
                    out.append("*" if not head or float(head) == 1 else "%g*" % float(head))
                else:
                    f = float(raw); out.append(int(f) if f == int(f) else f)
            return out
    return []

print("== 1) 解析 9 个 axaml ==")
roots = {fn: ET.parse(os.path.join(SRC, fn)).getroot() for fn in AXAML}
check(len(roots) == 9, "解析 axaml 文件数 = %d（期望 9）" % len(roots))
win_files = [fn for fn in AXAML if local(roots[fn].tag) not in ("Application", "ResourceDictionary")]
print("  非窗口资源文件：%s" % ", ".join(fn for fn in AXAML if fn not in win_files))
check(len(win_files) == 7, "工程实际窗口数 = %d（派单写的 9 含 2 个资源字典，见 README §4 冲突 1）" % len(win_files))
print("== 2) 主窗骨架数字 ==")
mw = roots["MainWindow.axaml"]
check(float(attr(mw, "Width")) == 1500 and float(attr(mw, "Height")) == 950, "MainWindow 1500x950")
check(float(attr(mw, "MinWidth")) == 1100 and float(attr(mw, "MinHeight")) == 720, "MainWindow min 1100x720")
root_grid = first_grid(mw)
rows, cols = tracks(root_grid, "RowDefinitions"), tracks(root_grid, "ColumnDefinitions")
check(rows == [36, 0, 48, "*", 30], "根 Grid 行 = %s（期望 [36,0,48,*,30]）" % rows)
print("  根 Grid 列 = %s（根 Grid 未声明列；工作区列在嵌套 Grid 上）" % cols)
workspace = None
stack = [root_grid]
while stack and workspace is None:
    g = stack.pop(0)
    for ch in visible_children(g):
        if local(ch.tag) == "Grid":
            if len(tracks(ch, "ColumnDefinitions")) == 4: workspace = ch
            stack.append(ch)
wcols = tracks(workspace, "ColumnDefinitions")
check(wcols == [72, "*", 5, 360], "嵌套工作区 Grid 列 = %s（期望 [72,*,5,360]）" % wcols)
check(len(wcols) == 4, "工作区 Grid 列数 = %d（派单要求 4）" % len(wcols))

print("== 3) 与产出 JSON 交叉校验 ==")
data = json.load(open(OUT, encoding="utf-8"))
check(True, "json.load 解析成功，文件 %d 字节" % os.path.getsize(OUT))
print("  顶层键：%s" % list(data.keys()))
jwins = data["windows"]
check(len(jwins) == len(win_files), "JSON 窗口数 %d == 重解析 %d" % (len(jwins), len(win_files)))
jm = [x for x in jwins if x["name"] == "MainWindow"][0]
check(jm["width"] == 1500 and jm["height"] == 950 and jm["min_width"] == 1100 and jm["min_height"] == 720,
      "JSON 里主窗 1500x950 / min 1100x720")
check(jm["grid_rows"] == [36, 0, 48, "*", 30], "JSON 里主窗根 Grid 行 = %s" % jm["grid_rows"])
def cnt(nodes): return sum(1 + cnt(n["children"]) for n in nodes)
total_parse = sum(1 + count_tree(first_grid(roots[fn])) for fn in win_files)
total_json = sum(cnt(w["children"]) for w in jwins) + len(jwins)
check(total_parse == total_json, "节点总数：重解析 %d == JSON %d" % (total_parse, total_json))

print("== 4) 统计行 ==")
theme_colors, k, u = len(data["theme"]["colors"]), len(data["code_built"]), len(data["unknowns"])
print("windows=%d, total_nodes=%d, theme_colors=%d, code_built=%d, unknowns=%d"
      % (len(jwins), total_json, theme_colors, k, u))

print("== 5) 下限断言 ==")
check(theme_colors >= 20, "theme_colors=%d >= 20" % theme_colors)
check(total_json >= 80, "total_nodes=%d >= 80" % total_json)
check(len(data["theme"]["semantic"]) >= 40, "semantic %d >= 40" % len(data["theme"]["semantic"]))
check(all(x["line"].split(":")[1] for x in data["code_built"]), "code_built 全部带 file:line")

print()
if fails:
    print("验收门 FAILED（%d 项）" % len(fails))
    for f in fails: print("  - " + f)
    sys.exit(1)
print("验收门全部通过")
```

实跑输出（把上面这段脚本原样存成 `verify_cs_ui.py` 后执行，2026-11-04）：

```
== 1) 解析 9 个 axaml ==
  [PASS] 解析 axaml 文件数 = 9（期望 9）
  非窗口资源文件：App.axaml, EditorControls.axaml
  [PASS] 工程实际窗口数 = 7（派单写的 9 含 2 个资源字典，见 README §4 冲突 1）
== 2) 主窗骨架数字 ==
  [PASS] MainWindow 1500x950
  [PASS] MainWindow min 1100x720
  [PASS] 根 Grid 行 = [36, 0, 48, '*', 30]（期望 [36,0,48,*,30]）
  根 Grid 列 = []（根 Grid 未声明列；工作区列在嵌套 Grid 上）
  [PASS] 嵌套工作区 Grid 列 = [72, '*', 5, 360]（期望 [72,*,5,360]）
  [PASS] 工作区 Grid 列数 = 4（派单要求 4）
== 3) 与产出 JSON 交叉校验 ==
  [PASS] json.load 解析成功，文件 514877 字节
  顶层键：['source', 'extracted_at', 'xaml_files', 'theme', 'windows', 'code_built', 'unknowns']
  [PASS] JSON 窗口数 7 == 重解析 7
  [PASS] JSON 里主窗 1500x950 / min 1100x720
  [PASS] JSON 里主窗根 Grid 行 = [36, 0, 48, '*', 30]
  [PASS] 节点总数：重解析 499 == JSON 499
== 4) 统计行 ==
windows=7, total_nodes=499, theme_colors=71, code_built=52, unknowns=18
== 5) 下限断言 ==
  [PASS] theme_colors=71 >= 20
  [PASS] total_nodes=499 >= 80
  [PASS] semantic 55 >= 40
  [PASS] code_built 全部带 file:line

验收门全部通过
```

---

## 7 复现与维护

1. **抽取器**：一个纯函数式的 Python 脚本（标准库 `xml.etree` + `json` + `re`），按 §2 规则把 9 个 axaml 折成树，再拼上 `code_built` / `unknowns` 两张手写表。按"只允许写两个产出文件"的约束，脚本未落在仓库里（跑完即删），但规则在 §2 已完整写明，可直接照此重写。
2. **手写表自带校验**：`code_built`/`metrics`/`semantic`/`inline_colors` 里的每一条 `文件:行号`，在写盘前都会**回读该行并断言目标片段存在**，一条不符就中止退出——这轮就靠它抓出 22 处行号/写法偏差（如 `App.axaml:70` 的键是 `Muted/Accent/Line` 而非 `Theme929CAB`；`Color.FromRgb(228,158,151)` 没有十六进制字面量）。所以 JSON 里的行号是"机器验过的"，不是抄的。
3. **改动源工程后**：JSON 的断言值（窗口数 7、主窗数字、节点数 499）都是**该快照的事实**，源变了必须重新抽取并重新取基线，不要复用本文数字。
4. **给 Rust 侧的三条提醒**：
   - 别照 `MainWindow.axaml` 直接复刻主窗——运行期是 `DockWorkspace` 的停靠面板（§5 第 8 条）；
   - 颜色别只取 `App.axaml` 静态值——运行期会被 `ThemeService.ApplyPalette` 按系统 accent 重算（§5 第 9 条）；
   - 图标别只存 `Viewbox` 尺寸——墙体的 18×18 是例外，且 MDL2 字形需要自备矢量（§5 第 1、2 条）。
