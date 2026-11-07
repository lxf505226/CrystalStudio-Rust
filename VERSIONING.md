# 版本号与发布规则

> 适用对象：本仓（公开仓 `CrystalStudio-Rust`）的**每一次对外发布**。
> 相关文件：[`CHANGELOG.md`](CHANGELOG.md)（每个版本改了什么）、[`README.md`](README.md)（构建/运行/资产）、[`docker/README.md`](docker/README.md)（容器部署）。
> 本文只写**规则与流程**；版本号的数值写在 `Cargo.toml` 里（见 §二），本文不重复抄写，避免出现第二个真源。

## 一、一句话

版本号 = `<YYYYMMDD>.<当天轮次>.<轮内修正>`，**唯一真源是根 `Cargo.toml` 的 `[workspace.package].version`**。
一次发布只递增一次：先把内容记进 `CHANGELOG` 的 `[Unreleased]`，发布时改成带**真实日期**的版本标题，并把 tag `v<版本>` 打在发布提交上。

> **2026-11-07 方案变更（用户拍板）**：不再使用 `0.0.0` / `0.x.y` 递增。
> 改为「日期 + 当天序号」，避免同一天多次发布时版本号撞车。
> 旧的 `0.x.y` 版本、tag、CHANGELOG 小节**原样保留不清理**，历史可查。
> 本文档里凡涉及「升 MINOR / 升 PATCH」的**分类判据仍然有效**，只是最终号段改按下表映射。

## 二、版本号格式（日期版本）

| 形态 | 何时用 | 示例 | 备注 |
|---|---|---|---|
| `<YYYYMMDD>.0.0` | 当天**首次**发布 | `20261107.0.0` | 第一段是 8 位合法日历日（`yyyyMMdd`） |
| `<YYYYMMDD>.R.0` | 当天**另起一轮**（通常夹带新能力） | `20261107.1.0` | R 从 0 起，每次 +1；修正段归零 |
| `<YYYYMMDD>.R.F` | 同一轮内**再推一次的修正** | `20261107.1.2` | F 从 0 起，每次 +1；只修不增 |
| `<YYYYMMDD+1>.0.0` | 换一天发布 | `20261108.0.0` | 日期段取系统当天，轮次/修正归零 |

- 三段都是十进制整数、不写前导零；它仍是 Cargo 接受的 `MAJOR.MINOR.PATCH` 形状，但**语义不再是 SemVer 的兼容级别**。
- 比较大小按三段数值：日期越大越新；同日轮次越大越新；同轮修正越大越新。
- tag 名恒为 `v` + 版本号：`v20261107.0.0`。**tag 里不带其它后缀**。
- `+g<sha>` 构建元数据**只用于显示**（取 `git rev-parse --short HEAD`），不进 `Cargo.toml`、不进 tag、不参与比较；git 工作树之外构建时省略，不回填编造的 sha。
- 版本号只描述**本仓的产品版本**；`vendor/wil2atlas` 是 vendored 的上游解码器，保留自己的版本号，不跟本仓走（见 §三）。

### 2.1 递增动作（机械口径，与改动分类的关系见 §四/§5.2）

| 命令 | 结果 | 语义 |
|---|---|---|
| `-Part patch` | `<日期>.<轮次>.<修正+1>` | 同一轮内再推一次；只修不增 |
| `-Part minor` | `<日期>.<轮次+1>.0` | 当天另起一轮（通常因为有新能力） |
| `-Part major` | `<今天>.0.0` | 换一天；**今天已发过就拒绝**（防止一天两次靠日期段蒙混） |
| `-Set <YYYYMMDD>.R.F` | 显式指定 | 用于方案切换首版、或手工纠偏；日期必须是合法日历日 |

> 一次发布只 bump 一次，取本次全部改动里的**最高类别**（见 §5.2）：
> `breaking` / `feature` ⇒ 用 `minor`（另起一轮）或 `major`（换天）；`fix` ⇒ 用 `patch`（轮内修正）；
> 只动测试/注释 ⇒ 不 bump，随下一版带上。
> **分类判据（什么算 breaking/feature/fix）与 §八 的兼容性承诺完全不变**，与版本号形态无关。

### 2.2 Windows exe 数值版本（避免 u16 溢出）

`FILEVERSION` / `PRODUCTVERSION` 只有四个 `u16`，装不下 `20261107`。`crates/miredit-gui/build.rs`
把 8 位日期拆成「年 / 月日」：`20261107.1.2` ⇒ `[2026, 1107, 1, 2]`。
字符串 `FileVersion` / `ProductVersion` 仍是完整的 `CARGO_PKG_VERSION`，两者各司其职；
旧 `0.x.y` 构建时按 `[0, x, y, 0]` 兼容。

## 三、单一真源与展示面

### 3.1 真源

根 `Cargo.toml` 里放**唯一一处**版本号，各 crate 引用它，不各写各的：

```toml
[workspace.package]
version = "20261107.0.0"   # 日期版本；数值以文件为准
```

```toml
[package]
name = "miredit-web"
version.workspace = true
```

- 发布递增时**只改根 `Cargo.toml` 这一行**；任何 crate 里出现写死的 `version = "x.y.z"` 都算缺陷。
- 例外：`vendor/wil2atlas/` 是解码器子集的 vendored 副本（decoder-only，MIT），它有自己的上游版本（现为 `0.2.0`），**不随本仓 bump 改动**。
- 现状（2026-11-06 落地）：根 `Cargo.toml` 已有上面的 `[workspace.package]` 段，`crates/` 下 8 个 crate 全部是 `version.workspace = true`；`pwsh -File scripts/bump-version.ps1 -Check` 退出码 0。
  `-Check` 是**门禁**：CI（`.github/workflows/ci.yml`）在 `cargo test` 之前跑它，发布前也必须先跑绿（§5.3 / §六）。任何 crate 里再出现写死的 `version = "x.y.z"` 都会被它判红。

### 3.2 展示面（用户在哪里看得到版本）

| 展示面 | 口径 | 现状 |
|---|---|---|
| 网页后端 `GET /api/health` | `{"ok":true,"version":"<版本>"}`，取 `CARGO_PKG_VERSION` | ✅ 已实现 |
| 网页前端连接状态 | 「后端已连接 · \<版本\>」/「已连接 \<版本\>」，版本来自 `/api/health` | ✅ 已实现（`web/js/ui.js`） |
| `miredit-web` 启动横幅 | `[启动] miredit-web v<版本>` | ✅ 已实现 |
| README / GitHub Release 标题 | `Crystal Studio v<版本>` | 发布时填（§六） |
| GUI `--version` | 打印 `Crystal Studio <版本>`，版本取 `CARGO_PKG_VERSION` | ✅ 已实现：`miredit-gui --version`（`--help` 用法行也列出）；GUI 内的「关于」对话框同样只许从 `CARGO_PKG_VERSION` 取版本，落地并实测后才能补进本表 |
| 包自身 | `cargo pkgid -p miredit-web` 输出末段 `#<版本>` | ✅ 可直接核对 |

纪律：

- 前端只能经 `/api/health` 取版本，不许在 HTML/JS 里另写一份数字。
- 源码、文档、Issue 模板里不硬编码"当前版本"，要指就指向 `Cargo.toml` 或 Release 页。
- 新增展示面（GUI 关于对话框、CLI `--version`、镜像标签等）必须**先真的打印出来**，再写进上表；做不到就标「尚未实现」。

## 四、递增判据（针对地图编辑器）

满足任一条件即升该级别；**一次发布只 bump 一次**，取本次全部改动中的最高级别。

| 级别 | 触发条件 | 本项目具体例子 |
|---|---|---|
| **MAJOR** | 破坏已发布的兼容性 | ① 已支持格式的 `.map` **写回字节改变**且没有迁移手段；② 删掉对某种格式的支持；③ 删除/改名 CLI 开关（如 `--edit-fill`、`--export-minimap`、`--minimap-preset` 改名）；④ 网页 API 不兼容（删端点、`/api/edit` 参数语义改变、JSON 字段改名、错误码语义反转）；⑤ 改环境变量名（`MIREDIT_BIND` / `MIREDIT_MAP_ROOT` / `MIREDIT_CLIENT_ROOT` / `MIREDIT_PALETTE_JSON` / `MIREDIT_WEB_ROOT` / `MIR2_STAMP_PALETTE` 任一改名或删除）；⑥ 改缺省监听地址/端口（存量部署会失联） |
| **MINOR** | 新增能力，且老用法照旧能用 | 新增第 10 种地图格式；新增编辑工具或导出类型；新增面板/菜单项；新增网页入口（含新增端点）；新增平台构建（Linux/macOS）；新增**可选**环境变量（不设也能跑） |
| **PATCH** | 只修不增，且不改变落盘字节 | bug 修复；渲染/数值修正但**不改 `.map` 保存字节**（例：状态栏正文色 `#ADB8C9`/`#434D5A`、地表吸管 2×2 锚点吸附、粘贴地表按源奇偶锚点、前景加性混合）；小地图导出尺寸档位；文档/文案/界面译文；依赖安全升级（无接口破坏）；内部重构（零行为变化） |
| **不 bump** | 不影响任何对外行为 | 只动测试、CI 配置、注释、拼写。这类改动**不单独发版**，随下一个版本带上；`CHANGELOG` 里可列在「内部」小节，也可以不列 |

两条容易踩的边界：

1. **「行为变化」不等于「格式不兼容」。** 判据分开：
   - 格式不兼容 = 旧版本**读不了或读错**新版本写的文件，或同一文件**读写往返字节**变了 ⇒ 按 MAJOR（0.x 阶段见 §五）。
   - 行为变化 = 同一个操作**结果不同**（例：印章层排序键改成 `(dy,dx)` 后，同样的放置动作写入的图号序列变了，但文件仍是同一种格式、旧版照样能打开）⇒ 属 PATCH，但**必须**在 `CHANGELOG` 的修复条目里明写「行为变化」，免得用户以为丢数据。
2. **拿不准就取高级别。** 宁可按 MINOR 发并说明，也不要把可能影响存量数据的改动塞进 PATCH。

## 五、0.x 阶段策略（**历史方案，2026-11-07 起不再用于新版本号**）

> ⚠ 本节记录 `0.0.1 → 0.7.0` 那段历史，**保留作历史依据**（旧 tag / 旧 CHANGELOG 不清理）。
> 新版本号一律按 §二 的日期方案，**不要再按本节升 `0.x`**。
> 下面提到的「PATCH 不动保存字节 / MINOR 允许含 breaking 但必须单列不兼容变更」等
> **分类纪律仍然有效**，只是号段映射改为 §2.1。

- 当前版本线为 `0.0.1`（**首个公开可用版本**，见 [`CHANGELOG.md`](CHANGELOG.md)；数值以根 `Cargo.toml` 的 `[workspace.package].version` 为准）。档位阶梯见 §5.1：`0.0.1` → `0.0.x`（只修不增）→ `0.1.0`（首个 MINOR 功能里程碑）→ `0.1.x`（该线补丁）→ `0.2.0`（后续 MINOR）→ `1.0.0`（稳定承诺）。
- 0.x 阶段按 SemVer 原意"公共接口尚未定稿"执行：**MINOR 允许含破坏性变更**，但必须：
  - 在 `CHANGELOG` 该版本下**单列 `### 不兼容变更`**，逐条写「以前什么样 / 现在什么样 / 用户该怎么办」；
  - 没有迁移工具就明写「需手工重建/手工重设」，不许含糊。
- **PATCH 在 0.x 同样不得改变已支持格式的保存字节**；真要改，升 MINOR 并列进「不兼容变更」。
- 下一批**只含修复** ⇒ 建议发 `0.0.2`（PATCH）；一旦夹带任何新能力（新增网页入口 / 新增面板 / 新导出类型）或改了已支持格式的保存字节 ⇒ 发 `0.1.0`（MINOR，判据见 §四），breaking 按本节单列 `### 不兼容变更`。
- 什么时候升 `1.0.0`（四条同时成立才升，缺一条就继续 0.x）：
  1. 9 种格式在**真实素材**上「打开 → 编辑 → 保存 → 重开 → 出图」往返稳定（以实跑记录为准，不拿测试条数当"稳定"）；
  2. `.map` 保存字节连续两个 MINOR 版本未变；
  3. 网页 API（端点/参数/JSON 字段/错误码）与 GUI CLI 开关连续两个 MINOR 未改；
  4. 至少一轮带真实素材的全流程验收通过并留证。
- 别为了"看起来成熟"提前打 1.0.0：1.0 是**兼容性承诺生效**的那一刻（§八），不是荣誉勋章。

### 5.1 档位判据（`0.0.x` / `0.1.x` / `0.2.x`）与 `1.0.0` 的门槛

判据只看一件事：**这一版会让「已经用上的人」付出什么代价**——什么都不用改（PATCH）、要改用法（MINOR）、还是连写出的 `.map` 都得重做（§四 的 MAJOR；1.0 之前压到 MINOR）。

| 档位 | 什么时候算这一档 | 本项目实例 | 承诺 |
|---|---|---|---|
| `0.0.1` | **首个公开可用版本**（当前版本）：陌生人 clone 本仓、自备素材，能走通「打开 → 编辑 → 保存 → 重开」 | `0.0.1`（2026-11-06），仓库第一个对外版本，见 [`CHANGELOG.md`](CHANGELOG.md) | 0.0 线内 PATCH 不动保存字节（§八） |
| `0.0.x`（`0.0.2` 起） | 相对上一版**只修不增**：修缺陷、修渲染/数值、改文档与界面文案；**不改 `.map` 保存字节、不加功能、不改 GUI 开关与 CLI 参数、不改 Web API（端点/参数/JSON 字段/错误码）** | 修复条目 **A-016**（粘贴落点 = 源锚点按源奇偶重算）、**A-010**（印章层改行优先 `(dy, dx)` 排序）、**F2**、**A-002**（同批修复，编号取自开发期台账）：都属「只修不增、保存字节不变」⇒ 若这批修复单独发一版即 `0.0.2` | 同 `0.0.1` |
| `0.1.0` | **首个 MINOR 功能里程碑**：第一次在已发布版本上**新增能力**——新功能 / 新格式 / 新面板 / 新网页入口 / 新导出类型 / 新增可选环境变量。0.x 阶段这一级**允许夹带 breaking**，但必须按 §五 单列 `### 不兼容变更` | 网页版导出小地图（新增网页入口 `GET /api/export/minimap`）、PC 版导出小地图尺寸 UI（新增导出入口）：若它们相对 `0.0.1` 另发一版，就是 `0.1.0` | 0.1 线内 PATCH 不动保存字节（§八） |
| `0.1.x`（`0.1.1` 起） | `0.1.0` 之后的**该线补丁**：判据与 `0.0.x` 相同（只修不增，不改保存字节/不加功能/不改 CLI 与 Web API） | 就是 `0.0.x` 那批修复，只是基线换成 `0.1.0` ⇒ `0.1.1` | 同 `0.1.0` |
| `0.2.0` | **后续 MINOR**（**不是当前版本**）：`0.1.0` 之后的下一个功能里程碑，判据同 `0.1.0` | 目前没有属于这一级的内容；只有在 `0.1.0` 已发布、之后又攒够一批新能力时才用它 | 仍无跨 MINOR 承诺；回退前先备份地图（§7.5） |
| `1.0.0` | 同时满足本节上面四条（实素材往返稳定 / 保存字节连两个 MINOR 未变 / Web API 与 CLI 连两个 MINOR 未改 / 一轮实素材验收留证） | 目前不满足，别提前打（§九） | **兼容性承诺自此生效**（§八） |

> `0.0.2` 与 `0.0.1`、`0.1.1` 与 `0.1.0` 的差别都只有「修了什么」；一旦这一版里出现**任何**新能力，它就已经升到下一个 MINOR（`0.0.x` → `0.1.0`，`0.1.x` → `0.2.0`），不是 PATCH。

### 5.2 判定算法：一次发布只 bump 一次，取「最高类别」

1. **先归类**：把这次要发的内容逐条归类，只有四种——`breaking` / `feature` / `fix` / `不发版`。
2. **取最高**：一次发布只递增一次，版本号由**全部条目里最高的那一类**决定；低类别条目跟着这一次走，不单独补一次发版。
3. **映射**：

| 类别 | 本项目的判据 | 1.0 之前 | 1.0 之后 |
|---|---|---|---|
| `breaking` | 改已支持格式的 `.map` 保存字节；删掉某格式；删/改 CLI 开关、Web API 端点·参数·JSON 字段·错误码；改环境变量名；改缺省端口（§四 MAJOR 行） | 升 **MINOR**（`0.0.x` → `0.1.0`，`0.1.x` → `0.2.0`），并在 `CHANGELOG` 单列 `### 不兼容变更`，逐条写「以前什么样 / 现在什么样 / 用户该怎么办」 | 升 **MAJOR** |
| `feature` | 新增功能 / 格式 / 面板 / 网页入口 / 导出类型 / 可选环境变量（§四 MINOR 行） | 升 **MINOR**（`0.0.1` → `0.1.0`） | 升 **MINOR** |
| `fix` | 只修缺陷、渲染或数值，保存字节不变、对外接口不变（§四 PATCH 行） | 升 **PATCH**（`0.0.1` → `0.0.2`；`0.1.0` → `0.1.1`） | 升 **PATCH** |
| `不发版` | 只动测试 / CI / 注释 / 文档 / 拼写 | **不 bump**，随下一个版本带上（`CHANGELOG` 记「内部」小节，或不记） | 同左 |

4. **演算四个例子**（都取本项目的真实场景）：
   - 只有 3 条修复（A-016 / F2 / A-010）⇒ `0.0.1` → **`0.0.2`**（PATCH）。
   - 3 条修复 + 1 个网页导出入口（`feature`）⇒ `0.0.1` → **`0.1.0`**：修复条目写进 `0.1.0` 的 `### 修复`，**不**先发 `0.0.2` 再发 `0.1.0`（一次发布只 bump 一次）。
   - 只有测试 / CI / 文档改动 ⇒ **不发版**，留在 `[Unreleased]` 等下一个版本。
   - 改 `.map` 保存字节（例：给某格式加一个建模字段）⇒ 1.0 之前按 **MINOR**（`0.0.1` → `0.1.0`，0.x 的 MINOR 即「1.0 前的 breaking 位」）发并写 `### 不兼容变更`；1.0 之后按 **MAJOR** 发。
5. **拿不准就取高级别**（§四 边界第 2 条）：宁可按高一级发并说明，也不要把可能影响存量数据的改动塞进 PATCH。

### 5.3 机械执行：`scripts/bump-version.ps1`

版本号只改一处，但「改根版本 + 搬 `[Unreleased]` 标题 + 打印发布清单」容易漏一步，统一走脚本（`pwsh`，脚本本身 UTF-8 无 BOM）：

| 命令 | 作用 |
|---|---|
| `pwsh -File scripts/bump-version.ps1 -Check` | 只读校验：根 `[workspace.package].version` 可读且是合法日期版本（或旧 `0.x.y` 切换起点）、`crates/` 下 8 个 crate 都是 `version.workspace = true`、`vendor/wil2atlas` 作为 vendored 上游例外单独报；不一致 ⇒ 退出码 1 |
| `... -Part patch` / `-Part minor` / `-Part major` | 按 §2.1 递增：`patch` 升轮内修正、`minor` 另起一轮并把修正归零、`major` 换一天（今天已发过直接拒绝） |
| `... -Set <YYYYMMDD>.R.F` | 显式设成指定日期版本（严格格式校验：日期须为合法日历日；旧 `0.x.y` 不能再 `-Set`，只能切到日期版本） |

脚本只做两件事：改根 `Cargo.toml` 的那一行、把 `[Unreleased]` 改成 `## [<日期版本>] - <系统当天日期>` 并在顶部重建空的 `[Unreleased]`。**它不会替你写变更条目**（§九 第 3 条），也不碰 git、不打 tag，最后打印 §六 的发布清单供人执行。

> 现状（2026-11-06 落地）：根 `Cargo.toml` 已有 `[workspace.package]`，`-Check` 退出码 0。它同时是 CI 门禁（`.github/workflows/ci.yml` 在 `cargo test` 之前执行）与发布门禁（§六 第 1 步）。

## 六、发布清单（按顺序执行）

1. **改版本号**：把根 `Cargo.toml` 的 `[workspace.package].version` 改成新版本（该段已按 §3.1 落地；8 个 crate 是 `version.workspace = true`）。除根这一处，其它文件不许出现版本数字。
   机械执行：先 `pwsh -File scripts/bump-version.ps1 -Check` 验真源——**必须退出码 0，这就是发布门禁**（CI 也跑同一条）；再用 `-Part patch|minor|major` 递增（判据见 §5.2，脚本说明见 §5.3）；它会顺带把 `[Unreleased]` 搬成带日期的版本标题，**但不会替你写变更条目**。
2. **改 CHANGELOG**：把 `[Unreleased]` 下的条目整体归到新版本标题下，标题形如「`## [<日期版本>] - ` + 当天真实日期」（**写实际发布那天的日期**，不写占位符），并在顶部重建空的 `## [Unreleased]`。
3. **跑门禁**（本仓不跑 `cargo fmt`，见 [`CONTRIBUTING.md`](CONTRIBUTING.md)）：

   ```bash
   pwsh -File scripts/bump-version.ps1 -Check   # 版本号门禁：必须退出码 0（CI 跑同一条）
   cargo test --workspace
   cargo clippy --all-targets -- -D warnings
   cargo check --workspace
   ```

   有素材环境变量（`MIREDIT_ASSET_ROOT` / `MIREDIT_LIB_ROOT`）时，素材用例应实跑通过；没设时打印 `[skip]` 也算绿——但**发布说明里要讲清这次是不是带素材跑过**。
4. **核对版本展示**：
   - 网页：`cargo run -p miredit-web --release` 后 `curl -s http://127.0.0.1:8080/api/health`，`version` 必须等于新版本；
   - 前端：连接状态显示同一版本；
   - 包：`cargo pkgid -p miredit-web` 末段 `#<日期版本>`；
   - GUI：`miredit-gui --version` 必须输出 `Crystal Studio <日期版本>`（源码树用 `cargo run -p miredit-gui -- --version`）；若这一版带「关于」对话框，必须实测它与 `--version` 是同一个版本串，没实测就别写成已核对。
   - Windows exe：文件属性里字符串 `ProductVersion` = 完整 `<日期版本>`，数值版本按 §2.2 拆成 `[年, 月日, 轮次, 修正]`（不溢出）。
5. **干净构建**（不复用工作区增量产物，确保"从零也能编出来"）：

   ```bash
   CARGO_TARGET_DIR=<独立的一次性目录> cargo build --release --workspace
   ```

   产出 `miredit-gui` / `miredit-web` 两个可执行文件；构建目录用一次性的，别塞进仓库树。
   同时导出一份源码包给 Release 用（**从 tag 导出**，保证与发布提交逐字节一致）：

   ```bash
   git archive --format=zip --prefix=CrystalStudio-<日期版本>/ v<日期版本> -o CrystalStudio-<日期版本>.zip
   ```
6. **提交并打 tag**（tag 打在**发布提交**上，不是之前某个提交）：

   ```bash
   git add -A
   git commit -m "发布 v<日期版本>：<一句话摘要>"
   git tag -a v<日期版本> -m "Crystal Studio v<日期版本>"
   git push origin main --tags
   ```

   日期版本没有 `-rc.N` 预发布形态；要验收就按 §2.1 用同日更小轮次，正式发布另起一轮。
7. **GitHub Release**：只在**本仓** `CrystalStudio-Rust`（`https://github.com/lxf505226/CrystalStudio-Rust`）发布，标题 `Crystal Studio v<日期版本>`，正文直接取 `CHANGELOG` 该版小节，末尾附「如何更新」（§七）。**不再向上游 `fluttergo/CrystalStudio` 提 PR、也不在其仓库发 Release。**
8. **Docker 镜像 tag**：镜像构建出来后按 `v<日期版本>` / `latest` 两个标签发布（`crystalstudio/miredit`、`crystalstudio/miredit-front`）。
   ⚠ 现状：`docker/compose.yaml` 用的是 `:dev`，且整套编排**尚未在 CI 或任何机器上实跑验证**（见 [`docker/README.md`](docker/README.md)）。第一次真正出镜像前，这一步只能是"按规则待做"，不许写成已完成。
9. **公告「如何更新」**：Release 正文与公告里给三条路径（源码 / Release 包 / Docker）并指向 §七，明确"要不要更新"的判据：看 `CHANGELOG` 里有没有你需要的那条修复。
10. **收尾复核**：tag 指向的提交上 `/api/health` 版本 = tag 版本；`CHANGELOG` 无占位日期、无"待定"版本号；工作树干净（发布后不再有未提交改动混进同一版本）。

## 七、用户如何更新

### 7.1 源码构建（当前唯一**确定可用**的路径）

```bash
git fetch --tags
git checkout v<日期版本>    # 换成要装的 tag；跟最新开发线就 git pull 停在 main
cargo build --release
```

> ⚠ 这条路要求**那个 tag 已经存在**：本仓的 tag 只按 §六 第 6 步随发布打出（一个版本一个 tag，`v<日期版本>`）。
> tag 还没打出来的版本只能 `git pull` 跟 `main`；`git checkout v<日期版本>` 会报「pathspec did not match」。

产物：`target/release/miredit-gui`（PC 版）、`target/release/miredit-web`（网页版后端，前端在 `web/`）。
需要客户端素材才能出图，环境变量要求见 [`README.md`](README.md) §四/§五；**没配就是如实不可用（503/404），不会画占位图**。

### 7.2 Release 页

到 `https://github.com/lxf505226/CrystalStudio-Rust/releases` 取对应版本。Release **可能附 Windows x64 exe**（从 tag 构建、未签名；校验以 GitHub asset 的 digest 为准，正文不写死附件大小/哈希）；没有附件时正文会写明——**不要假设每次都有二进制**。

### 7.3 Docker

```bash
docker compose -f docker/compose.yaml pull
docker compose -f docker/compose.yaml up -d
```

前提：镜像确实以 §六 第 8 步的版本标签发布过；在编排放到实机之前，这条路径以你本机 `docker/verify.sh` 的结果为准。

### 7.4 看手头是哪一版

| 场景 | 怎么查 |
|---|---|
| 网页版 | 界面右上连接状态显示「后端已连接 · \<版本\>」 |
| 后端进程 | `curl -s http://127.0.0.1:8080/api/health` |
| 启动日志 | 第一行 `[启动] miredit-web v<版本>` |
| 源码树 | `git describe --tags`；或 `cargo pkgid -p miredit-web` |
| PC 版 | `miredit-gui --version` ⇒ `Crystal Studio <版本>`；源码树用 `cargo run -p miredit-gui -- --version` |

### 7.5 想回退

```bash
git checkout <要回到的 tag>     # 例：git checkout v20261107.0.0
cargo build --release
```

- Docker：把 compose 文件里的镜像 tag 改回旧版本再 `docker compose pull && up -d`。
- ⚠ **先备份地图目录**：0.x 阶段的 MINOR 可能带「不兼容变更」（§五），新版写出的 `.map` 未必能被旧版正确读回；回退前把地图整体复制一份，别原地回滚。

## 八、兼容性承诺

| 承诺 | 内容 |
|---|---|
| 同 MINOR 内 | 已支持格式的 `.map` **读写往返字节不变**；同一版本下「打开 → 不改动 → 保存」应逐字节回到原文件（带尾块的地图保持未建模区原样） |
| PATCH | **不得**改变已支持格式的保存字节；改了就说明这是格式/行为口径变化，按 §四 升级并按需列「不兼容变更」 |
| 0.x | 允许在 MINOR 里改格式口径，但必须写进「不兼容变更」并给出处理办法；PATCH 依旧不许动保存字节 |
| 1.0 之后 | 任何改变 `.map` 读写字节、删除格式/CLI/API/env 名的改动只能进 MAJOR |
| 素材缺失时 | 一律如实报错（404/503/状态栏「资源库装载失败」），**不伪造**像素、不生成占位图冒充成功 |

## 九、纪律（这几条不接受例外）

1. **不伪造版本号**：没发布就是没发布，不许提前把 `Cargo.toml` 改成"看得过去"的数字，也不许在文档里写一个与 `Cargo.toml` 不一致的版本。
2. **不写日期占位**：`CHANGELOG` 里的日期只有两种状态——真实发布日期，或该条还在 `[Unreleased]` 下（没有日期）。禁止 `2024-XX-XX`、`待定`、`TBD`。
3. **`[Unreleased]` 是唯一入口**：开发期改动先记这里；发布时整体搬进版本标题，不边发边补。
4. **一版一 tag，一 tag 一提交**：tag 必须指向发布提交本身；发现打错就删 tag 重打（`git tag -d` + `git push origin :refs/tags/v<日期版本>`），不许靠"下一个版本掩盖"。
5. **只在自家仓发布**：`CrystalStudio-Rust` 是本仓唯一发布出口，不再 fork/PR 上游。
6. **文档不写本机路径**：一律相对路径或公开 URL，避免把开发机目录带进公开仓。
