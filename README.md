# CrystalStudio-Rust

CrystalStudio 地图编辑器的 **Rust 全量移植**：把 C#/Avalonia 编辑器重写为 8 个单向依赖的 crate，同时保留 PC 原生版并新增网页版。

> 上游参照：[fluttergo/CrystalStudio](https://github.com/fluttergo/CrystalStudio)，本地取证基线 `901dbe2f`，线上复核至 `acfbbf8a`。

## 一、能力

| 项 | 内容 |
|---|---|
| 地图格式 | **9 种**：Old12 / Wemade2010 / Shanda14 / Shanda2012(36B) / AntiHack / Mir3-Wemade / Mir3-Shanda / Heroes34 / Crystal-C# |
| 语料 | 全仓 1,211 张真实 `.map`（639 个唯一哈希），9 种合成样本 9/9 往返 |
| 渲染 | 三层合成、多倍率、门帧偏移、地图光照（全屏遮罩 + 逐格光罩按真帧锚点定位）、阻挡标记 |
| 编辑 | 图层/笔刷/填充/撤销重做/选区剪贴板/印章放置/九格式保存 |
| PC 版 | egui，40 项菜单、浅色金界面（中文）、网格 `step` 抽稀、地图列表面板 |
| 网页版 | axum 后端 + 原生 JS 前端，PC 与手机浏览器适配 |
| 导出 | 导出选区、小地图（支持官方三档预设与指定像素）、分块渲染等值 |

## 二、架构（依赖单向）

```
mir2-map      地图字节 / 单元格
   ↓
mir2-assets   图库解码 / 帧锚点
   ↓
mir2-format   9 种格式判别与读写
mir2-stamp    .X 印章格式
   ↓
miredit-render  合成 / 缩放 / 光照 / 门帧
   ↓
miredit-core    编辑内核（命令 / 会话 / 撤销）
   ↓
miredit-gui     PC 原生 egui
miredit-web     网页后端 + web/ 前端
```

## 三、构建

```bash
cd tools/miredit
CARGO_INCREMENTAL=0 cargo test -p mir2-map -p mir2-assets -p mir2-format \
  -p mir2-stamp -p miredit-render -p miredit-core -p miredit-gui -p miredit-web
cargo clippy --all-targets -- -D warnings
cargo build -p miredit-gui --release
cargo build -p miredit-web --release
```

本机实测基线：**935 passed / 0 failed**（8 crate），workspace clippy `EXIT 0`。

## 四、运行

**PC 版**

```bash
cargo run -p miredit-gui --release -- --help
```

**网页版**

```bash
MIREDIT_MAP_ROOT=/path/to/maps \
MIREDIT_PALETTE_JSON=/path/to/palette.json \
cargo run -p miredit-web --release
```

前端静态文件在 `web/`，Docker 部署见 `docker/README.md`（⚠ Docker 镜像在本机**从未构建/运行**，只做了静态与运行时等价验证）。

## 五、文档

`docs/` 为架构蓝图，`deliverables/` 为移植总账、取证报告与审查记录；主入口是 `deliverables/移植总账-2026-11-05.md`。

## 六、说明

- 界面与文档全中文；官方译文取自上游 `Localization/Strings.json`。
- **禁假数据**：所有数值（格式字节布局、光照 alpha、网格阈值、小地图尺寸）均回源上游；上游确实不可得时才标注「原版不可得 ⇒ 自定值」并给出依据。
- `GodotClient/maps/` 与 `client/` 下素材为客户端资源，**只读**，不随本仓分发。
