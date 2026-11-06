# 审计 · CrystalStudio（C#/Avalonia）→ Rust 移植（`tools/miredit`）功能级对照

> 审计日期：**2026-11-04**｜性质：**只读审计**（未修改任何源码 / 配置 / 文档 / `.map` / `catalogue` / `Objects`）
> C# 侧：`E:\GodM2\CrystalStudio\CrystalStudio\**`（158 个 `.cs` / 21,174 行业务代码 + 9 个 `.axaml`）
> Rust 侧：`E:\GodM2\tools\miredit\crates\**`（33 个 `.rs` / 17,367 行：`mir2-map` 1,321 · `mir2-assets` 2,259 · `miredit-render` 1,926 · `miredit-gui` 11,868）
> 数据侧：`catalogue\**` 167 文件 / 69.5 MB｜`Objects\**` 9,170 个 `.X` / 12.3 MB

---

## 一、一句话结论

**Rust 侧目前是一个「能读图、能看、能改地表单帧、能原子保存」的渲染+浏览骨架，还不是地图编辑器**：把 C# 拆成 125 个可比对的功能点后（逐项见 §八 附录），Rust **有 23 项（18.4%）、部分 8 项（6.4%）、完全无 94 项（75.2%）**。
最突出的三块空白是：**① 三层里只有地表层能写**（中层/物件层/碰撞层全无编辑能力）；**② 选择/剪贴板/笔刷体系整体缺席**（框选、复制粘贴、自动图块、墙/树/特殊笔刷、填充全无）；**③ 生成器与数据资产（`catalogue/*.json` + 9,170 个 `.X`）一条都没接**（C# 侧这部分有 **11,868 行 / 70 个 `.cs`**，另加 58.9 MB 的墙体数据）。
已完成度最高的是**撤销重做（90%）**与**视图缩放平移（55%）**——这两块不仅对齐，还比 C# 多做了逐字节往返验证。

### 13 域完成度打分

| # | 功能域 | 完成度 | 判定依据（一句话） |
|---|---|---|---|
| 1 | 地图文件 | **40%** | 打开（CLI）+ 保存（原子）+ 关闭拦截有；新建/另存为/多文档/最近打开全无 |
| 2 | 撤销重做 | **90%** | 笔画合并、100 步上限、保存点关联、逐字节还原全对；缺多文档独立栈与印章快照 |
| 3 | 笔刷系统 | **8%** | 只有单帧地表画笔 + 橡皮；无自动图块、无尺寸、无填充、无墙/树/特殊笔刷 |
| 4 | 选择与剪贴板 | **0%** | 框选/复制/剪切/粘贴/区域填充/清除/属性批量 全无 |
| 5 | 印章（.X） | **0%** | 浏览/放置/导出/识别/分类写回 全无（仅存了一个图标定义） |
| 6 | 素材与分类 | **45%** | 库枚举/缩略图/虚拟滚动/图号搜索/分页 有；收藏/最近/分类树/分组 无 |
| 7 | 图层与显示 | **20%** | 三层可见性有；碰撞层、灯/门高亮、光照、动画、门开合、网格 全无 |
| 8 | 视图 | **55%** | 缩放/平移/适应窗口/纹理缓存有；小地图是假图、无导航、无导出、无概览 mipmap |
| 9 | 停靠与布局 | **10%** | 固定几何常量布局；拖拽/浮动/显隐/持久化/响应式 全无 |
| 10 | 生成器 | **0%** | 城镇/地形/省/河流/神庙/墙围合 六类全无（C# 侧 **11,868 行 / 70 个 `.cs`**：城镇 991 + 地形 2,359 + 省 1,530 + 河流 4,832 + 神庙 619 + 墙 1,537） |
| 11 | 设置 | **0%** | 数据目录/主题/配置持久化 全无；图库根目录写死在源码常量里 |
| 12 | 菜单/工具栏/快捷键 | **15%** | 5 个菜单标签只画不响应；快捷键只有 Ctrl+Z/Y/S/Esc |
| 13 | 数据资产利用 | **0%** | `catalogue/*.json` 零引用、9,170 个 `.X` 零读取；只用了客户端 WIL/WZL 图库 |

> 两种口径都给，避免被误读成"退步"或"差不多了"：
> ① **13 域打分的简单平均 = 21.8%**（把每域百分比等权相加）；
> ② **按功能点计数 = 18.4%**（125 项里 23 项"有"，部分项不计入"有"）；
> ③ 若把"部分"按 0.5 计入，功能点口径 = **21.6%**（(23 + 8×0.5) / 125）。

---

## 二、总表（13 域 × C# 证据 / Rust 现状 / 完成度 / 分级）

分级口径：**P0** = 不做就没法当编辑器用｜**P1** = 影响效率或数据完整性｜**P2** = 可省 / 可后置。

| # | 功能域 | C# 证据 | Rust 现状 | 完成度 | 分级 |
|---|---|---|---|---|---|
| 1 | 地图文件 | `MainWindow.axaml.cs:411-441`（新建，16–4096 校验 + 预设尺寸）· `:443-455`（打开，多选）· `:485-494`（保存/另存为）· `MainWindow.MapTabs.cs:18-101`（多文档 Tab）· `MainWindow.axaml.cs:397-409`（逐页关闭拦截）· `RecentMapStore.cs:8,13,31`（最近 20 条）· `Document.cs:70-79`（保存走自有容器） | 打开=`--map`（`cli.rs:103-106` → `mapdoc.rs:101-144`）；保存=`save.rs:47-…` 原子替换；关闭拦截=`app.rs:457-473`；**无新建/无另存为/单文档**（`mapdoc.rs:86` 注释"本单只支持一张图"） | 40% | **P0** |
| 2 | 撤销重做 | `Document.cs:149-204`（`EditHistory`：`Record`/`Commit`/`Swap`/`Saved`；上限 100 见 `:176-181`；保存点 `:157-158,203`）· `MapViewport.cs:158-159` | `edit.rs:143-448`（`Step`/`stroke` 合并 `:307-369`；`UNDO_LIMIT=100` `:168,364-367`；`saved_pos` `:190,253-255,289-291`）；23 条单测含逐字节还原 | **90%** | P1（缺项） |
| 3 | 笔刷系统 | `AutoTileBrush.cs:15,18`（50/30/5/60 帧块）· `:27-95`（顶点掩码 15 型 + 连通对角线修补）· `:103-108`（`Shape`）· `:285-442`（4 族随机选帧）· `WallBrush.cs:503-771`（墙体笔画规划）· `TreeBrush.cs:44-79`（哈希散点）· `SpecialBrushKind.cs:3`（树/灌木/岩石）· `MapViewport.cs:26-34`（3..31 奇数、地面 2×2 对齐）· `:485-539`（Bucket 连通填充） | 只有 `Tool::Brush/Eraser/Pickup`（`edit.rs:60-108`）；`paint()` 只写 `bk_img`/`tiles`（`:475-492`）；**7 个工具下标一律 `Unwired`**（`edit.rs:74-82`，`app.rs:983-989` 断言 1/2/4/5/7/8/9 不接线）；无尺寸概念（全仓 grep `BrushSize` **0**）；无 `AutoTile`（grep **0**）；无 `flood`（grep **0**） | 8% | **P0** |
| 4 | 选择与剪贴板 | `MapViewport.cs:387-401`（框选 + 夹边）· `RegionCopy.cs:4-92`（快照/剪切/粘贴/锚点）/ `:94-133`（复制/剪切/粘贴/预览着色）· `MapViewport.cs:540`（区域填充）· `:541-545`（Light/Door/Offset 批量）· `:546`（按层清除）· `MainWindow.axaml.cs:631-678`（右键菜单 Ctrl+C/X/V） | **全无**：grep `clipboard`/`RegionCopy`/`paste`/`Paste` 均 **0**；`Selection` 全仓仅 1 处命中，是 `layout.rs:80` 的注释 | 0% | **P0** |
| 5 | 印章（.X） | `StampCatalog.cs:7-18`（递归枚举 9,170 个 `.X`）· `StampBrowser.cs:24-86`（分页/搜索/预览/放置）· `Document.cs:99-147`（v0/v1/v2 读取、`CropStamp` 偶数对齐、保存）· `PlacedStamps.cs`（放置/移动/克隆/删除 + 元数据存取）· `StampIdentifier.cs:15-130`（模板匹配）· `MainWindow.StampEditing.cs:138-199`（分类索引原子回写 + 失败回滚） | **全无**：`stamp`/`Stamp` 全仓 5 处命中，全部在 `icons.rs:165-252` 的图标定义与 tooltip；无 `.X` 读取器（grep `LoadStamp` **0**） | 0% | **P0** |
| 6 | 素材与分类 | `MainWindow.axaml.cs:728-759`（分页/过滤）· `AssetPalette.cs:19-36`（4 页签）· `AssetHistoryStore.cs:33,39,58,65,73`（收藏 + 最近 48 条，`assets.json`）· `CategoryPalette.cs:92-151`（分类树）· `StampCategories.cs:61-67`（按 Type/Region/Environment/Facing 分组）· `terrain.json`/`water.json` 驱动图案组 | `palette.rs:1-…`：库枚举来自真实 `.wil/.wzl`（`:1001-1017`）、按需解码 + LRU 512（`:44-49`）、虚拟滚动（`:11-19`）、图号跳转搜索（`:441-476`，仅接受纯数字，越界/非数字如实给文案）、`prev/next` 整屏翻页（`:741-753`）、库下拉（`:853-886`）；**收藏/最近/分类三页签只显示"该页签未实现"**（`palette.rs:25-27`、`dock_right.rs:582-583`、`ui_text.rs:222`） | 45% | **P1** |
| 7 | 图层与显示 | `MapViewport.cs:22-23`（碰撞层掩码 0x2000_0000/0x4000_0000）· `:480`（碰撞层写格）· `:485-539`（碰撞填充）· `CollisionMarkers.cs:1-30`（粉/蓝/双色）· `MapViewport.Presentation.cs:43-50`（灯/门高亮）· `:59-97`（3 档暗度 205/0/255 + 加法光叠加）· `MapPreview.cs:21-82`（`PreviewLights.{445,685,925}.png` 内嵌资源）· `MapPreview.cs:43-47`（门开合）+ `MapViewport.cs:132,139`（100 ms 动画帧推进） | 三层可见性=`LayerMask`（`canvas_view.rs:80-81`、`dock_left.rs:54`）；**低/高碰撞两行 UI 存在但 `layer_wired()` 返回 false**（`dock_left.rs:39-70`、`app.rs:953-955` "未接线的行在这里什么都不做"）；`light` 只作为字段被读出显示（`cell.rs:171`、`cellinfo.rs:18`），**无渲染**（grep `Lighting` **0**）；无动画（grep `animation` 仅 1 处文案 `ui_text.rs:125`）；**网格被刻意撤掉**（`canvas.rs:12-14`） | 20% | **P0**（碰撞）+ P1（其余） |
| 8 | 视图 | `MapViewport.cs:173-180`（`SetZoom`）/`:447`（滚轮 1.15）/`:162`（夹在 `FitZoom..1`）/`:161`（`Fit`）· `MapMinimap.Avalonia.cs:1-92`（真实缩略图 + 视口框 + 点击居中 + 拖拽 + 滚轮）· `MapViewport.Overview.cs:28-47` + `OverviewBatch.cs`（<25% 走 mipmap 概览批量）· `MinimapExporter.cs:15-53`（12×8 px/格 PNG 导出） | 缩放 0.25..4.0（`metrics.rs:285,288`）、滚轮以鼠标为中心（`app.rs:976-985`）、`Fit Map`（`:1010-1021`）、整块纹理缓存 + 变化才重渲（`canvas_view.rs:18-50,209-…`，含 16,000 格预算护栏 `metrics.rs:326`）；**小地图是写死的浅灰方块 + 装饰小框**（`dock_right.rs:8` 注释、`:415-442`）；无概览 mipmap（grep `overview` **0**）；无 PNG 导出（grep `export` **0**） | 55% | P1（小地图/导出/概览） |
| 9 | 停靠与布局 | `DockWorkspace.cs:6-…`（327 行：拖拽停靠 `:101-120+`、浮动 `:149`、显隐、锁定 `:44-52`、`workspace.json` 持久化 `:12,78`、默认布局内嵌 `:38-42`）· `MainWindow.axaml.cs:241-265`（响应式素材面板）· `ResponsivePanel.cs` | 布局全部由 `Layout::compute` 按常量算死（`layout.rs:1-90`：左 156 / 右 356 / 标题 36 / 菜单 30 …）；**无拖拽、无浮动、无显隐菜单、无持久化**（grep `float`/`Float` **0**） | 10% | **P1** |
| 10 | 生成器 | 城镇 `TownGenerator.cs:25-332` + `TownWalls.cs:268` + `TownStreetLayout.cs:143` + `TownCatalogue.cs:65`（`towns.json` 1.5 MB）· 地形 `TerrainGenerator.cs:24-424` + `TerrainCatalogue.cs:18`（`terrain.json`）+ `TerrainWater*.cs`（`water.json` 1.7 MB）+ `TerrainCoastObjects.cs:399`· 省 `ProvinceGenerator.cs:38-259` + 9 个附属（1,271 行）· 河流 28 文件 **4,832 行**（`river-artwork.json` + `river-coast-layouts/**`）· 神庙 `TempleGenerator.cs:5-341`（11 种地牢预设）· 墙 `WallBrush.cs:944` + 7 附属（合计 1,537 行，19 族 58.9 MB 墙体数据）+ 6 个设置窗口 `.axaml` | **全无**：grep `town`/`Town`/`temple`/`Temple`/`province`/`Province`/`river`/`River`/`generator`/`Generator` 在 33 个 `.rs` 里 **全部 0 命中**；`terrain` 4 处命中全是截图文案（`metrics.rs:359,469,539`、`ui_text.rs:236`） | 0% | **P0**（就"编辑器"整体定位而言）/ P2（若只看"能改图"） |
| 11 | 设置 | `SettingsStore.cs:10-25`（`settings.json` 存图库目录）· `SettingsWindow.cs:1-43`（目录选择 + 首次引导 + 失败原文）· `MainWindow.axaml.cs:498-535`（`OpenSettings`）· `ThemeService.Avalonia.cs:11,18,26`（`appearance.json` 深色偏好）+ `:35-58`（按系统 accent 重算调色板）· `EditorPaths.cs:5`（`CRYSTALSTUDIO_PROFILE` 重定向） | **全无**：grep `settings`/`Settings` **0**；图库根目录是硬编码常量 `DEFAULT_CLIENT_ROOT = r"E:\GodM2\client\热血传奇23周年客户端"`（`mir2-assets/src/lib.rs:85`），改路径必须改源码重编译 | 0% | **P0**（目录） / P1（主题） |
| 12 | 菜单/工具栏/快捷键 | `MainWindow.axaml:5`（File 13 项 / Edit 5 项 / View 9+ 项 / Tools 6 项 / Help 1 项，含 `InputGesture`）· `MainWindow.axaml.cs:832-869`（B/P/F/E/M/O/V/K/T/H/G/C/F1/`[`/`]`/WASD/←→/Delete/Esc）· `:1080-1102`（F1 快捷键表对话框）· `MainWindow.axaml.cs:372-391`（View 菜单动态挂停靠面板项） | 5 个标签 + 品牌字，**只画 hover，无任何下拉**（`menubar.rs:7` 注释、`:85-95`）；快捷键只有 `Ctrl+Z/Y/S/Esc`（`app.rs:303-336`）；工具按钮 10 个但只有 3 个可落格（`edit.rs:44-49`） | 15% | **P0**（菜单）/ P1（快捷键） |
| 13 | 数据资产利用 | 见 §五：`palette.json` 9,159 条、`terrain.json`、`water.json`、`towns.json`、`walls.json`(索引)+19 族、`river-artwork.json`+`river-coast-layouts` 120 文件、`island/catalogue.json`+20 个 `.X`、`province-decoration-ground.json`、`Objects/**` 9,170 个 `.X` | 只有客户端图库（`mir2-assets`：`LibPaths::discover` + `WilSource`）；`catalogue`/`palette.json`/`walls.json`/`river-*` 等关键词在 Rust 侧 **全部 0 命中**；`.X` 零读取 | 0% | **P0**（.X）/ P1（catalogue） |

---

## 三、遗漏清单

### 3.1 P0（不做就没法当编辑器用）—— **11 条**

| # | 功能 | C# 证据 | Rust 现状 | 影响 |
|---|---|---|---|---|
| P0-1 | **新建地图**（尺寸对话框、16–4096 校验、3 个预设、`Ctrl+N`） | `MainWindow.axaml.cs:411-441` | 无。只能打开已存在的 `.map`（`cli.rs:26`），没有"从零造一张图"的入口 | 做新地图必须先用外部工具生成空文件；编辑器闭环缺第一环 |
| P0-2 | **另存为 / 保存到新路径**（`Ctrl+Shift+S`） | `MainWindow.axaml.cs:485-494`（`saveAs` 参数 → `SaveFileDialog`） | 无。`save::save_session` 只写回 `doc.path`（`save.rs`+`app.rs:342-352`） | 改坏原图无法另存对比；做模板/派生图只能先手工复制文件 |
| P0-3 | **多文档（Tab）**：每页独立 `Doc/History/Zoom/origin/Selection`、切换、单页关闭、逐页脏提示 | `MainWindow.MapTabs.cs:5-101`（`OpenMapTab` 持 `MapEditorState`）· `MapViewport.cs:142-155`（`CaptureEditorState`/`RestoreEditorState`）· `MainWindow.axaml.cs:399-409`（逐页确认） | 无。`DocState.doc: Option<MapDoc>`（`mapdoc.rs:86-95`），页签条上只画**一个**写死的标签（`canvas.rs:82-90,136-146`），关闭符只画不响应（`app.rs:866-1022` 无页签交互） | 一次只能开一张图；A 图抄 B 图必须开两个进程 |
| P0-4 | **中层（SmTiles）/物件层（Objects）可编辑** | `MapViewport.cs:479-480`（按 `Layer` 分别写 `BackImage`/`MiddleImage`/`Foreground`）· `AutoTileBrush.cs:112`（`Put` 三层分支） | ❌ 只写地表。`edit.rs:475-492` 的 `paint()` 只动 `bk_img`+`tiles`；模块文档明写"中层 `mid_img` / 物件层 `fr_img` / 门 / 动画 / 区域 / 光照 / 36 档尾区一律不碰"（`edit.rs:16-17`） | **三层里两层完全不能编辑** ⇒ 做不出"墙 + 装饰 + 地表"的完整场景 |
| P0-5 | **选择与剪贴板**：框选、复制（按可见层快照）、剪切、粘贴（锚点对齐/越界拒绝/半透明预览/可重复粘贴）、区域填充、按层清除、右键菜单 | `MapViewport.cs:387-401` · `RegionCopy.cs:4-133` · `MapViewport.cs:540-546` · `MainWindow.axaml.cs:631-678` | ❌ 无（grep `clipboard`/`RegionCopy`/`paste` 全 0） | 无法搬运地块、无法复制现成建筑；所有重复劳动只能手点 |
| P0-6 | **笔刷尺寸/形状**（3..31 奇数、地面 2×2 对齐、`[` `]` 调整、铅笔/填充恒 1×1） | `MapViewport.cs:26-34` · `MainWindow.axaml.cs:861` · `MainWindow.axaml:6`（滑条 3–31 步进 2） | ❌ 无尺寸概念（grep `BrushSize` 0）；每次只落 1 格（`edit.rs:320-347`） | 铺一片 30×30 的地面要点 900 次；实际不可用 |
| P0-7 | **填充（Bucket）**：按"同库同图号"连通填充；地面/中层/低高碰撞；步长 2（地面） | `MapViewport.cs:485-539`（BFS + `visited` 去重 + 一次 `Commit`）· `MainWindow.axaml.cs:629`（`Fill Selection`） | ❌ 无（grep `flood` 0；`bucket` 5 处命中全在图标表 `icons.rs:10,80,228`） | 无法一次刷满封闭区域 |
| P0-8 | **自动图块（AutoTile）**：Mir2 50 帧 / Mir3 30 帧（含 B 段 +5 偏移）/ Mir3Mini 5 帧 / SmTile 60 帧；顶点覆盖掩码 → 15 种形状；对角线断连修补（mask 6/9 → 15）；按形状随机取帧；预览 | `AutoTileBrush.cs:15,18,27-95,103-108,161-283,285-442` · `MapViewport.cs:378-386`（`PlanBrush` 预览）· `MainWindow.axaml.cs:726,734-738`（图案组聚合显示） | ❌ 无（grep `AutoTile`/`autotile` 0）。Rust 画笔只把同一个图号重复写进每一格 | 这是地表能不能"连成图案"的核心；缺它 ⇒ 地表只能画成一片同帧方块 |
| P0-9 | **碰撞层（低/高）编辑 + 可视化**：把 `0x2000_0000`/`0x4000_0000` 当可编辑层，画/擦/填充；粉/蓝/双色描边；`C` 键一键开关 | `MapViewport.cs:22-23,480,485-539` · `CollisionMarkers.cs:1-30` · `MainWindow.axaml.cs:859` · `MainWindow.axaml:46-56`（两行图层 + 颜色标记） | ❌ 有 UI 行但未接线：`dock_left.rs:39-70` 的 `layer_wired()` 只对 0/1/2 返回 true，`app.rs:953-955` 注释"低/高阻挡在这里什么都不做"；`LayerMask` 只有 `ground/middle/front` 三个字段（`canvas_view.rs:80-81`） | 做出来的图**能不能走人都定不了**；这是"地图编辑器"的硬指标 |
| P0-10 | **印章（.X）放置体系**：递归索引 9,170 个 `.X`、搜索分页、载入、放置/拾取/移动/克隆/删除、随地图存取 `PlacedStamp`、导出选区（偶数对齐裁剪）、v0/v1/v2 兼容 | `StampCatalog.cs:7-18` · `StampBrowser.cs:24-86` · `Document.cs:99-147` · `MapViewport.cs:44-74` · `MainWindow.axaml.cs:572-584` · `MainWindow.MapTabs.cs:10` | ❌ 全无。`stamp` 5 处命中全是图标（`icons.rs:165-252`） | 物件（树/建筑/摊位…）全靠印章搬；缺它 ⇒ 物件层等于没有素材来源 |
| P0-11 | **菜单可下拉**（File 13 项 / Edit 5 项 / View 9+ 项 / Tools 6 项 / Help 1 项） | `MainWindow.axaml:5`（一次声明全部菜单项与 `InputGesture`）· `MainWindow.axaml.cs` 各 `*Click` 处理器 | ❌ `menubar.rs:7` 明写"本单不做下拉菜单"，`:85-95` 只做 hover 高亮 | 所有功能只剩快捷键；用户看不见"能做什么" |

> 补充（同样 P0 级但归到域 11）：**图库/数据目录设置** —— C# `SettingsWindow.cs:1-43` + `SettingsStore.cs:10-25`；Rust 硬编码 `mir2-assets/src/lib.rs:85`。他人机器上跑必然"0 个库"（前端已有降级，但无法在界面里改）。
>
> **为什么"生成器"（域 10）没算进这 11 条**：生成器是"造内容"的能力，不是"改内容"的能力 —— 没有它仍可当编辑器用（C# 的 4 个生成器入口 `Generate Town / Terrain / Province / Dungeon` 与 `Identify Objects` 并列挂在 `Tools` 菜单下（`MainWindow.axaml:5`），**不是编辑前置**）。所以 §二 给它标的是「P0（替代 C# 编辑器整体）/ P2（仅就"能改图"）」，**两种定位下分级不同**，本清单按"能改图"口径不计入 P0。

### 3.2 P1（影响效率或数据完整性）—— **21 条**

| # | 功能 | C# 证据 | Rust 现状 | 影响 |
|---|---|---|---|---|
| P1-1 | 墙笔刷（19 族 58.9 MB：`walls.json` 索引 → 族文件；双格栅格、内/外面、转角、连接、门洞、剔除/加宽、预览、`Shift` 删段） | `WallBrush.cs:30-333`（`WallFamily` 42 个属性）· `:335-489`（目录加载）· `:503-771`（笔画规划）· `WallTurnRouting.cs:172` · `WallEntrances.cs:93` · `WallBrushWindow.axaml.cs:63` | 无（`WallBrush` grep 0） | 建筑内部/城墙/隧道全部做不了 |
| P1-2 | 树/灌木/岩石散点笔刷（哈希散布、间距、密度、圆形范围、变体轮换、可选碰撞、`.treelist` 存取） | `TreeBrush.cs:44-79` · `SpecialBrushKind.cs:3` · `TreeBrushList.cs:135-183` · `SpecialBrushWindow.axaml.cs:384` | 无（`tree` 15 处命中全是 `cargo tree` 等词） | 森林/植被只能手摆 |
| P1-3 | 笔刷落格预览（半透明叠加，随尺寸/图号/层实时变） | `MapViewport.cs:308-326` · `:378-386`（`PlanBrush`） | 无。只有悬停格 2 px 金色描边（`canvas.rs:9`） | 落笔前看不到会改成什么 |
| P1-4 | 材质动画预览（100 ms 定时器推进 `MiddleFrames/MiddleTick`、`Front.Frames/Tick`） | `MapViewport.cs:132,139,259-261,279-280,344-346` | 无（`animation` 1 处命中是文案 `ui_text.rs:125`） | 水面/火焰/门等动态帧全部静止 |
| P1-5 | 灯光预览（3 档：Dawn/Dusk 205、Day 0、Night 255；`PreviewLights.{445,685,925}.png` 加法叠加） | `MapPreview.cs:21-82` · `MapViewport.Presentation.cs:59-97` · `MainWindow.axaml.cs:616-623` + `MainWindow.axaml:5` 的 `MapLightingMenu` | 无（grep `Lighting` 0） | 洞里是黑的、夜图看起来正常 —— 光照设计无从校验 |
| P1-6 | 灯格/门格高亮（琥珀/青，各占半格） | `MapViewport.Presentation.cs:43-50` · `MainWindow.axaml:61-62` | 无 | 找不到需要调 Light/Door 的格子 |
| P1-7 | 门开/关预览（`doorsOpen` 时叠加 `DoorOffset`） | `MapPreview.cs:43-47` · `MainWindow.axaml:5`（`DoorsOpenToggle`） | 无 | 门的位置/偏移无法目视校验 |
| P1-8 | 网格显示（`G` 键，缩放 ≥0.4 才画） | `MapViewport.Presentation.cs:51-55` · `MainWindow.axaml.cs:857` | **刻意撤掉**（`canvas.rs:12-14` 理由：避免与客户端比对产生色差） | 对齐/数格子只能靠眼估；建议做成"默认关、可开"而不是删掉 |
| P1-9 | 小地图真实渲染 + 导航（点击居中、拖视口框平移、滚轮缩放，随编辑增量失效） | `MapMinimap.Avalonia.cs:41-92` · `MapViewport.cs:223`（`MinimapInvalidated`） | 写死假图：`dock_right.rs:415-442`（浅灰方块 200×138 + 14×14 装饰框） | 大地图找不到自己在哪 |
| P1-10 | 缩略总览（缩放 <25% 时按 2^k 级 mipmap 批量绘制 + 缓存 + 编辑行带增量） | `MapViewport.Overview.cs:28-47` · `OverviewBatch.cs:1-265` | 无（grep `overview` 0）。0.25× 仍按 1:1 渲染可见格，超 16,000 格直接不画（`canvas_view.rs:249-265`，`metrics.rs:326`） | 缩到最小看全图会白屏（只留诊断 `DIAG_RENDER_BUDGET`） |
| P1-11 | 小地图导出 PNG（12×8 px/格，含全层、首帧、闭门） | `MinimapExporter.cs:15-53` · `MainWindow.MinimapExport.cs:6-35` | 无（grep `export` 0） | 无法出图给策划/文档 |
| P1-12 | 收藏 + 最近使用（★/☆ 切换、48 条、按层/库/图号/路径搜索、跨会话持久化 `assets.json`） | `AssetHistoryStore.cs:33-73` · `AssetPalette.cs:103-149` · `MainWindow.axaml.cs:793-831` | 页签存在但整页空（`palette.rs:25-27`、`ui_text.rs:222` "该页签未实现"） | 常用素材每次都要翻页找 |
| P1-13 | 分类树（`palette.json` 9,159 条 → Type/Region/Environment/Facing 四维分组、搜索、懒展开预览、双击开印章编辑器） | `CategoryPalette.cs:92-221` · `StampCategories.cs:12-73` | 无（`catalogue`/`palette.json` grep 0） | 9,170 个 `.X` 没有分类就只能是 5,000 页的无序列表 |
| P1-14 | `Identify Objects`（把地图上已拼好的物件反向识别成印章实例、写进 `PlacedStamps`） | `StampIdentifier.cs:15-130`（按图号建倒排索引 → 最稀有格先比 → 认领去重）· `MainWindow.axaml.cs:457-475` | 无（grep `Identify` 0） | 老图无法转成可复用的印章语义；捡取/移动/克隆都用不了 |
| P1-15 | 印章分类索引写回（`palette.json` 原子改写 + 双路径回滚 + 新 ID 查重） | `MainWindow.StampEditing.cs:138-199` · `StampCategories.cs:48-60` | 无 | 新印章进不了分类体系 |
| P1-16 | 停靠面板体系（拖拽停靠 4 边、双击浮动、右键 Dock/Float/Hide、锁定、`workspace.json` 持久化、`Reset Panel Layout`、"面板"分节菜单） | `DockWorkspace.cs:6-327` · `MainWindow.axaml.cs:267-391` | 固定几何（`layout.rs`）；无拖拽/浮动/持久化（`float` grep 0） | 双屏/小屏体验差；C# 用户习惯的布局搬不过来 |
| P1-17 | 主题（深色/跟随系统 + `appearance.json` + 按系统 accent 重算 40+ 个 `ThemeXXXXXX` 色键） | `ThemeService.Avalonia.cs:11-58` · `MainWindow.axaml.cs:483` + `MainWindow.axaml:5`（Dark Mode 勾选） | 无（`theme` 37 处命中是 `crate::theme` 常量模块，非主题切换） | 亮色/暗色不可切换 |
| P1-18 | 快捷键体系（B/P/F/E/M/O/V/K/T/H/G/C/F1/`[`/`]`/WASD/←→/Delete/Esc + Ctrl+N/O/S/Shift+S/Z/Y/Shift+Z/C/X/V/Shift+N） | `MainWindow.axaml.cs:832-869` · `:1080-1102`（F1 表） | 只有 `Ctrl+Z/Y/S` + `Esc`（`app.rs:303-336`） | 全部操作都得点图标 |
| P1-19 | 地图格式兼容（8 种方言：C# 容器 v1–v5、wemade mir3、shanda mir3、anti-hack、2010、shanda2012/older、3-4 heroes、old school 12B） | `LegacyMapReader.cs:55-117`（判据分派）· `:119-570`（各 `LoadMapTypeN`；`ValidateDimensions` 的格长分别 12/14/15/20/26/33/35/36/38） | 只认 3 档：`Old12` / `New14` / `Ext36`，且用"文件长度精确整除"判定（`map.rs:53-82`、`cell.rs`）。**其余 5 种方言一律 `LengthMismatch` 报错** | 老资料/韩版图打不开；见 3.3 关于 36B 方言不兼容 |
| P1-20 | 单元格属性批量编辑（Light 0–255 / Door / DoorOffset，作用于单格或选区） | `MainWindow.axaml.cs:680-698` · `MapViewport.cs:541-545` | 只读显示（`dock_left.rs` 的检查器、`cellinfo.rs`） | 改一个灯值必须手改二进制或外部脚本 |
| P1-21 | 忙碌态/失败态（全界面禁用 + 进度条 + 状态栏原文 + 弹窗） | `MainWindow.axaml.cs:871-1032`（`busy` 全量闸门）· `:121`（`Performance` 文案） | 部分：状态栏有失败原文（`screen_text::build`）、`DIAG_*` 打到 stderr；无忙碌禁用/进度条 | 长耗时操作（大图重渲）界面无反馈 |

### 3.3 P2（可省 / 可后置）—— **9 条**

| # | 功能 | C# 证据 | Rust 现状 | 影响 |
|---|---|---|---|---|
| P2-1 | 最近打开菜单（20 条、失效置灰 + tooltip） | `RecentMapStore.cs:8-39` · `MainWindow.RecentMaps.cs:23-40` | 无（`recent` 6 处命中是素材页签文案） | 效率项，命令行传路径可替代 |
| P2-2 | 锁定面板布局 | `DockWorkspace.cs:44-52` · `MainWindow.axaml.cs:382-385` | 无 | 依附 P1-16 |
| P2-3 | 响应式面板（窄→宽自动改列、图层行图例自适应） | `MainWindow.axaml.cs:241-265` · `ResponsivePanel.cs:1-39` | 无（`layout.rs` 单套固定几何） | 1500×950 基准下无影响 |
| P2-4 | `.X` 当文档编辑（双击印章 → 独立 Tab，可画可存，尺寸 = 内容 + 17） | `MainWindow.StampEditing.cs:57-89` · `:40-56`（新建印章） | 无 | 依赖 P0-10 |
| P2-5 | 印章预览合成（缩略图 120×84，按层序画） | `StampPreview.cs:1-85` · `CategoryPalette.cs:190-200` | 无 | 依赖 P0-10 |
| P2-6 | 渲染统计上界面（`Prepare X ms · Batches N · Sprite cache M`） | `MainWindow.axaml.cs:121`（`Performance.Text`） | 等价信息只在 stderr + `--autodrag` 报告（`app.rs:534-557`）；不占界面 | 无用户可见影响 |
| P2-7 | 图案组预览（把 50 帧块合成一张"地形组"缩略图） | `TilePatternPreview.cs:1-41` · `AssetPalette.cs:84-86` | 无 | 依附 P0-8 |
| P2-8 | 高对比度/无障碍名（`AutomationProperties.Name`、highContrast 调色） | `ThemeService.Avalonia.cs:43-58` · `MainWindow.axaml` 多处 | 无 | 非功能项 |
| P2-9 | 页签关闭符可点（当前只画） | — | Rust 画了 `×` 但未接线（`canvas.rs:82-90`，`app.rs` 无对应交互） | 单文档下无意义，随 P0-3 一起做 |

### 3.4 两边都没有的（列出以免被当成移植缺口）

| 功能 | 检索口径 | 结论 |
|---|---|---|
| **裁剪地图（CutMap）/ 改地图尺寸** | C# 全 158 个 `.cs` grep `CutMap\|TrimMap\|CropMap\|ResizeMap\|SetSize` → **0**；`\bResize\b\|\bCrop\b\|\bTrim\b` 的 40 处命中全是文字 `Trim()`、面板尺寸、`CropStamp`、地形裁剪等无关项 | **C# 侧不存在"裁剪/改尺寸"功能**；Rust 也没有 ⇒ **不是移植缺口**。C# 的 `MapDocument.Width/Height` 是只读（`Document.cs:35-36`），尺寸只在新建时定 |
| 快捷键自定义 | C# 快捷键全部硬编码（`MainWindow.axaml.cs:832-869` + `.axaml` 的 `InputGesture`），无配置文件、无设置界面 | 两边都无 ⇒ 不是缺口 |
| 地图格式 `36B` 的"唯一性" | **需订正**：C# 也有 36 字节格格式（`LegacyMapReader.cs:272-326` `LoadMapType3`，`ValidateDimensions(52, 36)`），但**布局与 GodM2 的 Ext36 完全不同**（见下） | 见 §四 第 1 条的口径订正 |

> **36B 方言差异（重要，可能影响互操作）**
> - **GodM2 / Rust `Ext36`**：= 14 字节结构 + 22 字节尾区；库号在 `btTiles`/`btsmTiles`（偏移 12/13）；尾区原样保留、原样写回（`mir2-map/src/map.rs:15-18` 两条铁律）。
> - **C# `LoadMapType3`**：36 字节，但库号**不存在于格内**，是运行时**编造**的（`frindex = Bytes[offset]+120`、`BackIndex = Bytes[offset]+100`、`MiddleIndex = Bytes[offset]+110`，`LegacyMapReader.cs:296-299`），后 21 字节（7+14）被当作不透明数据跳过（`:301,304`）。
> ⇒ **同一个"36 字节"在两侧语义不同**。C# 读 GodM2 的高清图会得到错库号；Rust 读 C# 的 Type3 会因布局不符解析出乱的 `fr_img`。这是**移植后仍然存在的数据互操作风险**，建议单列验证项。

---

## 四、Rust 已有的但 C# 没有的（我们多做的）

| # | 能力 | 位置 | 为什么算"多做的" |
|---|---|---|---|
| 1 | **36B/格（`Ext36`）三档精确判别 + 尾区零丢失** | `mir2-map/src/cell.rs`（`CellFormat::detect`）· `map.rs:53-82,84-99` · 逐格往返测试 `roundtrip_tests.rs:1-586` | C# 虽有 36 字节读取函数，但**语义不同**（见 §3.4 订正），且 C# 从不写回 36B —— 它把一切都转成自有容器。Rust 是"原样进、原样出" |
| 2 | **52 字节文件头逐字节保真 + 原格式写回** | `map.rs:15-18,29-32`（`header_raw` 原样照抄）· `save.rs:47-…` | C# `MapDocument.Save` 一律写成自有 `"C#"` v1 容器（`Document.cs:70-79`：`short 1` + `'C'` + `'#'` + w + h + 26 字节/格），**原始 Mir2 格式丢失**。Rust 保存后仍是原格式 ⇒ 可直接给客户端用 |
| 3 | **原子保存 + 读回逐字节核对** | `save.rs:47-80`（写同目录 tmp → 读回 memcmp → rename） | C# `AtomicFile.cs:8-26` 只做"tmp + `File.Move(overwrite)`"，**不回读校验**；短写/缓写在 C# 侧不会被发现 |
| 4 | **无窗 CLI 编辑驱动**（`--edit-pick/brush/erase/undo/redo/check`、`--save`、`--probe`、`--dump-render`） | `cli.rs:23-93,123-143` · `editrun.rs:1-189` · `edit.rs:578-749`（`apply_script`） | C# 无任何无窗入口（`Program.cs:1-8` 只启 GUI）。Rust 这条链路让"改图→存盘→逐字节比对"可无人值守复现 |
| 5 | **截图/比对/性能自检门**（`--shot`、`--gate2-client`+`--gate2-at` 同视点分块比对 ≥95%、`--autodrag`、`--palette-autoscroll`、`--palette-report`） | `app.rs:654-729`（`run_gate2`）· `selfcheck.rs:1-377` · `miredit-render/src/compare.rs:1-264`（`compare_blocks`） | C# 只有 15 条 headless UI 测试（`CrystalStudio.Tests/MigrationTests.cs`），**没有像素级客户端同视点比对**。Rust 的"门2 命中率 ≥95%"是 C# 完全不具备的回归护栏 |
| 6 | **197 条单元/验收测试**（33 个 `.rs` 里 32 个带 `#[test]`） | 计数口径：`Select-String -Pattern '^\s*#\[(test\|test_case)\]'` → **197**；最密 `edit.rs` 23 条、`cli.rs` 10 条、`palette.rs` 10 条 | C# 侧 **15 条**（`MigrationTests.cs` 的 `[Fact]/[AvaloniaFact]`）。Rust 的 `edit.rs` 测试逐字节断言"撤销 3 步 == 原文件"（`edit.rs:1022-1048`） |
| 7 | **缺帧显式口径 + 负缓存**（取不到就不画、计数进 `missing_*`；空帧/偏移 0 做负缓存不重复解码） | `miredit-render/lib.rs:43-48`（禁洋红占位）· `mir2-assets/wil_source.rs`（`failures()`）· `palette.rs:17-18,101-…`（`Option<Thumb>` 负缓存） | C# 是 `assets?.Get(...)  is not Sprite` 直接 return（`MapViewport.cs:364,372`），无统计口径；且印章/图案预览会画"部分成功"的合成图 |
| 8 | **中文字体自解析**（系统单面 TTF → `ab_glyph` → egui 首选字体；拿不到 `exit(2)`） | `fonts.rs:1-213` | C# 依赖 Avalonia + 系统字体族名（`MainWindow.axaml:1` `FontFamily="Segoe UI"`），**没有"必定排出中文"的显式闸门** |
| 9 | **界面文案总表 + 占位一致性测试** | `ui_text.rs:1-751`（全部模板 + `TEMPLATES` 自检表）· `ui_text.rs:4` 的测试 | C# 文案散落在 40+ 个 `.cs` 与 `.axaml` 里（如 `Status.Text = "…"` 常量拼接），无法机械核对 |
| 10 | **渲染预算护栏**（可见格 > 16,000 直接不渲并打 `DIAG_RENDER_BUDGET`） | `canvas_view.rs:249-265` · `metrics.rs:326` | C# 没有等价上限（`OverviewBatch` 是分批 + 取消，不是硬闸）；Rust 这条防止"拖到 0.25× 卡死" |
| 11 | **显式"未接线"报告**（`Tool::Unwired`、`PALETTE_TAB_UNIMPLEMENTED`、图库失败如实报 0） | `edit.rs:69-107` · `mapdoc.rs:59-83` · `ui_text.rs:222` | 这是诚实的工程纪律（正面），但也正是本报告 P0 的来源 |

---

## 五、数据资产利用现状

### 5.1 `catalogue/**`（167 文件 / 69.5 MB）

| 资产 | 体量 | C# 消费点 | Rust |
|---|---|---|---|
| `palette.json` | 5.09 MB / **9,159 条**（`schemaVersion:1`，字段 `id/file/name/type/subtype/regions/regionGroups/environments/tags/direction/directionStatus/preview/previewComplete/reviewStatus`） | `StampCategories.cs:16-38`（读 + 校验 + 路径越界防护）· `CategoryPalette.cs:123`（分类树）· `ProvinceGenerator.cs:44`（省景挑选）· `MainWindow.StampEditing.cs:163-182`（**写回**） | ❌ 0（grep `palette.json`/`catalogue` 均 0） |
| `terrain.json` | 263 KB | `TerrainCatalogue.cs:18,39`（地形集 + 与 `water.json` 配对） | ❌ 0 |
| `water.json` | 1.70 MB | `TerrainCatalogue.cs:39` · `TerrainWaterCatalogue.cs` | ❌ 0 |
| `towns.json` | 1.55 MB | `TownCatalogue.cs:22`（城镇构件族 + 道路/正面朝向） | ❌ 0 |
| `walls.json`（索引，`schemaVersion:2`）+ `walls/**/walls.json` **19 族** | 766 B + **58.9 MB** | `WallBrush.cs:338-489`（`DefinitionPaths` 展开 → 每族 `WallCatalogue.Read`） | ❌ 0 |
| `river-artwork.json` | 312 KB | `RiverArtworkCatalogue.cs:1-141`；被 8 个 `River*Component.cs` 各 `Load` 一次 | ❌ 0 |
| `river-coast-layouts/**`（120 文件 / 1.95 MB：`chains` `edges` `forks` `caps` `bridges` `winding` `variants` `modules` + `catalogue.json` + `piece-ports.json` + 10 个顶层 `.X`） | 1.95 MB | `RiverCoastPieces.cs:137-139` · `RiverCoastLayoutCatalogue.cs:249` · `TerrainCoastObjects.cs:47-94` · `RiverShoreComponents.cs:11` · `RiverBridgeComponent.cs:73` | ❌ 0 |
| `island/`（`catalogue.json` 140 KB + 20 个 `mir_0*.X`） | 140 KB + 67 KB | `IslandCoastObjects.cs:15` | ❌ 0 |
| `province-decoration-ground.json` | 29 KB | `ProvinceSceneryPalette.cs:10` | ❌ 0 |
| `previews/**` | **目录不存在**（0 个 PNG） | `palette.json` 里的 `preview` 字段指向 `previews/…png`，但 C# 实际走**运行时合成**（`CategoryPalette.cs:190-200`），不读 `preview` 文件 | — （不是缺口，只是字段空转） |

### 5.2 `Objects/**` —— **9,170 个 `.X` / 12.3 MB**

- 目录结构：`mir/`（主体）、`InsectTemplate/`、`StoneTemplate/`、`WallEntrances/`（后三者被 `ProvinceGenerator`/`WallEntrances` 当模板用）。
- C# 消费点：`StampCatalog.cs:15`（`Directory.EnumerateFiles("*.X", RecurseSubdirectories)` 递归索引）→ `StampBrowser`（列表/搜索/预览/放置）→ `StampIdentifier.cs:47`（模板匹配）→ `TreeBrush.cs:14`（树笔刷素材）→ `ProvinceGenerator.cs:58`（省景素材）。
- Rust：**零读取**。全仓 grep `.X`/`.x` 的 57 处命中全部是 `0x`/十六进制字面量或 `.xml` 类字样，没有一处打开 `.X` 文件；也没有 `.X`（印章）解析器。

### 5.3 结论

**Rust 侧目前只接了一半数据源**：客户端图库（`.wil/.wzl` 帧源）接通且质量不错（负缓存、按需解码、LRU 512）；**而 `catalogue` 69.5 MB 语义数据 + 9,170 个 `.X` 造型资产，一条都没接**。
这直接解释了为什么 §三 的 92 个"无"里，生成器、印章、分类、墙体四大块全是 0 —— **它们不是"代码没写"，是"数据没接，代码无从写起"**。反过来说：这四块的移植**先天比 C# 更好做**，因为 C# 侧的 schema（`palette.json` 字段表、`walls.json` 的 `WallFamily` 42 个属性、`catalogue.json` 的 `RiverCoastLayout`）已经现成，只需照读。

---

## 六、建议优先级（10 条，仅按"解锁面积 / 单位成本"排序，非"该不该做"）

1. **P0-4 打通三层写入**（中层 `mid_img`/`btsmTiles`、物件层 `fr_img`/`btArea`、门/动画字段）。改动集中在 `edit.rs::paint()` 与 `Brush` 结构；这是**最小改动、最大解锁**（4 个域同时受益：笔刷、印章、碰撞、选择）。
2. **P0-6 + P0-7 笔刷尺寸与填充**。`paint()` 已有单格路径，扩成 footprint 循环 + BFS 即可；C# 的步长/夹边规则可直接照抄（`MapViewport.cs:26-34,485-539`）。
3. **P0-9 碰撞层**。`LayerMask` 加两个 bool、`layer_wired` 放开 3/4、画笔按 `0x2000_0000/0x4000_0000` 读写、渲染侧加半透明标记。**它是"这图能不能走人"的唯一判据**，成本却很小。
4. **P0-1/2/3 文档生命周期**（新建 + 另存为 + 多文档 Tab）。建议一次做：`DocState` → `Vec<DocTab>`，每 Tab 持 `MapDoc + EditSession + CanvasView`，再补 `--new`/`--save-as` 与页签交互。
5. **P0-11 菜单下拉**。`menubar.rs` 已有 5 个命中矩形与文案，加 `Popup` + 菜单项表（照 `MainWindow.axaml:5` 抄）即可；**这是把已实现功能"变可见"的性价比最高项**。
6. **P0-10 + P1-13 印章 + 分类树（一起做）**。`palette.json` 是索引、`.X` 是内容，两者必须同批；先做"读 + 列表 + 放置"，再做 `Identify` 与写回。
7. **P0-8 自动图块**。优先级高但成本最高（4 族 × 15 形状 × 顶点掩码 + 断连修补）。建议先做 Mir2 50 帧（覆盖 `Tiles`/`Tiles5` 主库），Mir3 系列后置。
8. **P1-5/6/7 光照与灯门高亮**。`PreviewLights` 三张源图**已在树内可用**：`CrystalStudio\PreviewLights\445.png`（66.7 KB）/ `685.png`（127.3 KB）/ `925.png`（198.5 KB），C# 以 `EmbeddedResource` 打包（`CrystalStudio.csproj`），代码取 `intensity = rgba.r` 做加法（`MapPreview.cs:77-80`）——Rust 只需按同名路径读 PNG 并取 R 通道，无需自造光贴图。
9. **P1-9/10/11 小地图与总览**。三者共用一套"低分辨率概览位图"：先做概览位图缓存的生成（可复用 `miredit-render` 的 `render_region` + 隔格采样），再叠视口框与导出。
10. **图库/数据目录设置（§三 域11 的 P0 项）**。把 `DEFAULT_CLIENT_ROOT` 改成"配置文件 → 环境变量 → 默认常量"三级回落，配一个最简单的目录输入框；否则任何非本机环境都跑不起来。

> 不建议先做的：P2-7 图案组预览、P2-4 `.X` 文档编辑器、P2-2 布局锁定 —— 都依附于前 7 条。

---

## 七、审计口径与局限

### 7.1 否定结论的检索口径（逐条可复现）

**Rust 侧**：目录 `E:\GodM2\tools\miredit\crates\`（递归 `*.rs`，33 文件），`Select-String -AllMatches` 全文匹配。

| 判定为"无"的能力 | 检索词 | 命中数 |
|---|---|---|
| 分类目录 / 数据资产 | `catalogue` `palette.json` `terrain.json` `walls.json` `towns.json` `water.json` `river-artwork` `province-decoration` | **全部 0** |
| 自动图块 | `AutoTile` `autotile` | **0** |
| 墙 / 树 / 特殊笔刷 | `WallBrush`（0）· `\bwall\b`（6，全在 `icons.rs` 图标表）· `\btree\b`（1，`cargo tree` 注释）| 无实现 |
| 填充 / 剪贴板 | `flood`（0）· `clipboard` `copy_selection` `cut_selection` `RegionCopy` `paste` `Paste`（全 0）· `\bbucket\b`（5，全在 `icons.rs`） | 无实现 |
| 生成器 | `town` `Town` `temple` `Temple` `province` `Province` `river` `River` `generator` `Generator` | **全部 0** |
| 印章 / `.X` | `LoadStamp` `StampCell` `load_stamp` | **0**；`stamp`/`Stamp` 5 处全为图标 |
| 小地图导航 / 概览 / 导出 | `overview` `Overview`（0）· `export` `Export`（0）· `minimap`（29，全为"画一块假方块"的几何常量） | 无实现 |
| 光照 / 动画 / 选择 | `Lighting` `lighting`（0）· `\banimation\b`（1，文案）· `Selection`（1，`layout.rs:80` 注释） | 无实现 |
| 设置 / 浮动窗 | `settings` `Settings`（0）· `float` `Float`（0） | 无实现 |
| 笔刷尺寸 | `BrushSize` `brush size` | **0** |

**C# 侧**：目录 `E:\GodM2\CrystalStudio\CrystalStudio\`（递归 `*.cs`，**排除 `obj\`**），共 158 个业务 `.cs`。

| 判定为"无"的能力 | 检索词 | 命中 |
|---|---|---|
| 裁剪地图 / 改尺寸 | `CutMap` `TrimMap` `CropMap` `ResizeMap` `SetSize` | **0**（`\bResize\b\|\bCrop\b\|\bTrim\b` 40 处均为 `string.Trim()`、面板尺寸、`CropStamp`、地形裁剪等无关项） |
| 快捷键自定义 | 无配置文件、无设置界面；快捷键只在 `MainWindow.axaml.cs:832-869` 与 `.axaml` 的 `InputGesture` | 不存在 |

### 7.2 本次审计**读到**的东西

- C#：通读 `MainWindow.axaml(.cs)`、`Document.cs`、`MapViewport.cs` 及其 10 个 partial、`AutoTileBrush.cs`、`RegionCopy.cs`、`LegacyMapReader.cs`（格式判据 + 8 个 `LoadMapTypeN` 的关键段）、`StampIdentifier.cs`、`StampCategories.cs`、`CategoryPalette.cs`、`AssetPalette.cs`、`AssetHistoryStore.cs`、`DockWorkspace.cs`（前 120 行）、`MinimapExporter.cs`、`MapMinimap.Avalonia.cs`、`MapViewport.Presentation.cs`、`MapViewport.Overview.cs`（前 60 行）、`MapPreview.cs`、`CollisionMarkers.cs`、`TreeBrush.cs`、`SpecialBrushKind.cs`、`TreeBrushList.cs`、`SpecialBrushList.cs`、`SettingsStore.cs`、`SettingsWindow.cs`、`RecentMapStore.cs`、`MainWindow.RecentMaps.cs`、`MainWindow.MinimapExport.cs`、`MainWindow.StampEditing.cs`、`MainWindow.MapTabs.cs`、`ThemeService.Avalonia.cs`、`EditorPaths.cs`、`DataPaths.cs`、`AtomicFile.cs`、`StampCatalog.cs`、`StampBrowser.cs`、`WallBrush.cs`（前 200 行 + 符号表）、四个生成器的文件头（`Town/Terrain/Province/Temple`）、`CrystalStudio.csproj`、`CrystalStudio.Tests/MigrationTests.cs`（前 60 行 + 测试名列表）。
- Rust：**通读** `miredit-gui` 的 `lib.rs`、`app.rs`、`edit.rs`、`cli.rs`、`mapdoc.rs`、`save.rs`（前 80 行）、`menubar.rs`、`dock_right.rs`、`palette.rs`（前 130 行 + 函数表）、`layout.rs`（前 90 行）、`canvas.rs`（前 80 行）、`canvas_view.rs`（前 120 行 + 关键符号）；**读头部/文档**：`icons.rs`（TOOLS 表段）、`metrics.rs`（画布常量段）、`selfcheck.rs`、`editrun.rs`、`ui_text.rs`（菜单段）、`miredit-render/lib.rs`（前 120 行）、`mir2-map/map.rs`（前 100 行）、`mir2-assets/lib.rs`（`DEFAULT_CLIENT_ROOT`）、`mir2-assets/lib_paths.rs`（符号表）。
- 数据：`catalogue` 目录清单与体量、`palette.json` 头 700 字节 + 条目计数（`"id":` → **9,159**）、`walls.json` 索引全文（19 族）、`river-coast-layouts` 清单、`Objects` 递归计数（**9,170**）。

### 7.3 **没读到 / 未验证的**（如实登记）

1. **`WallBrush.cs` 只读了前 200 行（共 944 行）** + 符号表。`Compose`（`:184-333`）、`NativeTrims`、`SpriteFootprints`、`PlanStroke`（`:508-748`）、`ApplyWallPlan`（`:962-…`）的**算法细节未逐行读**，只按符号与注释判"存在且体量大"。若需精确复刻墙体算法，须另开一份逐行取证。
2. **`River*` 4,832 行只读了 3 个文件的少量片段**（`RiverBridgeComponent.cs:73,133`、`RiverCoastPieces.cs:137-139`、`RiverForkComponents.cs:13` 等目录加载点）。**河流生成算法本身未读**，本报告的"×"仅表示"Rust 侧没有任何对应代码"，不代表我已理解 C# 算法。
3. **`Province*` 1,530 行只读 `ProvinceGenerator.cs` 前 60 行**；`ProvinceTowns.cs:403`、`ProvinceCliffTerrain.cs:257` 未读。
4. **`Terrain*` 2,359 行只读 `TerrainGenerator.cs` 前 60 行 + `TerrainCatalogue.cs:18,39`**；`TerrainWaterGenerator.cs:327`、`TerrainCoastObjects.cs:399`、`TerrainRiverGenerator.cs:179` 未读。
5. **`TempleGenerator.cs` 只读前 60 行**（341 行中约 281 行未读：房间布局、走廊、边房 `ZumaTempleSideRooms.cs:113`、`TempleFloorCoverage.cs:111`、`StoneTempleSpecialWalls.cs:54`）。
6. **`DockWorkspace.cs` 只读前 120 行（共 327 行）**：`:121-327` 的 `DropTarget`/`Rebuild`/`Save`/`Restore`/`Float` 细节未读，但类结构与 `workspace.json` 持久化已确认。
7. **`MapViewport` 的 10 个 partial 未全读**：`DetailScene.cs`、`Minimap.cs`、`PickupHints.cs`、`Province.cs`、`StampRendering.cs`、`Temple.cs`、`Terrain.cs`、`Town.cs`、`TreeBrush.cs`、`Wall.cs` 未逐行读（仅通过 grep 与调用点确认能力存在）。
8. **`LegacyMapReader.cs` 的 8 个 `LoadMapTypeN` 只读了 Type0/Type1(部分)/Type3/Type4 的头部**；Type2/5/6/7/100 只读了 `ValidateDimensions` 的行号与判据，字段布局未逐字核对。因此 §P1-19 里"格长 12/14/15/20/26/33/35/36/38"是从 `ValidateDimensions(header, cell)` 的参数读出的**最小格长**，不等于精确布局。
9. **未运行任何代码**：本次是静态审计，**`cargo test` / `cargo build` 未执行**，"197 条测试"是 `#[test]` 属性计数（`Select-String '^\s*#\[(test|test_case)\]'`），**不是实测通过数**；C# 的"15 条测试"同理是方法计数。Rust 侧真实通过数与警告集合**未验证**。
10. **`miredit-render` 的 `compose.rs`/`frames.rs`/`canvas.rs` 未读**：因此"渲染口径与客户端一致"这一条**我没有独立核实**，只是引用其文档声明（`lib.rs:25-41` 的对照表）。若要判"渲染是否保真"，须另开渲染审计。
11. **`palette.json` 的 9,159 条只验证了条数与头 1 条**；字段完整性、`regions`/`environments` 取值域、`reviewStatus` 分布**未统计**。
12. **`catalogue/previews/` 目录不存在**这一点已确认（`Test-Path` = False），但**未追查 C# 是否有别处生成它**（`preview` 字段可能是离线流水线产物）；只确认运行期 C# 走合成预览、不读该目录。
13. **未评估 C# 侧代码质量/缺陷**：本报告只做"有没有"，不做"对不对"。C# 里可能存在与客户端不一致的实现（例如 `LoadMapType3` 编造库号 `+100/+110/+120`），我按"原工程行为"记录，未判其正确性。
14. **工具权限**：本会话为只读审计；除本报告外未创建/修改/删除任何文件。`..\obj\` 下的 3 个自动生成 `.cs` 已从统计与检索中排除。

---

## 八、附录：125 项逐项判定表（§一 三个数字的底账）

> 口径：每行一个可比对的功能点；**有** = Rust 侧有等价实现并可落到代码；**部分** = 有路径但能力显著不完整；**无** = Rust 侧不存在（检索口径见 §7.1）。
> 「两边皆无」的三项（4.9 / 11.5 / P2 类）单列，不计入"移植缺口"，但在总数里如实占位。

**域 1 地图文件（9 项：有 3 / 部分 1 / 无 5）**

| # | 功能点 | 状态 |
|---|---|---|
| 1.1 | 打开 `.map`（含命令行传路径） | 有 |
| 1.2 | 打开对话框（多选、文件过滤） | 无 |
| 1.3 | 新建地图（尺寸/预设/16–4096 校验） | 无 |
| 1.4 | 多文档 Tab | 无 |
| 1.5 | 另存为 | 无 |
| 1.6 | 保存（原地、原子替换） | 有 |
| 1.7 | 最近打开（20 条 + 失效置灰） | 无 |
| 1.8 | 关闭前拦截 | 部分（单页脏检查有；无"保存/不保存/取消"三选，无逐页遍历） |
| 1.9 | 窗口标题/页签随文档与脏标记变化 | 有 |

**域 2 撤销重做（6 项：有 4 / 无 2）**

| # | 功能点 | 状态 |
|---|---|---|
| 2.1 | 一次拖动 = 一步（笔画合并，同格只记首次） | 有 |
| 2.2 | 栈上限 100（丢最老） | 有 |
| 2.3 | 保存点关联（撤销回保存点自动清脏） | 有 |
| 2.4 | 撤销/重做逐字节还原 | 有（`edit.rs:1022-1048` 断言） |
| 2.5 | 每文档独立撤销栈 | 无（单文档，不存在该结构） |
| 2.6 | 印章快照一起进撤销 | 无 |

**域 3 笔刷系统（14 项：有 1 / 部分 2 / 无 11）**

| # | 功能点 | 状态 |
|---|---|---|
| 3.1 | 铅笔单帧落格 | 部分（仅地表层） |
| 3.2 | 画笔写中层 / 物件层 | 无 |
| 3.3 | Mir2 50 帧自动图块 | 无 |
| 3.4 | Mir3 30 帧自动图块（含 B 段 +5 偏移） | 无 |
| 3.5 | Mir3Mini 5 帧 | 无 |
| 3.6 | SmTile 60 帧 | 无 |
| 3.7 | 顶点掩码 → 15 形状 + 断连对角修补 | 无 |
| 3.8 | 笔刷尺寸 3..31 + 地面 2×2 对齐 | 无 |
| 3.9 | 填充 Bucket（连通、按层、步长） | 无 |
| 3.10 | 墙笔刷（19 族） | 无 |
| 3.11 | 树/灌木/岩石笔刷 | 无 |
| 3.12 | 落格半透明预览 | 无 |
| 3.13 | 橡皮（保留 `0x8000` 阻挡位） | 有 |
| 3.14 | 吸管 | 部分（只取地表；C# 还能吸取印章实例） |

**域 4 选择与剪贴板（9 项：无 9）**

| # | 功能点 | 状态 |
|---|---|---|
| 4.1 | 框选（夹边、反选取消） | 无 |
| 4.2 | 复制（按可见层快照） | 无 |
| 4.3 | 剪切 | 无 |
| 4.4 | 粘贴（锚点对齐/越界拒绝/预览着色/可重复） | 无 |
| 4.5 | 区域填充 Fill Selection | 无 |
| 4.6 | 按层清除选区 | 无 |
| 4.7 | 单元格/选区属性（Light/Door/Offset） | 无 |
| 4.8 | 右键上下文菜单（Ctrl+C/X/V） | 无 |
| 4.9 | 裁剪地图 CutMap | **两边皆无**（不计为移植缺口） |

**域 5 印章 .X（10 项：无 10）**

| # | 功能点 | 状态 |
|---|---|---|
| 5.1 | `.X` 浏览（递归索引 9,170 + 搜索 + 分页） | 无 |
| 5.2 | `.X` 载入并放置 | 无 |
| 5.3 | 导出选区（偶数对齐裁剪） | 无 |
| 5.4 | 印章拾取/移动/克隆/删除 | 无 |
| 5.5 | 印章元数据随地图存取 | 无 |
| 5.6 | Identify Objects 模板匹配 | 无 |
| 5.7 | 分类索引写回（`palette.json`） | 无 |
| 5.8 | 新建印章文档（Ctrl+Shift+N） | 无 |
| 5.9 | `.X` v0/v1/v2 兼容读取 | 无 |
| 5.10 | 印章预览合成 | 无 |

**域 6 素材与分类（12 项：有 6 / 无 6）**

| # | 功能点 | 状态 |
|---|---|---|
| 6.1 | 图库枚举（真实 `.wil/.wzl`） | 有 |
| 6.2 | 缩略图按需解码 + LRU 缓存 + 负缓存 | 有 |
| 6.3 | 虚拟滚动（只解可见行） | 有 |
| 6.4 | 图号搜索/跳转 | 有 |
| 6.5 | 翻页 | 有 |
| 6.6 | 库下拉（真实文件名 + 族名） | 有 |
| 6.7 | 收藏（★/☆ + 持久化） | 无 |
| 6.8 | 最近使用（48 条 + 持久化） | 无 |
| 6.9 | 分类树（`palette.json` 9,159 条） | 无 |
| 6.10 | Type/Region/Environment/Facing 四维分组 | 无 |
| 6.11 | 图案组聚合显示（50 帧块 → 一套） | 无 |
| 6.12 | 按层自动选库 | 无 |

**域 7 图层与显示（13 项：有 3 / 无 10）**

| # | 功能点 | 状态 |
|---|---|---|
| 7.1 | 三层可见性开关 | 有 |
| 7.2 | 低/高碰撞层可见性 | 无 |
| 7.3 | 碰撞层画/擦/填充 | 无 |
| 7.4 | 碰撞标记绘制（粉/蓝/双色） | 无 |
| 7.5 | 灯格高亮 | 无 |
| 7.6 | 门格高亮 | 无 |
| 7.7 | 光照预览 3 档 | 无 |
| 7.8 | PreviewLights 加法叠加 | 无 |
| 7.9 | 门开/关预览 | 无 |
| 7.10 | 动画预览（100 ms 推进帧） | 无 |
| 7.11 | 网格（G） | 无（刻意撤掉） |
| 7.12 | 悬停格高亮 | 有 |
| 7.13 | 单元格检查器（只读六行） | 有 |

**域 8 视图（10 项：有 4 / 部分 2 / 无 4）**

| # | 功能点 | 状态 |
|---|---|---|
| 8.1 | 缩放（滚轮/按钮/以鼠标为中心） | 有 |
| 8.2 | 缩放范围约束 | 部分（C# 夹在 `FitZoom..100%`；Rust 固定 0.25..4.0，可放大到 400%） |
| 8.3 | 平移（中键 / 空格+左键） | 有 |
| 8.4 | Fit Map | 有 |
| 8.5 | 小地图真实渲染 | 无（假图） |
| 8.6 | 小地图导航（点击/拖拽/滚轮） | 无 |
| 8.7 | 概览 mipmap（<25%） | 无 |
| 8.8 | 小地图导出 PNG | 无 |
| 8.9 | 渲染缓存/批量 | 有（不同做法：整块纹理缓存 + 变化判定） |
| 8.10 | 渲染统计上界面 | 部分（stderr + `--autodrag` 报告，不占界面） |

**域 9 停靠与布局（8 项：有 1 / 无 7）**

| # | 功能点 | 状态 |
|---|---|---|
| 9.1 | 面板拖拽停靠（4 边 + 落点预览） | 无 |
| 9.2 | 浮动窗口（双击/右键） | 无 |
| 9.3 | 面板显隐菜单 | 无 |
| 9.4 | 布局持久化 `workspace.json` | 无 |
| 9.5 | 重置布局 | 无 |
| 9.6 | 锁定布局 | 无 |
| 9.7 | 响应式面板 | 无 |
| 9.8 | 固定几何常量布局 | 有 |

**域 10 生成器（9 项：无 9）**

| # | 功能点 | 状态 |
|---|---|---|
| 10.1 | 城镇生成（991 行） | 无 |
| 10.2 | 地形生成（2,359 行） | 无 |
| 10.3 | 省生成（1,530 行） | 无 |
| 10.4 | 河流生成（4,832 行） | 无 |
| 10.5 | 神庙/地牢生成（341 行 + 附属） | 无 |
| 10.6 | 墙围合（作为城镇构件，1,537 行） | 无 |
| 10.7 | 6 个生成器设置窗口（`.axaml`） | 无 |
| 10.8 | 生成参数校验与预览 | 无 |
| 10.9 | 生成结果一次性进撤销 | 无 |

**域 11 设置（6 项：无 6）**

| # | 功能点 | 状态 |
|---|---|---|
| 11.1 | 数据目录选择 | 无 |
| 11.2 | `settings.json` 持久化 | 无 |
| 11.3 | 主题切换（深色/跟随系统） | 无 |
| 11.4 | `appearance.json` 持久化 | 无 |
| 11.5 | 快捷键自定义 | **两边皆无** |
| 11.6 | Profile 目录重定向（`CRYSTALSTUDIO_PROFILE`） | 无 |

**域 12 菜单/工具栏/快捷键（11 项：部分 3 / 无 8）**

| # | 功能点 | 状态 |
|---|---|---|
| 12.1 | 5 个菜单可下拉 | 无 |
| 12.2 | File 13 项 | 无 |
| 12.3 | Edit 5 项 | 无 |
| 12.4 | View 9+ 项（含面板分节、缩放、光照、门） | 无 |
| 12.5 | Tools 6 项（4 生成器 + Identify） | 无 |
| 12.6 | Help 1 项 | 无 |
| 12.7 | 工具按钮组（11 个） | 部分（10 个按钮，3 个可落格） |
| 12.8 | 工具快捷键 B/P/F/E/M/O/V/K/T/H | 无 |
| 12.9 | Ctrl 组合（N/O/S/Shift+S/Z/Y/Shift+Z/C/X/V/Shift+N） | 部分（仅 Z/Y/S） |
| 12.10 | 其他键（G/C/F1/`[`/`]`/WASD/←→/Delete/Esc） | 部分（仅 Esc） |
| 12.11 | F1 快捷键表对话框 | 无 |

**域 13 数据资产（8 项：有 1 / 无 7）**

| # | 功能点 | 状态 |
|---|---|---|
| 13.1 | `palette.json`（9,159 条分类索引） | 无 |
| 13.2 | `terrain.json` + `water.json` | 无 |
| 13.3 | `towns.json` | 无 |
| 13.4 | `walls.json` + 19 族（58.9 MB） | 无 |
| 13.5 | `river-artwork.json` + `river-coast-layouts/**` | 无 |
| 13.6 | `island/catalogue.json` + `province-decoration-ground.json` | 无 |
| 13.7 | 9,170 个 `.X` | 无 |
| 13.8 | 客户端 WIL/WZL 图库 | 有 |

**合计（可复核）**

| 状态 | 域1 | 域2 | 域3 | 域4 | 域5 | 域6 | 域7 | 域8 | 域9 | 域10 | 域11 | 域12 | 域13 | 合计 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 有 | 3 | 4 | 1 | 0 | 0 | 6 | 3 | 4 | 1 | 0 | 0 | 0 | 1 | **23** |
| 部分 | 1 | 0 | 2 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 3 | 0 | **8** |
| 无 | 5 | 2 | 11 | 9 | 10 | 6 | 10 | 4 | 7 | 9 | 6 | 8 | 7 | **94** |
| 小计 | 9 | 6 | 14 | 9 | 10 | 12 | 13 | 10 | 8 | 9 | 6 | 11 | 8 | **125** |

⇒ **23 + 8 + 94 = 125**；有 = 18.4%，部分 = 6.4%，无 = 75.2%。

---

*报告路径：`E:\GodM2\deliverables\审计-CrystalStudio移植-功能对照-2026-11-04.md`｜生成于 2026-11-04｜只读审计，未改动任何源码*
