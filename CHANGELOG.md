# 更新日志

本文件记录 **Crystal Studio**（仓库 `CrystalStudio-Rust`）每个对外版本改了什么，方便判断「要不要更新」。

- 格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)；版本号遵循 [SemVer](https://semver.org/lang/zh-CN/)，递增判据与发布流程见 [`VERSIONING.md`](VERSIONING.md)。
- 版本号按 [`VERSIONING.md`](VERSIONING.md) §5.1 的 **0.x 判据**递增：1.0 之前 `breaking` 与 `feature` 都升 MINOR、`fix` 升 PATCH、只动测试/文档不 bump；机械递增用 `scripts/bump-version.ps1`（§5.3）。`[Unreleased]` 只积累**尚未发布**的改动：下一版号要到发布那一刻才定，本文件不预写版本号与日期。
- 小节名固定为：`不兼容变更` / `新增` / `变更` / `弃用` / `移除` / `修复` / `安全` / `已知缺口` / `内部`。没有内容的小节直接省略，不留空壳。
- 日期只有两种状态：真实发布日期，或条目仍在 `[Unreleased]` 下（无日期）。**不写占位日期**。
- 开发期的改动先记进 `[Unreleased]`；发布时整体搬到版本标题下（步骤见 `VERSIONING.md` §六）。
- 版本之间的 `compare` 链接等打过 tag 后再补：本文件不提前写会 404 的链接。

## [Unreleased]

> 暂无条目。新改动先记在这里，发布时整体搬进下一个版本标题。

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
