# CrystalStudio-Rust · 资料索引

> 本项目是独立工程（root `I:\miredit`，独立 `.git`），**与 `E:\GodM2` 是两条不同的工作线**。
> 2026-11-06 从 `E:\GodM2` 迁入，避免双项目并行时日志/派单/路径互相污染。

## 一、目录

| 路径 | 内容 | 来源 |
|---|---|---|
| `_upstream/` | 上游 C# 源码（19,041 文件 / 300.7 MB，**只读参照**） | 原 `E:\GodM2\CrystalStudio`，2026-11-06 **搬移**（E 盘已删） |
| `crates/` `web/` `docker/` | 8 crate + 网页前端 + Docker 骨架 | 项目本体，已随代码迁到 `I:\miredit` |
| `layout/` | UI 真值数据（`crystalstudio-ui.json` / `crystalstudio-strings.json`）+ 截图 | 随代码迁入 |
| `docs/deliverables/` | 18 份移植线交付文档 + 架构蓝图 + 猎手-引用路径核验目录 | 复制自 `E:\GodM2\deliverables\` 与 `E:\GodM2\docs\`（**E 盘原件保留**） |
| `docs/工作日志/` | 2026-11-05 / 11-06 的**移植线段**（按段落从混写日志拆出） | 复制拆分自 `E:\GodM2\.workbuddy\memory\`（**E 盘原件保留**） |
| `evidence/上游小地图源码取证/` | `MinimapExporter.cs` 等 7 件 | 原 `I:\cs-new`，已复制 |
| `evidence/上游核心源码取证/` | `MapDimensions.cs` 等 13 件 | 原 `I:\cs-upstream-new`，已复制 |
| `evidence/小地图回源订正出图证据/` | 22 文件 / 212.4 MB 出图与日志 | 原 `I:\miredit-minimap-sourceproof`，已复制 |

## 二、主入口

- 移植总账：[`docs/deliverables/移植总账-2026-11-05.md`](deliverables/移植总账-2026-11-05.md)
- 架构蓝图：[`docs/移植-CrystalStudio-Rust-架构蓝图-2026-11-04.md`](移植-CrystalStudio-Rust-架构蓝图-2026-11-04.md)
- 9 格式规格：[`docs/deliverables/规格-地图格式全量9种-2026-11-04.md`](deliverables/规格-地图格式全量9种-2026-11-04.md)
- 上游变更评估：[`docs/deliverables/上游变更-6f2dadde到901dbe2f-2026-11-05.md`](deliverables/上游变更-6f2dadde到901dbe2f-2026-11-05.md)

## 三、路径变更对照（重要）

文档里若出现下列旧路径，**一律按本表映射到新位置**；历史证据里的旧路径保留不改（它们是当时的真值）。

| 旧路径 | 新路径 |
|---|---|
| `E:\GodM2\tools\miredit` | `I:\miredit` |
| `E:\GodM2\CrystalStudio` | `I:\miredit\_upstream` |
| `E:\GodM2\deliverables\<移植线文档>` | `I:\miredit\docs\deliverables\<同名>` |
| `E:\GodM2\docs\移植-CrystalStudio-Rust-架构蓝图-2026-11-04.md` | `I:\miredit\docs\移植-CrystalStudio-Rust-架构蓝图-2026-11-04.md` |
| `I:\cs-new` | `I:\miredit\evidence\上游小地图源码取证` |
| `I:\cs-upstream-new` | `I:\miredit\evidence\上游核心源码取证` |
| `I:\miredit-minimap-sourceproof` | `I:\miredit\evidence\小地图回源订正出图证据` |

## 四、两条线的边界（避免冲突）

- **`E:\GodM2`** = GodM2 服务端/Godot 客户端那条线，其 `.workbuddy\memory\` 里仍混有移植线的历史段落（**保留不动**，不删别人的记录）。
- **`I:\miredit`** = 本移植线，日志与交付文档自 2026-11-06 起只在本项目内追加。
- 派单槽位是平台全局共享的：两条线同时跑时会互相占槽，属预期，按 `multi-agent-dispatch-hygiene` 落档排队，不硬撞。
- 上游源码 `_upstream/` 是**只读参照**；所有移植结论都要标注它对应的上游提交号（本地基线 `901dbe2f`，线上复核至 `acfbbf8a`）。

## 五、迁移校验记录（2026-11-06）

| 项 | 证据 |
|---|---|
| 上游源码搬移 | 复制后 19,041 文件 / 300.7 MB 与源**文件数+字节数一致**，8 个随机抽样 sha256 一致 ⇒ 才删源 |
| 18 份文档 | 逐文件 sha256 与 E 盘原件一致 |
| 证据目录 | 5 个大文件抽样 sha256 一致 |
| 日志拆分 | 11-05 保留 31 段 / 11-06 保留 13 段，服务端关键词残留 **0** |
| 凭据 | `.git/config` 与索引文件均无 token |
