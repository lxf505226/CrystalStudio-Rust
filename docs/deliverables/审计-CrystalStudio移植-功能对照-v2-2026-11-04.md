# 审计 · CrystalStudio（**新版**）→ Rust 移植（`tools/miredit`）功能缺口清单 **v2**

> 审计日期：**2026-11-04**（系统时间实测 `2026-11-04 21:0x`）｜性质：**只读审计**
> 本会话**未修改**任何源码 / 配置 / 文档 / `.map` / `catalogue` / `Objects`；唯一产出是本文件。
>
> **v2 为什么存在**：v1（`deliverables/审计-CrystalStudio移植-功能对照-2026-11-04.md`，125 功能点：有 23 / 部分 8 / 无 94）基于**旧版源码**。
> 上游今日两批更新：① `d2e8e436` 中文切换（`LocalText.cs` + `Localization/Strings.json` 893 条 + 39 文件改造）；
> ② `6f2dadde` 素材提取工具（`ObjectExtraction.{Models,Service,Export}.cs` + `ObjectExtractionWindow.cs` + `MainWindow.ObjectExtraction.cs` + 339 行测试）。
> 本报告**重新从源码取证**，不继承 v1 的任何单条判定（只继承它的**问题清单骨架**以便对账）。

---

## 〇、路径与检索口径约定（全文通用）

| 简写 | 实际路径 | 说明 |
|---|---|---|
| `CS/` | `E:\GodM2\CrystalStudio\CrystalStudio\` | C# 业务源码，**排除** `bin\` `obj\` |
| `CST/` | `E:\GodM2\CrystalStudio\CrystalStudio.Tests\` | C# 测试 |
| `RS/` | `E:\GodM2\tools\miredit\crates\` | 5 个 crate，检索面 = `**/src/**/*.rs` + `**/tests/**/*.rs`，**排除** `target\` |
| `ASSET/` | `E:\GodM2\CrystalStudio\catalogue\` + `Objects\` | 数据资产 |

**实测规模（本会话用命令数出来的，不用任务书数字）**

| 侧 | 文件数 | 行数 | 命令 |
|---|---|---|---|
| C# 业务 | **162** 个 `.cs` | **22,423** 行 | `Get-ChildItem CS -Recurse -Filter *.cs`（排除 bin/obj）再 `Measure-Object -Line` |
| C# 测试 | 2 个 `.cs`（`MigrationTests` 644 行 + `ObjectExtractionTests` 335 行） | 979 行 | 同上（`CST/`） |
| C# 合计 | **164** 个 / **23,402** 行 | | |
| Rust | **59** 个 `.rs` | **22,202** 行 | `Get-ChildItem RS -Recurse -Filter *.rs`（排除 target） |
| C# 测试属性 | `[Fact]/[Theory]/[AvaloniaFact]` **45** 条（Migration 22 + 提取 23） | | `Select-String '^\s*\[(Fact|Theory|AvaloniaFact|InlineData)'` |
| Rust 测试属性 | `#[test]` **258** 条 | | `Select-String '^\s*#\[(test\|test_case)'` |

> ⚠ **与任务书口径的差异（如实登记）**：任务书说「165 个 `.cs` / 22,454 行」，我实测 **164 / 23,402（含测试）**、**162 / 22,423（不含测试）**。
> 差 1 文件 / 31 行，我判断是统计口径差异（是否含测试目录、行尾空行计法），**未找到第 165 个文件**。
> 下文所有结论以**我实测的这 164 个文件**为准。
>
> ⚠⚠ **另一路正在改 `miredit-gui`**（不是本会话）：`edit.rs` 2,010→2,196 行、`ui_text.rs` 981→1,023 行、`metrics.rs` 481→483 行，
> 全部发生在 **21:03–21:16** 我读完之后。本报告对这些文件的引用**同时给符号名**，并在 **§8.2.1** 给出「读取时行号 → 21:17 复解行号」对照表 ——
> **复现请按符号检索**，不要死记行号。

---

## 一、一句话结论

把新版运行期 UI 拆成 **63 个界面级功能点（表 A）** + **66 个能力级功能点（表 B）**，共 **129 项**：
Rust **有 21 项（16.3%）、部分 15 项（11.6%）、无 93 项（72.1%）**；其中 **P0 共 31 项，P0 且"无/部分"（真缺口）22 项**。

三句话概括变化：

1. **比 v1 强的地方是真强**：三层（地表/中层/物体）**都可写**了（v1 只有地表）、`mir2-stamp` 已把 **9,170 个 `.X` 全部逐字节往返验证**（v1 是零读取）、`miredit-render` 已有**阻挡可视化通道**、小地图从"假灰块"变成**真视口框**。
2. **比 v1 弱/更糟的地方**：这些新能力**全部停在 crate 层，一个都没接进 GUI** —— `mir2-stamp` 连 `miredit-gui` 的依赖表都没进（`RS/miredit-gui/Cargo.toml` 只依赖 `mir2-map`/`mir2-assets`/`miredit-render`），`CollisionOverlay` 在 GUI 里 **0 命中**。
3. **新版新增的两块（i18n、素材提取）我们 0 覆盖**，且 i18n 的价值不只是"多语言"：它把 **893 条界面文案**做成了一张可机检的表，这恰好是我们 `ui_text.rs`（**1,023 行**、自有中文模板）的**对照基准**。

### 13+2 域完成度打分（口径：功能点计数，部分不计入"有"）

| # | 功能域 | 功能点 | 有 | 部分 | 无 | 完成度 |
|---|---|---|---|---|---|---|
| 1 | 文件菜单（10） | 10 | 2 | 1 | 7 | 20% |
| 2 | 编辑菜单（4） | 4 | 2 | 0 | 2 | 50% |
| 3 | 视图菜单（17） | 17 | 3 | 6 | 8 | 18% |
| 4 | 工具菜单（6） | 6 | 0 | 0 | 6 | 0% |
| 5 | 帮助（1） | 1 | 0 | 0 | 1 | 0% |
| 6 | 工具栏（10） | 10 | 2 | 1 | 7 | 20% |
| 7 | 面板（7） | 7 | 5 | 2 | 0 | 71% |
| 8 | 图层行（7） | 7 | 3 | 0 | 4 | 43% |
| 9 | 独立窗口（1） | 1 | 0 | 0 | 1 | 0% |
| 10 | 编辑内核（16） | 16 | 2 | 1 | 13 | 13% |
| 11 | 印章体系（8） | 8 | 1 | 0 | 7 | 13% |
| 12 | 素材与数据资产（10） | 10 | 1 | 1 | 8 | 10% |
| 13 | 显示与视图内核（16） | 16 | 0 | 2 | 14 | 0% |
| 14 | 文档与格式（3） | 3 | 0 | 1 | 2 | 0% |
| 15 | 设置内核（3） | 3 | 0 | 0 | 3 | 0% |
| 16 | **i18n（4，新增）** | 4 | 0 | 0 | 4 | 0% |
| 17 | **素材提取（6，新增）** | 6 | 0 | 0 | 6 | 0% |
| | **合计** | **129** | **21** | **15** | **93** | **16.3%** |

> 三种口径都给，避免被读成"退步"：
> ① 按功能点 = **16.3%**（21/129）；
> ② 部分按 0.5 计 = **22.1%**（(21+7.5)/129）；
> ③ **17 域简单平均 = 19.5%**。
> ⚠ **不要与 v1 的 18.4% 直接比** —— v2 把 v1 的 1 条"面板显隐菜单"拆成 6 条、把"工具按钮组"拆成 10 条、并新增 i18n 4 + 提取 6，
> **分母口径变了**。可比的只有逐项映射（见 §五）。

---

## 二、逐项判定表 **A**：新版运行期界面骨架（63 项）

> 骨架来自**新版运行期 UI Automation 实读**（`tools/miredit/layout/refs/crystalstudio-new-zh.png` + 实测 UIA），
> 与源码逐条对齐后的结论：**§三 清单与源码 100% 一致**（含 10 个工具按钮、7 个面板、7 个图层行、1 个独立窗口）。
> **分级口径**：P0 = 缺了就不像"地图编辑器"｜P1 = 影响效率或数据完整性｜P2 = 可省/可后置。
> ⚠ 对**已实现**项，分级表示"该功能点本身的紧要度"，**不是缺口等级**；缺口等级 = 分级 × 状态。

### 2.1 文件(F)（10 项：有 2 / 部分 1 / 无 7）

| # | 功能点 | C# 证据（文件:行） | Rust 现状 | 分级 |
|---|---|---|---|---|
| A1 | 新建地图(N)… `Ctrl+N` | `CS/MainWindow.axaml:5`（菜单项）；`CS/MainWindow.axaml.cs:420-450`（`NewClick`：400 宽对话框 + 3 个预设 800×600/400×800/1000×1000 + 16–4096 校验 `:448`）；`CS/MainWindow.MapTabs.cs:40-55`（`AddMapTab`） | **无**。GUI 无入口；库层也没有空白图构造函数 —— `mir2-map` 的 `Map` 只有 `parse`/`to_bytes`/`cell`/`walkable`（`RS/mir2-map/src/map.rs:43,60,117,128,139`），字段虽全公开（`:28-41`）但需调用方自己拼 52 字节头。检索口径：`new_map` `NewMap` `--new` 在 `RS/` 全 **0** | **P0** |
| A2 | 打开地图(O)… `Ctrl+O` | `CS/MainWindow.axaml.cs:452`（`OpenFileDialog`，`Multiselect = true`）；`:454-464`（同路径去重、`Task.Run` 读盘、状态栏文案）；`CS/MainWindow.axaml:5`（`InputGesture="Ctrl+O"`） | **部分**。能打开，但**只能命令行传路径**：`--map`（`RS/miredit-gui/src/cli.rs:26,114` → `RS/miredit-gui/src/mapdoc.rs:101-144`）。**无文件对话框**（GUI 依赖表里没有 `rfd`/任何对话框库，`RS/miredit-gui/Cargo.toml:9-24`），无多选，无去重。检索口径：`rfd` `FileDialog` `OpenFileDialog` 全 **0** | P1 |
| A3 | 最近打开(R) 子菜单 | `CS/MainWindow.RecentMaps.cs:8-14`（`InitializeRecentMaps`）、`:23-40`（逐条菜单 + 失效置灰 `:33` + tooltip `:35`）；`CS/RecentMapStore.cs:8-39` | **无**。检索口径：`recent`/`Recent` 在 `RS/` **6 处命中，全非本功能** —— `RS/miredit-gui/src/dock_right.rs:214`、`RS/miredit-gui/src/metrics.rs:377`、`RS/miredit-gui/src/ui_text.rs:267` 都是"最近使用"素材页签文案 | P2 |
| A4 | 保存地图(S) `Ctrl+S` | `CS/MainWindow.axaml.cs:488,494-503`（`SaveMap` → `Document.Save`） | **有**。`RS/miredit-gui/src/save.rs:292-330`（`save_session`：只在 `dirty` 时真写）；原子替换 `RS/miredit-gui/src/save.rs:186-…`（写同目录 tmp → **读回 memcmp** → rename）；入口 `RS/miredit-gui/src/app.rs:309`（`Ctrl+S`） | P0 |
| A5 | 另存为(A)… `Ctrl+Shift+S` | `CS/MainWindow.axaml.cs:490`（`SaveAsClick`）、`:494-503`（`saveAs=true` ⇒ 走 `SaveFileDialog`） | **无**。`save_session` 只写回 `d.path`（`RS/miredit-gui/src/save.rs:301`）；检索口径：`save_as` `SaveAs` 全 **0**，CLI 无 `--save-as`（`RS/miredit-gui/src/cli.rs:113-230` 全部选项已逐条读过） | **P0** |
| A6 | 导出小地图(M)… | `CS/MainWindow.MinimapExport.cs:6-35`（`SaveFileDialog` + `MinimapExporter.ExportAsync` + 完成文案 `:33`）；`CS/MinimapExporter.cs:15-53`（12×8 px/格） | **无**。检索口径：`export`/`Export` 在 `RS/` 全 **0** | P1 |
| A7 | 新建物体(S) `Ctrl+Shift+N` | `CS/MainWindow.axaml:5`（`NewStampButton`）→ `CS/MainWindow.StampEditing.cs:40-56`（新建印章文档、尺寸 = 内容 + 17） | **无**。检索口径：`NewStamp`/`new_stamp` 全 **0** | P2 |
| A8 | 导入物体(I)… | `CS/MainWindow.axaml.cs:582`（`StampClick`：`OpenFileDialog` 过滤 `*.X` → `CellCodec.LoadStamp` → `SetTool(Stamp)`）；`CS/MainWindow.axaml:5` | **无（GUI）**。⚠ 底层**已就绪**：`RS/mir2-stamp` 能读全部 9,170 个 `.X` 并逐字节写回（`RS/mir2-stamp/src/lib.rs:1-125` 文档 + `RS/mir2-stamp/src/stamp.rs`），但**没有任何 crate 依赖它**（`RS/*/Cargo.toml` 只有它自己的），GUI 更没接 | **P0** |
| A9 | 导出选区(E)… | `CS/MainWindow.axaml.cs:584`（`ExportClick`）→ `CS/MainWindow.StampEditing.cs:57-89`（`CropStamp` 偶数对齐） | **无**。检索口径：`Export`/`Crop`/`导出` 全 **0** | P1 |
| A10 | 退出(X) | `CS/MainWindow.axaml.cs:486`（`ExitClick` ⇒ `Close()`）；关窗拦截 `:408-418` | **有**。`RS/miredit-gui/src/app.rs:909-923`（右上 × ⇒ 脏则先弹模态）+ `:481-499`（`guard_close_request`）+ `:512-535`（模态绘制） | P2 |

### 2.2 编辑(E)（4 项：有 2 / 无 2）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| A11 | 撤销(U) `Ctrl+Z` | `CS/MainWindow.axaml.cs:635`（`UndoClick`）→ `CS/Document.cs:149-204`（`EditHistory`：`Record/Commit/Swap/Saved`，上限 100 `:176-181`） | **有**。`RS/miredit-gui/src/edit.rs:374`（`EditSession`）、`:307-369`（笔画合并）、`:168,364-367`（`UNDO_LIMIT=100`）、`:190,253-255,289-291`（保存点）；快捷键 `RS/miredit-gui/src/app.rs:307`；逐字节还原断言 `RS/miredit-gui/src/edit.rs:1022-1048`（v1 口径） | P0 |
| A12 | 重做(R) `Ctrl+Shift+Z` | `CS/MainWindow.axaml.cs:637`（`RedoClick`）；`CS/MainWindow.axaml:5` 的 `InputGesture="Ctrl+Shift+Z"` | **有**。`RS/miredit-gui/src/app.rs:308`（`Ctrl+Y`）；⚠ 与 C# 的键位不同（C# 是 `Ctrl+Shift+Z`，Rust 是 `Ctrl+Y`，`Shift+Z` 未接） | P0 |
| A13 | 填充选区(F) | `CS/MainWindow.axaml.cs:639`（`FillClick`：无选区时状态栏提示）；算法 `CS/MapViewport.cs:485-539`（BFS + `visited` + 一次 `Commit`）、`:540-546`（按层清除） | **无**。检索口径：`flood` `fill_region` 全 **0**；`bucket`/`Bucket` **5 处全在图标表**（`RS/miredit-gui/src/icons.rs:10,80,82,228` 等），`Tool::from_index` 把下标 2 判成 `Unwired`（`RS/miredit-gui/src/edit.rs:211-218`） | **P0** |
| A14 | 设置(S)… | `CS/MainWindow.axaml.cs:505-546`（`OpenSettings`：目录选择 + 失败原文 + 首次引导）；`CS/SettingsWindow.cs:1-43`；`CS/SettingsStore.cs:10-25`（`settings.json`） | **无**。检索口径：`settings`/`Settings` 全 **0**；图库根目录是硬编码常量 `RS/mir2-assets/src/lib.rs:85`（`DEFAULT_CLIENT_ROOT`），且**无环境变量回落**（`RS/` 里 `env::var` 仅 1 处：`RS/miredit-gui/src/canvas_view.rs:338` 的 `MIREDIT_QUIET`） | **P0** |

### 2.3 视图(V)（17 项：有 3 / 部分 6 / 无 8）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| A15 | 深色模式(D)（勾选） | `CS/MainWindow.axaml:5`（`DarkModeToggle`）；`CS/MainWindow.axaml.cs:492`（`ThemeService.SetDarkOverride`）；`CS/ThemeService.Avalonia.cs:11-58`（按系统 accent 重算 40+ 色键 + `appearance.json`） | **无**。`RS/miredit-gui/src/theme.rs:1-11` 是**一套固定浅色**（取运行期截图直方图，注释明写"C# 的深色是 XAML 声明值，上屏是浅色 + 金色强调"）。检索口径：`dark_mode` `DarkMode` `深色` 全 **0**；`dark`/`Dark` **15 处全是 `ACCENT_DARK` 色常量**（如 `RS/miredit-gui/src/theme.rs:77`） | P2 |
| A16 | 单元格网格(G)（勾选，默认开） | `CS/MainWindow.axaml:5`（`GridToggle IsChecked="True"`）；`CS/MapViewport.Presentation.cs:51-55`（缩放 ≥0.4 才画）；`CS/MainWindow.axaml.cs:867`（`G` 键） | **无（刻意撤掉）**。`RS/miredit-gui/src/canvas.rs:12-14` 明写"装饰网格与装饰选框**故意撤掉**"，理由是避免与客户端同视点比对产生色差。检索口径：`show_grid` 0；`grid`/`GRID` 命中全为资源面板网格（`PALETTE_GRID_COLS` 等） | P2 |
| A17 | 地图光照(L) 三档 Dawn/Dusk·Day·Night | `CS/MainWindow.axaml:5`（`MapLightingMenu` + 3 个子项 `Tag="DawnDusk/Day/Night"`）；`CS/MainWindow.axaml.cs:626-633`（`LightingClick`）；`CS/MapPreview.cs:21-82`（`PreviewLights.{445,685,925}.png` 取 R 通道做加法） | **无**。检索口径：`lighting`/`Lighting` 全 **0**；`light`/`Light` 55 处**全是只读显示**（`RS/miredit-gui/src/cellinfo.rs:71,175` 取 `c.light` 打字；`RS/miredit-gui/src/ui_text.rs:148` 是"灯光单元格"文案） | P1 |
| A18 | 开启门(O)（勾选） | `CS/MainWindow.axaml:5`（`DoorsOpenToggle`）；`CS/MapPreview.cs:43-47`（`doorsOpen` 时叠加 `DoorOffset`）；`CS/MainWindow.axaml.cs:611-625`（`ViewChanged`） | **无**。检索口径：`doors_open`/`DoorsOpen` 全 **0**；`door`/`Door` 73 处全是只读显示与文案（`RS/miredit-gui/src/cellinfo.rs:72,176`） | P1 |
| A19 | 放大(I) | `CS/MainWindow.axaml.cs:1043`（`SetZoom(Zoom*1.2)`）；`CS/MapViewport.cs:173-180` | **有**。`RS/miredit-gui/src/app.rs:1051-1061`（+ 按钮）；滚轮以鼠标为中心 `:1028-1037`；夹在 `CANVAS_MIN_SCALE..=CANVAS_MAX_SCALE`（`RS/miredit-gui/src/canvas_view.rs:156-182`） | P1 |
| A20 | 缩小(O) | `CS/MainWindow.axaml.cs:1045`（`/1.2`） | **有**。`RS/miredit-gui/src/app.rs:1040-1050`（− 按钮） | P1 |
| A21 | 实际大小（100%）(A) | `CS/MainWindow.axaml.cs:1047`（`SetZoom(1)`） | **无**。只有"适应窗口"与 ± 两档（`RS/miredit-gui/src/app.rs:1040-1073` 三个可点区：`zoom_minus`/`zoom_plus`/`zoom_fit`），**没有 1.0 的按钮/快捷键**；只能靠滚轮凑（`--scale` 是命令行参数，`RS/miredit-gui/src/cli.rs:31`） | P2 |
| A22 | 适应地图(F) | `CS/MainWindow.axaml.cs:710`（`FitClick` ⇒ `Viewport.Fit()`） | **有**。`RS/miredit-gui/src/app.rs:1062-1073`（`zoom_fit` ⇒ `fit_map`）；实现 `RS/miredit-gui/src/canvas_view.rs:166` | P1 |
| A23 | 语言（重启后生效）子菜单（简体中文 / English） | `CS/MainWindow.Localization.cs:7-17`（动态建菜单 `LanguageMenu` + 两个勾选项）；`:19-29`（`ChooseLanguage` ⇒ `LocalText.SaveLanguage` + 状态栏"重启后生效"）；`:31-36`（回读勾选态）；挂载点 `CS/MainWindow.axaml.cs:121` | **无**。检索口径：`language`/`Language` `locale` `i18n` `LocalText` 全 **0**；界面文案是**编译期中文常量**（`RS/miredit-gui/src/ui_text.rs:981` 行，`MENU_ITEMS` 5 项见 `:96-102`），无运行时切语言机制 | P1 |
| A24 | 工具（面板显隐） | `CS/DockWorkspace.cs:269-280`（`AddSectionMenu` → 每面板一个 `CheckBox` 菜单项）；`:281-287`（`SetVisible`）；面板注册 `CS/MainWindow.axaml.cs:338` | **部分**。面板**在**（`RS/miredit-gui/src/layout.rs:65`），但**无菜单项、无显隐开关**（固定几何，`RS/miredit-gui/src/layout.rs:1-90`）。检索口径：`dock`/`section` 菜单 0；`menubar.rs:7` 明写"本单不做下拉菜单" | P2 |
| A25 | 工具选项（面板显隐） | 同上；注册 `CS/MainWindow.axaml.cs:340` | **部分**。面板在（`RS/miredit-gui/src/layout.rs:66`），内容只是**装饰**：色块 + 滑杆轨道 + "3×3" 胶囊（`RS/miredit-gui/src/dock_left.rs:440-465`），滑杆**不可交互** | P2 |
| A26 | 图层（面板显隐） | 同上；注册 `CS/MainWindow.axaml.cs:344` | **部分**。面板在（`RS/miredit-gui/src/layout.rs:67`），无显隐开关 | P2 |
| A27 | 小地图（面板显隐） | 同上；注册 `CS/MainWindow.axaml.cs:348` | **部分**。面板在（`RS/miredit-gui/src/layout.rs:71`），无显隐开关 | P2 |
| A28 | 资源面板（面板显隐） | 同上；注册 `CS/MainWindow.axaml.cs:353` | **部分**。面板在（`RS/miredit-gui/src/layout.rs:72`），无显隐开关 | P2 |
| A29 | 单元格检查器（面板显隐） | 同上；注册 `CS/MainWindow.axaml.cs:379` | **部分**。面板在（`RS/miredit-gui/src/layout.rs:68`），无显隐开关 | P2 |
| A30 | 锁定面板布局 | `CS/MainWindow.axaml.cs:391-394`（`lockLayout` 勾选 ⇒ `docking.SetLayoutLocked`）；`CS/DockWorkspace.cs:44-52`（锁定时 `SetVisible` 直接 return） | **无**。检索口径：`lock_layout`/`LockLayout` 全 **0** | P2 |
| A31 | 重置面板布局 | `CS/MainWindow.axaml.cs:389-396`（`reset` 菜单项 ⇒ `docking.Reset()`）；`CS/DockWorkspace.cs:219-226` | **无**。检索口径：`reset_layout`/`Reset` 全 **0**（布局无持久化，故无从重置） | P2 |

### 2.4 工具(T)（6 项：无 6）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| A32 | 生成城镇(T)… | `CS/MainWindow.axaml:5`（`TownStampMenuButton` → `TownStampClick`）；`CS/TownGenerator.cs:1-332` + `TownWalls.cs:268` + `TownStreetLayout.cs:143` + `TownCatalogue.cs:65`（`towns.json` 1.55 MB）；窗口 `CS/TownStampWindow.axaml.cs:1-143` | **无**。检索口径：`town`/`Town` `generator`/`Generator` 全 **0**（`generate`/`Generate` 8 处命中全在 `RS/mir2-stamp/src/roundtrip_tests.rs:171-175` 的测试辅助函数名） | P2 |
| A33 | 生成地形(G)… | `CS/TerrainGenerator.cs:1-424` + `TerrainCatalogue.cs:105` + `TerrainWater*.cs`（合计 2,359 行）；窗口 `CS/TerrainGeneratorWindow.axaml.cs:197` | **无**。同上检索口径，0 | P2 |
| A34 | 生成省域(P)… | `CS/ProvinceGenerator.cs:1-259` + 9 个附属（`ProvinceTowns.cs:403`、`ProvinceCliffTerrain.cs:257` 等，合计 1,530 行）；窗口 `CS/ProvinceGeneratorWindow.axaml.cs:131` | **无**。检索口径：`province`/`Province` 全 **0** | P2 |
| A35 | 生成地牢(D)… | `CS/TempleGenerator.cs:1-341`（11 种预设 `:10-20`）+ `ZumaTempleSideRooms.cs:113` + `TempleFloorCoverage.cs:111` + `StoneTempleSpecialWalls.cs:54`；窗口 `CS/TempleGeneratorWindow.axaml.cs:124` | **无**。检索口径：`temple`/`Temple` 全 **0** | P2 |
| A36 | 识别物体(I)… | `CS/MainWindow.axaml.cs:466-484`（`IdentifyStampsClick`：`StampScanWindow` → `RememberIdentifiedStamps` → 状态栏 N/M 匹配数）；`CS/StampIdentifier.cs:15-130`（按图号倒排索引 → 最稀有格先比 → 认领去重） | **无**。检索口径：`identify`/`Identify` 全 **0** | P1 |
| A37 | **提取地图物体**（新版新增） | `CS/MainWindow.axaml:5`（`ExtractObjectsButton` → `ExtractObjectsClick`）；`CS/MainWindow.ObjectExtraction.cs:8-21`；`CS/ObjectExtractionWindow.cs:34-82`；服务/导出/模型见 §六 | **无**。检索口径：`extract`/`Extract` `ObjectExtraction` 全 **0** | P1 |

### 2.5 帮助(H)（1 项：无 1）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| A38 | 键盘快捷键(S)… `F1` | `CS/MainWindow.axaml:5`（`InputGesture="F1"`）；`CS/MainWindow.axaml.cs:1089-1111`（19 行快捷键表对话框，**每行 action/key 都过 `LocalText.T`** 见 `:1105`）；`F1` 分发 `:865` | **无**。检索口径：`shortcuts`/`Shortcuts`/`快捷键` 全 **0** | P2 |

### 2.6 工具栏（10 项：有 2 / 部分 1 / 无 7）

> 10 个按钮源码实证：`CS/MainWindow.axaml:9-19` 的 `Tag=` 依次为
> `Brush` `Pencil` `Bucket` `Eraser` `Select` `Stamp` `Pickup` `Clone` `Wall` `SpecialBrush`。
> ⚠ **"移动"= `Pickup`**（`CS/../Strings.json` 的 `Pickup_B685076A` 英文 `Pickup`、中文 `移动`），与运行期 UIA 读到的"移动"一致；
> 源码里**没有** `Move` 这个 `EditorTool`（枚举 `CS/MapViewport.cs:4`：`Brush, Pencil, Bucket, Eraser, Select, Stamp, Pickup, Clone, SpecialBrush, Wall, TownStamp`）。

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| A39 | 笔刷（B） | `CS/MainWindow.axaml:9`；`CS/MainWindow.axaml.cs:598`（`ToolClick`）、`:861`（`B` 键）；落笔 `CS/MapViewport.cs:479-480`（按层写 `BackImage`/`MiddleImage`/`Foreground`） | **有**。10 个按钮全在（`RS/miredit-gui/src/app.rs:963-971`），`Brush` 是下标 0（`RS/miredit-gui/src/edit.rs:180,213`）；落笔 `RS/miredit-gui/src/edit.rs:800-835`（**按当前层写**，含库号字节） | P0 |
| A40 | 铅笔（P） | `CS/MainWindow.axaml:10`；`:861`（`Key.P`） | **无（未接线）**。下标 1 ⇒ `Tool::Unwired`（`RS/miredit-gui/src/edit.rs:211-218`），点画布 `PaintOutcome::NoOp`（`:832`） | P0 |
| A41 | 填充（F） | `CS/MainWindow.axaml:11`（`BucketToolButton`）；`:861`（`Key.F`）；算法 `CS/MapViewport.cs:485-539` | **无（未接线）**。下标 2 ⇒ `Unwired`；无 BFS（`flood` 0） | P0 |
| A42 | 橡皮擦（E） | `CS/MainWindow.axaml:12`；`:861`（`Key.E`）；写格 `CS/MapViewport.cs:480` | **有**。下标 3（`RS/miredit-gui/src/edit.rs:182,214`）；只清当前层图号并**保留 `0x8000`**（`RS/miredit-gui/src/edit.rs:827-831`、`COLLISION_BIT` `:187`） | P0 |
| A43 | 选择（M） | `CS/MainWindow.axaml:13`；`:861`（`Key.M`）；框选 `CS/MapViewport.cs:387-401` | **无（未接线）**。下标 4 ⇒ `Unwired`；检索口径：`selection`/`Selection` 全仓 **1 处**且是注释（`RS/miredit-gui/src/layout.rs:80`）；`clipboard`/`paste`/`RegionCopy` 全 **0** | **P0** |
| A44 | 物体模板（O） | `CS/MainWindow.axaml:15`；`:861`（`Key.O` ⇒ `EditorTool.Stamp`）；放置 `CS/MapViewport.cs:44-74` | **无（未接线）**。下标 5 ⇒ `Unwired`；`.X` 读取器在 `RS/mir2-stamp` 但未接 GUI（见 A8） | **P0** |
| A45 | 移动（V，`Pickup`） | `CS/MainWindow.axaml:16`；`:861`（`Key.V`）；语义 = 点已记住的印章再点移动 / `Delete` 删除（`CS/MainWindow.axaml.cs:799,873`；tooltip `Strings.json` 的 `PickupV…`） | **部分**。下标 6 = 吸管（`RS/miredit-gui/src/edit.rs:184,215`），只把**当前层**图号取成笔刷（`:203,832`）；**没有"已放置印章"这个概念**（无 `PlacedStamps`），故"移动/删除已放置物体"不存在 | P1 |
| A46 | 克隆（K） | `CS/MainWindow.axaml:17`（`CloneToolButton`）；`:861`（`Key.K`） | **无（未接线）**。下标 7 ⇒ `Unwired` | P1 |
| A47 | 墙体（H） | `CS/MainWindow.axaml:18`（`WallToolButton` → `WallBrushClick`）；`:863`（`Key.H`）；`CS/WallBrush.cs:1-944`（19 族 58.9 MB） | **无**。下标 8 ⇒ `Unwired`；检索口径：`WallBrush` 0；`wall` 命中全在图标表（`RS/miredit-gui/src/icons.rs`） | P1 |
| A48 | 特殊笔刷（T） | `CS/MainWindow.axaml:19`（`SpecialBrushToolButton`）；`:862`（`Key.T`）；`CS/TreeBrush.cs:44-79` + `SpecialBrushWindow.axaml.cs:384` | **无**。下标 9 ⇒ `Unwired`；检索口径：`tree` 命中全是 `cargo tree` 等无关词 | P1 |

### 2.7 面板（7 项：有 5 / 部分 2）

> C# 的 7 个面板由 `DockWorkspace.Add` 注册（`CS/MainWindow.axaml.cs:338-379`），名字依次为
> `Tools` / `Tool Options` / `Layers` / `Minimap` / `Palette` / `Active Asset` / `Cell Inspector` —— **与运行期 UIA 读到的 7 个完全对应**。
> Rust 侧 7 个面板矩形齐备：`RS/miredit-gui/src/layout.rs:65-73`。

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| A49 | 工具面板 | `CS/MainWindow.axaml.cs:338`；内容 `CS/MainWindow.axaml:8-20`（`ResponsivePanel`） | **有**。`RS/miredit-gui/src/layout.rs:65` + 绘制 `RS/miredit-gui/src/dock_left.rs`（工具网格）+ 点击 `RS/miredit-gui/src/app.rs:963-971` | P1 |
| A50 | 工具选项面板 | `CS/MainWindow.axaml.cs:340`；内容 `CS/MainWindow.axaml:6`（`BrushSize` 滑杆 3–31 步进 2 + `SizeControls`） | **部分**。面板在（`RS/miredit-gui/src/layout.rs:66`），内容**纯装饰**（色块/轨道/"3×3"胶囊，`RS/miredit-gui/src/dock_left.rs:440-465`），**滑杆不可拖**、无尺寸概念（`BrushSize`/`brush_size` 全 **0**） | P1 |
| A51 | 图层面板 | `CS/MainWindow.axaml:29-63`（5 个 `ListBoxItem` + 1 个"单元格高亮"子组 2 个 `CheckBox`） | **有（3/5 接线）**。5 行：`RS/miredit-gui/src/dock_left.rs:48-50`（`layer_wired(i) = i<3`）；行 0-2 眼睛切可见性 + 行选为编辑层（`RS/miredit-gui/src/app.rs:977-1008`、`RS/miredit-gui/src/dock_left.rs:86-112`）；行 3/4 **点了没反应**（`:105-111`） | **P0** |
| A52 | 小地图面板 | `CS/MapMinimap.Avalonia.cs:1-92`（真实缩略图 + 视口框 + 点击居中 + 拖拽 + 滚轮） | **部分**。真视口框（`RS/miredit-gui/src/dock_right.rs:431-437` 的 `MiniMap{map,view}`、`:462-500` 的 `minimap_rects` 按 `pan/scale` 换算）；但**没有地图缩略内容**（`RS/miredit-gui/src/dock_right.rs:533` 注释"本单没有真缩略图，如实不画假图"），**无点击/拖拽/滚轮** | P1 |
| A53 | 资源面板 | `CS/AssetPalette.cs:19-36`（4 页签）+ `CS/MainWindow.axaml.cs:728-759`（分页/过滤） | **有**。`RS/miredit-gui/src/dock_right.rs:39-40` + `RS/miredit-gui/src/palette.rs`（库下拉/缩略图网格/图号搜索/点选设笔刷/翻页）；页签 4 个（`RS/miredit-gui/src/ui_text.rs:263-267`），仅页签 0 有内容（`:314` 明写"该页签未实现"） | P1 |
| A54 | 单元格检查器 | `CS/MainWindow.axaml.cs:188-248`（六组 + 详情 + 提示） | **有**。`RS/miredit-gui/src/cellinfo.rs:1-354`（纯函数取数，六行 + 取不到给 `—`）；绘制 `RS/miredit-gui/src/dock_left.rs:213-243` | P1 |
| A55 | 当前资源 | `CS/MainWindow.axaml:79`（`SelectedPreview`/`SelectedLabel`/`SelectedPinButton`）；`CS/MainWindow.axaml.cs:375` | **有**。`RS/miredit-gui/src/layout.rs:73` + `RS/miredit-gui/src/dock_right.rs:42-49,799`（高亮帧真缩略图，没选就是空，不画假图） | P2 |

### 2.8 图层（7 行：有 3 / 无 4）

> C# 7 行实证：`CS/MainWindow.axaml:30-57`（地表/中层/物体/低位阻挡/高位阻挡，各带 `CheckBox` 可见性）
> + `:59`（"单元格高亮"子组标题 `CellHighlights_E3B54000`）+ `:61-62`（灯光单元格 / 门单元格两个 `CheckBox`）。
> ⚠ 也就是说：**前 5 行是"图层"，后 2 行是"单元格高亮"子组** —— 运行期 UIA 把 7 行一起读出来，源码里是 5+2 结构。
> Rust 侧 5 行齐备（`RS/miredit-gui/src/ui_text.rs:131-137`），**后 2 行只有文案常量、根本没画**
> （`HIGHLIGHT_LIGHT`/`HIGHLIGHT_DOOR` 定义于 `RS/miredit-gui/src/ui_text.rs:148-150`，`HIGHLIGHT_NAMES` 在 `dock_left.rs` **0 引用**）。

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| A56 | 地表图层（眼睛 + 可作编辑层） | `CS/MainWindow.axaml:30-31`；写 `bk_img` `CS/MapViewport.cs:479` | **有**。眼睛 `RS/miredit-gui/src/dock_left.rs:65-68`；编辑层 `RS/miredit-gui/src/edit.rs:64-67`；写 `bk_img`+`btTiles` `:135,144,822-825` | P0 |
| A57 | 中层图层 | `CS/MainWindow.axaml:36-37`；写 `MiddleImage` | **有**。`RS/miredit-gui/src/dock_left.rs:69-72`；写 `mid_img`+`btsmTiles`（`RS/miredit-gui/src/edit.rs:67,136,145`） | P0 |
| A58 | 物体 | `CS/MainWindow.axaml:42-43`；写 `Foreground` | **有**。`RS/miredit-gui/src/dock_left.rs:73-76`；写 `fr_img`+`btArea`（`RS/miredit-gui/src/edit.rs:69,137,146`） | P0 |
| A59 | 低位阻挡 | `CS/MainWindow.axaml:46-50`（`LowCollisionVisible` + 粉色 `#F05569` 标记）；`CS/MapViewport.cs:22-23`（`0x2000_0000`）、`:480`（写格）、`:485-539`（填充）；`CS/CollisionMarkers.cs:1-30` | **无**。UI 行在但**未接线**（`RS/miredit-gui/src/dock_left.rs:48-50` 只 0..2 为真；`:92` 明写"如实点了没反应"）。⚠ 该模块 `:21-24` 的理由"渲染库没有阻挡层接口"**已过期** —— 见 B36 | **P0** |
| A60 | 高位阻挡 | 同上（`HighCollisionVisible` + 蓝色 `#41A5F5`；`0x4000_0000`） | **无**。同 A59 | **P0** |
| A61 | 灯光单元格（琥珀高亮） | `CS/MainWindow.axaml:61`（`LightCellsVisible`）；`CS/MapViewport.Presentation.cs:43-50` | **无**。仅文案常量（`RS/miredit-gui/src/ui_text.rs:148`），未绘制、无开关 | P1 |
| A62 | 门单元格（青色高亮） | `CS/MainWindow.axaml:62`（`DoorCellsVisible`）；同上 `:43-50` | **无**。仅文案常量（`RS/miredit-gui/src/ui_text.rs:149`） | P1 |

### 2.9 独立窗口（1 项：无 1）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| A63 | `ObjectExtractionWindow`（1420×900，代码建窗） | `CS/ObjectExtractionWindow.cs:5`（`internal sealed class ObjectExtractionWindow : EditorWindow`）、`:37`（`Width=1420; Height=900; MinWidth=1120; MinHeight=700`）、`:34-82`（全部控件代码创建，**无 `.axaml`**） | **无**。检索口径：`extract`/`Extract`/`ObjectExtraction` 全 **0**；`RS/` 无任何二级窗口（`RS/miredit-gui/src/window.rs` 46 行，只有主窗参数） | P1 |

**表 A 小计**：63 项 = 有 17 + 部分 10 + 无 36。
分级：**P0 20 / P1 23 / P2 20**。

---

## 三、逐项判定表 **B**：能力级（66 项，v1 域 2–13 的延续 + 两个新域）

> 这一层是"界面上看不见但决定能不能用"的能力。v1 的 125 项主要落在这一层，故这里保留其编号语义以便对账（§五）。

### 3.1 编辑内核（16 项：有 2 / 部分 1 / 无 13）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| B1 | 撤销内核：笔画合并 / 上限 100 / 保存点 / 逐字节还原 | `CS/Document.cs:149-204`；`CS/MapViewport.cs:158-159` | **有**。`RS/miredit-gui/src/edit.rs:307-369`（合并）、`:168,364-367`（100）、`:190,253-291`（保存点）；`RS/miredit-gui/src/edit.rs:1022-1048`（撤销 3 步 == 原文件） | P1 |
| B2 | 每文档独立撤销栈 | `CS/MainWindow.MapTabs.cs:5-11`（每 Tab 持 `MapEditorState`）、`:63-65`（`CaptureEditorState`/`RestoreEditorState`） | **无**。单文档：`RS/miredit-gui/src/mapdoc.rs:85-95`（`DocState.doc: Option<MapDoc>`，注释"本单只支持一张图，故不是 Vec"） | P1 |
| B3 | 印章快照一起进撤销 | `CS/PlacedStamps.cs`（放置/移动/克隆/删除 + 元数据）+ `CS/Document.cs:149-204` | **无**。无 `PlacedStamps` 概念（`stamp` 138 处命中全在 `RS/mir2-stamp`，GUI 0） | P1 |
| B4 | 自动图块：Mir2 50 / Mir3 30（+5 偏移）/ Mir3Mini 5 / SmTile 60 帧；顶点掩码 → 15 形状；断连对角修补；按形状随机取帧 | `CS/AutoTileBrush.cs:15,18`（帧块）、`:27-95`（掩码 15 型 + 对角修补）、`:103-108`（`Shape`）、`:285-442`（4 族随机选帧）；预览 `CS/MapViewport.cs:378-386` | **无**。检索口径：`AutoTile`/`autotile` 全 **0**；画笔只把同一图号重复写进每一格（`RS/miredit-gui/src/edit.rs:800-835`） | **P0** |
| B5 | 笔刷尺寸 3..31 奇数 / 地面 2×2 对齐 / `[` `]` 调整 | `CS/MapViewport.cs:26-34`；`CS/MainWindow.axaml.cs:871`；`CS/MainWindow.axaml:6`（`Slider Minimum="3" Maximum="31" TickFrequency="2"`） | **无**。检索口径：`BrushSize`/`brush_size` 全 **0**；一次只落 1 格（`RS/miredit-gui/src/edit.rs:320-347` 的 `line_cells` + 单格 `paint`） | **P0** |
| B6 | 填充算法：同库同图号连通 / 按层 / 地面步长 2 / 一次 `Commit` | `CS/MapViewport.cs:485-539` | **无**（同 A13/A41） | **P0** |
| B7 | 墙笔刷（19 族 58.9 MB：双格栅格、内/外面、转角、连接、门洞、剔除/加宽、`Shift` 删段） | `CS/WallBrush.cs:30-333`（`WallFamily` 42 属性）、`:335-489`（目录加载）、`:503-771`（笔画规划）；`CS/WallTurnRouting.cs:172`、`CS/WallEntrances.cs:93` | **无**（同 A47） | P1 |
| B8 | 树/灌木/岩石散点笔刷（哈希散布、间距、密度、圆形范围、变体轮换、可选碰撞、`.treelist`） | `CS/TreeBrush.cs:44-79`、`CS/SpecialBrushKind.cs:3`、`CS/TreeBrushList.cs:135-183`、`CS/SpecialBrushWindow.axaml.cs:384` | **无**（同 A48） | P1 |
| B9 | 落格半透明预览（随尺寸/图号/层实时变） | `CS/MapViewport.cs:308-326`、`:378-386` | **无**。只有悬停格 2px 金色描边（`RS/miredit-gui/src/canvas.rs:9`） | P1 |
| B10 | 橡皮保留 `0x8000` 阻挡位 | `CS/MapViewport.cs:480`（只清图号） | **有**。`RS/miredit-gui/src/edit.rs:827-831`（`img & COLLISION_BIT`） | P1 |
| B11 | 吸管取整格三族（C# 还能吸取印章实例） | `CS/MapViewport.cs`（`EditorTool.Pickup` 分支）+ `CS/MainWindow.axaml.cs:799` | **部分**。取**当前层**一个字段（`RS/miredit-gui/src/edit.rs:203,317`，模块文档明写"两件事分开，才不会把中层图号塞进 bk_img"）；无印章实例可吸 | P1 |
| B12 | 框选（夹边、反选取消） | `CS/MapViewport.cs:387-401` | **无**（`selection`/`Selection` 全仓 1 处注释，`RS/miredit-gui/src/layout.rs:80`） | **P0** |
| B13 | 复制 / 剪切 / 粘贴（按可见层快照、锚点对齐、越界拒绝、半透明预览、可重复粘贴） | `CS/RegionCopy.cs:4-133`；`CS/MainWindow.axaml.cs:848`（`Ctrl+C/X/V`） | **无**（`clipboard` `paste` `Paste` `RegionCopy` `copy_selection` 全 **0**） | **P0** |
| B14 | 区域填充 / 按层清除 | `CS/MapViewport.cs:540-546` | **无**（同 B6） | **P0** |
| B15 | 单元格/选区属性批量（Light 0–255 / Door / DoorOffset） | `CS/MainWindow.axaml.cs:690-708`（`OpenCellProperties`，`Field()` 内部 `LocalText.T` `:702`）；`CS/MapViewport.cs:541-545` | **无**。只有只读显示（`RS/miredit-gui/src/cellinfo.rs:71-72,175-176`） | P1 |
| B16 | 右键上下文菜单（复制/粘贴/属性/按层清除） | `CS/MainWindow.axaml.cs:641-689`（`InitializeCellMenu`） | **无**。检索口径：`ContextMenu`/`right_click` 全 **0** | P2 |

### 3.2 印章体系（8 项：有 1 / 无 7）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| B17 | `.X` 索引浏览（递归 9,170 + 搜索 + 分页 + 预览） | `CS/StampCatalog.cs:7-18`（`EnumerateFiles("*.X", RecurseSubdirectories)`）、`CS/StampBrowser.cs:24-86` | **无（GUI）**。库能解析，但无索引/浏览 UI | P1 |
| B18 | 放置 / 拾取 / 移动 / 克隆 / 删除（含碰撞与边界校验、红/琥珀色阻塞预览） | `CS/MapViewport.cs:44-74`；`CS/StampPlacement.cs:3-29`；`CS/MainWindow.axaml.cs:799,873` | **无** | **P0** |
| B19 | 印章元数据随地图存取（`PlacedStamps`） | `CS/Document.cs:99-147`；`CS/PlacedStamps.cs` | **无** | P1 |
| B20 | `Identify Objects` 模板匹配（倒排索引 → 最稀有格先比 → 认领去重） | `CS/StampIdentifier.cs:15-130` | **无**（同 A36） | P1 |
| B21 | 分类索引写回 `palette.json`（原子改写 + 双路径回滚 + 新 ID 查重） | `CS/MainWindow.StampEditing.cs:138-199`；`CS/StampCategories.cs:48-60` | **无**。检索口径：`palette.json` `catalogue` 全 **0** | P1 |
| B22 | 新建印章文档（独立 Tab，尺寸 = 内容 + 17，`Ctrl+Shift+N`） | `CS/MainWindow.StampEditing.cs:40-56` | **无** | P2 |
| B23 | `.X` v0/v1/v2 兼容读取 + 逐字节写回 | `CS/Document.cs:108`（读 v0/v1/v2）、`:137-147`（`SaveStamp` 只写 v2） | **有（未接 GUI）**。`RS/mir2-stamp`：`RS/mir2-stamp/src/cell.rs`（v0 34B / v1 31·35B / v2 29·35B）+ `RS/mir2-stamp/src/stamp.rs`；实测 **9,170/9,170 逐字节往返**（`RS/mir2-stamp/src/lib.rs:102-108`）；⚠ **无任何 crate 依赖它** | P1 |
| B24 | 印章预览合成（缩略图 120×84，按层序画） | `CS/StampPreview.cs:1-85` | **无** | P2 |

### 3.3 素材与数据资产（10 项：有 1 / 部分 1 / 无 8）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| B25 | 收藏（★/☆ + 跨会话 `assets.json`） | `CS/AssetHistoryStore.cs:33-73`；`CS/AssetPalette.cs:103-149` | **无**。页签在但整页空（`RS/miredit-gui/src/dock_right.rs:241-242,700`） | P2 |
| B26 | 最近使用（48 条 + 持久化） | `CS/AssetHistoryStore.cs:58-73` | **无**（同上） | P2 |
| B27 | 分类树（`palette.json` 9,159 条） | `CS/CategoryPalette.cs:92-221` | **无**（`catalogue` 全 0） | P1 |
| B28 | Type/Region/Environment/Facing 四维分组 | `CS/StampCategories.cs:12-73` | **无**（同上） | P1 |
| B29 | 图案组聚合显示（50 帧块 → 一套） | `CS/TilePatternPreview.cs:1-41`；`CS/AssetPalette.cs:84-86` | **无** | P2 |
| B30 | 按层自动选库 | `CS/MainWindow.axaml.cs:712-727`（`SelectLibraryForLayer`） | **无**。库由用户下拉手选（`RS/miredit-gui/src/palette.rs:853-886`） | P2 |
| B31 | `palette.json`（5.09 MB / 9,159 条，含 `contentFingerprint` 等字段） | `CS/StampCategories.cs:16-38`；`CS/CategoryPalette.cs:123`；写回 `CS/MainWindow.StampEditing.cs:163-182` | **无**。检索口径：`palette.json`/`catalogue`/`schemaVersion` 全 **0**；⚠ Rust 侧**没有任何 JSON 库**（依赖只有 `image`/`png`，见 `RS/mir2-assets/Cargo.toml`、`RS/miredit-render/Cargo.toml`），现有 JSON 读取是测试里 326 行手写解析器 `RS/mir2-assets/tests/support/json_min.rs` | P1 |
| B32 | `terrain.json` / `water.json` / `towns.json` / `walls.json`+19 族 / `river-artwork.json`+layouts / `island` | `CS/TerrainCatalogue.cs:18,39`、`CS/TownCatalogue.cs:22`、`CS/WallBrush.cs:338-489`、`CS/RiverArtworkCatalogue.cs:1-141`、`CS/IslandCoastObjects.cs:15` | **无**（`terrain.json` `water.json` `towns.json` `walls.json` 全 **0**） | P2 |
| B33 | 9,170 个 `.X` 资产接入 | `CS/StampCatalog.cs:15`；`CS/StampIdentifier.cs:47`；`CS/TreeBrush.cs:14`；`CS/ProvinceGenerator.cs:58` | **部分**。库能读全部（`RS/mir2-stamp`，9,170 逐字节往返），**GUI 零接入**（`miredit-gui` 依赖表无 `mir2-stamp`） | **P0** |
| B34 | 客户端 WIL/WZL 图库（帧源） | `CS/Assets.cs:1-229` | **有**。`RS/mir2-assets`（`LibPaths::discover` `:230`、`WilSource::frame_of` `:149`、LRU + 失败计数 `:139-147`） | P1 |

### 3.4 显示与视图内核（16 项：部分 2 / 无 14）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| B35 | 碰撞层**编辑**（画/擦/填充 `0x2000_0000`/`0x4000_0000`） | `CS/MapViewport.cs:22-23,480,485-539` | **无**（同 A59/A60） | **P0** |
| B36 | 碰撞**可视化**（粉 `#F05569` / 蓝 `#41A5F5` / 同格双色各半） | `CS/CollisionMarkers.cs:1-30`；调用点 `CS/MapViewport.Presentation.cs:40-42` | **部分（通道有、GUI 未接）**。`RS/miredit-render/src/collision.rs:1-40`（判定按客户端 `0x8000`、色值/形状/时机照 C#）+ `CollisionOverlay` + `try_render_region_with(..., overlay)`（`RS/miredit-render/src/lib.rs:194-227`）；⚠ GUI 里 `CollisionOverlay` **0 命中**，仍调旧 `try_render_region`（`RS/miredit-gui/src/canvas_view.rs:317`） | **P0** |
| B37 | 灯格高亮（琥珀，各占半格） | `CS/MapViewport.Presentation.cs:43-50` | **无**（同 A61） | P1 |
| B38 | 门格高亮（青） | 同上 | **无**（同 A62） | P1 |
| B39 | 光照预览 3 档（Dawn/Dusk 205、Day 0、Night 255） | `CS/MapViewport.Presentation.cs:59-97`；`CS/MapPreview.cs:21-82` | **无**（`lighting`/`Lighting` 0） | P1 |
| B40 | `PreviewLights` 加法叠加（`445/685/925.png`，取 R 通道做 intensity） | `CS/MapPreview.cs:77-80`；资源 `E:\GodM2\CrystalStudio\PreviewLights\*.png`（`EmbeddedResource`） | **无** | P1 |
| B41 | 门开/关预览（叠加 `DoorOffset`） | `CS/MapPreview.cs:43-47` | **无**（`doors_open`/`DoorsOpen` 0） | P1 |
| B42 | 材质动画预览（100 ms 推进 `MiddleFrames/Tick`、`Front.Frames/Tick`） | `CS/MapViewport.cs:132,139,259-261,279-280,344-346` | **无**（`animation` 1 处命中是文案 `RS/miredit-gui/src/ui_text.rs:125`） | P1 |
| B43 | 小地图真实渲染（缩略图本体） | `CS/MapMinimap.Avalonia.cs:41-92` | **无**（只有视口框，`RS/miredit-gui/src/dock_right.rs:533` 明写无缩略图） | P1 |
| B44 | 小地图导航（点击居中 / 拖视口框 / 滚轮缩放 / 编辑增量失效） | `CS/MapMinimap.Avalonia.cs:41-92`；`CS/MapViewport.cs:223` | **无**。`MiniMap` 只有 `map`/`view` 两个只读字段（`RS/miredit-gui/src/dock_right.rs:431-437`），无交互分支 | P1 |
| B45 | 缩略总览 mipmap（缩放 <25% 按 2^k 级批量 + 缓存 + 行带增量） | `CS/MapViewport.Overview.cs:28-47`；`CS/OverviewBatch.cs:1-265` | **无**（`overview`/`Overview` 0）；0.25× 仍 1:1 渲染，超 16,000 格直接不画（`RS/miredit-gui/src/canvas_view.rs:249-265`、`RS/miredit-gui/src/metrics.rs:331,337` 的 `CANVAS_RENDER_MAX_CELLS`） | P1 |
| B46 | 渲染统计上界面（`Prepare X ms · Batches N · Sprite cache M`） | `CS/MainWindow.axaml.cs:121` | **部分**。只有 stderr `DIAG_*` 与 `--autodrag` 报告（`RS/miredit-gui/src/app.rs:534-557`） | P2 |
| B47 | 响应式面板（窄→宽自动改列、图层行图例自适应） | `CS/MainWindow.axaml.cs:241-265`；`CS/ResponsivePanel.cs:1-39` | **无**（`RS/miredit-gui/src/layout.rs` 单套固定几何） | P2 |
| B48 | 面板拖拽停靠（4 边 + 落点预览 + 半宽/半高判定） | `CS/DockWorkspace.cs:101-120,288-326`（`DropTarget`/`ShowDropPreview`） | **无**（`dock` 语义仅存在于布局常量） | P1 |
| B49 | 浮动窗口（双击标题 / 右键 Float / 拖出窗体成窗） | `CS/DockWorkspace.cs:149,262`（`Float`） | **无**（`float`/`Float` 0） | P1 |
| B50 | 布局持久化 `workspace.json` + 内嵌默认布局 | `CS/DockWorkspace.cs:12`（路径）、`:38-42`（`DefaultWorkspace.json`）、`:327`（`Save`）、`:227-268`（`Restore`） | **无**（`settings`/`workspace` 0） | P1 |

### 3.5 文档与格式（3 项：部分 1 / 无 2）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| B51 | 多文档 Tab（每页独立 `Doc/History/Zoom/origin/Selection`、切换、单页关闭、逐页脏提示） | `CS/MainWindow.MapTabs.cs:5-110`；`CS/MainWindow.axaml.cs:408-418`（逐页 `ConfirmDiscard`） | **无**。页签条只画**一个**写死标签（`RS/miredit-gui/src/canvas.rs:82-90,136-146`），关闭符不响应（`RS/miredit-gui/src/app.rs:898-1074` 无页签交互） | P1 |
| B52 | 地图格式方言（**9 个 loader**：Type0 52/12 · Type1 54/15 · Type2 52/14 · Type3 52/36 · Type4 64/12 · Type5 28+…/14 · Type6 40/20 · Type7 54/15 · Type100 8/(26\|38\|35\|33)） | `CS/LegacyMapReader.cs:55-117`（判据分派）、`:119-570`（各 `LoadMapTypeN` 的 `ValidateDimensions` 分别见 `:127,183,229,280,340,394,463,520,572`） | **部分**。只认 3 档：`Old12`/`New14`/`Ext36`（`RS/mir2-map/src/cell.rs:97-105`），其余 6 类一律 `LengthMismatch`（`RS/mir2-map/src/map.rs:55-56`）。⚠ **36B 语义两侧不同**（v1 §3.4 已订正，本次复核仍成立：C# 编造库号 `+100/+110/+120`，见 `CS/LegacyMapReader.cs:296-299`） | P1 |
| B53 | 忙碌态/进度条/全界面禁用 | `CS/MainWindow.axaml.cs:404`（`Run` 闸门）、`:121`（`Performance` 文案） | **无**。检索口径：`progress`/`busy` 在 GUI 无实现（`busy` 仅作为 `guard_close_request` 的局部变量语义） | P2 |

### 3.6 设置内核（3 项：无 3）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| B54 | `settings.json` 持久化（图库目录 + 语言） | `CS/SettingsStore.cs:10-25`；`CS/MainWindow.axaml.cs:517-521`（先 `AssetStore.Load` 成功才落盘，失败回滚） | **无**（`settings`/`Settings` 0；图库根硬编码 `RS/mir2-assets/src/lib.rs:85`） | **P0** |
| B55 | 主题跟随系统 + `appearance.json` + accent 重算 | `CS/ThemeService.Avalonia.cs:11-58` | **无**（`theme.rs` 固定浅色一套，`RS/miredit-gui/src/theme.rs:1-11`） | P2 |
| B56 | Profile 目录重定向（`CRYSTALSTUDIO_PROFILE`） | `CS/EditorPaths.cs:5` | **无**（`env::var` 仅 `MIREDIT_QUIET`，`RS/miredit-gui/src/canvas_view.rs:338`） | P2 |

### 3.7 **i18n（新域）**（4 项：无 4）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| B57 | `LocalText` 机制（键→英/中；`T()` **按英文原文查表**；`Source(key)` 缺键抛异常；`Format` 支持枚举项翻译） | `CS/LocalText.cs:10-88`（`T` `:50`、`Source` `:51`、`Format` `:52-57`、`LocExtension` `:84-88`） | **无**（`LocalText`/`i18n`/`locale` 全 **0**；界面文案是编译期常量 `RS/miredit-gui/src/ui_text.rs`） | P1 |
| B58 | 893 条词表（`Strings.json`，键 = 英文文本 + 8 位哈希后缀，重复串自动加后缀） | `CS/Localization/Strings.json`（**893 条**，实测 `ConvertFrom-Json` 计数）；加载 `CS/LocalText.cs:20-26`（`EmbeddedResource`） | **无**（无 JSON 库；`RS/miredit-gui/src/ui_text.rs` 现 **1,023 行**是**中文单语**常量 + 自检表 `TEMPLATES`） | P1 |
| B59 | 语言菜单（勾选态回读 + 落 `settings.json` + 重启生效） | `CS/MainWindow.Localization.cs:7-36`；`CS/LocalText.cs:41`（`Initialize` 只在启动读一次）、`:43-48`（`SaveLanguage`） | **无** | P1 |
| B60 | 全量覆盖（无漏译） | 见 §七.4（我实测 **21 处**真实漏译 + 7 个死代码键） | **无**（不适用） | P2 |

### 3.8 **素材提取（新域）**（6 项：无 6）

| # | 功能点 | C# 证据 | Rust 现状 | 分级 |
|---|---|---|---|---|
| B61 | 三类候选来源：①已确认放置元数据 ②模板匹配 ③前景碎片自动发现 | `CS/ObjectExtraction.Service.cs:70-78`（元数据，且要求"所有前景锚点仍一致"）、`:80-86`（模板）、`:87-100`（碎片发现） | **无**（`extract`/`Extract` 0） | P1 |
| B62 | 内容指纹 + 去重（SHA-256，规范化后序列化） | `CS/ObjectExtraction.Models.cs:48-57`（`Fingerprint`）、`:37-47`（`Normalize`）、`:12`（`ExtractionCandidate.Fingerprint`） | **无**（无哈希库：`RS/*/Cargo.toml` 无 `sha2`/`blake3`） | P1 |
| B63 | 评审（确认/取消确认/合并/拆分/排除/碰撞开关/撤销重做，上限 40 步） | `CS/ObjectExtraction.Models.cs:73-189`（`ExtractionReview`：`Update` `:87-93`、`Exclude` `:94-95`、`RemoveFragments` `:96-112`、`Collision` `:113-123`、`Merge` `:124-189`、栈上限 `:83`） | **无** | P1 |
| B64 | 导出（`.X` 按指纹命名 + `palette.json` 条目 + 报告 JSON + **原子提交 + 失败回滚**） | `CS/ObjectExtraction.Export.cs:16-114`（锁文件 `:22`、写前字节快照 `:23`、`Extracted/{Category}/{fingerprint}.X` `:44`、tmp→`File.Move` `:55-60`、ID 查重 `:70-71`、报告 `:89-102`、**提交前再比对目录字节** `:104`、失败删新建文件 `:109-113`） | **无** | P1 |
| B65 | 独立窗口交互（范围选择/目录选择/筛选/分页 48/分类页签/详情预览/进度/取消） | `CS/ObjectExtractionWindow.cs:10-82`（控件）、`:108-140`（`ScanAsync`，目录递归 `*.map` `:119`）、`:141-153`（`SaveAsync`） | **无** | P1 |
| B66 | 测试覆盖（23 条属性：事务回滚/并发改动/取消/真实资源批量往返） | `CST/ObjectExtractionTests.cs:30-335`（`TransactionFailureRetainsIndexAndRemovesOnlyNewOutputs` `:252`、`ConcurrentCatalogueChangeIsRetainedAndPreventsExportCommit` `:263`、`CancellationDuringScanAndExportNeverChangesSourcesOrCatalogue` `:272`、`RealResourcesBatchRoundtripAndCancellationWhenRequested` `:187`） | **无** | P2 |

**表 B 小计**：66 项 = 有 4 + 部分 5 + 无 57。
分级：**P0 11 / P1 40 / P2 15**。

---

## 四、分级汇总

### 4.1 总数与分档

| 状态 | 表 A（界面骨架 63） | 表 B（能力级 66） | 合计 | 占比 |
|---|---|---|---|---|
| **有** | 17 | 4 | **21** | 16.3% |
| **部分** | 10 | 5 | **15** | 11.6% |
| **无** | 36 | 57 | **93** | 72.1% |
| **合计** | **63** | **66** | **129** | 100% |

| 分级 | 表 A | 表 B | 合计 | 其中"无" | 其中"部分" | 其中"有" |
|---|---|---|---|---|---|---|
| **P0** | 20 | 11 | **31** | 20 | 2 | 9 |
| **P1** | 23 | 40 | **63** | 47 | 6 | 10 |
| **P2** | 20 | 15 | **35** | 26 | 7 | 2 |
| **合计** | 63 | 66 | **129** | 93 | 15 | 21 |

> **P0 真缺口（P0 且 无/部分）= 22 条**：
> A1 新建 · A5 另存为 · A8 导入物体 · A13 填充选区 · A14 设置 · A40 铅笔 · A41 填充工具 · A43 选择 · A44 物体模板 · A59 低位阻挡 · A60 高位阻挡
> + B4 自动图块 · B5 笔刷尺寸 · B6 填充算法 · B12 框选 · B13 复制粘贴 · B14 区域填充 · B18 印章放置 · B33 `.X` 资产接入（部分） · B35 碰撞层编辑 · B36 碰撞可视化（部分） · B54 `settings.json`。

### 4.2 P0 首 5 条（按"解锁面积 / 单位成本"排，**不是"该不该做"**）

| 序 | 条目 | 为什么排这个位置 | 成本判据（现状） |
|---|---|---|---|
| 1 | **B36 + B35 碰撞层（可视化通道已就绪，只差接线）** | 唯一一条"**底层已经写好、只差 GUI 调一个函数**"的 P0。渲染库已有 `try_render_region_with(..., CollisionOverlay)`（`RS/miredit-render/src/lib.rs:194-227`）与色值/形状口径（`RS/miredit-render/src/collision.rs:17-30`），GUI 仍调旧接口（`RS/miredit-gui/src/canvas_view.rs:317`）。同时 `RS/miredit-gui/src/dock_left.rs:21-24` 里"渲染库没有阻挡接口"的理由**已过期**，注释需同步订正 | 改 1 个调用点 + 放开 `layer_wired` 两位 + 加两个 `LayerMask` 字段；无新算法 |
| 2 | **A13/A41/B6 填充（Bucket）** | 铺一片 30×30 要点 900 次（无尺寸），填充是唯一能"一次做完"的替代路径；C# 算法现成（`CS/MapViewport.cs:485-539`，BFS + `visited` + 一次 `Commit`） | `paint()` 已有单格路径（`RS/miredit-gui/src/edit.rs:800-835`），加 BFS + 把工具下标 2 接线 |
| 3 | **A8/A44/B18/B33 印章体系（`.X` 已能读，差索引+放置）** | 物件层"没有素材来源"的根因。`.X` 读取器**已完成且经 9,170 文件逐字节验证**（`RS/mir2-stamp/src/lib.rs:102-108`），只缺"接进 GUI"与"索引/放置" | 依赖表加一行 `mir2-stamp`；放置 = 在 `EditLayer::Front` 上按 `StampCell` 批量写 + 一步撤销 |
| 4 | **A43/B12/B13/B14 选择与剪贴板** | 9 个功能点共用一套"选区"结构（框选/复制/剪切/粘贴/区域填充/按层清除/属性批量/右键菜单/删除）；不做则一切"搬运"只能手点 | 需新增选区状态 + 快照结构；C# 的 `RegionCopy.cs:4-133` 是完整参考实现 |
| 5 | **B4/B5 自动图块 + 笔刷尺寸** | 决定"地表能不能连成图案"。C# 侧 416 行（`CS/AutoTileBrush.cs`）+ 尺寸规则 9 行（`CS/MapViewport.cs:26-34`） | 尺寸是纯循环放大（低成本）；自动图块是 4 族 × 15 形状 + 断连修补（**最高成本**），建议先只做 Mir2 50 帧 |

> 说明：A1（新建）与 A14（设置）也是 P0 真缺口，但前者是"入口缺失"、后者是"环境可移植性"，
> 都不改变"能改什么"的解锁面积，故未进首 5。

---

## 五、与 v1 的差异

> v1 基线：125 功能点 = 有 23 / 部分 8 / 无 94。
> ⚠ **分母口径已变**（v2 = 129，且拆分了 v1 的若干聚合条目），所以**只能逐项比，不能比总数**。

### 5.1 v1 判「无」→ v2 判「有/部分」：**升级 5 条**

| # | v1 条目 | v1 判定 | v2 判定 | 证据（新版实测） |
|---|---|---|---|---|
| U1 | v1 P0-4「中层/物件层可编辑」 | **无**（"只写地表，`paint()` 只动 `bk_img`+`tiles`"） | **有** | `RS/miredit-gui/src/edit.rs:64-70`（`EditLayer::{Ground,Middle,Front}`）、`:133-146`（`img_of`/`set_img` 三层分派）、`:800-835`（`paint` 按层写，含库号字节）；UI 三层行可选为编辑层 `RS/miredit-gui/src/dock_left.rs:48-50` |
| U2 | v1 5.9 / 13.7「`.X` 读取 / 9,170 资产」 | **无**（"`.X` 零读取"） | **有（库）** / **部分（资产接入）** | `RS/mir2-stamp/`（新 crate，5 文件 / 1,478 行）：v0/v1/v2 逐格布局（`RS/mir2-stamp/src/cell.rs`）、`Stamp::parse`/`to_bytes`（`RS/mir2-stamp/src/stamp.rs`）、**9,170/9,170 逐字节往返**（`RS/mir2-stamp/src/lib.rs:102-108`）。⚠ 但 `miredit-gui` **不依赖**它 ⇒ 资产侧仍是"部分" |
| U3 | v1 7.4「碰撞标记绘制（粉/蓝/双色）」 | **无** | **部分** | `RS/miredit-render/src/collision.rs:1-40`（判定 `0x8000` 按客户端 `TMap.cpp:339`；色值/形状/时机照 `CollisionMarkers.cs:6-7,19-25`）+ `CollisionOverlay`（`RS/miredit-render/src/lib.rs:137,194-227`）；**GUI 未接**（`CollisionOverlay` 在 `RS/miredit-gui/` 0 命中） |
| U4 | v1 8.5「小地图真实渲染」 | **无**（"写死浅灰方块 + 14×14 装饰框"） | **部分** | 视口框已是真数据：`RS/miredit-gui/src/dock_right.rs:431-437`（`MiniMap{map:Option<(u16,u16)>, view:(i32,i32,u32,u32)}`）、`:462-500`（按 `pan/scale` 换算可见格窗口，含最小像素钳制 `:487`）、`:533`（缩略图本体如实不画）；单测 `:868-960`（6 条，含"未开图不画框"） |
| U5 | v1 12.7「工具按钮组」 | **部分（10 个按钮，3 个可落格）** | **部分（同）但新增"三层"维度** | 10 个按钮全在（`RS/miredit-gui/src/app.rs:963-971`），落笔工具仍 3 个（`RS/miredit-gui/src/edit.rs:211-218`）；升级点在"落笔的**层**"从 1 变 3 |

### 5.2 v1 判「有」→ v2 仍「有」：**保持 17 条**（逐条复核无回归）

保存原子（`RS/miredit-gui/src/save.rs:186,292`）· 撤销/重做内核（`edit.rs:307-369,168`）· 橡皮保留阻挡位（`edit.rs:827-831`）· 三层可见性（`dock_left.rs:63-79`）· 悬停高亮（`canvas.rs:9`）· 单元格检查器（`cellinfo.rs`）· 缩放/平移/Fit（`app.rs:1011-1073`）· 纹理缓存与变化判定（`canvas_view.rs:18-50`）· 图库枚举（`mir2-assets/lib_paths.rs:230`）· 缩略图按需+LRU+负缓存（`palette.rs:44-49,101`）· 虚拟滚动（`palette.rs:11-19`）· 图号搜索（`palette.rs:441-476`）· 翻页（`palette.rs:741-753`）· 库下拉（`palette.rs:853-886`）· 关窗拦截（`app.rs:909-923`）· 固定几何布局（`layout.rs`）· 客户端帧源（`wil_source.rs:149`）。

### 5.3 降级：**0 条**

复核未发现任何"v1 判有、v2 判无"的条目。
⚠ 但有两处**看起来像降级、实为口径拆分**，必须说明，否则会被误读：

| 现象 | 真相 |
|---|---|
| v1 3.1「铅笔单帧落格」= 部分（仅地表）→ v2 A40 铅笔 = **无** | v1 把"能单帧落笔"记在铅笔名下，实际是**画笔**在落笔。v2 拆成 A39 笔刷（**有**）+ A40 铅笔工具（**无**，下标 1 未接线），避免"铅笔能用"的误判 |
| v1 8.2「缩放范围」= 部分 → v2 未单列 | 现况：`CANVAS_MIN_SCALE..=CANVAS_MAX_SCALE`（`RS/miredit-gui/src/canvas_view.rs:156-182`），单测钉死 25%..400%（`:469-492`）；C# 是夹在 `FitZoom..1`。**差异仍在**（Rust 可放大到 400%），已并入 A19/A20 的"有"里说明，不再单列 |

### 5.4 v1 未覆盖的新条目：**10 条**（另 4 组是"1 拆 N"，不重复计入）

| 新条目 | 说明 |
|---|---|
| B57 `LocalText` 机制 | v1 完全不知道 i18n（`LocalText.cs` 是 `d2e8e436` 新增） |
| B58 893 条词表 | 同上 |
| B59 语言菜单（重启生效） | 同上（运行期 UI 有、v1 源码快照无） |
| B60 全量覆盖（漏译 0） | 同上 |
| B61 提取：三类候选来源 | `6f2dadde` 新增 |
| B62 提取：指纹去重 | 同上 |
| B63 提取：评审 | 同上 |
| B64 提取：导出原子写 | 同上 |
| B65 提取：独立窗口 | 同上 |
| B66 提取：测试 23 条 | 同上 |
| A37 提取地图物体（菜单项） | 同上 |
| A63 `ObjectExtractionWindow` | 同上 |

> 不重复计入的"1 拆 N"：v1 9.3「面板显隐菜单」（1 条）→ v2 A24–A29（6 条）；v1 12.7「工具按钮组」（1 条）→ v2 A39–A48（10 条）；v1 7.1–7.6「图层」（6 条）→ v2 A56–A62（7 条，多出"灯光/门"两行独立化）；v1 12.2–12.6「菜单各项」→ v2 A1–A38。

### 5.5 v1 需订正的判断：**2 条**（v1 原文有误/已过期）

| # | v1 原文 | 订正 |
|---|---|---|
| C1 | v1 域 9「停靠与布局」与域 12「菜单」都把 C# 的面板菜单描述成"9+ 项" | 实测：C# 的 View 菜单是 **`.axaml` 静态 12 项 + 运行时动态追加 8 项**（6 个面板 CheckBox `CS/DockWorkspace.cs:269-280`、锁定 `CS/MainWindow.axaml.cs:391-394`、重置 `:396`、语言 `CS/MainWindow.Localization.cs:15`）。v1 的"9+"低估了 1 项（语言），且没区分静态/动态 |
| C2 | v1 7.1–7.6 把图层记成 6 行 | 实测 **5 行图层 + 1 个"单元格高亮"子组（2 个 CheckBox）= 7 行**（`CS/MainWindow.axaml:30-57` + `:59-63`）。运行期 UIA 读到的 7 行与源码 5+2 结构**不矛盾**，但 v1 漏了子组标题 `CellHighlights_E3B54000` |

---

## 六、素材提取工具专章（`6f2dadde`）

### 6.1 文件与体量

| 文件 | 行 | 职责 |
|---|---|---|
| `CS/ObjectExtraction.Models.cs` | 186 | 候选/指纹/评审（值语义 + 撤销栈） |
| `CS/ObjectExtraction.Service.cs` | 262 | 扫描（三类来源 + 分组 + 上下文抓取） |
| `CS/ObjectExtraction.Export.cs` | 120 | 导出（`.X` + 目录条目 + 报告 + 事务） |
| `CS/ObjectExtractionWindow.cs` | 337 | 独立窗口（代码建窗，无 `.axaml`） |
| `CS/MainWindow.ObjectExtraction.cs` | 22 | 入口接线 |
| `CST/ObjectExtractionTests.cs` | 335 / 23 条属性 | 测试 |
| **合计** | **1,262 行** | |

### 6.2 扫描：三类候选来源（**证据分级**是设计核心）

| 来源 | 代码 | 判据 |
|---|---|---|
| ① **已确认放置元数据**（`ExtractionKind.Metadata`） | `CS/ObjectExtraction.Service.cs:70-78` | 仅当"**所有前景锚点仍与地图一致**"且未被认领时才采信（`:74-75`）；采信后 `Capture` 只取"元数据 ∩ 实际"的碰撞位（`:156-168`，`actual.BackImage & s.Cell.BackImage`）；注释明写"前景提取不继承无关格子的光照/门"（`:163-164`） |
| ② **模板匹配**（`ExtractionKind.Template`） | `:80-86` | 复用 `StampIdentifier.Scan`（`CS/StampIdentifier.cs:15-130`），模板目录 `ForegroundOnly=true`（`:28`） |
| ③ **前景碎片自动发现**（`ExtractionKind.Discovered`） | `:87-100` | 未被认领的 `HasForeground` 格 → 用 `MapSpriteDrawing.Bounds` 求包围盒（`:90-92`）→ 并查集分组（`:223-260`） |

**分组算法（`Group`，`CS/ObjectExtraction.Service.cs:223-260`）**——这是最容易移植错的一段：

- 空间桶 `192px`（`:233-235`）+ 同库（`:236`）+ **图号差 ≤32**（`:237`）；
- 包围盒必须**相邻**（`:243`，"gap ≤ 1"）；
- 两组的图号集合**不许重叠**（`:244`）且**合并后总跨度 ≤32**（`:246`）——注释说这是"对重复墙/树的刻意保守"（`:245`）；
- 超限降级：单组 > 512 格 **或** 跨度 ≥64 格 ⇒ 拆成逐个单碎片（`:250,256`）。

**分类判定（`Add`，`:132-144`）**——三级回落：
1. 有模板路径 ⇒ 目录里的 `Type`；模板命中还顺手改名（`:133`）；
2. 指纹在既有目录里精确命中 ⇒ 用既有 `Category.Type` + `Name`（`:134`）；
3. 否则按"前景图号 → 类型"关联表投票，**要求 ≥80% 且必须领先**（`:139-143`，理由文案 `:143`）。

**上下文（`ContextIndex`，`:170-221`）**：按 `384px` 桶索引所有前景，抓取时按包围盒 + `96×64` 外扩（`:200`），并**硬钳到 64 格**（`:202-203`），注释"上下文内存与源图尺寸解耦"（`:201`）。

**护栏**：候选上限 100,000（`:148`，超限抛 `InvalidDataException`）；缺图警告上限 1,000 条（`:53`）。

### 6.3 指纹与去重（`ExtractionContent`，`CS/ObjectExtraction.Models.cs:33-69`）

- **规范化** `Normalize`（`:37-47`）：丢空格 → 重复坐标即抛 `InvalidDataException`（`:40`）→ 图号为 0 时把库号也清 0（`:43-44`）→ `CropStamp` 裁到包围盒 → 按 `Y` 再 `X` 排序（`:46`）。
- **指纹** `Fingerprint`（`:48-57`）：`BinaryWriter` 写 **版本号 2**（`:53`）+ 格数 + 每格 `X/Y` + `CellCodec.Write(map:false)`（**印章分支**，不是地图分支，`:54`）→ `SHA256` → **小写 hex**（`:56`）。
- **去重**：扫描期以指纹为键（`:125`）；重复出现时**只累加 occurrence**，并把"建议碰撞"按**证据求交**收窄（`:127-130`：`prior.SuggestedCollisions` 逐格 `&` 新证据，为 0 的丢弃）——即"重复出现越多，碰撞证据越保守"。
- **导出期再按指纹分组**（`CS/ObjectExtraction.Export.cs:25-26`），每组取第一条。

### 6.4 评审（`ExtractionReview`，`CS/ObjectExtraction.Models.cs:73-189`）

| 能力 | 位置 | 关键约束 |
|---|---|---|
| 撤销/重做 | `:76-86` | 存的是**候选列表快照**（`Stack<List<...>>`），注释明写"撤销永远不持有编辑器文档或其历史"（`:71-72`）；上限 **40**（`:83`） |
| 确认 / 取消确认 / 改名 / 改分类 / 边界开关 | `:87-93` | 空名字抛 `InvalidDataException`（`:90`） |
| 排除 | `:94-95` | 直接移出候选集 |
| 拆分碎片 | `:96-112` | 只对 `HasForeground` 的点生效（`:100`）；锚点上的碰撞**跟着碎片走**（`:102-103` 注释 + `:103` 把前景/阻挡/门/光清零）；`split=true` 时新候选改名 `" (2)"`（`:110`） |
| 碰撞开关 | `:113-123` | **只能开源图里本来就有的位**（`:119` 抛 "Only collision present in the source can be included."）；掩码 `0x60000000`（`:35,115`） |
| 合并 | `:124-189` | 需 ≥2 个（`:127`）；以第一个的 occurrence 为原点、其余按"同源最近 occurrence"求偏移（`:130-136`）；**前景锚点重叠即拒绝**（`:160`）；未被匹配的 occurrence **必须保留**为原候选（`:181-187` 注释"评审命令绝不能丢弃源记录"） |

### 6.5 导出与原子写（`CS/ObjectExtraction.Export.cs`）

顺序（每步都可失败，失败即整体回滚）：

1. **进程间串行**：`FileStream(indexPath + ".extraction.lock", FileMode.OpenOrCreate, FileShare.None)`（`:22`，注释"协作写入者串行化，其他编辑器靠提交前的字节比对发现"）。
2. **写前快照**：`File.ReadAllBytes(indexPath)`（`:23`）——提交前用它比对。
3. 只导出 `CanExport`（`:25`）：**已确认** 且 **有前景** 且（**允许越界** 或 **至少一次出现没跨越选区**）（`Models.cs:13-14`）。
4. 已有目录条目先全部 `LoadStamp` 建指纹表（`:28-34`，坏条目"保留但不复用"）。
5. 命中指纹 ⇒ **复用**文件；未命中 ⇒ 写 `Extracted/{Category}/{fingerprint}.X`：**先写 `*.tmp` 再 `File.Move`**（`:55-60`），已存在则比对指纹，不一致抛 `IOException`（`:47-50`）。
6. 目录条目字段固定 14 项（`:62-68`，含 `contentFingerprint`、`reviewStatus:"reviewed"`、`tags:["extracted"]`），**ID 查重**（`:70-71`，注释"防恶意/手改条目"）。
7. occurrence 记录按**序列化后的 JSON 去重**（`:77-85`）。
8. 报告写到 `catalogue/extractions/{sessionId}-{8hex}.json`，`FileMode.CreateNew`（`:89-93`）——注释"会话 ID 永不成为未校验路径、永不覆盖既有报告"（`:90`）。
9. **提交前再读一次目录字节比对**（`:104`），不等则抛 `IOException`（"导出期间目录已变化，请重新扫描"）。
10. `AtomicFile.Write`（`:106`）→ `committed = true`。
11. `finally`：未提交则**逆序删除本次新建的文件**（`:109-113`），且"永不删除既存文件"（`:112` 注释）。

**路径净化**：`CategoryFolder`（`:115-119`）只留字母/数字/`-`/`_`，截断 64，空则 `Unclassified`。

### 6.6 窗口与交互（`CS/ObjectExtractionWindow.cs`）

- **1420×900 / min 1120×700**（`:37`），`WindowStartupLocation=CenterOwner`；
- 范围三选一：`Current map` / `Current selection` / `Map directory`（`:10`；目录模式下递归枚举 `*.map`，`:119`；选"当前选区"但无选区 ⇒ 抛"请先选择地图区域"`:124`）；
- 筛选六选一：`All candidates` / `Unreviewed` / `Reviewed` / `Existing objects` / `New objects` / `Selection boundary`（`:14`）；
- 卡片列表：多选 + `WrapPanel`，**每页 48**（`:19,33,56`）；
- 左置页签按 `Category` 分组（`:20,154-178`）；
- 详情：碎片列表 + 上下文预览 + 碰撞开关（`:228-305`）；
- 评审按钮：`Undo/Redo/Review selected/Mark unreviewed/Merge selected/Exclude selected`（`:62-68`）+ 详情内 `Apply name/category/boundary`、`Split selected fragments`、`Exclude fragments`（`:237,252,253`）；
- 进度：`ProgressBar` + 相位文案（`:18,131`）；取消按钮 `:72`；
- 预览并发限 2（`SemaphoreSlim(2)`，`:24`）；
- 统计行：`{existing} existing · {new} new · {duplicates} duplicates · {unreviewed} unreviewed · {ready} ready`（`:104-106`，其中 duplicates = `Σ(occurrences-1)`）。

### 6.7 测试覆盖（`CST/ObjectExtractionTests.cs`，23 条）

按能力归类（全部为**行为级**断言，不是纯函数快照）：

| 能力 | 测试 |
|---|---|
| 多部件树去重不误合 | `:49` |
| 发现需同库同序列 + 可见接触 | `:57` |
| 前景提取忽略大量"仅地面"模板 | `:64` |
| 最大模板优先 + 只取**已确认**碰撞 + 保留真实动画 | `:72` |
| 元数据优先 + 失效元数据不占位 | `:85` |
| 选区要求整物体 + 边界开关 | `:99` |
| 去重保留全部来源 + 动画/碰撞变体 | `:107` |
| 邻近碰撞默认不选 + 跨重复来源保守 | `:117` |
| 评审六操作互相独立 + 撤销 | `:128` |
| 合并重复部件保留未匹配 occurrence | `:139` |
| 全拆后锚点外碰撞保留 + 合并保留源地面 | `:150` |
| 缺图如实报 + 绝不用于合并 | `:161` |
| 分类需 80% + 超大组保持独立 | `:167` |
| 帧头读取只校验尺寸不解码像素 | `:178` |
| 真实资源批量往返 + 取消 | `:187` |
| 导出重载逐字段一致 + 复用原生放置契约 | `:220` |
| 精确模板复用 + 重复导出不重复记来源 | `:235` |
| 事务失败（三阶段参数化）保留索引、只删新建 | `:252` |
| 并发目录改动被保留且阻止提交 | `:263` |
| 扫描/导出期间取消不改源与目录 | `:272` |
| 导出物出现在资源面板 + 可预览 + 可放置 | `:282` |
| 工具窗扫描/评审/保存不动编辑器历史 | `:312` |

### 6.8 移植它需要哪些 crate 能力（**只评估，不实现**）

| # | 需要的能力 | 现状 | 结论 |
|---|---|---|---|
| 1 | 读 `.X`（v0/v1/v2） | ✅ `RS/mir2-stamp`（`stamp.rs` + `cell.rs`，9,170 逐字节往返） | **已具备** |
| 2 | 写 `.X`（v2） | ✅ `Stamp::to_bytes` | **已具备** |
| 3 | 读 `.map`（多输入、只读） | ✅ `RS/mir2-map`（3 档） | **已具备**（方言缺口见 B52） |
| 4 | 读 `palette.json`（9,159 条 + `contentFingerprint`） | ❌ **无 JSON 库**：5 个 crate 依赖只有 `image`（`mir2-assets`）与 `png`（`miredit-render`）；现有 JSON 读取是测试里 326 行手写解析器 `RS/mir2-assets/tests/support/json_min.rs` | **缺**（需自写或引入 `serde_json`；按本仓"零新依赖"惯例倾向自写，但要能处理 5 MB 文件 + 嵌套数组） |
| 5 | 写回 `palette.json`（保序 + 保未知字段 + 原子） | ❌ 同 #4；且"保留未知字段"要求**不丢键的 DOM**，纯 struct 序列化做不到 | **缺**（需要可保留未知键的 JSON DOM） |
| 6 | **SHA-256**（指纹） | ❌ 无哈希库（`sha2`/`blake3` 均无） | **缺**（要么引 `sha2`，要么自写 SHA-256 ≈ 150 行；**不能用 CRC/自定义哈希替代**，否则与 C# 指纹不互通） |
| 7 | **只读帧头**（宽高，不解码像素） | ⚠ 只有 `SpriteSource::frame` 返回**已解码** `Frame{width,height,rgba}`（`RS/mir2-assets/src/frame.rs:10-17,44-50`）——提取器每格都要包围盒，全解码会**爆内存/爆时间** | **缺**（需加 `header_of(lib, index) -> Option<(u32,u32)>`，直接读 WIL/WZL 帧头 + 调色板前 4 字节，不展开像素） |
| 8 | 跨进程文件锁（`.extraction.lock`，独占） | ❌ Rust 标准库**没有**跨进程独占锁；无 `fs2`/`fd-lock` | **缺**（可用 `OpenOptions::new().create_new(true)` 的锁文件近似，语义弱于 `FileShare.None`，需显式记录降级） |
| 9 | 原子写 + 提交前字节比对 | ✅ 更强：`RS/miredit-gui/src/save.rs:186`（写 tmp → **读回 memcmp** → rename）+ 目录 fsync 分类（`:132-153`） | **已具备**（比 C# `AtomicFile.cs:8-26` 多一道回读校验） |
| 10 | 后台线程 + 进度回调 + 取消 | ❌ 无 `rayon`/`tokio`；GUI 是 egui 单线程即时模式（`RS/miredit-gui/src/app.rs:768` 的 `update`） | **缺**（需 `std::thread` + `mpsc` + 自建 `Cancel` 标志；egui 侧要 `request_repaint` 驱动进度条） |
| 11 | 递归目录枚举 | ✅ `std::fs::read_dir` | **已具备** |
| 12 | 候选/评审的**值语义快照**（撤销不持有文档） | ⚠ 现有 `EditSession` 的撤销是**差分步**（`RS/miredit-gui/src/edit.rs:307-369`），不是列表快照 | **需新增结构**（照 C# `ExtractionReview` 的"整表快照 + 40 步"即可，与 `EditSession` 并行、互不干扰） |

**结论（只评估）**：提取工具**不是"把 C# 逻辑翻一遍"**就能移植的 —— 它压在 4 个我们**完全没有**的基础能力上（JSON DOM、SHA-256、只读帧头、跨进程锁），
另有 2 个是"有但形态不匹配"（原子写够用、撤销模型要新建）。`mir2-stamp` + `mir2-map` 这一层已经就绪，是唯一的现成地基。

---

## 七、i18n 专章（`d2e8e436`）

### 7.1 机制（`CS/LocalText.cs`，98 行）

| 要素 | 实现 | 行 |
|---|---|---|
| 词表载体 | `Assembly.GetManifestResourceStream("CrystalStudio.Localization.Strings.json")` | `:22` |
| 数据结构 | `record Translation(string English, string Chinese)`；`Dictionary<key, Translation>` | `:12-13` |
| **反查表** | `byEnglish`：`English → Translation`，`GroupBy(English).ToDictionary(First)` —— **重复英文只留第一条** | `:14-15` |
| 取词 | `T(source)`：仅当 `Language=="zh-CN"` 且 `byEnglish` 命中时返回中文，否则**原样返回 source** | `:50` |
| 按键取词 | `Source(key)`：缺键**抛 `KeyNotFoundException`**（不是静默回落） | `:51` |
| 格式化 | `Format(source, args)`：先把 `args` 里的**枚举**翻译成字符串，再 `string.Format(GetCultureInfo(Language), T(source), args)` | `:52-57` |
| XAML 绑定 | `{local:Loc Key}` → `LocExtension`（编译期 key，运行期 `T(Source(Key))`） | `:84-88` |
| 数据项翻译 | `ChoiceTemplate`（`IDataTemplate`）+ `EditorWindow.Opened` 给**所有无模板的 ComboBox** 自动挂上 | `:81`；`CS/EditorWindow.cs:30-33` |
| 语义保持 | 注释明写"翻译选项标签，但选中值与落盘值仍保留原始标识符" | `:80` |

### 7.2 词表（`CS/Localization/Strings.json`，109,126 B / **893 条**）

- **键名规则**：英文原文去掉非字母数字 + `_` + 8 位哈希（例：`Brush_8624CA0F`、`WidthinCells164096_B58B17A5`）。
  同一英文重复出现时自动加后缀（例：`Stamp_2910B6D7` 与 `Stampboundsexceed…`、`Mytrees_95E7DBAD` 与 `My_EBF2D770`）。
- **内容分布（我按用途粗分）**：菜单/对话框标题/状态栏/提示 ≈ 300；生成器参数与预设名 ≈ 200；提取工具 ≈ 90；印章分类枚举（Type/Region/Environment/Facing/材质/朝向）≈ 150；其余为快捷键表、图层名、面板名。
- **与 Rust 的关系**：我们 `RS/miredit-gui/src/ui_text.rs`（**1,023 行**，2026-11-04 21:15 实测）是**中文单语常量表 + 自检表**，每条注释里手工标注了官方 key
  （例：`TITLE_LAYERS` 注 `官方: Layers_0BCD66E6`、`LAYER_GROUND` 注 `官方: GroundTiles_3931E191`）。
  ⇒ **893 条表正好可以当我们的对照真值**：哪些 key 我们用了、译法是否一致，可以机检（当前是手工注释，无自动校验）。

### 7.3 「语言（重启后生效）」到底是怎么生效的

1. `LocalText.resources` 与 `byEnglish` 都是 **`static readonly`**，进程内只加载一次（`CS/LocalText.cs:13-15`）；
2. `Language` 是 `static` 属性，只在 `Initialize()` 里赋值一次（`:16,41`），调用点 `CS/MainWindow.axaml.cs:121`（构造期）；
3. XAML 的 `{local:Loc …}` 在**加载期**求值（`:87`）⇒ 已上屏的窗口不会变；
4. `ChooseLanguage` 只做两件事：`SaveLanguage`（写 `settings.json`）+ 状态栏提示"已保存，请重启"（`CS/MainWindow.Localization.cs:23-24`）+ 回读勾选态（`:28,31-36`）；
5. ⇒ 所以"重启后生效"不是**偷懒**，而是**架构决定**：静态词表 + 加载期求值。**新开的窗口**（同一进程内）会用**旧**语言（`Language` 没变）。

> 佐证：`CST/MigrationTests.cs:168`（`LanguageMenuSavesPreferenceWithoutChangingCurrentWindowsOrEditingState`）、
> `:199`（`ChineseStartupSupportsCategorySearchAndPendingPreferenceKeepsSelectionAndPage`）、
> `:241`（`SettingsSavesLanguageWithoutApplyingLibrariesAndNewWindowsUseItAfterStartup`）—— 三条测试就是在钉这个行为。

### 7.4 官方漏译清单（**我自己复核的结论**，与"已知"不同处已标出）

#### 7.4.1 先给"没有漏"的强证据（避免误判）

| 检查 | 方法 | 结果 |
|---|---|---|
| XAML 引用了不存在的 key | 扫 9 个 `.axaml` 的 `{local:Loc Key}` 共 **286 个 key**，与 893 条表比对 | **0 个坏 key**（不会抛 `KeyNotFoundException`） |
| `LocalText.T/Format` 传了表里没有的英文 | 扫 162 个 `.cs`，提取 **337 个** 字面量，反转义（`\u2013` `\u2014` `\n` `\u00d7` …）后比对 | **0 个缺失** |
| 表里 893 条是否有"完全没人用"的死键 | 排除 axaml 引用 + 字面量 T/Format 后剩 **285 条**，逐条看用途 | 绝大多数是**动态 T()**（枚举/数据值/异常消息经 `T(ex.Message)`）或 `ChoiceTemplate` 间接翻译，**不是死键** |

#### 7.4.2 真实漏译：**21 处**（全部"有 key、但调用点绕过 `LocalText`"）

> 性质说明：C# 的 `T()` **按整串查表**，所以只要调用点直接写英文（不包 `T()`），中文界面就必然显示英文。
> 下面每一处我都在 893 条表里找到了对应 key —— 也就是说**词表有、代码没调用**。

| # | 位置 | 漏译内容 | 表里对应 key | 可见性 |
|---|---|---|---|---|
| 1 | `CS/MainWindow.axaml.cs:406` | `"Save your changes before continuing?"` + `"Unsaved stamp"`/`"Unsaved map"` | `Saveyourchangesbeforecontinuing_51CB8DA7`、`Unsavedstamp_C1399FFA`、`Unsavedmap_554489B5` | **高**（关窗必现） |
| 2 | `CS/MainWindow.axaml.cs:448` | `"Enter dimensions between 16 and 4096."` | `Enterdimensionsbetween16and4096_D92996B4` | 高（新建地图时） |
| 3 | `CS/MainWindow.axaml.cs:501` | 标题 `"Save failed"` | `Savefailed_53AD6F99` | 高（保存失败） |
| 4 | `CS/MainWindow.axaml.cs:592` | 标题 `"Export failed"` | `Exportfailed_E94D3EE0` | 中 |
| 5 | `CS/MainWindow.axaml.cs:706` | `"One or more values are outside the supported range."` | `Oneormorevaluesareoutsidethesupportedrange_9FF5C036` | 中（改灯/门值时） |
| 6 | `CS/MainWindow.axaml.cs:404` | 标题 `"Crystal Studio"`（品牌，可不算）+ 正文 `ex.Message` | `CrystalStudio_F96955A7` | 低 |
| 7 | `CS/MainWindow.MinimapExport.cs:11` | 正文 + 标题 `"Export Minimap"` | `ChooseyourCrystallibraryfolderinSettingsbeforeexportingmapartwork_AB160BE9`、`ExportMinimap_6D54773A` | 中 |
| 8 | `CS/MainWindow.MinimapExport.cs:17` | `SaveFileDialog.Title = "Export Minimap"` | 同上 | 中 |
| 9 | `CS/MainWindow.ObjectExtraction.cs:12` | `"The stamp category catalogue was not found."` | `Thestampcategorycataloguewasnotfound_88F1B573` | 中（**新功能也漏**） |
| 10 | `CS/MainWindow.StampEditing.cs:45` | `"The copied region exceeds the stamp editor limit."` | `Thecopiedregionexceedsthestampeditorlimit_46A229BD` | 中 |
| 11 | `CS/MainWindow.StampEditing.cs:88` | 标题 `"Unable to open stamp"` | `Unabletoopenstamp_BEF81B24` | 中 |
| 12 | `CS/MainWindow.StampEditing.cs:96` | 标题 `"Unable to read categories"` | `Unabletoreadcategories_4E57EE35` | 中 |
| 13 | `CS/MainWindow.StampEditing.cs:131` | `"Enter a stamp name."` | `Enterastampname_19D06A0E` | 中 |
| 14 | `CS/MainWindow.StampEditing.cs:202` | 标题 `"Save stamp failed"` | `Savestampfailed_B52A1F0E` | 中 |
| 15 | `CS/ThemeService.Avalonia.cs:27` | `"The colour preference could not be saved: "` + 标题 `"Appearance"` | `Thecolourpreferencecouldnotbesaved_C43A8315`、`Appearance_3907FA7F` | 低 |
| 16 | `CS/SettingsWindow.cs:51` | `OpenFolderDialog.Title = "Select Crystal Data or Map Folder"` | `SelectCrystalDataorMapFolder_A2048ED1` | 中 |
| 17 | `CS/StampScanWindow.cs:28` | `OpenFolderDialog.Title = "Choose .X Objects to Identify"` | `ChooseXObjectstoIdentify_1FE79B95` | 中 |
| 18 | `CS/SpecialBrushWindow.axaml:33` | `TextBox Text="My trees"`（默认列表名） | `Mytrees_95E7DBAD` | 中（特殊笔刷窗口一开就是英文） |
| 19 | `CS/MainWindow.axaml.cs:940` | `$"{n:N0} {assets\|stamps\|sets\|sprites}"` —— **复数名词永不变中文** | `assets_8BF729FF`、`stamps_48306FD0`、`sets_82C6DB3D`、`sprites_A34B89CC` | **高**（资源面板计数常显） |
| 20 | `CS/WallBrush.cs:46` | `DisplayName => $"{Name} · {(DrawInterior ? "Inner" : "Outer")}"` —— **拼接串命中不了词表** | `Inner_FA338A5E`、`Outer_5E426F92` | 中（墙体设置里） |
| 21 | `CS/ProvinceGenerator.cs:186` | `$"{towns} towns · {houses} houses"`（生成结果特征行） | `house_D6E21286`（仅单数） | 低 |

#### 7.4.3 `0 sets` 的**精确成因**（这条值得单列）

- XAML 里 `AssetCount` 的**初始值**是本地化的：`Text="{local:Loc 0Sets_5DB992C2}"`（`CS/MainWindow.axaml:65`）⇒ 中文界面刚打开显示 **`0 组`**；
- 但 `BuildPage()` 每次刷新都会覆盖它：`AssetCount.Text = $"{filteredCount:N0} {(SavedPalette ? "assets" : StampMode ? "stamps" : PatternPalette ? "sets" : "sprites")}"`（`CS/MainWindow.axaml.cs:940`）；
- ⇒ 只要刷新过一次，就变成 **`0 sets`**（中文模式下也一样）。
- **不是"忘了翻"**，而是**设计缺陷**：数量与名词用插值拼在一起，`T()` 只能整串查表，拼接后必然落空。
  同类病灶还有 #20（`WallBrush.DisplayName`）与 #21（`ProvinceGenerator` 特征行）。

#### 7.4.4 死代码里的同类问题（**不算漏译**，但移植时别抄）

`CS/StampPlacement.cs:8,18-25` 的 `Feedback`/`Details` 属性用英文拼装（`"Stamp can be placed"`、`"Occupied objects (red)"`、`"cell"/"cells"`、`", +N more"` 等 **7 个 key** 都有对应中文），
但**全仓无人调用** —— 实际 UI 走的是 `LocalText.Placement(...)`（`CS/LocalText.cs:59-78`，调用点 `CS/MapViewport.cs:56,71,456`、`CS/MapViewport.Town.cs:18`）。
⇒ 这是"同一逻辑两套实现、旧的一套没删"，移植时**以 `LocalText.Placement` 为准**。

#### 7.4.5 与"已知 4 处 / 8 处"的差异（**显式订正**）

| 任务书给的已知 | 我实测 | 差异说明 |
|---|---|---|
| "4 处有 key 未调用" | **21 处**（见 7.4.2） | 我按"UI 可见 × 表里有 key × 调用点绕过 `T()`"穷举；任务书的 4 处可能只统计了非 MessageBox 路径（如 #19 #20 #21 + 1 处 axaml） |
| "8 处 MessageBox 无 key" | **14 处 MessageBox 调用点**，且**全部"有 key 未调用"**（不是"无 key"） | 15 个 `MessageBox.Show` 调用里，只有 `CS/MainWindow.Localization.cs:27` 真正包了 `LocalText.Format`；其余 14 处参数都是裸字面量，且 13 处的 key 在表里**已存在** ⇒ 定性应为"漏调用"而非"缺词条" |
| "`0 sets` 未译" | **确认**，且成因更精确：初始值本地化、运行时被覆盖（见 7.4.3） | 补充：同病灶还有 2 处拼接串 |

### 7.5 结构性局限（比单条漏译更值得记）

1. **`T()` 只按整串查表**（`CS/LocalText.cs:50`）⇒ 任何 `$"…"` 拼接、任何"数量 + 名词"组合、任何"名字 + 状态后缀"都**永远翻不了**。
   实测受此影响的功能点：资源面板计数、墙体显示名、省域生成特征行（+ 死代码里的放置反馈）。
2. **`byEnglish` 重复英文只留第一条**（`:14-15`）⇒ 同一英文若要按语境译成不同中文，**做不到**（例：`Library` 既是"资源库"也是面板名）。
3. **缺键抛异常**（`:51`）⇒ 加界面文案必须先加词表，否则运行期崩（这是"强制完整"的设计，但也意味着 XAML 里写错 key 是**致命**而非降级）。
4. **静态词表 + 加载期求值** ⇒ 运行期不可切语言（见 7.3）。

---

## 八、口径与局限

### 8.1 「无」的检索口径（逐条可复现）

**检索面**：`E:\GodM2\tools\miredit\crates\**\{src,tests}\**\*.rs`（59 文件 / 22,202 行，排除 `target\`），
命令形如 `Select-String -Path "$dir\*\src\*.rs","$dir\*\src\*\*.rs","$dir\*\tests\*.rs","$dir\*\tests\*\*.rs" -Pattern <词> -AllMatches`。

| 判定为「无」的能力 | 检索词 | 命中 | 备注 |
|---|---|---|---|
| 新建地图 | `new_map` `NewMap` `--new` | **0** | 另核 `mir2-map` 的 `pub fn` 表：只有 `parse/to_bytes/cell/walkable` |
| 文件对话框 | `rfd` `FileDialog` `OpenFileDialog` | **0** | 另核 `RS/miredit-gui/Cargo.toml` 依赖仅 eframe/egui/ab_glyph/mir2-* |
| 最近打开 | `recent` `Recent` | 6 | 全为"最近使用"素材页签文案（`dock_right.rs:214`、`metrics.rs:377`、`ui_text.rs:267`） |
| 另存为 | `save_as` `SaveAs` | **0** | CLI 选项表 `cli.rs:113-230` 已逐条读过，无 `--save-as` |
| 导出（小地图/选区） | `export` `Export` `导出` `Crop` | **0** | |
| 填充 | `flood` `fill_region` | **0** | `bucket`/`Bucket` 5 处全在图标表 |
| 设置 | `settings` `Settings` `workspace` | **0** | |
| 深色模式 | `dark_mode` `DarkMode` `深色` | **0** | `dark`/`Dark` 15 处全为 `ACCENT_DARK` 色常量 |
| 单元格网格 | `show_grid` `grid_on` | **0** | `grid` 命中全为资源面板网格常量 |
| 地图光照 | `lighting` `Lighting` | **0** | `light`/`Light` 55 处全为只读显示/文案 |
| 开启门 | `doors_open` `DoorsOpen` | **0** | `door`/`Door` 73 处全为只读显示/文案 |
| 语言 / i18n | `language` `Language` `locale` `i18n` `LocalText` | **0** | |
| 锁定/重置布局 | `lock_layout` `LockLayout` `reset_layout` | **0** | |
| 生成器（城镇/地形/省/地牢） | `town` `Town` `temple` `Temple` `province` `Province` `generator` `Generator` | **0** | `generate`/`Generate` 8 处全在 `mir2-stamp/src/roundtrip_tests.rs:171-175` 的测试辅助函数 |
| 识别物体 | `identify` `Identify` | **0** | |
| 提取地图物体 | `extract` `Extract` `ObjectExtraction` | **0** | |
| 快捷键表 | `shortcuts` `Shortcuts` `快捷键` | **0** | |
| 选择/剪贴板 | `selection` `Selection` `clipboard` `paste` `Paste` `RegionCopy` `copy_selection` | **1** | 唯一命中是注释 `RS/miredit-gui/src/layout.rs:80` |
| 笔刷尺寸 | `BrushSize` `brush_size` | **0** | |
| 自动图块 | `AutoTile` `autotile` | **0** | |
| 墙/树笔刷 | `WallBrush` `TreeBrush` `SpecialBrush` | **0** | |
| 概览 mipmap | `overview` `Overview` | **0** | |
| 浮动窗口 | `float` `Float` | **0** | |
| 数据资产 | `catalogue` `palette.json` `terrain.json` `water.json` `towns.json` `walls.json` | **0** | |
| 面板显隐菜单/下拉 | `menubar.rs:7` 明写"本单不做下拉菜单"；`menubar.rs:85-95` 只做 hover | — | 代码注释即证据 |

### 8.2 「有/部分」的取证方式

- **有**：必须给出**可执行的调用点**（函数名 + `文件:行`），且该函数在 GUI 的交互路径上被调用（不是只有单测调用）。
- **部分**：给出"有哪一半 / 缺哪一半"，且两半都能落到 `文件:行`。

#### 8.2.1 ⚠ **行号漂移登记（重要，复现前必读）**

审计期间**另一路正在改 `miredit-gui`**（不是我改的；我全程只读）。实测 mtime：

| 文件 | 审计开始时（21:00） | 审计结束时（21:17） | 是否影响本报告引用 |
|---|---|---|---|
| `RS/miredit-gui/src/edit.rs` | 2,010 行 / mtime 21:08 | **2,196 行 / mtime 21:16** | **是**（引用最多） |
| `RS/miredit-gui/src/ui_text.rs` | 981 行 | **1,023 行 / mtime 21:15** | 是（页签/图层文案行号） |
| `RS/miredit-gui/src/metrics.rs` | 481 行 | 483 行 / mtime 21:05 | 是（缩放/预算常量） |
| `RS/miredit-gui/src/screen_text.rs` / `statusbar.rs` / `lib.rs` / `editrun.rs` | — | mtime 21:03–21:06 | 否（本报告未引其行号） |
| 其余 `RS/**`（含 `mir2-stamp` / `mir2-render` / `mir2-map` / `mir2-assets`） | — | mtime ≤ 20:57 | 否（引用稳定） |

**处理办法**：`ui_text.rs` / `metrics.rs` 的引用已按 21:17 复解值改正；`edit.rs` 引用**同时给出符号名**，并在下表给"读取时行号 → 21:17 复解行号"对照 —— 复现时**按符号检索**，不要死记行号。

| 本报告引用的 edit.rs 行号（读取时） | 符号 / 内容 | 21:17 复解行号 |
|---|---|---|
| `:64-70` | `EditLayer` 枚举 + 三层"写哪个字段"文档 | **`:106-114`**（`pub enum EditLayer` 在 `:106`） |
| `:67` / `:69` | 中层 / 物件层 写字段说明 | `:110` / `:112` |
| `:133-146` | `img_of` / `set_img` | **`:239` / `:248`**（另 `set_lib_byte` `:273`） |
| `:135,144` / `:136,145` / `:137,146` | 三层的读/写分派 | `:241,250` / `:242,251` / `:243,252` |
| `:822-825` | 14/36 档写库号字节 | `:973`（`layer.set_lib_byte`） |
| `:180` / `:182` / `:184` | `TOOL_BRUSH_IDX` / `TOOL_ERASER_IDX` / `TOOL_PICKUP_IDX` | **`:294` / `:296` / `:298`** |
| `:187` | `COLLISION_BIT = 0x8000` | **`:301`** |
| `:196-207` | `pub enum Tool`（Brush/Eraser/Pickup/Unwired） | **`:312-323`** |
| `:211-218` | `Tool::from_index`（其余 7 个 ⇒ `Unwired`） | **`:325-332`** |
| `:203` / `:317` | 吸管只取当前层、"两件事分开" | `:431` |
| `:252-253` | "那个文件 19:2x 正被另一路改注释、行号会漂" | `:367` |
| `:168` | `UNDO_LIMIT = 100` | **`:482`** |
| `:190,253-255,289-291` | `saved_pos`（保存点） | `:508,527,575,580,645` |
| `:307-369` | 笔画合并（一次拖动 = 一步） | 见 `:69` 文档 + `Step` 实现段（`:1585` 有断言"一次拖动只占一层撤销"） |
| `:320-347` | `line_cells`（Bresenham 插值） | **`:1020`** |
| `:374` | `pub struct EditSession` | **`:488`** |
| `:800-835` | `fn paint`（按层写 / 族不匹配拒绝 / 橡皮保留 `0x8000`） | **`:930-987`** |
| `:827-831` / `:832` | 橡皮分支 / `Pickup\|Unwired ⇒ NoOp` | `:976-980` / `:986` |
| `:1022-1048` | "撤销 3 步 == 原文件逐字节"断言 | **`:1613`** |

> 同理，`RS/miredit-render/src/compose.rs` 的注释也在被另一路改（`edit.rs:367` 的注释就是这么说的），故渲染侧的引用我一律给**函数名/常量名**（`try_render_region_with` / `CollisionOverlay` / `CANVAS_RENDER_MAX_CELLS` 等）而非纯行号。

### 8.3 **没读到 / 未验证**（如实登记）

1. **未运行任何代码**：`cargo build`/`cargo test` **一次都没跑**。文中"258 条 `#[test]`"、"45 条 C# 测试属性"都是**属性计数**，不是实测通过数。
2. **未打开 CrystalStudio 界面**：所有 C# 结论来自源码 + 任务书给的运行期 UIA 清单。我**没有**自己截图或跑 UIA 去复验菜单文字（只核了源码与 `Strings.json` 的一致性）。
3. **未逐行读的 C# 大文件**（只读关键段 + 符号表）：
   `WallBrush.cs`（944 行，读了 1-50 + 388 + 符号表）· `RiverPieceRiverBuilder.cs`（1,575 行，未读）· `RiverPieceGroundAssembler.cs`（577）· `TerrainGenerator.cs`（424，读了头部与调用点）· `ProvinceGenerator.cs`（259，读了 1-60 + 87-208）· `TownGenerator.cs`（332，只读了 grep 命中行）· `MapViewport.cs`（546，读了 1-40 + grep 命中）· 10 个 `MapViewport.*.partial`（未逐行）· `DockWorkspace.cs`（327，读了 262-321 + grep）· `LegacyMapReader.cs`（612，只读 `ValidateDimensions` 与各 loader 的首行）。
   ⇒ 涉及这些文件的判定是"**存在性**判定"（有没有这个能力），**不是算法正确性判定**。
4. **`ObjectExtractionTests.cs` 只读了方法名与关键行**（未逐条读断言体）⇒ §6.7 的"测试覆盖"是**按方法名归类**，不代表我已验证每条断言强度。
5. **893 条词表的"译文质量"未审**：我只做了**机械完整性**核对（key 是否存在、是否被调用）。中文译法是否准确、是否有错译/机翻痕迹，**未评估**。
6. **`palette.json` 的 9,159 条未抽样校验**：只用了 v1 已核的条数与字段表；本次未重新统计 `reviewStatus` 分布、未验证 `contentFingerprint` 字段是否已全量回填。
7. **`miredit-render` 的渲染保真未复核**：`compose.rs`/`frames.rs`/`canvas.rs` 未逐行读，"渲染与客户端一致"仍只是其文档声明（`RS/miredit-render/src/lib.rs:25-41` 的对照表）。
8. **C# 侧代码质量/缺陷未评估**：本报告只做"有没有"，不做"对不对"。例如 `LegacyMapReader.cs:296-299` 编造库号（`+100/+110/+120`）我按"原工程行为"记录，未判其正确性；`StampPlacement.cs` 的死代码只登记、不判该不该删。
9. **未评估移植难度/工作量**：§4.2 的"成本判据"是**现状描述**（已有多少地基、要碰哪些文件），**不是工时估算**，也不是"该不该做"的判断。
10. **工具权限**：本会话为只读；除本文件外**未创建/修改/删除任何文件**，未触碰 `catalogue\`、`Objects\`、任何 `.map`、任何源码或配置。
11. **`miredit-gui` 在审计期间被另一路并发改写**（`edit.rs` 2,010→2,196 行、`ui_text.rs` 981→1,023 行、`metrics.rs` 481→483 行，mtime 21:03–21:16；`CrystalStudio\bin`/`obj` 同期也被重新构建）。
    ⇒ 这些文件的**行号是读取时刻的快照**，已在 **§8.2.1** 给出符号↔行号对照；同时意味着我读到的 `edit.rs` 可能与"最终版"有差异，**能力级结论（三层可写、3 个工具接线、碰撞位保留）已用符号级证据固定**，不受行号漂移影响。

### 8.4 与 v1 的关系（防误读）

- v1 的 **125 项**是按"能力"切的；v2 的 **129 项**是"**界面骨架 63 + 能力级 66**"两层切的。**两者分母不可比**。
- v1 的 13 域打分（21.8% / 18.4% / 21.6%）与 v2 的 16.3% / 22.1% / 19.5% **同样不可比**（v2 新增了两个 0% 的域）。
- 唯一可比的结论是 §5.1–5.5 的**逐项映射**：升级 5 条、降级 0 条、新增 10 条、v1 需订正 2 条。

---

*报告路径：`E:\GodM2\deliverables\审计-CrystalStudio移植-功能对照-v2-2026-11-04.md`｜生成于 2026-11-04｜**只读审计，未改动任何源码 / 配置 / 文档 / `.map` / `catalogue` / `Objects`***
