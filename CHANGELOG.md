# 更新日志

本文件记录 **Crystal Studio**（仓库 `CrystalStudio-Rust`）每个对外版本改了什么，方便判断「要不要更新」。

- 格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)；版本号遵循 [SemVer](https://semver.org/lang/zh-CN/)，递增判据与发布流程见 [`VERSIONING.md`](VERSIONING.md)。
- 版本号按 [`VERSIONING.md`](VERSIONING.md) §5.1 的 **0.x 判据**递增：1.0 之前 `breaking` 与 `feature` 都升 MINOR、`fix` 升 PATCH、只动测试/文档不 bump；机械递增用 `scripts/bump-version.ps1`（§5.3）。`[Unreleased]` 只积累**尚未发布**的改动：下一版号要到发布那一刻才定，本文件不预写版本号与日期。
- 小节名固定为：`不兼容变更` / `新增` / `变更` / `弃用` / `移除` / `修复` / `安全` / `已知缺口` / `内部`。没有内容的小节直接省略，不留空壳。
- 日期只有两种状态：真实发布日期，或条目仍在 `[Unreleased]` 下（无日期）。**不写占位日期**。
- 开发期的改动先记进 `[Unreleased]`；发布时整体搬到版本标题下（步骤见 `VERSIONING.md` §六）。
- 版本之间的 `compare` 链接等打过 tag 后再补：本文件不提前写会 404 的链接。

## [Unreleased]

### 新增

- **R9-F2 键位补齐**：`B/P/E/V/K` 切工具（P/K 为 Unwired 槽，只切槽并显式标未接线）、`C` 同切低/高阻挡、`WASD` 按屏幕 48px 平移；Ctrl+C/V/S 不被裸键吃掉；浮层新增三行并扩到 820px。

### 修复

- R9-F2：`pan_by_screen` 统一换算，`scale=2` 时世界 ±24、屏幕 ±48；overlay/搜索焦点时裸键让位。

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
- **A-010 印章层内行优先**（`4125b14`，对齐 `StampRendering.cs:141-144`）：层内排序键由 `(dx, dy)` 列优先改为 `(dy, dx)` 行优先。**行为变化**：同一放置动作写入的图号序列变了，但格式不变、旧版照样能打开。
- 导出小地图尺寸模态的回车不再被折成"假点击取消"（`560a0dd` 内一并修掉）。

### 已知缺口

- Docker 编排（`docker/compose.yaml`、`Dockerfile`、`verify.sh`）**尚未在 CI 或任何机器上实跑验证**（开发机无 Docker 环境），请以实际构建结果为准。
- 本版本条目定稿时仓库还没有 tag 与 GitHub Release 页；发布时按 `VERSIONING.md` §六 打出 `v0.0.1` 并建 Release。

### 内部

- 上游基线：上游 [fluttergo/CrystalStudio](https://github.com/fluttergo/CrystalStudio)（C#/Avalonia），本仓对齐基线提交 `901dbe2f`；2026-11-06 复核上游 HEAD `f84566b`（新增模块化生成/分割/连接，未改 `.map` 格式/保存字节；本版未跟进模块族）。
- 数值口径：格式字节布局、光照 alpha、网格阈值、小地图尺寸等一律回源上游；上游确实不可得时才标「原版不可得 ⇒ 自定值」并写明依据。
- 本仓不使用 `cargo fmt`，代码风格靠既有约定与 clippy 维持，见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。
- 许可证 MIT，见 [`LICENSE`](LICENSE)。
