# GodM2 地图编辑器 —— Docker 部署

> **一句话结论**：本机（`E:\GodM2` 开发机）**没有 Docker**（`docker` / `podman` / `nerdctl` 全不存在，
> WSL 也未安装发行版）⇒ **镜像从未在本机实跑过**。
> 能给的证据是：**4 项静态校验全通过 + 1 项运行时等价验证通过**（§7、§8.1–8.4）
> **+ 两个验证脚本的断言逻辑端到端实跑通过**（§8.5，`docker` 用垫片替代）。
> 真 Docker 验证请在有 Docker 的机器上跑 **§2 的三条命令**，看到 `DOCKER_VERIFY: PASS` 才算数。

---

## 1. 它是什么

一个容器（服务名 `miredit`）里同时装：

| 组件 | 说明 |
|---|---|
| `miredit-web` | Rust 后端（axum）：8 个 HTTP 端点 + 1 个 WebSocket（`/api/events`） |
| 静态前端 | `tools/miredit/web/`（原生 HTML/CSS/JS，含瓦片夹具） |
| 监听 | `0.0.0.0:8080`（容器内），宿主默认 `8080` |

后端自己就能答 `GET /` 与 `/api/*`，**不依赖 nginx**。nginx 前置层是可选件（§6）。

---

## 2. 在 NAS 上跑（三条命令）

```bash
# ① 进目录（compose.yaml 就在这儿）
cd /vol1/1000/GodM2/tools/miredit/docker

# ② 一条命令验证（构建 → 起 → 探活 → 四项断言 → 收工）
./verify.sh
```

```bash
# ③ 长期起服务（验证通过后再做这步）
docker compose up -d --build
```

**看什么算成功**：

| 步骤 | 成功标志 |
|---|---|
| `./verify.sh` | 最后一行是 **`DOCKER_VERIFY: PASS`**，退出码 `0` |
| `docker compose up -d --build` | `docker compose ps` 的 STATUS 列出现 **`healthy`** |
| 浏览器 | 打开 `http://<NAS的IP>:8080/` → 三栏编辑器界面；顶栏后端徽章为绿色 |

> `verify.sh` 跑完会 `docker compose down`（除非加 `--keep`），所以第 ③ 步要单独执行。

**首次跑之前先确认路径**（默认值假设仓库就在 NAS 上；若地图/图库在别处，用环境变量覆盖）：

```bash
export MIREDIT_MAP_DIR=/vol1/1000/GodM2/GodotClient/maps
export MIREDIT_CLIENT_DIR=/vol1/1000/GodM2/client/热血传奇23周年客户端
export MIREDIT_CATALOGUE_DIR=/vol1/1000/GodM2/CrystalStudio/catalogue
export MIREDIT_OBJECTS_DIR=/vol1/1000/GodM2/CrystalStudio/Objects
```

> ⚠ 目录名含中文（`热血传奇23周年客户端`）。Compose 会原样传给 Docker，**不需要转义**；
> 但如果 NAS 的文件系统不是 UTF-8，路径会失效 —— 此时把该目录软链成纯 ASCII 名字再指过去。

---

## 3. 两个验证脚本

| 脚本 | 平台 | 用法 |
|---|---|---|
| `verify.sh` | Linux / macOS / fnOS（bash） | `./verify.sh` |
| `verify.ps1` | Windows（PowerShell 5.1 / 7） | `.\verify.ps1` |

两者做**完全相同的 5 项断言**，输出同一行判据 `DOCKER_VERIFY: PASS`。

```text
1. GET  /api/health            → 200 且 body 含 "ok":true
2. POST /api/map/open          → 200 且 body 含 "width"
3. GET  /api/tiles             → 200，PNG 魔数 89504E470D0A1A0A，字节数 > 1000
4. GET  /api/stamps?q=market   → 200 且 total > 0
5. GET  /                       → 200 且 Content-Type: text/html
```

常用开关：

```bash
./verify.sh --keep                 # 验完不停容器（排查用）
./verify.sh --skip-build           # 镜像已存在（无源码的部署目录用这个）
./verify.sh --port 18080           # 宿主端口不是 8080
./verify.sh --map 41.map           # 指定要打开的地图（默认自动找 /data/maps 下第一个 .map）
./verify.sh --help
```

```powershell
.\verify.ps1 -Keep
.\verify.ps1 -SkipBuild -Port 18080
Get-Help .\verify.ps1 -Detailed
```

**脚本不读源码**：它只驱动 `docker compose` + `curl` 打已发布端口。
所以可以把 `verify.sh` 单独拷到一台只有 `compose.yaml` 的机器上，配 `--skip-build` 照样能验。

---

## 4. 运行时最小文件集

这一节是**实测**结论（见 §8）：每一条都由「去掉它 → 看端点怎么坏」得出。

### 4.1 镜像自带（必需，已 `COPY` 进镜像，不靠挂载）

| 镜像内路径 | 大小 | 来源 | 去掉会怎样 |
|---|---|---|---|
| `/usr/local/bin/miredit-web` | 10.6 MB | 阶段 `build` 编译 | 容器起不来 |
| `/app/web/`（129 文件） | 16.6 MB | `COPY miredit/web/ /app/web/` | `GET /` → 404 |

`/app/web/` 里就是前端那 129 个文件：`index.html`（19,503 B）、`css/app.css`、`js/*.js`（11 个）、
`runtime-config.js`、`tiles/`（3 张图 × 4 档缩放的真实瓦片夹具 + `manifest.json`）、`README.md`。
`tiles/gen_tiles.py` 在 `RUN` 里被删掉（镜像里不需要 Python）。

### 4.2 必须挂卷（否则端点如实报错，不会伪造数据）

| 宿主目录（默认值） | 容器路径 | 谁需要它 | 不挂的实测结果 |
|---|---|---|---|
| `GodotClient/maps` | `/data/maps` | `/api/map/open` | **404** `{"code":"not_found","message":"文件不存在：/data/maps/0.map"}` |
| `client/热血传奇23周年客户端` | `/data/client` | `/api/tiles`、`/api/palette` | **503** `{"code":"assets_unavailable","message":"图库未就绪：未扫描到任何 Tiles/SmTiles/Objects 库"}` |
| `CrystalStudio/catalogue` | `/data/crystalstudio/catalogue` | `/api/stamps` | **503** `{"code":"assets_unavailable","message":"印章目录不可用：印章目录索引文件不存在：…/palette.json"}` |

> ⚠ **`CrystalStudio/catalogue` 必须挂到 `/data/crystalstudio/catalogue`，不能挂到 `/data/client/catalogue`。**
> 原因：`mir2-stamp` 用 `dataRoot + objectRoot` 定位印章 `.X` 文件，
> `dataRoot` = **`palette.json` 所在目录的父目录**，而 `palette.json` 里写的是 `"objectRoot": "Objects"`。
> 所以 `dataRoot` 必须是 `/data/crystalstudio`（其下并列 `catalogue/` 与 `Objects/`）。
> 挂错位置的后果：`/api/stamps` 仍能 200，但每条的 `cells` / `bounds` 全是 `null`，放置印章会失败。

### 4.3 可选挂载（不挂也能跑，只是能力降级）

| 宿主目录 | 容器路径 | 不挂的实测结果 |
|---|---|---|
| `CrystalStudio/Objects`（9,170 个 `.X`，12.9 MB） | `/data/crystalstudio/Objects` | `/api/stamps` 仍 **200 / total=25**，但 **`cells` 与 `bounds` 全为 `null`**（实测 0/25 有条目能解析格数）⇒ 印章能浏览、不能放置 |

### 4.4 什么都不挂时（只有镜像）

实测（§8 阶段 A）：

| 请求 | 结果 |
|---|---|
| `GET /api/health` | **200** `{"ok":true,"version":"0.1.0"}` |
| `GET /` | **200** `text/html`，19,503 B |
| `GET /css/app.css` | **200** `text/css`，27,170 B |
| `POST /api/map/open` | 404 |
| `GET /api/tiles` | 503 |
| `GET /api/stamps` | 503 |
| `GET /api/palette` | 503 |

⇒ **镜像本身可独立运行**，只是没有数据。这符合「取不到就如实报错，不画假地图」的口径。

---

## 5. 端口、卷、环境变量

| 项 | 默认值 | 覆盖方式 |
|---|---|---|
| 宿主端口 | `8080` | `MIREDIT_PORT=18080` 或 `verify.sh --port 18080` |
| 容器端口 | `8080` | 固定（改它没意义，改映射即可） |
| 地图目录 | `../../../GodotClient/maps` | `MIREDIT_MAP_DIR` |
| 图库目录 | `../../../client/热血传奇23周年客户端` | `MIREDIT_CLIENT_DIR` |
| 印章索引目录 | `../../../CrystalStudio/catalogue` | `MIREDIT_CATALOGUE_DIR` |
| 印章 `.X` 目录 | `../../../CrystalStudio/Objects` | `MIREDIT_OBJECTS_DIR` |
| 健康检查 | `GET /api/health` 每 30s | 改 `compose.yaml` 的 `healthcheck` |

容器内环境变量（`Dockerfile` 的 `ENV`，与后端源码 `state.rs` 一一对应）：

| 变量 | 值 | 说明 |
|---|---|---|
| `MIREDIT_BIND` | `0.0.0.0:8080` | 必须 `0.0.0.0` 才能被 `-p` 映射到 |
| `MIREDIT_MAP_ROOT` | `/data/maps` | `open` 的白名单根（含 `..` 一律拒） |
| `MIREDIT_WEB_ROOT` | `/app/web` | 静态前端目录 |
| `MIREDIT_CLIENT_ROOT` | `/data/client` | 图库扫描根（深度 2，认 `.wil/.wzl` + `.wix/.wzx`） |
| `MIREDIT_PALETTE_JSON` | `/data/crystalstudio/catalogue/palette.json` | 印章索引（`dataRoot` 规则见 §4.2） |

> 宿主目录不存在时 Docker 会**建一个空目录**（不报错）⇒ 端点报 503/404 而不是崩溃。
> 若不想让 compose 碰这些路径，把 `volumes:` 里对应行注释掉即可。

---

## 6. 可选：nginx 前置层

默认编排**不用** nginx。需要 gzip / 静态短缓存 / 不暴露后端端口时：

```bash
docker compose --profile front up -d --build
# 然后访问 http://<IP>:8081/   （MIREDIT_FRONT_PORT 可改）
```

此时 nginx（容器 8080 → 宿主 8081）把 `/api/` 反代到 `miredit:8080`，靠的是：

* `compose.yaml` 的 `MIREDIT_API_UPSTREAM: "http://miredit:8080"`
* `entrypoint.sh` 拿它生成 `location /api/ { proxy_pass <upstream>; … }`
* `depends_on: miredit: condition: service_healthy` —— **必须等后端 healthy 再起 nginx**，
  因为 nginx 在配置加载阶段就要解析 `miredit` 主机名，解析不到会直接退出（不是「稍后重试」）。

> ⚠ `proxy_pass` **结尾不能带 `/`**。带斜杠会把 `/api/health` 改写成 `/health` 转发出去，
> 而后端只有 `/api/health` ⇒ 全部 404。这一条已在 `nginx-default.conf` 里注释标明。

---

## 7. 静态校验（4 项，全部通过）

| # | 查什么 | 怎么查 | 结果 |
|---|---|---|---|
| 1 | `compose.yaml` YAML 解析 + 字段合法性 | PyYAML `safe_load` + 白名单字段 + `context`/`dockerfile`/`target`/`healthcheck`/`ports`/`volumes` 逐项断言 | ✅ 24 项全 PASS |
| 2 | `Dockerfile` 逐行复核 | 解析 `FROM`/`COPY`/`USER`/`EXPOSE`/`HEALTHCHECK`；每个 `COPY` 源按上下文 `tools/` 解析后 `os.path.exists`；`--from=` 校验阶段名 | ✅ 9 个 COPY 源全存在；`runtime` 与 `web` 都有非 root `USER` + `EXPOSE 8080` + `HEALTHCHECK` |
| 3 | `entrypoint.sh` 语法 + 变量默认值 | `bash -n`（GNU bash 5.3）+ 变量引用清单 + bashism 扫描 | ✅ `bash -n` exit 0；无 `[[` / `$(` / `function` / 数组；`${MIREDIT_*:-}` 全有默认值 |
| 4 | `nginx-default.conf` / `api-location.stub.inc` | 花括号配平（忽略注释与引号内）+ 语句首词白名单 + `proxy_pass` 与 entrypoint 生成块比对 | ✅ 两个文件 `depth=0`、引号全闭合；`proxy_pass ${UPSTREAM}` 无结尾斜杠，与 compose 的 upstream 同一变量 |
| 5 | 行尾 / BOM | 逐字节统计 `\r\n` 与 UTF-8 BOM | ✅ 9 个文件全部 `CRLF=0 BOM=False`、末尾有换行 |

补充说明：

* **没有 `nginx -t`**：本机没装 nginx，容器也跑不了 ⇒ nginx 配置**只做了静态配平与指令拼写核对**，
  「nginx 能接受这份配置」这一条**未验证**。这是本单最大的未验证项，见 §10。
* `bash -n` 用的是 bash 5.3（Git for Windows 自带），而容器里 shebang 是 `/bin/sh`（busybox ash）。
  脚本已确认只用 POSIX 构造（`case` / `[ ]` / heredoc / `exec`），但**没有用 dash/ash 实跑过**。

---

## 8. 运行时等价验证（1 项，通过）

**思路**：把「容器里会发生什么」在本机复现 —— 造一个只含「镜像里会有的东西」的模拟根目录，
用 `Dockerfile` 里**完全相同**的环境变量与工作目录启动二进制，然后打端点。

### 8.1 模拟根目录（证明不含源码 / 不含 target）

```text
I:\miredit-docker-sim\
├── app\
│   ├── miredit-web.exe            ← 镜像里的 /usr/local/bin/miredit-web（10,587,131 B）
│   └── web\                       ← 镜像里的 /app/web/（COPY miredit/web/）
│       ├── index.html  css\  js\  runtime-config.js  README.md
│       └── tiles\ …（gen_tiles.py 已按 RUN 删掉）
└── data\                          ← 挂载点（阶段 B/C 才有内容）
```

| 断言 | 结果 |
|---|---|
| 文件总数 | **129** |
| `.rs` 文件数 | **0** |
| `Cargo.toml` 数 | **0** |
| `target/` 目录 | **不存在** |
| `.git` | **不存在** |
| 总体积 | 26.67 MB（二进制 10.1 MB + 前端 16.6 MB） |

启动方式（工作目录 = `/app` 的等价物）：

```powershell
$env:MIREDIT_BIND          = '0.0.0.0:8080'
$env:MIREDIT_MAP_ROOT      = '<sim>\data\maps'
$env:MIREDIT_WEB_ROOT      = '<sim>\app\web'
$env:MIREDIT_CLIENT_ROOT   = '<sim>\data\client'
$env:MIREDIT_PALETTE_JSON  = '<sim>\data\crystalstudio\catalogue\palette.json'
cd <sim>\app ; .\miredit-web.exe
```

> **两处已知偏差**（不影响任何行为判定，如实记录）：
> ① 本机构建出的是 **Windows PE**，不是 Linux ELF ⇒ 验证的是「需要哪些文件 + 端点返回什么」，
>   **不是**「Linux 二进制能不能跑」；
> ② 容器里是 `/app`、`/data/maps`，Windows 上必须换成盘符路径（`<sim>\app`、`<sim>\data\maps`），
>   路径前缀不同、语义相同。二进制在容器里叫 `miredit-web`，Windows 上必须叫 `miredit-web.exe`。

### 8.2 四个端点 + 静态页（阶段 B：挂上全部卷）

**① `GET /api/health`**

```text
{"ok":true,"version":"0.1.0"}
[HTTP 200]
```

**② `POST /api/map/open`**（body `{"path":"0.map"}`）

```text
{"dirty":false,"disk_cell_len":12,"edit_warning":"本格式（Old12（12B/格，TMapInfo 旧版））未解区语义未知，编辑仅保证字节保真，不保证动画/联动正确","editable":false,"format":"Old12","format_label":"Old12（12B/格，TMapInfo 旧版）","ground_paint_supported":true,"height":700,"path":"…\\data\\maps\\0.map","tolerant_trailer_bytes":0,"width":700}
[HTTP 200]
```

含 `"width":700` ✅

**③ `GET /api/tiles?x=0&y=0&w=64&h=48`**

```text
[HTTP 200] content-type=image/png size=4543576
PNG 魔数（前 8 字节）= 89 50 4E 47 0D 0A 1A 0A
```

**与 `layout/refs/render-0.png` 的 sha256 对照**：

| 来源 | SHA-256 | 字节数 |
|---|---|---|
| `/api/tiles` 出的 PNG | `5F122CFA3EF4F590A1EAF652B5F73FA935E590CB775A3F5304D2CC8322343EA7` | 4,543,576 |
| `layout/refs/render-0.png` | `5F122CFA3EF4F590A1EAF652B5F73FA935E590CB775A3F5304D2CC8322343EA7` | 4,543,576 |

**逐字节相同** ✅（这条同时坐实了「`/api/tiles` 与 `render_maps` 走同一条渲染+编码路径」）

**④ `GET /api/stamps?q=market`**

```text
[HTTP 200] size=3265
total = 25，items = 25
首条 = {"bounds":[-1,-2,4,3],"cells":25,"id":"mir/mir_00000","kind":"Building","kind_zh":"建筑","name":"Market stall - mir_00000"}
```

`total = 25 > 0` ✅；`cells` / `bounds` 非 `null` ⇒ **`Objects/` 也确实被读到了**（不只是 `palette.json`）

**⑤ `GET /`**

```text
[HTTP 200] content-type=text/html; charset=utf-8 size=19503
```

### 8.3 阶段 A：只有镜像、什么都不挂

见 §4.4 的表 —— health 与静态页正常，四个数据端点分别 404/503，**不伪造数据**。

### 8.4 阶段 C：挂了 `catalogue/` 但没挂 `Objects/`

```text
GET /api/stamps?q=market → [HTTP 200] total = 25
首条 cells = None   bounds = None
cells 非 null 的条数 = 0 / 25
```

⇒ 坐实 `Objects/` 是「可选但影响能力」：能浏览、不能解析格数/放置。

### 8.5 附：两个验证脚本的断言逻辑实跑（`docker` 用垫片替代）

**这一步验的是脚本，不是 Docker。** 做法：造一个 `docker` 垫片（`compose build/up/down/ps/logs` 返回 0、
`compose exec` 回一个 `/data/maps/0.map`）放到 `PATH` 最前面，然后对**真在跑的**后端跑脚本。

| 用例 | 命令 | 结果 |
|---|---|---|
| 正例（bash） | `verify.sh --skip-build --keep` | `DOCKER_VERIFY: PASS`，退出码 0 |
| 正例（PowerShell） | `verify.ps1 -SkipBuild -Keep` | `DOCKER_VERIFY: PASS`，退出码 0 |
| 负例 1：端口无人监听 | `verify.sh --port 9 --timeout 3` | 健康检查 3s 超时 → 打印容器状态/日志/诊断 → `DOCKER_VERIFY: FAIL`，退出码 1 |
| 负例 2：地图名不存在 | `verify.sh --map nope-not-here.map` | `POST /api/map/open → HTTP 404` + 原始 JSON + 诊断 → `FAIL`，退出码 1 |

正例实测输出（两个脚本逐项一致）：

```text
[ OK ] /api/health 200 → {"ok":true,"version":"0.1.0"}
      将打开地图：0.map
[ OK ] POST /api/map/open 200 且含 width → {"dirty":false,…,"width":700}
[ OK ] GET /api/tiles 200，PNG，519937 字节
[ OK ] GET /api/stamps?q=market 200，total = 25
[ OK ] GET / 200，Content-Type=text/html; charset=utf-8，19503 字节

DOCKER_VERIFY: PASS
```

> 垫片只替代 `docker compose` 的**调度**，四项断言打的都是真后端的真响应
> （`tiles` 那 519,937 字节是真 PNG）。所以它证明的是「脚本的解析/轮询/断言/退出码都对」，
> **不证明**「`docker compose build/up` 能成功」—— 那一条仍在 §10 未验证。

---

## 9. 文件清单

| 文件 | 作用 |
|---|---|
| `Dockerfile` | 三阶段：`build`（rust 编译）→ `web`（nginx 前置层，可选）→ `runtime`（**默认产物**：后端+前端） |
| `Dockerfile.dockerignore` | 把 7.5 GB 的 wil2atlas 产物与 33 MB 参考图挡在构建上下文外 |
| `compose.yaml` | 服务 `miredit`（默认）+ 服务 `front`（profile: front） |
| `nginx-default.conf` | nginx 站点配置：监听 8080、gzip、`/tiles/` 短缓存、`/api/` 由 include 决定 |
| `api-location.stub.inc` | `/api/` 的 503 JSON 兜底（未配反代时） |
| `entrypoint.sh` | 生成运行时配置、可选开反代、`nginx -t` 失败就回退到 503 桩 |
| `verify.sh` | Linux/NAS 一条命令验证 |
| `verify.ps1` | Windows 一条命令验证 |

### 为什么构建上下文是 `tools/` 而不是 `tools/miredit/`

`crates/mir2-assets/Cargo.toml` 里写着：

```toml
wil2atlas = { path = "../../../wil2atlas" }   # → tools/wil2atlas
```

`crates/mir2-assets/` 往上三层正好是 `tools/`。若上下文取 `tools/miredit/`，
cargo 解析依赖时会直接报「failed to load manifest for dependency `wil2atlas`」。

**唯一**越出 `miredit/` 的路径依赖就是这一条（已全量核对 8 个 crate 的 `path =` 声明）。
所以：compose 写 `context: ../..`；手工构建在 `tools/` 下执行：

```bash
cd E:\GodM2\tools
docker build -f miredit/docker/Dockerfile -t godm2/miredit:dev .
```

> ⚠ `Dockerfile.dockerignore` **只有 BuildKit 会读**（Docker 20.10+ / compose v2 默认走 BuildKit）。
> 若设了 `DOCKER_BUILDKIT=0`，上下文会变成 ~7.6 GB（全是 wil2atlas 的产物），构建仍能成功但会慢很多。

---

## 10. ⚠ 仍未验证的（如实列出）

| 项 | 状态 | 为什么 |
|---|---|---|
| `docker build` 真的能构建成功 | ❌ **未验证** | 本机无 Docker |
| `docker compose up` 真能起 | ❌ **未验证** | 同上 |
| Linux 二进制能跑（glibc/依赖） | ❌ **未验证** | 本机只能构建 Windows PE；运行等价验证用的是 Windows 二进制 |
| `nginx -t` 能接受这份配置 | ❌ **未验证** | 本机无 nginx、无容器 |
| `entrypoint.sh` 在 busybox ash 下能跑 | ❌ **未验证** | 只做了 `bash -n`；ash 兼容性靠人工核对 POSIX 构造 |
| 非 root（uid 10001）对挂载目录的读权限 | ❌ **未验证** | 取决于宿主目录权限；若挂载目录对 others 不可读，需 `chown`/`chmod` |
| 22.4 GB 客户端目录的挂载耗时 | ❌ **未验证** | bind mount 不复制，理论无耗时；但 NAS 上首次 `LibPaths::discover` 扫 628 组库要时间 |
| 容器内 `/api/save` 写盘 | ❌ **未验证** | compose 里地图卷是 `:ro` ⇒ **保存必然失败**（有意为之，先只读验渲染）；要能保存需去掉 `:ro` |
| 两个验证脚本在**真 Docker** 下的表现 | ❌ **未验证** | §8.5 只验了脚本的断言逻辑（`docker` 用垫片），没验 `build`/`up` |

**首次在有 Docker 的机器上跑时，请按 §2 的三条命令逐条确认，看到 `DOCKER_VERIFY: PASS` 再认定「可部署」。**
