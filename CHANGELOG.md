# 更新日志

本文件记录 **Crystal Studio**（仓库 `CrystalStudio-Rust`）每个对外版本改了什么，方便判断「要不要更新」。

- 格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)；版本号的**形态、递增与发布流程**见 [`VERSIONING.md`](VERSIONING.md)。
- **2026-11-07 版本号方案变更**：版本号改用 `<YYYYMMDD>.<当天轮次>.<轮内修正>`（例 `20261107.0.0`），不再使用 `0.x.y`。旧版本/tag/小节**原样保留不清理**。`[Unreleased]` 只积累尚未发布的改动：下一版号到发布那一刻才定，本文件不预写版本号与日期。
- 小节名固定为：`不兼容变更` / `新增` / `变更` / `弃用` / `移除` / `修复` / `安全` / `已知缺口` / `内部`。没有内容的小节直接省略，不留空壳。
- 日期只有两种状态：真实发布日期，或条目仍在 `[Unreleased]` 下（无日期）。**不写占位日期**。
- 开发期的改动先记进 `[Unreleased]`；发布时整体搬到版本标题下（步骤见 `VERSIONING.md` §六）。
- 版本之间的 `compare` 链接等打过 tag 后再补：本文件不提前写会 404 的链接。

## [Unreleased]

> 暂无条目。新改动先记在这里，发布时整体搬进下一个版本标题。

## [2026.11.8+a] - 2026-11-08

### 修复

- **Type5（`Mir3Wemade`）13 字节/格变体给出定向诊断**（`crates/mir2-format/src/formats/mir3_wemade.rs:233-281` + `registry.rs:311`）：
  用户提供的 `G:\WeMade Entertainment\Map` **992 个真实文件**里 937 个 14 B/格**完美往返**（`out == raw`，0 字节差异），
  另 55 个是 **13 B/格变体**，此前只报泛化的 `UnknownFormat`。现在按「形似 Type5 + W/H 为正 + 长度短于 14 B/格公式」三条合取给出五要素诊断
  （形似哪个格式 / W×H / 实际长度 vs 公式长度 / 差值**恰为 W×H×1** / **上游 `LoadMapType5` 亦拒绝**，`LegacyMapReader.cs:49-56` + `:396` 的 `minimumCellBytes=14`）。
  **不实现 13 B/格**：格内布局零证据（上游全族只有 14 B/格；格式规格、`T_MapInfo` 均无 13 B 形态），照猜就是编造。
- **测试稳定性：4 处「墙钟等待」改成事件驱动**（`crates/miredit-gui/src/app.rs`、`minimap_job.rs`、`stamp_panel_tests.rs`）：
  原来 `while … && t0.elapsed() < Duration::from_secs(5|30)` 用**墙钟**当判据，`cargo test --workspace` 多 binary 并行、CPU 争抢时会假红；
  其中 `stamp_panel_tests.rs` 的那处**失败模式不是红而是 `[跳过]`**（静默丢覆盖，更隐蔽）。
  现在判据换成状态机（`is_loading()` / `pending()` / `poll()` 返回 `true`），上限取**迭代次数**（只兜真挂死 + 给诊断，不是判据）。
  **反假绿已证**：8 路并发下旧代码目标用例 **8/8 必红**、新代码 **8/8 全绿**；`stamp_panel_tests` 把上限置 0 时 6 个真实数据用例**全部 panic 而不是跳过**。
  机理：`StampCatalog::load` 空闲中位 302 ms（5 s = 16.6× 余量），8 路并发下中位 1784 ms（**5.9× 膨胀**）⇒ 墙钟被吃穿。

### 变更

- **版本号方案切换**：`<YYYYMMDD>.<轮次>.<修正>` → **`<年>.<月>.<日>` + 同日字母**。
  `Cargo.toml` / `Cargo.lock` / `CHANGELOG.md` 用 **`2026.11.8+a`**（`+a` 是 build metadata，`a` = 当天第 1 个版本、`b` = 第 2 个…）；
  **git tag 用 `v2026.11.08a`**（tag 不受 SemVer 约束）。**`2026.11.08a` 不能直接写进 `Cargo.toml`** ——
  cargo 报 `invalid leading zero in patch version number`（`08` 的前导零先被拦，**不是字母错**）。
  `crates/miredit-gui/build.rs` 的 `version_quad` 新增 `letter_ordinal`：`+a` ⇒ 第四段 `1`、`+b` ⇒ `2`、…、`+z` ⇒ `26`（旧 8 位日期与 `0.x.y` 行为 100% 保留）。
  `scripts/bump-version.ps1` 新增 `-Part same-day|next-day`（旧名 `patch`/`minor`/`major` 保留为**会打印映射的别名**），
  `-Check` 判据改为「日期段不得倒退 + 同日字母不得重复/倒退」（build metadata 不参与 SemVer 比较，旧「三段严格递增」失效）。
  规则全文见 `VERSIONING.md` §二（2.0–2.6，历史方案整表保留）。

### 新增

- **发布方案 `RELEASE.md`**（8 章）：`VERSIONING.md` 管版本号规则，本文件管**怎么把一次发布做完** ——
  G1–G5 门禁（命令 + 判据）、**10 步**发布流程（每步命令 + 判据 + 失败处置）、产物清单、回滚四类、凭据纪律、Release Ledger、7 条不接受例外的纪律。
- **部署指南 `DEPLOY.md`**（225 行）：四条路径（源码构建 / Release 预编译 / Docker 编排 / NAS 飞牛 fnOS），每节「前提 / 步骤 / 验证 / 已知限制」；
  末尾 8 行状态表（4 已解决 + **4 未验证**）。
- **README 顶部在线预览**：`http://dayz.dpdns.org/`（网页版部署在飞牛 NAS 上，经 Cloudflare 反代；**该实例未挂素材卷**，所以地图能开但画布不出精灵，`/api/tiles` 如实 503 不伪造像素）。

### 内部

- **引用体系重构（16 笔，152 文件次，+2609/−2591）**：R1 补全 2 处文件名并登记 15 处消息串残留；R2/R5/R6-a 处理 `layout` 裸引用（219 条实测 98 条坏、改 97 条；151 处变步长重锚，起点坏 14→0）；R3 对 `docs/地图格式规格.md` 537 处按符号锚重锚（改 472 / 保持 65），并揭穿「两个批次混装」；R4 与收尾批去 `@acfbbf8a`（gui 14→0、core+render 66→7）、去 `@901dbe2f`（3→0）、单前缀按真实根归一；其余按 crate 逐批（`metrics.rs`/`theme.rs`、gui 39 文件、render 13 文件、core 15 文件、format/map、assets、stamp、`Document.cs` 印章段）去 `@sha` + 补全文件名 + 修坐标。
- **引用订正（7 笔，20 文件次，+78/−54）**：D6 8 处（含 2 处基准文件判错）、d7 6 处行号（通读另报 11 处同类，只报不改）、d9 selection 12 处 + theme 1 处、d10 stamp 闸门 + edit 路径 3 处（含 1 处指向不存在的文件）、d11 12 处既存漂移清零 + 生产真值并列（15+/15−）；R6-b/c/d 3 处（含 2 处有意订正的 assert 消息），`lighting.rs` 一处引用加「上游 wil2atlas 的」限定并去掉不可复核 sha。
- **地图格式规格旧口径订正（5 笔，`docs/地图格式规格.md`，+200/−81）**：§二「17 字节」两处（`+22/−0`）、四块「17/40 字节」旧口径（`+73/−0`）各加显式订正块、原文零删除；「40 口径」8 处 + 12 簇同缺陷面残留（`+43/−33`）；§十二「15 主档 + 17 备档双实现」两处（`+34/−2`）；另 4 块旧口径改方案 B。
- **Docker 编排首次实跑验证（2026-11-08，飞牛 NAS）**：`docker build -f docker/Dockerfile --target runtime` 成功（Rust 编译 3m58s + runtime `apt-get` 约 8 分钟）；`docker run` 后容器 `Up (healthy)`、`Restarts=0`、`/api/health` = `{"ok":true,"version":"20261107.0.0"}`、根路径 `/` = HTTP 200（29163 B HTML）、`/css/app.css` 与 `/js/ui.js` 均 200；无素材时 `/api/tiles`、`/api/stamps` 如实 503（不伪造像素）。镜像 `crystalstudio/miredit:20261107.0.0`（134 MB），已导出 tar 归档；**未推任何 registry**。
  仍未验证：`front`（nginx）层、素材卷挂载、`/api/save` 写盘。逐项状态见 `docker/README.md` §7。
- **注释订正（1 笔，2 文件，+6/−6）**：`crates/miredit-gui/src/canvas.rs`、`crates/miredit-gui/src/metrics.rs` 的「离线不可复核」改为「可复核（给路径）」。
- **台账 §8.104：Type5 真实语料入库（`G:\WeMade Entertainment`，992 文件）**：本仓 `FormatId::Mir3Wemade` 即上游 `LoadMapType5`（`crates/mir2-format/src/formats/mir3_wemade.rs` 的 `HEADER_LEN=28` / `CELL_LEN=14` / `TAIL_LEN=4` / `BACK_GROUP_LEN=3`；`crates/mir2-format/src/model.rs:176` 映射），无需新增格式即可读该语料。
- **台账 §8.105：Type5 语料实测 937/992 完美往返**：937 个 14 B/格文件 `read→write` 逐字节相等、0 字节差异；55 个 13 B/格变体当时判 `UnknownFormat`（`crates/mir2-format/tests/real_samples.rs:235` 报 `left: 55 right: 0`）。`Data` 下 174 个库基名 0 个命中 `Tiles/SmTiles/Objects` 三族规则 ⇒ 图可读入内存但画布零精灵，属已知未覆盖（`crates/miredit-core/src/map_check.rs:44`、`crates/mir2-assets/src/lib.rs:102` 已登记）。

## [20261107.0.0] - 2026-11-07

### 不兼容变更

- **版本号方案**：从 `0.x.y`（SemVer 递增）改为 `<YYYYMMDD>.<轮次>.<修正>`（例 `20261107.0.0`）。旧 tag / 旧 CHANGELOG 小节**原样保留**，但 `git checkout v0.7.0` 仍是旧版；要装新版请用 `v20261107.0.0`。规则全文见 `VERSIONING.md` §二。

### 新增

- **F-R13-1 窗内小地图（整链打通）**：
  - 渲染层 `render_overview`：level 4 总览、64 MiB 预算护栏（超则抬档、仍超返回中文原因不分配）、per-sprite 预缩；T2 逐字节等价门 + 真语料 555.8 MiB 差分同哈希。
  - A 段：`minimap_world_at`（上游 `:70-75` 逐字）、地图区 fit 进 `block.shrink(6)`（`:14-23`）、`center_on_world` 带 clamp。
  - B 段：`Assets.src` 改 `Arc<WilSource>`、后台渲染 + `(map_gen, 尺寸, level, 目标px, 图层, 选项)` 代次缓存、纹理绘制；截图前有界等待落定，保住 `--shot` 确定性。
  - C 段：点击居中（grab 捕获语义）/ 拖拽 / 滚轮 ×1.15 绕画布中心；视口框按上游 `:61-68` 三层绘制（`fill α35` → 黑 4 → 白 2 + 地图区 1px 外框），逐像素剖面实证黑在外、白在内。

### 修复

- **A-001 锚点例外（跟 C#）**：按上游 `MapSpriteDrawing.cs:24/53-55` 补 `tile` 判定（48×32 或 96×64 不锚定）、`anchored && (!tile || blend)` 底边锚定、`front && library==28` 只叠锚点；地表仍不锚定（`anchored:false`）。真实语料 61 张 / 26,767 帧中仅 **4 格**真正变位（表层 96×64 的 3 格、中层非 tile 的 1 格），其余逐像素不变；A-018 加性分支一行未动（有逐字节回归）。
- 已知缺口：`MiddleBlend` 判据已实现但当前不可达——`Cell` 无「中层动画帧」字节，渲染传缺省 0；36B 档打通需给 `Cell` 加字段（另开单）。
- **`--shot` 旧口径订正**：0.3.0 记「同构建两次 430 px」**未复现**（口径不明）；2026-11-07 实测同构建同命令差 **2,166 px**，根因是截图帧的「当前格」跟随**操作系统光标**（`update_hover` 不检查 `input_locked`，且窗口无固定落点）⇒ 金框 304 + 检查器 1,354–1,372 + 状态栏 464–490。已修：`--shot` 下 hover 锁 `None`（除非显式 `--hover`）、窗口钉 `(0,0)`、等异步任务落定、自动化模式不上屏耗时；新增 `scripts/shot-determinism.ps1`，多组 5/5 逐字节相同。
- **交互判据修正**：小地图可见区改**像素精确**世界矩形（上游 `VisibleWorldBounds`），不再用 `visible_cells` 格量化（边缘 1 格判定相反）；「框内不跳」用例改为非中心点并做变异自证；补拖拽/区外/右键阴性用例，修全量顺序依赖。
- **注释精度**：上游滚轮无显式 `Contains`（Avalonia 事件路由隐含），本仓用显式判定模拟，已在代码注释写明。

### 内部

- `build.rs` 数值版本避免 u16 溢出：日期拆 `[年, 月日, 轮次, 修正]`，字符串版本仍是完整日期版本。
- `bump-version.ps1` 改为日期版本校验/递增，兼容旧 `0.x.y` 仅作切换起点（旧格式不许再 `-Part` 递增）。

## [0.7.0] - 2026-11-07

### 新增

- **A-015 工具栏裁剪按钮**：Tools 面板新增独立 `crop_button`（32×32，row2-col2；不扩 10 个工具槽/键位/47 项菜单）；`paint_crop_icon` 线段四角框；有图且有选区时可用，禁用提示「先框选」；点击复用 `Action::CropSelection`（crop_to/adopt_resized/fit/回执/undo 同菜单）。

## [0.6.1] - 2026-11-07

### 修复

- 接线工具条悬停提示：`icons::tip_of` 唯一取数口，工具按钮 `on_hover_text`；P/K 明确显示「未接线」，已接线工具不显示；面板隐藏时无热区/无气泡。真实气泡无头不可断言，只验证文案/热区/响应。

## [0.6.0] - 2026-11-07

### 新增

- **R9-F2 键位补齐**：`B/P/E/V/K` 切工具（P/K 为 Unwired 槽，只切槽并显式标未接线）、`C` 同切低/高阻挡、`WASD` 按屏幕 48px 平移；Ctrl+C/V/S 不被裸键吃掉；浮层新增三行并扩到 820px。
- **F-R13-2 定位到格**：视图菜单「定位到格…」（`view.locate_cell`，View 21 项/总 47）；X/Y 模态默认选区中心/hover/(0,0)，复用 `locate_cell`；成功 `scale=max(scale,0.5)`、格中心对画布中心、1×1 选区、清待放置/笔画；不脏/不入撤销/字节不变。CheckMap Locate 共用并自动获得最小 0.5。

### 修复

- R9-F2：`pan_by_screen` 统一换算，`scale=2` 时世界 ±24、屏幕 ±48；overlay/搜索焦点时裸键让位。
- **R12-F2 换层纠正工具**：Eyedropper 只在 Ground/Middle；Wall/SpecialBrush 只在 Objects；碰撞层 + Stamp/Pickup/Clone 切 Pencil（槽 1，Unwired，状态显式「未接线」）；`--layer`/面板/`Command::SetLayer`/`--edit-layer` 四入口统一，清 `armed`/`sel_drag`。

### 已知缺口

- R9-F2：P/K 为 Pencil/Clone 的 Unwired 槽，只切槽并显示「未接线」，不产生画布动作。
- R9-F2：WASD 为**每个按键事件一步**（48 屏幕像素）、**每帧最多一步**、没有按键事件的帧不漂移（不做按帧积分）。⚠ 按住不放时**会**随系统自动重复一次一次地走 —— `i.key_pressed()` 含 egui 的 key-repeat 事件（`egui-0.33.3/src/input_state/mod.rs:814-826`），上游 WinForms `KeyDown` 同样被重复触发；若将来要求"按住只走一步"，需显式过滤 `Event::Key{repeat:true}`，属行为变更。
- R9-F2：`C` 额外写一句「阻挡标记：开/关」状态栏回执 —— 上游只改两个 `CheckBox`、不写回执；本仓按"状态变更必有回执"的既有纪律（同 `G` 网格、门开关），有意保留。
- 工具条 `icons::TOOLS[].tip` 当前没有渲染路径（预存问题）；Pencil/Clone 的 tip 已诚实写「未接线」，悬停提示接线待另立 P3。

## [0.5.1] - 2026-11-07

### 修复

- **D-003 另存为按钮去重**：模态标题保持「另存为」，主按钮改「保存副本」，不再与标题同文；补标题/按钮分离与真实排版宽度测试。
- **D-004 选区填充状态栏去重**：选区填充成功只保留「填充 N 格」，不再同时写「填充选区 N 格」回执；被拒/非 Fill 命令仍保留回执；键盘 Ctrl+Shift+V 与菜单同判据。
- **D-008 关于常量去重**：`MENU_ABOUT` 改为引用 `ABOUT_TITLE`，消除双字面量。

### 变更

- D-007/D-009 仅补注释：资源库数量在状态栏/面板底部属不同信息面；「资源库」页签与行标签属同词不同义，均有意保留。

## [0.5.0] - 2026-11-07

### 新增

- **A-015 Crop 裁剪到选区**：Tools 最前项；选区按 2 格偶数对齐（右/下可留奇数），只拷区内格；整图同尺寸 no-op；支持 7 档，Mir3Wemade/CrystalCs/trailer 明确拒绝；`Step::Resize` 整文档一步 undo/redo；成功后清选区、重 fit、保存按新尺寸回读。
- core `Document::replace_resized`：先同步未保存编辑、改头部宽高、搬格/空白边框、重建视图与计数；校验全在赋值前，失败不写/不压步/不脏。
- **A-015 Expand 地图扩边**：Tools → 地图扩边（440 宽模态，四向偶数输入）；`expand_target` 校验非负/偶数/非全 0/单边 32767/总面积 16,000,000；原格平移、四周空白、7 档头部宽高；整档 `Step::Resize` 一步 undo/redo；非法输入 Apply 灰显、不关窗。

## [0.4.0] - 2026-11-07

### 新增

- **A-015 CheckMap 只读地图检查**：Tools → CheckMap（Identify 前，菜单 44）；检查逐格图引用（缺库/越界/空帧/不可解码，含开门帧）、门格两警告、8 邻不连通可走区；`total` 全量、清单上限 1000；结果模态支持定位（1×1 选区 + 居中）、`map_gen` 陈旧提示、无库提示；`--check-map` 供自动化。
- core 新增 `check_map(&RawMap, &dyn LibProbe)`：只用只读 `RawMap`，不改 `Cell`/9 格式；库号按族内页号，CrystalCs/Mir3 具名槽显式 `未检查`；Mir3 可走性用 `RawCell.flag`；`mir2-assets` 新增 `frame_state` 只读帧头五态。

### 已知缺口

- CheckMap：Wemade2010（Type1）的 bk bit15 按本仓统一位判（上游漏搬，本仓更严格、不会误放行）；CrystalCs 已按原生 bit29/30 判，不在该差异内。CrystalCs 的 `has_foreground` 仍用 `objects_image != 0`，未走完整 `csharp_has_front`（零样本，P3）。Mir3 `255` 哨兵有库时报 `Missing library`、无库不报，其余具名槽未检查；`--shot` 像素门禁仍不可复现。

## [0.3.0] - 2026-11-07

### 新增

- **A-011 印章落位预览**：Alt 预览显示待放置印章的半透明内容（允许 0.55 / 受阻 0.18），按格画受阻诊断——出界琥珀、占用物体红、填充 α95、主/副对角线按上游；只画可见区；`--preview-alt` 为取证开关。core 新增只读 `check_stamp_placement`（收集全部 blocked，不改 `plan_stamp`）。

### 修复

- **A-012 视口 fit / 窗口尺寸**：`fit_map` 改用 `canvas-80`（两轴各 80px 总边距，仍 clamp 0.25..4）；窗口尺寸变化时仅当 `scale < new_fit` 才重 fit，并保持上一尺寸中心的世界点；手动放大不被覆盖，手动平移按上游不 opt-out。

### 已知缺口

- `--shot` PNG 同构建同命令不可逐字节复现（实测同构建两次 430 px 不同，跨改动 626 px，与噪声同量级）；当前用视图状态等价（scale/pan/visible_cells 一致）+ 渲染库确定性基线代替像素门禁，截图确定化另立一单。
- A-011 Type5 预览：`check_stamp_placement` 不判地表能力（无 `FormatId` 入参）⇒ Type5 图带地表印章时预览可能显示可放（0.55），点击仍由 `plan_stamp` 拒绝；本版先如实登记，后续给 check 加 `Option<FormatId>` 再对齐。
- A-015 Resize：尺寸变化可撤销也可重做（上游 `History.Resize` 会清重做）；`Mir3Wemade`（块级背景在头部）/`CrystalCs`（变长格 + 印章元数据）与 `raw.trailer` 非空一律显式拒绝；本仓无印章元数据层，placed stamps 平移不适用。

## [0.2.0] - 2026-11-07

### 新增

- **R12-F1 单元格元数据编辑**：编辑菜单「单元格属性…」（`edit.cell_properties`）；选区或悬停格上编辑 Light / Door Index / Door Offset。共同值不同 ⇒ 空框 + `Mixed — leave unchanged`，未改动字段不写（绝不抹平 Mixed）；`light=0` 与 None 分开；非法值红字、不关窗、不写任何格；整批一步撤销。core `Command::SetMetadata` + `apply_metadata` 单点，web `/api/edit` 同步；Mir3 两档 Door 框禁用（格布局无门字节）、Light 提示 0..15。

### 已知缺口

- R12-F1：Mir3 端到端字节无真实样本（仅能力拒绝 + 光照落点）；上游 Objects 图层锁/右键入口在本仓无对应物，当前入口为编辑菜单（按本仓新增记档）。
- R12-F1/Mir3 光照口径：C# 装载期按 `(raw & 0x0F) * 4` 显示 0..60，本仓存/显示原字节 0..15；零真实 Mir3 样本，待有样本时回源订正。

## [0.1.0] - 2026-11-07

### 新增

- **A-008 地砖信息悬浮（CellHover）**：视图菜单新增「地砖信息悬浮」开关（默认关、按上游不持久化）；悬停格显示坐标、可行走/不可行走（红字）与六组真实取数（阻挡/地表/中层/物体/元数据/图块动画），与左侧常驻检查器同源；浮层 +16/+8 定位、右侧越界翻左、四边夹 8。

### 修复

- **A-018 前景加性混合帧落格锚点**：按上游 C# `MapSpriteDrawing.cs:43-52@f84566b` 的 `front && blend` 分支实现——Old12 三格库分支（库号 `14/27/(99,199)`）、0-based 帧号窗口 `2723..2732`、门偏移参与窗口判断；非混合帧仍走底边锚点、输出逐字节不变。A-018b（14 字节档 202 格因视图档位推不出库号）仍留旧位置，按「推不出就不猜」登记。
- **A-024 `.X` 三族库号换算**：`.X` 的 `BackLibrary/MiddleLibrary/Front.Library` 是 C# 全局分配号（Wemade `0/1/2..99`、Shanda `100..109/110..119/120..150`），地图侧是族内页号；换算单点收敛到 `miredit-core::cells_from_stamp(stamp, 号段)`，GUI 与 Web 共用，未建模的 Sanxiu/Mir3/AniTiles 一律拒绝（不猜、不原样透传）。Old12 落位 `area=0`、New14 `sm_tiles=0` 已逐格验证。
- **A-008 P3 模态让位**：`hover_panel` 在任意 `Overlay` 打开时返回 `None`，悬停框不会从对话框边缘露出；关闭后立即恢复。

### 已知缺口

- A-018b：14B 档前景混合帧的三格库分支未实现（需把 `FormatId` 带进 `mir2_map::Map`，独立决策）。
- A-008 text_fit：悬停浮层文字未登记进 `text_fit::all_dyn_labels`，运行期靠逐行 `truncate_to_fit` 兜底（P3）。
- A-024 Sanxiu/Mir3：号段未建模，按「不在已建模号段」拒绝，不猜；支持需另设映射（Mir3 是具名库槽位表，不是减基址）。

### 内部

- **A-020 真实印章像素基线**：测试改走 `client_root()`，支持 `.lib`/`.wzl` 双根；`mir_00000` 原始/缩略图 FNV、内容框、计数整结构断言（`.lib` / WIL 两套基线），200 枚抽样记录缺帧差异；`miredit-core` 仅加入 render 的 `[dev-dependencies]`，产品依赖图不变。

## [0.0.2] - 2026-11-07

### 修复

- GUI 打开模态时不再把对话框标题写进状态栏回执：About / OpenMap / SaveAs / ExportSelection / ExportMinimap / KeyboardShortcuts 统一走 `Outcome::opened_overlay()`（`receipt: None`），修掉「标题 + 状态栏同句」重复；About 的 `ABOUT_OPEN_RECEIPT` 同值常量已删除。
- 网页版错误只保留一个持久出口：`/api/map` 读失败只显示空态正文（不再 toast 同一句）；编辑 409 `no_map` 只显示「尚未打开地图」空态；离线启动的 banner 与空态不再同为「后端未连接」，空态如实写「本页不会自动重试，需刷新」。

## [0.0.1] - 2026-11-06

首个公开可用版本 **0.0.1**：C# / Avalonia 版 Crystal Studio 的 Rust 全量移植落地，PC 原生版与网页版共用同一编辑内核。此前没有任何对外版本，所以下面就是这一版的**全部内容**（不存在「相对上一版改了什么」的比较）。

### 新增

- **9 种地图格式**的判别、解析与原格式写回：`Old12` / `Wemade2010` / `Shanda14` / `Shanda2012` / `AntiHack` / `Mir3-Wemade` / `Mir3-Shanda` / `Heroes34` / `CrystalCs`。
  - 写回走原格式；未建模的尾部/未知区字节原样保留，不猜语义。带尾块的地图按宽松口径也能打开，并如实标注未解字节数。
- **8 个单向依赖 crate**：`mir2-map` / `mir2-format` / `mir2-stamp` / `mir2-assets` / `miredit-render` / `miredit-core` / `miredit-gui` / `miredit-web`；
  WIL/WZL 解码器以 vendored 子集内置在 `vendor/wil2atlas/`（decoder-only，MIT），clone 后无需外部 path 依赖即可构建。
- **编辑能力**：图层切换、笔刷（3..31 奇数边长）、Bresenham 描线、BFS 连通填充、橡皮擦、吸管、撤销/重做、矩形选区与剪贴板、`.X` 印章放置、原格式保存。
- **渲染**：三层合成（地表/中层/物体）、四档缩放、门帧偏移（开/闭门）、三档地图光照、低/高阻挡标记。
- **PC 版**（egui 原生窗口）：中文界面、地图列表面板、网格抽稀、多种诊断与导出开关。
- **PC 版 `--version`**：输出 `Crystal Studio <版本>`（版本取 `CARGO_PKG_VERSION`，与 `/api/health` 同一个真源），`--help` 用法行同步列出；不需要开窗，纯函数有单测。
- **网页版**：axum 后端 + 原生 JS 前端（不引框架/CDN/npm），桌面与手机浏览器适配，WebSocket 推送脏标记与重绘区域；`GET /api/health` 返回进程版本。
- **导出**：选区导出、小地图（官方三档预设或指定像素）、1:1 渲染转储。
- **网页版导出小地图**（`6f55019`）：顶部导出按钮 + 模态，支持官方三档预设（small / medium / large）与自定义宽高；新增 `GET /api/export/minimap`（返回 PNG，并带 `X-Export-Width` / `X-Export-Height`）。大图走**分块渲染**后拼接（`render_tiled` / `tile_margins` 上移到 `miredit-render`，GUI 与网页共用同一份实现），并保留 512 MiB / 单边像素护栏，超限明确拒绝而不是硬算。
- **PC 版导出小地图尺寸 UI**（`560a0dd`，F-R14-1）：模态从"只填路径"扩为路径 + 尺寸面板，默认 Small；三档除数复用 `export.rs`，自定义宽高走 i32 解析 + 既有护栏，非法值不写盘、按钮置灰。命令行对应 `--minimap-preset` / `--minimap-width` / `--minimap-height`，可无人值守复现。
- **公开 CI**（`.github/workflows/ci.yml`）：先跑版本号门禁 `pwsh -File scripts/bump-version.ps1 -Check`，再跑 `cargo test --workspace` 与 `cargo clippy --all-targets -- -D warnings`；且**不设任何素材环境变量**——素材相关用例自行打印 `[skip]` 后返回，所以「没有游戏素材」也是全绿。

### 变更

- 所有外部路径改为**显式环境变量**，仓库内不含任何指向某台开发机的内建路径；未配置时如实不可用（端点 503 / 页面 404 / 状态栏「资源库装载失败」），不去猜目录。
- 文档与界面全中文；官方界面译文的 key 与上游 `Localization/Strings.json` 对齐。
- **版本号收敛为单一真源**：根 `Cargo.toml` 新增 `[workspace.package].version`，`crates/` 下 8 个 crate 全部改为 `version.workspace = true`（不再各写各的）。新增 `scripts/bump-version.ps1`：`-Check` 只读校验、`-Part patch|minor|major` 递增、`-Set X.Y.Z` 显式设置；CI 增加 `-Check` 门禁。`vendor/wil2atlas` 是 vendored 上游解码器，保留自己的 `0.2.0`，不参与本仓 bump。
- 文档按单一真源口径改写：`README.md` 不再写死当前版本号；`VERSIONING.md` §3.1 / §六 记为已落地，并写明 `-Check` 是发布与 CI 的门禁。

### 修复

- **A-016 粘贴地表按源奇偶锚点**（`bf5ffaf`，对齐上游 `RegionCopy.cs:70`）：剪贴板记下复制时的源锚点，粘贴先按锚点对齐再放置；负锚点整块拒绝（剪贴板放回、一格不写），不再直接拿鼠标格当落点。
- **F2 地表吸管按 2×2 偶对锚点吸附**（`4e5286c`，对齐 `MapViewport.cs:405`）：地表吸管先取 `x & !1, y & !1` 锚点再取色，点到的是鼠标所在 2×2 单元的正确那一格。
- **A-002 前景加性混合**（`8435eb0`，对齐 `MapSpriteDrawing.Composite`）：前景层按 `ani_frame & 0x80` 选加性通道，预乘相加 + alpha 取最大值 + 反预乘；缺省路径仍逐字节不变。
- **A-010 印章层内行优先**（`4125b14`，对齐 `_upstream/CrystalStudio/MapViewport.StampRendering.cs:141-144`）：层内排序键由 `(dx, dy)` 列优先改为 `(dy, dx)` 行优先。**行为变化**：同一放置动作写入的图号序列变了，但格式不变、旧版照样能打开。
- 导出小地图尺寸模态的回车不再被折成"假点击取消"（`560a0dd` 内一并修掉）。

### 已知缺口

- Docker 编排（`docker/compose.yaml`、`Dockerfile`、`verify.sh`）**尚未在 CI 或任何机器上实跑验证**（开发机无 Docker 环境），请以实际构建结果为准。
- 本版本条目定稿时仓库还没有 tag 与 GitHub Release 页；发布时按 `VERSIONING.md` §六 打出 `v0.0.1` 并建 Release。

### 内部

- 上游基线：上游 [fluttergo/CrystalStudio](https://github.com/fluttergo/CrystalStudio)（C#/Avalonia），本仓对齐基线提交 `901dbe2f`；2026-11-06 复核上游 HEAD `f84566b`（新增模块化生成/分割/连接，未改 `.map` 格式/保存字节；本版未跟进模块族）。
- 数值口径：格式字节布局、光照 alpha、网格阈值、小地图尺寸等一律回源上游；上游确实不可得时才标「原版不可得 ⇒ 自定值」并写明依据。
- 本仓不使用 `cargo fmt`，代码风格靠既有约定与 clippy 维持，见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。
- 许可证 MIT，见 [`LICENSE`](LICENSE)。
