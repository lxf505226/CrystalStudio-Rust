# Crystal Studio · 部署指南

> 四条部署路径的总文档（`docker/README.md` 只覆盖 Docker 专项），每条给「前提 / 步骤 / 验证 / 已知限制」。只写能核对的内容，**没有实测证据的如实标「未验证」**；版本号现读根 `Cargo.toml` 的 `[workspace.package].version`，不锁定。核对日期 2026-11-08。

## 目录

- **一、源码构建** —— 有 Rust 工具链；要 PC 版 + 网页版或改代码
- **二、Release 预编译 exe** —— 只要 PC 版、不装工具链（Windows x64）
- **三、Docker 编排** —— 服务器/内网共享，一个容器装后端 + 静态前端
- **四、NAS（飞牛 fnOS）** —— 家用 NAS 常驻；本仓唯一有实跑记录的一路
- **五、已知限制与未验证项** —— 8 行状态表（4 行已解决 + 4 行未验证）

---

## 一、源码构建

### 1.1 前提

- **Rust stable + cargo**；仓内无 `rust-toolchain.toml`，`Cargo.toml` 无 `rust-version`，CI 用 `Install Rust stable`。
- **解码器已 vendored**：`vendor/wil2atlas/`（MIT，逐文件 SHA256 见 `NOTICE`），`crates/mir2-assets` 用仓库内相对路径 ⇒ **clone 后可直接构建，无外部 path 依赖**。
- **目标平台 Windows**：`miredit-gui` 依赖 eframe/egui 与 Windows 窗口后端；ubuntu runner 缺 X11/Wayland 开发库会构建失败（`ci.yml:15-17`）。
- **`CARGO_INCREMENTAL=0`**（CI 与 `Dockerfile` 都设）：增量编译会 panic `encode_metadata`（`ci.yml:35-36`）。

### 1.2 步骤

```bash
git clone https://github.com/lxf505226/CrystalStudio-Rust.git
cd CrystalStudio-Rust

# 与 CI 相同的三条
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings

# 发布构建
cargo build -p miredit-gui --release
cargo build -p miredit-web --release
```

PC 版（开关：`--map <文件>`、`--map-list-root <目录>`、`--no-window` 配 `--dump-render` / `--edit-*`）：

```bash
cargo run -p miredit-gui --release -- --map /path/to/some.map
```

网页版（后端配置**全部走环境变量**，没有命令行参数）：

```bash
MIREDIT_BIND=127.0.0.1:8080 \
MIREDIT_MAP_ROOT=/path/to/maps \
MIREDIT_CLIENT_ROOT=/path/to/client-root \
MIREDIT_PALETTE_JSON=/path/to/crystalstudio/catalogue/palette.json \
MIREDIT_WEB_ROOT=/path/to/repo/web \
cargo run -p miredit-web --release
```

> PowerShell 不认 `\` 续行：先逐条 `$env:MIREDIT_MAP_ROOT='D:\path\to\maps'` 设完，再跑最后一行。

素材（`README.md` §五，**不随本仓分发**）：地图 `.map` → 网页版 `MIREDIT_MAP_ROOT`、PC 版 `--map` / `--map-list-root`；图库客户端根（`.wil/.wix`、`.wzl/.wzx`、Crystal `.lib`）→ 两版**都**用 `MIREDIT_CLIENT_ROOT`；印章 `catalogue/palette.json` 与并列的 `Objects/*.X` → 网页版 `MIREDIT_PALETTE_JSON`、PC 版 `MIR2_STAMP_PALETTE`。

### 1.3 验证

1. `cargo test --workspace` **0 failed**；未设素材变量时素材用例打印 `[skip]` / `[跳过]` 后返回，所以没素材也全绿（`ci.yml:5-6`）。
2. `cargo clippy --all-targets -- -D warnings` **0 warning**。
3. **版本**：`cargo run -p miredit-gui -- --version` 输出 `Crystal Studio <版本>`，`<版本>` **必须等于**根 `Cargo.toml` 的 `[workspace.package].version`；`cargo pkgid -p miredit-web` 末段 `#<版本>` 也必须一致。
4. **后端版本**：`curl -s http://127.0.0.1:8080/api/health` → 200、含 `"ok":true`，`version` **必须等于**第 3 条那个版本；启动日志第一行 `[启动] miredit-web v<版本>`。
5. **静态页**：`curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/` → `200` 且 `Content-Type` 为 `text/html`。
6. **不伪造数据**：没配素材时 `/api/tiles`、`/api/stamps` 必须 **503**；PC 版状态栏报「资源库装载失败」、图库 0 个且**不 panic**。出现 200 的空白图即失败。

### 1.4 已知限制

- **本项目不使用 `cargo fmt`**（`README.md` §三、`ci.yml:11`）；CI 只跑 test + clippy，不下载素材、不跑 Docker（`ci.yml:10-13`）。
- 四个路径变量**必须显式设置**，不设就是如实 503/404 + 启动横幅点名缺哪个；仓内**没有任何内置路径**指向开发机（`README.md` §四.2）。
- 两版共用同一套素材变量，PC 版没有内置常量根（§4.3）。

---

## 二、Release 预编译 exe

### 2.1 前提

- **Windows x64**；exe **未签名 / GNU 工具链构建**，SmartScreen 可能提示（各 Release 正文原话）。
- 2026-11-08 GitHub API 实查：**12 个 Release 各挂一个 exe**，与 `git tag --list` 的 12 个 tag 一一对应（`v0.0.1` … `v20261107.0.0`）。
- 体积 **16,949,047–17,242,678 B（约 16.9–17.2 MB）**；区间上界是 `v20261107.0.0` = 17,242,678 B。
- 名字形 `CrystalStudio-<版本>.exe`；**唯一例外**是最早的 `v0.0.1`，叫 `CrystalStudio-0.0.1-windows-x64.exe`。
- `VERSIONING.md` §7.2 口径是「**可能**附 …… 不要假设每次都有二进制」：12/12 是**当前事实**，不是永久承诺。exe 只含 **PC 版**，网页后端不在附件里（走 ① 或 ③）。

### 2.2 步骤

```powershell
# <版本> 换成目标版本，如 20261107.0.0；v0.0.1 的文件名要换成 CrystalStudio-0.0.1-windows-x64.exe
curl.exe -L -o "CrystalStudio-<版本>.exe" `
  "https://github.com/lxf505226/CrystalStudio-Rust/releases/download/v<版本>/CrystalStudio-<版本>.exe"
```

或到 `releases/tag/v<版本>` 页手动取。更新 = 下新 exe 替换旧文件；**不要**盖到正在运行的同名文件上。

### 2.3 验证

1. **体积**：`(Get-Item .\CrystalStudio-<版本>.exe).Length` 必须等于该 asset 的 `size`（快照：`20261107.0.0` = `17242678`）。
2. **哈希**：`(Get-FileHash .\CrystalStudio-<版本>.exe -Algorithm SHA256).Hash.ToLower()` 必须等于该 asset 的 `digest`（形如 `sha256:dde4761f…`）—— `VERSIONING.md` §7.2 指定的**唯一校验口径**。
3. **自报版本**：`.\CrystalStudio-<版本>.exe --version` 必须输出 `Crystal Studio <版本>`，且与文件名里的版本一致（判据：`README.md` §三「查手头这份是哪一版」第 1 条）。
4. **能开图**：`--map <某个 .map>` 能开窗渲染；未设 `MIREDIT_CLIENT_ROOT` 时状态栏如实报「资源库装载失败」、图库 0 个、不 panic。

### 2.4 已知限制

- 未签名 + GNU 构建，SmartScreen 可能拦；无安装器/代码签名（`v0.0.1` 正文原话）。
- 只有 **Windows x64**；Linux / macOS 无预编译产物，走 ① 或 ③。
- 附件**不含素材**（`README.md` §五）；正文**不写死**附件大小/哈希（`VERSIONING.md` §7.2）⇒ 大小与 digest 现查 Release API。

---

## 三、Docker 编排

### 3.1 前提

- 宿主机有 `docker` + **compose v2**（`docker compose version` 能跑）与 `curl` —— `docker/verify.sh:104-106` 的硬前置，缺一即 FAIL。
- 构建上下文是**仓库根**：`compose.yaml:24` 写 `context: ..` + `dockerfile: docker/Dockerfile`，`COPY` 全是仓库内相对路径 ⇒ 独立 clone 后目录名无关。
- 三阶段：`build`（`rust:slim-bookworm`）→ `web`（`nginx:1.27-alpine`，可选 `profile: front`）→ **`runtime`（`debian:bookworm-slim`，默认产物，非 root uid 10001）**；裸 `docker build` 产出 `runtime`。

### 3.2 步骤

```bash
cd docker

# 四个素材目录（不指也能起，数据端点会如实 404/503）
export MIREDIT_MAP_DIR=/path/to/maps
export MIREDIT_CLIENT_DIR=/path/to/client-root
export MIREDIT_CATALOGUE_DIR=/path/to/crystalstudio/catalogue
export MIREDIT_OBJECTS_DIR=/path/to/crystalstudio/Objects

# 一条命令验证：构建 → 起 → 探活 → 五项断言 → 收工
./verify.sh              # Windows PowerShell：.\verify.ps1

# 长期起服务（verify 跑完会 down，这步单独执行）
docker compose up -d --build
```

手工构建（在**仓库根**执行）：

```bash
docker build -f docker/Dockerfile --target runtime -t crystalstudio/miredit:dev .
```

可选 nginx 前置层：`docker compose --profile front up -d --build` → `http://<主机 IP>:8081/`。`verify.sh` 开关：`--keep`（验完不停容器）、`--skip-build`（镜像已存在）、`--port 18080`、`--map 41.map`、`--timeout <秒>`。

### 3.3 验证

**判据只有一行**：最后一行 `DOCKER_VERIFY: PASS`、退出码 `0`。内含 5 项断言（`verify.sh:12-18` 与 `verify.ps1:10-15` 相同）：

1. `GET /api/health` → 200 且含 `"ok":true`。**并且** `version` 必须等于根 `Cargo.toml` 的 `[workspace.package].version` —— 脚本只查 `"ok":true`，**版本这层要自己补一道**。
2. `POST /api/map/open`（body `{"path":"<地图文件名>"}`）→ 200 且含 `"width"`。
3. `GET /api/tiles?x=0&y=0&w=16&h=16` → 200，前 8 字节 `89504e470d0a1a0a`（PNG 魔数），字节数 **> 1000**（空白区会 ≤1000，改 `MIREDIT_TILE_W=64 MIREDIT_TILE_H=48`）。
4. `GET /api/stamps?q=market` → 200 且 `total > 0`。
5. `GET /` → 200 且 `Content-Type` 以 `text/html` 开头。

另加一条：`docker compose ps` 的 STATUS 出现 `healthy`（healthcheck 探 `/api/health` 不是 `/`，`compose.yaml:51-57`）。

### 3.4 已知限制

- **本机没有 Docker CLI**（`docker` / `podman` / `nerdctl` 均不存在，`docker/README.md:3-5`）⇒ 本机跑不了 `docker build`；实跑验证在飞牛 NAS 上完成（见 §四，2026-11-08 `BUILD_EXIT=0`）。
- **镜像没推任何 registry**（`CHANGELOG.md:16`）⇒ `VERSIONING.md` §7.3 的 `docker compose pull` 拿不到镜像（该节前提是「镜像确实以版本标签发布过」），现阶段只能本地构建。
- 除地图卷外**全部只读**（`:ro`）⇒ 容器内 `/api/save` 必然 `io_error`（`docker/README.md:114`，有意为之：先只读验渲染）。
- `catalogue/` 与 `Objects/` 必须**并列在同一 dataRoot**（`/data/crystalstudio/`），否则列表仍 200 但每条 `cells` / `bounds` 全 `null`（`docker/README.md:111-113`）。
- 宿主目录不存在时 Docker 建**空目录且不报错** ⇒ 端点如实 404/503（`docker/README.md:115`）。
- `front` 层首启即失败点：nginx 配置加载阶段就要解析 `miredit` 主机名，解析不到**直接退出**；`proxy_pass` **结尾不能带 `/`**（带了会把 `/api/health` 改写成 `/health`、全部 404）（`docker/README.md:134-136`）。

---

## 四、NAS（飞牛 fnOS）

### 4.1 前提

- Linux 宿主机（2026-11-08 实跑用**飞牛 fnOS**），有 `docker` + compose v2；构建需联网拉基础镜像。
- **构建成本（同环境实测）**：Rust 编译 **3m58s**（100 个依赖 crate）+ runtime `apt-get` 约 **8 分钟**，总墙钟约 **13 分钟**，`BUILD_EXIT=0`（`docker/README.md:156`）。
- 素材卷可选；那次**没挂任何素材卷**（`/data/*` 用镜像内空目录）。

### 4.2 步骤

```bash
# 在 NAS 上的仓库根（构建上下文必须是仓库根，不是 docker/）
docker build -f docker/Dockerfile --target runtime -t crystalstudio/miredit:<版本> .
# 产物：crystalstudio/miredit:20261107.0.0（134 MB）

# 起服务走 §三 的 compose
cd docker && docker compose up -d
```

> ⚠ 那次 `docker run` 的完整命令行**没有落档**（`CHANGELOG.md:16` 只记了 `docker build` 与运行后观测）。复现请用 compose —— `ENV` / `HEALTHCHECK` / 非 root `uid 10001` 都写死在 `docker/Dockerfile:155-170`。

镜像搬运（离线）：那次把镜像**导出成 tar 归档**，没推 registry（`CHANGELOG.md:16`）。

### 4.3 验证

2026-11-08 在飞牛 fnOS 上的**实测结果**，可逐条复现：

1. `docker build -f docker/Dockerfile --target runtime` 退出码 **0**（`BUILD_EXIT=0`）。
2. 容器 `Up (healthy)`、`Restarts=0`、CPU 0%、内存 1.33 MiB（`docker/README.md:157`）。
3. `curl -s http://127.0.0.1:8080/api/health` = `{"ok":true,"version":"20261107.0.0"}` —— `version` 必须等于根 `Cargo.toml` 的 `[workspace.package].version`。
4. `/` = **HTTP 200**（29163 B HTML）；`/css/app.css` 与 `/js/ui.js` 均 **200**。
5. 无素材时 `/api/tiles`、`/api/stamps` 如实 **503**（不伪造像素）。

> 只证明「构建成功 + 容器健康 + 静态页与 health 能应答」。**素材卷挂载、`front` 层、`/api/save` 写盘不在其中**，见 §五。

### 4.4 已知限制

- 实跑**未启用** `profile: front`（默认直连后端）、**未挂素材卷** ⇒ nginx 配置、`entrypoint.sh`、uid 10001 读权限、`/api/save` 写盘四项均未验证（逐字状态见 §五后四行）。
- 本文用 `<NAS_IP>` 代替实际内网地址（本仓纪律：文档不写本机路径，`VERSIONING.md` §九 第 6 条）。

---

## 五、已知限制与未验证项

前四行照抄 `docker/README.md:154-157`（已解决项）；后四行照抄 `:158-161`（**未验证项，状态逐字保留**）。

| 项 | 状态 | 说明 |
|---|---|---|
| `compose.yaml` 的构建上下文 | ✅ **已解决（2026-11-06）** | 已改为 **仓库根** `context: ..` + `dockerfile: docker/Dockerfile`；`Dockerfile` 的 `COPY` 也改为仓库内相对路径（`Cargo.toml` / `Cargo.lock` / `crates/` / `vendor/` / `web/`）。独立 clone 后目录名叫 `CrystalStudio-Rust` 还是别的都不影响 |
| `mir2-assets` 的 `wil2atlas` 依赖 | ✅ **已解决** | WIL/WZL 解码器已 vendored 到 `vendor/wil2atlas/`（decoder-only，MIT；来源与逐文件 SHA256 见 `vendor/wil2atlas/NOTICE`），`crates/mir2-assets/Cargo.toml` 已改用仓库内相对路径 `path = "../../vendor/wil2atlas"`，不再依赖开发机绝对路径 |
| `docker build` 能成功 | ✅ **已验证（2026-11-08）** | 飞牛 NAS 实跑：`docker build -f docker/Dockerfile --target runtime`，Rust 编译 **3m58s**（100 个依赖 crate）+ runtime 阶段 `apt-get` 约 8 分钟，总墙钟约 13 分钟，`BUILD_EXIT=0`。产物 `crystalstudio/miredit:20261107.0.0`（134 MB） |
| Linux 二进制能跑（glibc / 依赖） | ✅ **已验证（2026-11-08）** | 同上环境 `docker run` 起容器：`Up (healthy)`、`Restarts=0`、CPU 0%、内存 1.33 MiB；`/api/health` = `{"ok":true,"version":"20261107.0.0"}`、根路径 `/` = **HTTP 200**（29163 B HTML）、`/css/app.css` 与 `/js/ui.js` 均 200 |
| `nginx -t` 能接受这份配置 | ❌ 未验证 | 只做了静态配平与指令拼写核对；2026-11-08 的实跑**未启用** `profile: front`（默认路径直连后端） |
| `entrypoint.sh` 在 busybox ash 下能跑 | ❌ 未验证 | 只做了 `bash -n`；已确认只用 POSIX 构造。同上，`front` 层未启用 |
| 非 root（uid 10001）对挂载目录的读权限 | ❌ 未验证 | 取决于宿主目录权限，必要时 `chown` / `chmod`；2026-11-08 实跑**未挂任何素材卷**（`/data/*` 用镜像内空目录） |
| 容器内 `/api/save` 写盘 | ❌ 未验证 | 地图卷默认 `:ro` ⇒ 保存必然失败（有意为之，先只读验渲染）；实跑同样未挂素材，未触及 |
