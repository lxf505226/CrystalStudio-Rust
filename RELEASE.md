# Crystal Studio · 发布方案

> 一次发布 = 版本号 → CHANGELOG → 门禁 → 干净构建 → 源码包 → tag → Release → Packages → NAS → 验收 → 公告。
> 本文是**可执行**的方案：每一步给命令与判据；判据不满足就停下，不往下走。

> **分工**：`VERSIONING.md` 管**版本号规则与兼容性承诺**（§二 格式、§四 递增判据、§八 承诺、§九 纪律）；本文管**一次发布怎么跑完**（门禁、命令、判据、产物、回滚、凭据）。
> 版本号的**数值不抄进本文**，一律现读根 `Cargo.toml` 的 `[workspace.package].version`（`Cargo.toml:26`）。
> 核对时点 **2026-11-08**，基线提交 `7e360ba`。

## 一、版本号方案

### 1.1 当前方案（`2026.11.8+a` 形态）

> 2026-11-08 起生效，**已随 `v2026.11.08a` 发布**（`[workspace.package].version` = `2026.11.8+a`）。

- **格式**：`<年>.<月>.<日>` 首次发布；**同日再发**在末尾加构建元数据 `+a` / `+b` / `+c` …（例：`2026.11.8` → `2026.11.8+a` → `2026.11.8+b`）。
- **为什么不是 `2026.11.08a`**：**实测 cargo 拒绝**。本机 cargo 1.98.1，在 `I:\cargo\tmp` 下逐个建最小 crate 跑 `cargo metadata --no-deps --format-version 1`：

| `Cargo.toml` 里的 version | 实测 | cargo 原话 / 判读 |
|---|---|---|
| `2026.11.8a` | ❌ EXIT=101 | `error: unexpected character 'a' after patch version number` |
| `2026.11.8a0` | ❌ EXIT=101 | `error: unexpected character 'a' after patch version number` |
| `2026.11.08a` | ❌ EXIT=101 | `error: invalid leading zero in patch version number` |
| `2026.11.8` | ✅ EXIT=0 | 合法，正式版 |
| `2026.11.8+a` | ✅ EXIT=0 | 合法（build metadata）—— **采用** |
| `2026.11.8-a` | ✅ EXIT=0 | 合法，但 `-a` 是**预发布**，排序**小于**正式版 `2026.11.8`（实测：要求 `<=2026.11.8-a` 时 `2026.11.8` 不满足，报 `failed to select a version`）⇒ 语义反掉，不采用 |

> ⚠ **显式订正**：原口径把上面三条拒绝都记成 `unexpected character 'a'`。实测 `2026.11.08a` 的报错**不是**这条，而是 `invalid leading zero in patch version number`（根因是 patch 段的**前导零**，与 `a` 无关）。结论不变（都拒绝），但归因不同 —— 别拿它当「`08a` 也行」的依据。

- **tag 形态**：`v2026.11.08a`（**tag 名不受 SemVer 约束**，可用带字母的 tag 形态）；**`Cargo.toml` 里必须写 `2026.11.8+a`**。tag 与 manifest 形态不同是**有意为之**，不是笔误。
- **切换状态（已随 `v2026.11.08a` 落地）**：
  1. `VERSIONING.md` §二已按新格式重写；
  2. `scripts/bump-version.ps1` 的格式校验与递增判据已放宽 build metadata（`-Part same-day|next-day`，不再要求三段数值严格递增）；
  3. `.github/workflows/ci.yml:53-55` 的 `-Check` 门禁跟着改；
  4. `crates/miredit-gui/build.rs` 的 `version_quad` 已把 `+a` / `+b` 折算成第四段 `1` / `2`（同日再发在 Windows 属性里可区分）；
  5. `-Set` 已接受 `2026.11.8+a`，只拒绝预发布形态（含 `-`）。

### 1.2 历史方案（`<YYYYMMDD>.<轮次>.<修正>`）

- 2026-11-07 起用、2026-11-08 被新方案取代：`20261107.0.0` → `20261108.0.0`（`CHANGELOG.md:29`、`CHANGELOG.md:16`）。规则原文见 `VERSIONING.md` §二。
- 旧 tag 与旧 CHANGELOG 小节**原样保留不清理**（`VERSIONING.md:14`、`CHANGELOG.md:6`）：核对时点 `git tag --list` 共 **12 个** tag（`v0.0.1` … `v20261107.0.0`）。

### 1.3 单一真源

- 真源 = 根 `Cargo.toml` 的 `[workspace.package].version`（`Cargo.toml:25-26`）。`crates/` 下 8 个 crate 一律 `version.workspace = true`；`vendor/wil2atlas` 是 vendored 上游解码器，保留自身上游版本、不随本仓 bump（`VERSIONING.md:70-71`、`Cargo.toml:12-16`）。
- **⚠ 必须同时改 `Cargo.lock`**：`bump-version.ps1` 只改 `Cargo.toml` 的一行 + `CHANGELOG.md` 的标题（`scripts/bump-version.ps1:26-30`、`scripts/bump-version.ps1:691-699`），**不碰锁**。而 `cargo pkgid -p <crate>` **读锁** ⇒ 不补锁就永远报旧版本。
- 2026-11-08 实测复现（cargo 1.98.1，虚拟 workspace + `version.workspace = true`，根 version 由 `1.0.0` 改成 `1.0.1`、锁不动）：
  - `cargo pkgid -p a --offline` ⇒ `#1.0.0`（**旧**）
  - `cargo metadata --no-deps --offline` ⇒ `"version":"1.0.1"`（新，读 manifest）
  - 两条命令**不同源**：别拿 `cargo metadata` 的绿当「展示面已核」。
- 修法：`cargo update -w --offline`（实测输出 `Updating a v1.0.0 -> v1.0.1`）；锁更新后 `cargo pkgid` 转 `#1.0.1`。
- 本仓本次的实盘证据：`git diff --numstat -- Cargo.lock` = `8 8`，8 行全是版本行（`Cargo.lock:1460,1469,1473,1477,1485,1495,1510,1521`），`20261107.0.0` → `20261108.0.0`，**无依赖漂移**。判据就是这个形状：**diff 只应出现 version 行**。

### 1.4 展示面（发布后必须逐个核）

| # | 展示面 | 命令 | 判据 | 核对时点实测 |
|---|---|---|---|---|
| 1 | 包自身 | `cargo pkgid -p miredit-web` | 末段 `#<版本>` | ✅ `path+file:///I:/miredit/crates/miredit-web#20261108.0.0` |
| 2 | GUI | `cargo run -p miredit-gui -- --version` | 输出 `Crystal Studio <版本>`（`crates/miredit-gui/src/cli.rs:432-433`） | 未跑（本次只写文档，不构建） |
| 3 | 网页后端 | `curl -s http://127.0.0.1:8080/api/health` | `version` = 新版本（`crates/miredit-web/src/server.rs:92`） | 未跑（本地未起服务） |
| 4 | Windows exe | 文件属性 → 详细信息 | 字符串 `ProductVersion` = 完整版本（`crates/miredit-gui/build.rs:184`） | 未跑（本次未构建 exe） |
| 5 | 在线预览 | `curl -s http://dayz.dpdns.org/api/health` | `version` = 新版本（`README.md:9`） | ⚠ 实测 `{"ok":true,"version":"20261107.0.0"}`，HTTP 200 —— **线上是上一版**，本版尚未上线 |

> 第 5 行是唯一能证明「线上跑的是哪一版」的判据：**版本号对不上就是没发布**。

## 二、发布前门禁（Gate）

**必须全绿才能进入发布步骤。**

| # | 命令 | 判据 |
|---|---|---|
| G1 | `pwsh -File scripts/bump-version.ps1 -Check` | **EXIT=0**（脚本 `exit 0`；CI 跑同一条：`.github/workflows/ci.yml:53-55`） |
| G2 | `cargo test --workspace --no-fail-fast` | **0 failed** |
| G3 | `cargo clippy --all-targets -- -D warnings` | **0 warning** |
| G4 | `cargo check --workspace` | **0 error** |
| G5 | `git status --porcelain` | 只含本次发布该有的文件（发布提交前必须干净） |

补充口径：

- **G2 的规模**：上一轮门禁记录为 23 个 test binary、`miredit-gui` 673 passed + 1 ignored。该数字**来自上一轮记录、本次未复算**；每次发布必须复算，判据只有「0 failed」。
- **素材变量**：没设 `MIREDIT_ASSET_ROOT` / `MIREDIT_LIB_ROOT` 时素材用例打印 `[skip]` 后返回也算绿，但**发布说明必须讲清这次是不是带素材跑过**（`VERSIONING.md:193`、`.github/workflows/ci.yml:73-75`）。
- **本仓不跑 `cargo fmt`**（`VERSIONING.md:184`、`.github/workflows/ci.yml:11`）。
- **G5 现状**：核对时点工作树有 5 个发布准备文件（`CHANGELOG.md` / `Cargo.lock` / `Cargo.toml` / `README.md` / `DEPLOY.md`）与 1 个**非发布准备**改动（`crates/miredit-gui/src/stamp_panel_tests.rs`，`+51/−16`）。后者必须先排除或单独提交，否则发布提交会夹带无关改动。

### flaky 处置纪律（2026-11-08 实测，不接受「单跑绿就算过」）

1. 门禁偶发红**不等于**缺陷：先**单跑该用例** + **单 crate 全量**定性，把「时序 flake」与真缺陷分开。
2. **「单跑绿」不能当结论**：必须**加压到能稳定压出**。实测：8 路并发把 `app::tests::arm之后拿到格表且预览有诊断`（`crates/miredit-gui/src/app.rs:6514`）压到 **8/8 必红**，而原命令 `--workspace` 单跑复现不出来 —— 压不出来就等于没定性。
3. 修法：**把「墙钟超时」换成「事件驱动 + 迭代次数上限」**。墙钟会被 CPU 争抢线性吃掉，迭代次数不会。现盘实现即范例：循环条件只看 `is_loading()` 状态机，上限 `const 轮询上限: u32 = 20_000`（每次 sleep 5 ms，约 100 s）**只用于防挂死并给诊断**，不是判据（`crates/miredit-gui/src/app.rs:6521-6529`）。
4. **反假绿必做**：把判据改回旧代码（墙钟版）必须能复现红；复现不出来说明判据被架空，等于没测。

## 三、发布步骤（10 步，可执行）

> ⚠ **执行顺序订正**：Step 5 的 `git archive ... v<版本>` **依赖 tag 已存在**，所以实际执行序是 **1→2→3→4→6→5→7→8→9→10**。Step 6 的提交只含版本号 / CHANGELOG / 文档，导出源码包不改动提交内容，因此「先提交再导出」不影响「源码包与发布提交逐字节一致」。

### Step 1 定版本号

```powershell
pwsh -File scripts/bump-version.ps1 -Part major|minor|patch   # 新方案生效后改用 -Set（见 1.1 第 5 条卡点）
cargo update -w --offline                                     # 补锁，见 1.3
```

- **判据**：`Cargo.toml:26` 与 `CHANGELOG.md:16` 同步（脚本自己写后会复读复核，`scripts/bump-version.ps1:704`）；`git diff --numstat -- Cargo.lock` 只应是 version 行。
- **失败处置**：脚本 EXIT=1 且**未改动任何文件**（`scripts/bump-version.ps1:644-650`）⇒ 先修前置问题，别手工改；`-Part major` 在同一天会直接拒绝（`scripts/bump-version.ps1:243-252`）。

### Step 2 写 CHANGELOG

- 把 `[Unreleased]` 条目整体归到新版本标题下；小节名**只许用头部那 9 种**：`不兼容变更` / `新增` / `变更` / `弃用` / `移除` / `修复` / `安全` / `已知缺口` / `内部`（`CHANGELOG.md:7`）。
- **不写占位日期**（`VERSIONING.md:291`）。
- **判据**：新版本段有真实日期 + 至少一条变更条目；空段必须人工补写（`scripts/bump-version.ps1:708-711`）。
- **失败处置**：脚本只搬标题、不写条目（`scripts/bump-version.ps1:709`）—— 别指望它替你补内容。

### Step 3 跑门禁

§二 G1–G5 全绿。任一红就停，不进入 Step 4。

### Step 4 干净构建

```powershell
$env:CARGO_TARGET_DIR = 'I:\cargo\scratch\miredit-release-<时间戳>'
cargo build --release --workspace
```

- **判据**：输出里见到 `Compiling` 行（不是秒回）；产出 `miredit-gui.exe` / `miredit-web.exe`。
- **失败处置**：秒回且无 `Compiling` ⇒ 该目录被别的任务预热过（复用别人的 scratch target 会判新鲜），换全新目录重跑；**别拿 exe 哈希当「这是新构建」的判据**。
- 顺手记 SHA256（进 Release 正文）：`(Get-FileHash .\miredit-gui.exe -Algorithm SHA256).Hash.ToLower()`。

### Step 5 导出源码包（在 Step 6 之后执行）

```powershell
git archive --format=zip --prefix=CrystalStudio-<版本>/ v<版本> -o CrystalStudio-<版本>.zip
```

- **判据**：zip 顶层目录名 = `CrystalStudio-<版本>/`；`git rev-parse v<版本>^{commit}` = 发布提交。
- **失败处置**：`fatal: not a valid object name` ⇒ tag 还没打（顺序错了），回 Step 6。

### Step 6 提交 + 打 tag

```powershell
git status                       # 先核对只含本次发布的改动
git add -A
git commit -m "发布 v<版本>：<一句话摘要>"      # 中文六要素
git tag -a v<版本> -m "Crystal Studio <版本>"
git push origin main --tags
```

- **判据**：`git rev-parse HEAD` == `git rev-parse origin/main` == `git rev-parse v<版本>^{commit}`。
- **失败处置**：三者不等 ⇒ tag 打错对象，`git tag -d` + `git push origin :refs/tags/v<版本>` 重打（`VERSIONING.md:293`）。
- **核对时点（2026-11-08）**：当时 HEAD `7e360ba`、`origin/main` `0c50d09`（本地 ahead 2）⇒ 判据当时**不满足**，属发布前的正常态。

### Step 7 GitHub Release

本机**无 gh CLI**（实测 `gh` 不存在）⇒ 走 REST：

```powershell
# token 只从环境变量取，见 §六；上传 asset 用响应里的 upload_url（uploads.github.com）
curl.exe -s -X POST -H "Authorization: Bearer $env:GH_TOKEN" -H "Accept: application/vnd.github+json" `
  https://api.github.com/repos/lxf505226/CrystalStudio-Rust/releases `
  -d "{\"tag_name\":\"v<版本>\",\"name\":\"Crystal Studio <版本>\",\"body\":\"<CHANGELOG 该版段>\"}"
```

- **判据**：`GET /releases/latest` 的 `tag_name` = `v<版本>`；assets 名对得上（exe + zip 两个）；asset 的 `digest` 与本地 SHA256 一致。
- **失败处置**：403 ⇒ token scope 不够；422 ⇒ tag 不存在或 Release 已存在（改 PATCH，见 §五）。
- **历史口径**：核对时点 12 个 Release 各挂 1 个 exe（`DEPLOY.md:83`），asset 名 `CrystalStudio-<版本>.exe`，唯 `v0.0.1` 例外（`DEPLOY.md:85`）；`v20261107.0.0` 的 exe 为 17,242,678 B（`DEPLOY.md:84`）。

### Step 8 GitHub Packages（ghcr.io）

- **镜像名**：`ghcr.io/lxf505226/crystalstudio-miredit:<版本>` + `:latest`；front 层同理 `-front`（现盘是 `crystalstudio/miredit:dev` / `crystalstudio/miredit-front:dev`，`docker/compose.yaml:27`、`docker/compose.yaml:77`）。
- **⚠ 本机无 Docker CLI**（实测 `docker` / `podman` / `nerdctl` 均不存在；`DEPLOY.md:161`、`docker/README.md:3`）⇒ **必须在 NAS 上 build + push**：

```bash
echo "$GHCR_TOKEN" | docker login ghcr.io -u lxf505226 --password-stdin    # token 需 write:packages
docker build -f docker/Dockerfile --target runtime \
  -t ghcr.io/lxf505226/crystalstudio-miredit:<版本> .
docker push ghcr.io/lxf505226/crystalstudio-miredit:<版本>
docker tag  ghcr.io/lxf505226/crystalstudio-miredit:<版本> ghcr.io/lxf505226/crystalstudio-miredit:latest
docker push ghcr.io/lxf505226/crystalstudio-miredit:latest
```

- **判据**：`GET https://api.github.com/user/packages?package_type=container` 里出现该 package；`docker manifest inspect <镜像>:<版本>` 能取到。
- **失败处置**：`denied` ⇒ token 缺 `write:packages`，或该 package 从未建过权限；构建上下文必须是**仓库根**（`docker/compose.yaml:24`）。

### Step 9 NAS 部署

```bash
ssh lxf505226@10.10.10.10
id -nG          # 先复验前提：lxf505226 不在 docker 组
echo '<密码>' | sudo -S docker compose -f /path/to/docker/compose.yaml up -d
```

- **⚠ 前提（作业口径）**：SSH 用户 `lxf505226`，**不在 docker 组 ⇒ 必须 `sudo -S`** 把密码经 stdin 传进去；密码**不落盘、不进命令行历史**。该前提每条命令执行前用 `id -nG` 复验一次。
- **⚠ 必须挂素材卷**：`docker/compose.yaml:42-50` 那四个卷。线上那台当前 `docker inspect -f '{{json .Mounts}}' miredit` 是 `[]`（**空的**），后果是「图能读、画布零精灵」（`README.md:9`）。
- **判据**：`docker ps` 的 STATUS 含 `healthy`；`/api/health` 的 `version` = 新版本；`/api/tiles?x=0&y=0&w=16&h=16` 返回 **PNG**（前 8 字节 `89504e470d0a1a0a`，字节数 > 1000），**不是 503**。
- **失败处置**：`healthy` 但 tiles 503 ⇒ 卷没挂对（`docker/README.md:104-109`）；`catalogue/` 与 `Objects/` 必须并列在同一 dataRoot（`docker/README.md:111-113`）；健康检查探的是 `/api/health` 不是 `/`（`docker/compose.yaml:51-57`）。

### Step 10 验收 + 公告

- 逐个核 §1.4 的五个展示面（含 GUI `--version` 与 exe `ProductVersion`）。
- 在线预览 `http://dayz.dpdns.org/`（`README.md:9`）。
- `DEPLOY.md` §五 那 **8 行**状态表**按本次实际情况更新**（`DEPLOY.md:216-225`）：哪几行从 ❌ 变 ✅ 就改哪几行，没变的一律不动、未验证的仍标 ❌。
- 公告给三条路径（源码 / Release 包 / Docker）并指向 `VERSIONING.md` §七。

## 四、每次发布必须产出的产物清单

| 产物 | 形态 | 判据 |
|---|---|---|
| tag | `v<版本>`，annotated | peel 后 = 发布提交（Step 6） |
| GitHub Release | 1 个，标题 `Crystal Studio <版本>`，正文 = CHANGELOG 该版段 | `/releases/latest` 对得上 |
| Release assets | **2 个**：`CrystalStudio-<版本>.exe` + `CrystalStudio-<版本>.zip` | 名字与 `digest` 对得上 |
| ghcr 镜像 | 2 个 tag（`:<版本>` / `:latest`）；front 层再 2 个 | `/user/packages` 里出现 |
| CHANGELOG 条目 | 新版本段 + 顶部重建的空 `[Unreleased]` | 真实日期、只用那 9 种小节名 |
| `DEPLOY.md` 状态表 | §五 8 行按实况更新 | 未验证的仍标 ❌ |
| NAS 上跑的容器版本 | `/api/health` 的 `version` | = tag 版本 |
| 发布记录 | §七 加一行 | 六列填齐 |

## 五、回滚方案

- **代码回滚**：`git revert <发布提交>`（保留历史）或 `git checkout <上一个 tag>`。
- **Release 回滚**：`PATCH /repos/lxf505226/CrystalStudio-Rust/releases/<id>` 把 `draft` 改成 `true` —— **不要删**，保留审计痕迹。
- **镜像回滚**：NAS 上 `docker compose down` 后把 compose 里的镜像 tag 指回上一个版本，再 `up -d`（`VERSIONING.md:275`）。
- **数据回滚**：**无**。本项目不写用户数据，地图卷是 `:ro`（`docker/README.md:114`）；真要动地图，先按 `VERSIONING.md:276` 整体备份再操作。
- **判据**：回滚后 `/api/health` 的 `version` 必须等于**回滚目标**的版本；`git describe --tags` 指向目标 tag。

## 六、凭据与权限

- **GitHub token**：需要 `repo` + `write:packages`（实测可用 scopes：`repo, write:packages, delete:packages, workflow`）。
  **⚠ 只在本机环境变量里用**（`$env:GH_TOKEN` / `$env:GHCR_TOKEN`），不写进任何文件 / 日志 / 提交信息；用完**建议轮换**。
- **NAS**：SSH 账号 `lxf505226`，**不在 docker 组** ⇒ 用 `sudo -S` 经 stdin 传密码。
- **⚠ 两条纪律**：① 凭据**永不入库**（含 `compose.yaml`、脚本、CHANGELOG、Release 正文）；② 出现在对话 / 日志 / 截图里的凭据，发布后**轮换**。

## 七、发布记录（Release Ledger）

| 版本 | 日期 | tag | Release URL | 镜像 digest | NAS 上的容器 ID | 门禁结论 | 备注 |
|---|---|---|---|---|---|---|---|
| `20261107.0.0` | 2026-11-07 | `v20261107.0.0` | `https://github.com/lxf505226/CrystalStudio-Rust/releases/tag/v20261107.0.0` | 未记录 | 未记录 | 未记录 | asset `CrystalStudio-20261107.0.0.exe`（17,242,678 B，`DEPLOY.md:84`） |
| `20261108.0.0` | 2026-11-08 | `v20261108.0.0` | **本次发布，待填** | 待填 | 待填 | 待填 | 核对时点线上仍是 `20261107.0.0`（§1.4 第 5 行实测） |

> 空表就照上面两行的列填；**「未记录」不许改成猜测值** —— 查不到就写「未记录」。

## 八、纪律（不接受例外）

1. **门禁不全绿不发布**（§二 G1–G5）。
2. **tag 打在发布提交上**，不是之前某个提交（`VERSIONING.md:293`）。
3. **源码包从 tag 导出**，保证与发布提交逐字节一致。
4. **不伪造二进制**：exe 必须是本次干净构建的产物，**附 SHA256**；构建不出来就不挂 exe，正文写明（`VERSIONING.md:247`）。
5. **未验证项如实标 ❌**：`DEPLOY.md` §五 那 8 行不许因为「看着像好了」就改 ✅。
6. **凭据不入库、用完轮换**（§六）。
7. **发布后必须复算**：`/api/health` 版本 = tag 版本、`git status` 干净、tag 与 origin 一致。
