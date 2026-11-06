# Crystal Studio

> 仓库名 `CrystalStudio-Rust`；软件显示名 **`Crystal Studio`**（两个词，C/S 大写）。`miredit-*` / `mir2-*` 只是 crate 与二进制的技术标识，不是产品名。

**Crystal Studio** 是传奇 M2 系地图编辑器（上游为 C# / Avalonia 实现）的 **Rust 全量移植**：8 个单向依赖的 crate，PC 原生版与网页版共用同一编辑内核，覆盖 **9 种地图格式**的判别、解析与写回。

上游参照：[`fluttergo/CrystalStudio`](https://github.com/fluttergo/CrystalStudio)，本仓对齐的上游基线提交为 `901dbe2f`。

## 一、能力

| 项 | 内容 |
|---|---|
| 地图格式 | **9 种**：`Old12` / `Wemade2010` / `Shanda14` / `Shanda2012`（36 B/格）/ `AntiHack` / `Mir3-Wemade` / `Mir3-Shanda` / `Heroes34` / `CrystalCs`（C# 版） |
| 格式保真 | 写回走**原格式**；未建模的尾部/未知区字节原样保留，不猜测语义（带尾块的地图按宽松口径也能打开，并如实标注未解字节数） |
| 渲染 | 三层合成（地表 / 中层 / 物体）、四档缩放（25% / 50% / 100% / 200%）、门帧偏移（开/闭门）、地图光照三档（白天 / 黎明·黄昏 / 夜晚）、低/高阻挡标记 |
| 编辑 | 图层切换、笔刷（3..31 奇数边长）、Bresenham 描线、BFS 连通填充、橡皮擦、吸管、撤销/重做、矩形选区与剪贴板、印章（`.X`）放置、原格式保存 |
| PC 版 | egui 原生窗口，浅色金主题，中文界面；菜单 5 个 / 40 项（含 3 个子菜单）；地图列表面板；网格抽稀；多种诊断与导出开关 |
| 网页版 | axum 后端 + 原生 JS 前端（不引框架 / CDN / npm），桌面与手机浏览器适配，WebSocket 推送脏标记与重绘区域 |
| 导出 | 选区导出、小地图（官方三档预设或指定像素）、1:1 渲染转储 |

## 二、架构（8 crate，依赖单向）

```text
基础层（无内部依赖）
  mir2-map      地图字节 / 单元格模型
  mir2-format   9 种格式的判别与读写（只依赖 std）
  mir2-stamp    .X 印章格式
  mir2-assets   图库解码（Mir2 .wil/.wix、.wzl/.wzx；原版 Crystal .lib）/ 帧锚点

第二层
  miredit-render  合成 / 缩放 / 光照 / 门帧     ← mir2-map, mir2-assets, mir2-stamp

第三层
  miredit-core    编辑内核（命令 / 会话 / 撤销） ← mir2-map, mir2-format, mir2-stamp, mir2-assets

第四层（两者互不依赖）
  miredit-gui     PC 原生版（egui）             ← 上述 + miredit-render, miredit-core
  miredit-web     网页版后端（axum），前端在 web/ ← 上述全部
```

依赖只允许自上向下；`mir2-format` 故意保持零第三方依赖，便于把格式层单独复用与对拍。

## 三、构建与测试

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

通过/失败的具体条数以**实际运行结果**为准（`cargo test` 会自己打印），本文件不锁定会过期的数字。

发布构建：

```bash
cargo build -p miredit-gui --release
cargo build -p miredit-web --release
```

> **前置依赖**：`crates/mir2-assets` 通过 path 依赖引用 `wil2atlas`（WIL/WZL 解码层，本仓不复制其解析代码）。
> 当前清单里写的是开发机绝对路径，clone 之后需先把它指向你自己的 `wil2atlas` 副本（或换成发布版依赖），否则 `cargo build` 会在解析依赖时失败。
>
> 本项目**不使用 `cargo fmt`**，代码风格靠既有约定与 clippy 维持，见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 四、运行

### 4.1 PC 版

```bash
cargo run -p miredit-gui --release -- --map /path/to/some.map
cargo run -p miredit-gui --release -- --help
```

常用开关：`--map <文件>` 打开指定地图、`--map-list-root <目录>` 指定地图列表根、`--no-window` 配合 `--dump-render` / `--edit-*` 跑无人值守诊断。

### 4.2 网页版

后端配置**全部走环境变量**（没有对应命令行参数）：

```bash
MIREDIT_BIND=127.0.0.1:8080 \
MIREDIT_MAP_ROOT=/path/to/maps \
MIREDIT_CLIENT_ROOT=/path/to/client-root \
MIREDIT_PALETTE_JSON=/path/to/crystalstudio/catalogue/palette.json \
MIREDIT_WEB_ROOT=/path/to/repo/web \
cargo run -p miredit-web --release
```

| 变量 | 作用 |
|---|---|
| `MIREDIT_BIND` | 监听地址（缺省 `0.0.0.0:8080`） |
| `MIREDIT_MAP_ROOT` | `/api/map/open` 的白名单根 |
| `MIREDIT_CLIENT_ROOT` | 图库（客户端）扫描根 |
| `MIREDIT_PALETTE_JSON` | 印章目录索引 `palette.json` |
| `MIREDIT_WEB_ROOT` | 静态前端目录（本仓的 `web/`） |

> ⚠ 这几个变量的**内置缺省值指向开发机上的布局**，换机器请全部显式设置；前端静态目录不设就会 404（不伪造页面）。接口契约见 [web/README.md](web/README.md)。

## 五、游戏素材（不随本仓分发）

本仓**只含代码**。地图编辑器需要客户端素材才能出图：

- **地图**：`.map` 文件目录 → 网页版 `MIREDIT_MAP_ROOT`，PC 版 `--map` / `--map-list-root`。
- **图库**：客户端根目录（Mir2 的 `.wil/.wix`、`.wzl/.wzx`，或原版 Crystal `.lib`）→ 网页版 `MIREDIT_CLIENT_ROOT`；PC 版目前直接使用 `mir2-assets` 的内置客户端根常量（换机器需改该常量，GUI 侧尚未接 CLI / 环境变量）。
- **印章**：`catalogue/palette.json` 与并列的 `Objects/*.X` → 网页版 `MIREDIT_PALETTE_JSON`。

缺素材时各端点/界面如实报错（404 / 503 / 「图库未就绪」），**不会**画占位图冒充成功。

## 六、文档

| 文件 | 内容 |
|---|---|
| [docs/架构蓝图.md](docs/架构蓝图.md) | 分层、模块边界、里程碑 |
| [docs/地图格式规格.md](docs/地图格式规格.md) | 9 种格式的逐字段字节布局与判别/读写口径 |
| [docs/README.md](docs/README.md) | 文档索引 |
| [web/README.md](web/README.md) | 网页版接口契约（端点 / 错误码 / WebSocket / 前端行为） |
| [docker/README.md](docker/README.md) | 容器部署说明 |
| [layout/README-crystalstudio-ui.md](layout/README-crystalstudio-ui.md) | 上游界面真值数据（`crystalstudio-ui.json` / `crystalstudio-strings.json`）的口径 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 贡献流程 |

## 七、说明

- 界面与文档全中文；官方界面译文的 key 与上游 `Localization/Strings.json` 对齐。
- **不许假数据**：格式字节布局、光照 alpha、网格阈值、小地图尺寸等数值一律回源上游；上游确实不可得时才显式标注「原版不可得 ⇒ 自定值」并写明依据。
- 许可证：**MIT**，见 [LICENSE](LICENSE)。
- Docker：`docker/README.md` 描述的是**尚未在 CI 或任何机器上实跑验证**的编排（开发机无 Docker 环境），请以实际构建结果为准。
