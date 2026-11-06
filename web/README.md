# GodM2 网页版地图编辑器（前端 + 接口契约）

原生 HTML/CSS/JS，**不引任何框架、不引 CDN、不引 npm 包**，字体走系统栈。
后端 `tools/miredit/crates/miredit-web`（axum）**已实现**，本文档按**实际代码**写，
不是草案（对照文件：`crates/miredit-web/src/{json.rs,routes/*.rs,state.rs,error.rs}`）。

- 目录：`tools/miredit/web/`
- 入口：`index.html`（后端 `GET /` 直接 serve 本目录，见 `MIREDIT_WEB_ROOT`）
- 截图：`layout/refs/web-desktop.png`（1440×900）、`layout/refs/web-mobile.png`（375×812）

---

## 1. 目录结构

| 路径 | 行数 | 作用 |
|---|---|---|
| `index.html` | 426 | 骨架：顶栏 / 三栏工作区 / 状态栏 / 抽屉 / 浮层 / **光照三选** / **打开地图对话框（列表 + 手打路径退路）** / 常驻告警条 / 模式提示条 |
| `runtime-config.js` | 8 | 容器运行时注入点（默认空，`entrypoint.sh` 会覆盖） |
| `css/app.css` | 599 | 浅色金主题 + 响应式两档 + 刘海屏安全区（**599/600，新样式别再往里加**） |
| `css/brush-size.css` | 78 | 笔刷尺寸控件（拆出来守住「单文件 ≤600 行」） |
| `css/doors.css` | 30 | 「开启门」复选框 |
| `css/lighting.css` | 41 | 「地图光照」三选按钮（三档互斥；移动端按钮 ≥44px） |
| `css/maplist.css` | 61 | **地图列表**（`GET /api/maps`）：搜索行 / 计数行 / 可滚列表 / 手机底部抽屉 |
| `js/config.js` | 223 | 可覆盖配置、契约常量（含光照三档 / 归一化函数）、工具/图层表、事件总线、调试钩子 |
| `js/api.js` | 370 | HTTP / WebSocket 客户端 + 错误码中文映射 + **光照响应头真值** + **WS 帧解码** + `listMaps` |
| `js/maplist.js` | 209 | **地图列表**：按 500/页拉全量（1097 张 = 3 次请求）、**纯客户端过滤**、点一项走 `map:open-path` |
| `js/tiles.js` | 292 | 瓦片取图 + LRU 缓存（**键 = 图身份 + 出图参数，含 lighting**）+ 在途请求作废 + 像素采样 |
| `js/canvas.js` | 419 | canvas 2D 渲染、**前端四档缩放**、平移、适应地图、**选区框绘制**、光照参数透传 |
| `js/touch.js` | 265 | 指针手势：工具落点 / 平移 / 捏合 / 滚轮（**触摸单指延迟 120 ms 落笔，第二指落下即转捏合**） |
| `js/edit.js` | 145 | 编辑命令队列（串行 + **同一条命令**去重；含印章与七条选区命令） |
| `js/selection.js` | 254 | **矩形选区 + 剪贴板**：拖动框选、Ctrl+C/X/V/Delete、锚点、状态文本 |
| `js/stampmode.js` | 108 | **印章待放置态**：进入/取消、点画布放置、失败原文上顶部错误条 |
| `js/panels.js` | 307 | 资源库列表 / 印章列表（**行内「放置」+ 长按 + Enter**）/ 单元格检查器 |
| `js/ui.js` | 591 | 顶栏、工具、图层、**光照三选**、状态栏实测真值、横幅、告警条、模式提示条、抽屉、打开地图对话框、快捷键 |
| `js/app.js` | 592 | 启动流程、开图、编辑派发、保存/撤销/重做、WS 事件（未知类型计数报警）、画布事件路由 |
| `tiles/**` | — | **历史静态夹具（2026-11-04 生成，当前前端已不再引用）**，保留备查，可安全删除 |

---

## 2. 运行方式

### 2.1 起后端（**环境变量，不是命令行参数**）

```powershell
$env:MIREDIT_BIND       = '127.0.0.1:18080'
$env:MIREDIT_MAP_ROOT   = 'E:\GodM2'          # /api/map/open 的白名单根
I:\miredit-web-target\release\miredit-web.exe
```

| 变量 | 缺省 | 说明 |
|---|---|---|
| `MIREDIT_BIND` | `0.0.0.0:8080` | 监听地址 |
| `MIREDIT_MAP_ROOT` | `GodotClient/maps` | 开图白名单根（相对路径按进程 cwd 绝对化） |
| `MIREDIT_CLIENT_ROOT` | `mir2_assets::DEFAULT_CLIENT_ROOT` | 图库（`.wil/.wzl`）扫描根 |
| `MIREDIT_PALETTE_JSON` | `<仓库根>/CrystalStudio/catalogue/palette.json` | 印章目录索引 |
| `MIREDIT_WEB_ROOT` | `<仓库根>/tools/miredit/web` | 静态前端目录 |

浏览器打开 `http://127.0.0.1:18080/` 即可（前端与 API 同源，无 CORS 问题）。

### 2.2 前端可覆盖配置

| 全局变量 | 默认 | 说明 |
|---|---|---|
| `window.__API_BASE__` | `''` | 后端根，空 = 同源 |
| `window.__MIREDIT_CONFIG__` | `{}` | 对象形式覆盖 `apiBase` / `healthTimeout` / `tileCacheSize` / `lastMapKey` |

### 2.3 Docker

见 `tools/miredit/docker/README.md`（静态前端镜像，`docker compose up` 后访问 8080）。

---

## 3. 接口契约（**按后端实际实现**）

### 3.0 通用约定

- **编码**：JSON 一律 `application/json; charset=utf-8`；`/api/tiles` 返回 `image/png`。
- **路径**：`/api/map/open` 的 `path` 必须在 `MIREDIT_MAP_ROOT` 之内；**前端一律传相对路径**
  （如 `GodotClient/maps/0.map`）。含 `..` 或指向根外 ⇒ **403 `forbidden_path`**。
- **坐标**：世界格整数，左上角 `(0,0)`；`x` 向右、`y` 向下。
- **错误体**：所有非 2xx 一律
  ```json
  { "error": { "code": "no_map", "message": "尚未打开地图" } }
  ```
- **健康判断**：只看 `GET /api/health` 是否 200 + `ok:true`；**不再用它判断图库是否就绪**
  （图库缺失由 `/api/tiles` 的 503 体现）。

---

### 3.1 `GET /api/health`

**200（恒）**
```json
{ "ok": true, "version": "0.1.0" }
```
> 进程能应答就返回 200。图库/印章目录不可用**不算**进程不健康。

---

### 3.2 `GET /api/map`

**200（已打开）** —— 实测原文（`EM001.map`，带 8,040 字节未解尾块）：
```json
{
  "path": "E:\\GodM2\\client\\热血传奇23周年客户端\\Map\\EM001.map",
  "width": 30, "height": 35,
  "format": "Old12",
  "dirty": false,
  "tolerant_trailer_bytes": 8040,
  "format_label": "Old12（12B/格，TMapInfo 旧版）",
  "editable": false,
  "ground_paint_supported": true,
  "edit_warning": "本图有 8040 字节未解释尾块（按头部 30×35 解释）——若真实尺寸不是头部声明值，列优先展开会让整图错位；本格式（Old12（12B/格，TMapInfo 旧版））未解区语义未知，编辑仅保证字节保真，不保证动画/联动正确",
  "disk_cell_len": 12
}
```

**200（未打开）** —— `path` / `format` / `format_label` / `tolerant_trailer_bytes` / `edit_warning` /
`disk_cell_len` / `editable` / `ground_paint_supported` 全为 `null`，`width/height` 为 0。

| 字段 | 类型 | 说明 |
|---|---|---|
| `path` | string \| null | 绝对路径（去掉 Windows `\\?\` 前缀） |
| `width` / `height` | int | 世界格 |
| `format` | string \| null | `Old12` / `New14` / `Ext36` —— **视图档位**（三档语义投影），不是磁盘格长 |
| `dirty` | bool | 有未保存修改 |
| `tolerant_trailer_bytes` | int \| null | **宽松口径下未解释的尾块字节数**；`null` = 算不出来（不写 0 冒充干净） |
| `format_label` | string \| null | **九种格式的真名**（带格长与出处），如 `Shanda14（14B/格，TNewMapInfo）` |
| `editable` | bool \| null | 该格式是否承诺「改格后语义自洽」（九种里只有 `Shanda14` 为 `true`）。⚠ `false` **不等于不能编辑** |
| `ground_paint_supported` | bool \| null | 地表能否**逐格**落笔（九种里只有 `Mir3Wemade`/Type5 为 `false`） |
| `edit_warning` | string \| null | **常驻告警原文**（`null` = 没事）；前端**整句原样上屏**，一个字符都不改写 |
| `disk_cell_len` | int \| null | **磁盘单格字节数**；`null` = 变长格式（`CrystalCs`）；`Wemade2010` 定长 15 B/格 ⇒ `15` |

> ⚠ 未开图时 `editable` / `ground_paint_supported` 是 **`null` 而不是 `false`**：
> `false` 表示「这个格式不支持」，`null` 才是「没有图，谈不上」。混用会让前端把
> "没开图"误报成"该格式不可落笔"（假数据）。

> **`edit_warning` 非空 ⇒ 顶部常驻黄条**（不是 toast，用户不点 × 就一直挂着）：
> 文案 = 后端原文；另加一个**格式标记**（`format_label` 短名 + `editable=false` 时追加
> 「未解区语义未知」），实测标记文本 = `Old12 · 未解区语义未知`。
> 告警的两种来源（未解尾块 / 该格式不承诺编辑语义）可能**同时**成立，后端用「；」连好。

> **`ground_paint_supported === false`（Type5）⇒ 地表图层行加「不可落笔」标记**，
> 且前端在**点画布时就先拦**（不等到后端拒绝）：提示里带 `format_label` 原文，
> 并说明「背景图号按 2×2 块编码，逐格写会被写回时丢弃」。当前编辑层是 `ground` 或
> `low_collision` 时都拦（与内核 `ground_refusal` 同口径）；吸管是只读的，不拦。

> 2026-11-05 起 `Document::load` 走 `mir2_format::read_any_tolerant` ⇒ **67 张带尾块的真实地图
> 现在能打开**（以前 422）。实测 `EM001.map` 200 + `tolerant_trailer_bytes=8040`。

---

### 3.3 `POST /api/map/open`

```json
{ "path": "GodotClient/maps/0.map" }
```

**200** —— 同 §3.2（`dirty` 必为 `false`，撤销栈清空）

| 错误 | 何时 |
|---|---|
| **400 `bad_request`** | `path` 缺失 / 空 |
| **403 `forbidden_path`** | 含 `..` 或指向 `MIREDIT_MAP_ROOT` 之外 |
| **404 `not_found`** | 文件不存在 / 不是普通文件 |
| **422 `parse_failed`** | 文件在，但九种格式**宽松判别**都不命中（带尾块的文件现在能开，不再走这条） |

---

### 3.3.1 `GET /api/maps`（地图列表，**只读元数据**）

```json
{"total": 61,
 "items": [{"name":"0.map","path":"0.map","bytes":5880052,"mtime":1793810990}]}
```

| 参数 | 取值 | 缺省 | 说明 |
|---|---|---|---|
| `offset` | `0..=4294967295` | 0 | 跳过多少条 |
| `limit` | `0..=500` | **100** | 每页条数。**超 500 ⇒ 400**（不静默截断）；`0` 合法（只要 `total`） |

| 字段 | 含义 |
|---|---|
| `total` | **总数**（不是本页条数）——前端靠它显示「共 1097 张」并决定翻页 |
| `items[].name` | 文件名（含 `.map` 后缀，不含目录） |
| `items[].path` | **相对 `MIREDIT_MAP_ROOT`** 的路径（`/` 分隔）—— **可直接喂 `POST /api/map/open`**（与 §3.3 同一套白名单四道闸） |
| `items[].bytes` | 文件字节数（**只 stat，不读内容**） |
| `items[].mtime` | 修改时间，**Unix 秒**；取不到 ⇒ `null`（不写 0 冒充 1970） |

**扫描口径（硬约束）**

| 项 | 取值 | 理由 |
|---|---|---|
| 只收 `*.map` | 扩展名**大小写不敏感**（`A.MAP` 也算） | Windows 上两种写法都可能出现 |
| 递归深度 | **上限 3 层子目录**（`map_root` 自身算第 0 层） | 两个真实目录都是平的（实测 0 个子目录），但换版本可能出现分组目录；不限深度会失控 |
| 符号链接 / 目录联接 | **一律不跟** | 防环，也防白名单根之外的东西混进列表 |
| 只读元数据 | `read_dir` + `file_type` + `metadata`，**零 `read` / 零 `open` / 零解析** | 1097 张 / 731 MB 逐张解析会把服务卡死（实测见 §11.2） |
| 排序 | **文件名自然序**（数字段按数值：`n2.map` 在 `n10.map` 前），同名再按 `path` 字节序兜底 | 结果对同一份目录**恒定**，前端本地过滤与断言才有意义 |
| 条目上限 | 50,000 张；超了 ⇒ **503** | 防病态目录吃干内存，**不返回半截列表冒充完整** |

| 错误 | 何时 |
|---|---|
| **400 `bad_request`** | `limit` > 500、`offset`/`limit` 不是非负整数 |
| **503 `assets_unavailable`** | 根不存在 / 不是目录 / 子目录或条目元数据读不了 —— **绝不返回 200 空列表**（那会让人以为「这个根下真的没有图」） |

**前端怎么用**（`js/maplist.js`）：按 `limit=500` 翻页把 `total` 全拉完（61 张 = 1 次请求、
1097 张 = 3 次），之后**搜索框只在本地过滤、一次请求都不发**；一次最多渲染 300 行
（`scrollHeight` 实测 14,997 / 17,394 px），超出时提示「输入关键词缩小范围」。

---

### 3.4 `GET /api/tiles`（**返回 PNG**）

| 参数 | 取值 | 缺省 | 说明 |
|---|---|---|---|
| `x` / `y` | `0..=2147483647` | 0 | 左上角世界格 |
| `w` / `h` | `1..=64` | 16 | 宽/高（格）。前端固定 16 |
| `scale` | `0.25` / `0.5` / `1` / `2`（也收 `quarter` / `half` / `one` / `two`，**大小写不敏感**） | 1 | 别的值 ⇒ **400**（不四舍五入、不静默降级成 1×） |
| `layers` | 名字列表 `ground,middle,front` **或** 位掩码 `0..=7` | 全开 | `1`=地表 `2`=中层 `4`=物体 |
| `collision` | 名字列表 `low,high` **或** 位掩码 `0..=3` | `0` | `1`=低位 `2`=高位 |
| `doors` | `0` / `1` | `0` | **开启门**：`1` = 门格前景帧号加 `door_offset`（门全开帧）；`0` = 闭门帧，**与改动前逐字节相同**。别的值（含空值 `doors=`、`2`、`true`、`01`）⇒ **400**，不静默当 0 |
| `lighting` | `day` / `dawn_dusk` / `night`（**大小写不敏感**） | `day` | **地图光照**：`day` 不画遮罩、**与改动前逐字节相同**；`dawn_dusk` 遮罩 alpha 205；`night` 遮罩 alpha 255。别的值（含空值、`xyz`、`dawn-dusk`）⇒ **400 + 中文原因**，不静默当白天 |

```
GET /api/tiles?x=0&y=0&w=16&h=16&scale=0.25&layers=ground,middle,front&collision=none&doors=0&lighting=day
```

**200** `image/png`，像素尺寸 **严格等于** `round(w×48×s) × round(h×32×s)`。
越界部分透明；缺帧不画（不画占位块）。响应头三条：
**`X-Render-Scale`** = 实际出图档位（取自 `Rendered.scale`，不是回显请求参数）；
**`X-Render-Doors`** = 本次门口径 `0|1`；
**`X-Render-Lighting`** = 本次光照档位 `day|dawn_dusk|night`（后两条取自真正传给渲染库的
`RenderOptions`，非法值在进渲染前已 400，故回显恒等于生效值）。
三条都列在 `server::EXPOSED_HEADERS`（`x-render-scale, x-render-doors, x-render-lighting`）里，
**跨源时页面 JS 才读得到**。光照状态栏只认 `X-Render-Lighting` 真值，读不到显示「未知（响应头不可读）」，
绝不用请求值冒充。

四档实测尺寸（同一块 `16×16` 格 / `64×48` 格）：

| `scale` | `16×16` 格 | `64×48` 格 | 说明 |
|---|---|---|---|
| `0.25` | 192×128 | 768×384 | 面积加权 `Box` |
| `0.5` | 384×256 | 1536×768 | 面积加权 `Box` |
| `1` | 768×512 | 3072×1536 | **与旧路径逐字节相同** |
| `2` | 1536×1024 | 6144×3072 ⇒ **413** | 最近邻；`64×48@2` 是 72 MiB > 64 MiB 上限 |

| 错误 | 何时 |
|---|---|
| **400 `bad_request`** | `w/h` 越界、`scale` 不在四档内、图层名不认识、`doors` 不是 `0`/`1`、`lighting` 不是三档之一（含空值） |
| **409 `no_map`** | 尚未打开地图 |
| **413 `too_large`** | 出图超限：目标图 > **64 MiB**（`RenderError::TooLarge`）或 1× 中间图 > **128 MiB**（`IntermediateTooLarge`）。**文案含 `want`/`limit` 数字 + 「缩小视口格数或降低倍率」**，且**不硬渲、不降级成 1×** |
| **503 `assets_unavailable`** | 图库不可用（没扫到 Tiles/SmTiles/Objects） |

> **缩小档由服务端出图**（2026-11-05 起）：前端按缩放档请求 `scale=0.25 / 0.5 / 1`，
> 画布对 `scale<1` 的瓦片 **1:1 贴图**（不再 `drawImage` 缩）。理由：25% 档一块 `16×16` 格
> 的 1× PNG 是 768×512（整屏 `64×48` 达 **4.5 MB**），而 `scale=0.25` 只要 **~12 KB**
> —— 体积差一个数量级以上，且 Box 面积加权比浏览器缩放的画质更准。
> **200% 仍请求 `1`**：`2×` 在 `64×48` 会 413，放大由画布最近邻完成（像素画放大不插值）。
> **缓存键 = `<mapToken>|<x,y,w,h,scale,layers,collision,doors,lighting>`**（`js/tiles.js` 的 `keyOf`）：
> 含 `scale` ⇒ 切档会重新取图，切回原档命中缓存（0 新请求）；含 `mapToken` ⇒ **换图后上一张图的
> 瓦片不可能按同一个键落进新图**（`clear()` 自增它，同时在途请求的票一并作废）；
> 含 `doors` / `lighting` ⇒ 勾门、切光照各走一批键（漏任一维就是「切了没反应」，而地图字节全对、极难查）。

---

### 3.5 `POST /api/edit`

请求体固定一层 `cmd`（`miredit_core::Command` 的 **JSON 直译**）：

```json
{ "cmd": { "Paint": { "x": 12, "y": 34 } } }
{ "cmd": "Undo" }
```

**200**
```json
{ "changed": 9, "dirty": true, "note": "落笔 9 格（地表图层）",
  "can_undo": true, "can_redo": false }
```

| 字段 | 说明 |
|---|---|
| `changed` | 真正改了几格（`0` = 没改，原因在 `note`） |
| `dirty` | 操作后是否有未保存修改 |
| `note` | 中文回执/拒绝原因；`null` = 无需说明 |
| `can_undo` / `can_redo` | **按钮禁用态用这两个字段** |

#### `cmd` 变体（**十九条内核命令** + 一条网页侧命令）

| 形态 | 语义 | 典型 `changed` |
|---|---|---|
| `{"SetLayer":{"layer":"middle"}}` | 切当前编辑层；只认 `ground`/`middle`/`front`/`low_collision`/`high_collision` | 0 |
| `{"SetBrushSize":{"size":5}}` | 笔刷边长（夹进 `3..=31` 的奇数） | 0 |
| `{"SetTool":{"tool":"eraser"}}` | **换工具**；`tool` 只认 `brush`/`bucket`/`eraser`/`pickup`（严格小写，**不收别名**） | 0 |
| `{"Paint":{"x":12,"y":34}}` | 按当前笔刷尺寸落一笔（默认 3×3 ⇒ 最多 9 格） | 0..N |
| `{"Stroke":{"from":[10,10],"to":[20,20]}}` | Bresenham 拖动，整条轨迹 = 一步撤销 | 0..N |
| `{"Erase":{"x":12,"y":34}}` | 按当前笔刷尺寸擦一笔（图号层清图号、保留 `bit15`）；**执行后工具留在橡皮** | 0..N |
| `{"Fill":{"x":12,"y":34}}` | BFS 连通填充（不支持物体层） | 0..N |
| `{"Pick":{"x":12,"y":34}}` | 吸管：把该格**当前层**取成笔刷（不改地图） | 0 |
| `{"SetSelection":{"x0":1,"y0":2,"x1":5,"y1":6}}` | 矩形选区（闭区间）；纯 UI 状态，不改图不置脏 | 0 |
| `"ClearSelection"` / `"CopySelection"` / `"CutSelection"` | 取消选区 / 复制 / 剪切（`Copy` 不改图；`Cut` 一步撤销） | 0 / 0 / 0..N |
| `{"PasteSelection":{"x":1,"y":2}}` | 以 `(x,y)` 为左上角粘贴（越界逐格跳过） | 0..N |
| `"DeleteSelection"` / `"FillSelection"` | 清空选区 / 用当前笔刷填满选区（各 = 一步撤销） | 0..N |
| `{"PlaceStamp":{"x":1,"y":2,"stamp":"mir/mir_00000"}}` | 放印章；后端先按 id 读 `.X` 注册格表再逐格贴（整次 = 一步撤销） | 0..N |
| `"Undo"` / `"Redo"` | 撤销/重做**一步**（也接受 `{"Undo":null}`） | 0..N |
| `"Save"` | 等价于 `/api/save` | 0 |
| `{"SetBrushFromPalette":{"family":"object","lib":0,"frame":3}}` | **网页侧命令**（不是 `Command` 变体）；`frame` 是 **0 基**帧索引（图号 = frame+1） | 0 |

> `Paint`/`Erase`/`Fill`/`Pick`/`PlaceStamp` 只吃坐标，**没有** `value` 字段——笔刷内容由
> `Pick` 或 `SetBrushFromPalette` 决定，不是每笔带一个值。

> **`SetTool` 必须显式发**（含"切回画笔"）：`Erase` 执行后内核工具**停在橡皮**
> （与 GUI 的 `--edit-erase` 一致），而 `Paint` 是**按当前工具分派**的 ⇒ 不切回去，
> 擦完再落笔会**继续擦**。前端 `ME.TOOLS[*].toolKey` 已把 `fill`/`pick` 翻成
> `bucket`/`pickup`，**线上词汇表只有一套**。

> `{"SetTool":{"tool":"laser"}}` 这类**非法键**不是 400：内核回 `200 + changed=0 + 中文原因`
> 且不改会话状态（"没认出来"不是"这次调用没完成"）。`PlaceStamp` 的 `stamp` 不存在则是
> **404**（见 §3.11）。

> **`touched_cell()`（内部，仅影响 WS `rerender` 区域）**：`Paint`/`Erase`/`Fill`/`Pick`/
> `PlaceStamp`/`PasteSelection` 报坐标，`Stroke` 报终点；`SetSelection`（矩形装不下一个点）、
> `Clear`/`Copy`/`Cut`/`Delete`/`Fill`（作用在"当前选区"、命令里没有坐标）、
> `SetLayer`/`SetBrushSize`/`SetTool`/`Undo`/`Redo`/`Save` 一律 `None`（= 整屏）。
> 2026-11-05 补齐 `Erase`（此前落进 `_ => None`，改格却只发整屏提示），并把兜底改成**逐条显式**。

| 错误 | 何时 |
|---|---|
| **400 `bad_request`** | `cmd` 结构不对 / 未知变体 / 字段越界 / 层名不认识 |
| **409 `no_map`** | 尚未打开地图 |

---

### 3.6 `POST /api/save`

无请求体（或 `{}`）。原子保存：`tmp → write → fsync → 读回核对 → rename`。

**200**
```json
{ "saved": true, "bytes": 5880052, "note": "[保存] 已写入 …" }
```
> `bytes` 是**保存后磁盘上的真实长度**。没有未保存修改时一个字节都不写（`note` 会说明）。
> **前端必须校验回执**（`app.js` 的 `saveMap`）：`saved === true` 且 `bytes` 是数字才算成功，
> 否则显示「保存未确认 · <原因>」并**不清脏标记** —— 只看 HTTP 200 会被「代理回 200 空体」
> 或将来新增的 `saved:false` 分支骗过去（旧实现就报过「已保存 undefined 字节」）。

| 错误 | 何时 |
|---|---|
| **409 `no_map`** | 尚未打开地图 |
| **500 `io_error`** | 写盘/读回/替换失败（`message` 带阶段名） |

---

### 3.7 `GET /api/palette`

| 参数 | 说明 |
|---|---|
| `kind` | `ground` / `middle` / `object`（`front`、`surface` 是 `object` 的别名），缺省 `ground` |
| `page` | 0 起，缺省每页 200 条（`size` 1..1000） |

**200**
```json
{ "items": [ { "lib": 0, "name": "Tiles", "frames": 31775 } ], "total": 22 }
```

| 字段 | 说明 |
|---|---|
| `lib` | 族内库号（= 地图格里的库号字节） |
| `name` | 磁盘上的库名（`Tiles` / `tiles3` / `Objects21` …） |
| `frames` | 索引表长度；**库打不开 ⇒ `null`**（不猜 0） |

> **item 是「库」不是「图块」**。后端**没有**按库取帧的端点 ⇒ 资源面板只列库，
> 点开写明「图块级待接入」，**不画假图块、不猜图号**。

| 错误 | 何时 |
|---|---|
| **400 `bad_request`** | `kind` 不认识 / `size` 越界 |
| **503 `assets_unavailable`** | 图库未就绪 |

---

### 3.8 `GET /api/stamps`

| 参数 | 说明 |
|---|---|
| `kind` | 分类（英文，精确、大小写不敏感），空 = 全部 |
| `q` | 关键词（只匹配 `id` / `name` 子串），空 = 不过滤 |
| `page` | 0 起，缺省每页 100 条（`size` 1..200） |

**200**
```json
{ "items": [ { "id": "mir/mir_00000", "name": "Market stall - mir_00000",
               "kind": "Building", "kind_zh": "建筑",
               "cells": 25, "bounds": [-1,-2,4,3] } ], "total": 25 }
```

| 字段 | 说明 |
|---|---|
| `cells` | `.X` 解析出的格数；读盘失败 ⇒ `null` |
| `bounds` | `[min_x,min_y,max_x,max_y]` 闭区间；同上 |

> **没有 `preview` 字段** ⇒ 印章列表只用「文字 + 格数 + 范围」展示，**不画缩略图**。
> 放置印章也没有对应命令（`Command` 只有九条变体）⇒ 只做目录浏览。

---

### 3.9 `WS /api/events`（服务端单向推送）

```json
{ "type": "dirty",    "payload": { "dirty": true } }
{ "type": "rerender", "payload": { "x": 0, "y": 0, "w": 1, "h": 1 } }
{ "type": "error",    "payload": { "code": "io_error", "message": "保存失败：磁盘只读" } }
```

| `type` | 前端行为 |
|---|---|
| `dirty` | 更新顶栏星号 + 状态栏 |
| `rerender` | 丢弃相交区域的瓦片缓存并重绘（`w/h = 0` 表示整屏） |
| `error` | 右下角轻提示 5 秒 |

> 断线**不自动重连**，顶部显示「事件通道断开（不影响编辑）」——
> 编辑仍走 HTTP `/api/edit`，只是收不到推送。
> **连接还在、但协议对不上**是另一回事，同样必须报警：JSON 解析不了 / 缺 `type` 的帧会
> 累计 `state.ws.badFrames`；**连续 3 条**未知 `type` 会累计 `state.ws.unknownStreak`；
> 任一触发即显示「事件通道异常」横幅（状态栏同步），文案里带条数与最近一条 `type`/原文。
> 旧实现是 `catch { return; }` + 未知类型只写调试日志 ⇒ 后端改协议时前端完全静默。

---

### 3.10 错误码总表

| `code` | HTTP | 含义 |
|---|---|---|
| `bad_request` | 400 | 参数缺失/越界/枚举不认识（含 `cmd` 结构不对） |
| `forbidden_path` | 403 | 路径逃出 `MIREDIT_MAP_ROOT`（含 `..`） |
| `not_found` | 404 | 文件 / 资源 / 端点不存在 |
| `no_map` | 409 | 尚未打开地图 |
| `parse_failed` | 422 | 文件在，但九种格式宽松判别都不命中；印章 `.X` 解析失败；印章注册被拒（空印章 / 超单枚上限 / 注册表满） |
| `io_error` | 500 | 读写磁盘失败 |
| `assets_unavailable` | 503 | 图库 / 印章目录 / **地图列表（`MIREDIT_MAP_ROOT` 不可用）** |
| `internal` | 500 | 其它内部错误 |

前端对未列出的 `code` 一律展示后端原文 `message`，不猜语义。

### 3.11 `PlaceStamp` 的失败口径（分工：**web 读盘，core 零 IO**）

`{"PlaceStamp":{"x","y","stamp":"<id>"}}` 进来后，后端先按 id 把印章格表**读盘并注册**，
再调内核逐格贴。四类失败各自不同码：

| 情况 | 状态码 | 说明 |
|---|---|---|
| 印章目录不可用（`palette.json` 读不了） | **503** `assets_unavailable` | 资源没就绪，不是调用方的错 |
| id 不在目录里 | **404** `not_found` | 提示用 `GET /api/stamps?q=…` 检索 |
| `.X` 读不了 / 解析失败 | **500** `io_error` / **422** `parse_failed` | 目录说它在、但文件坏了 |
| 印章注册被拒（0 格 / 超 1,000,000 格 / 注册表满） | **422** `parse_failed` | 原因用内核的中文串原文 |
| 放成了但整枚越界 | **200** + `changed=0` + 中文原因 | 内核语义拒绝，不是错误 |

> 同一会话里同一 id 只读一次盘（已注册就直接放）。

---

## 4. 前端行为约定

| 常量 | 值 | 位置 |
|---|---|---|
| 格尺寸 | `48 × 32` 像素（scale=1） | `js/config.js` `ME.CELL` |
| 一次请求的块 | `16 × 16` 世界格 | `ME.CHUNK` |
| 缩放档 | `0.25 / 0.5 / 1 / 2` | `ME.ZOOMS` |
| 取图 `scale` | **随档位算**：25% ⇒ `0.25`、50% ⇒ `0.5`、100% ⇒ `1`、**200% ⇒ `1`**（2× 会 413） | `ME.tileScaleFor(zoom)` |

**工具 → 命令**

| 工具 | 切工具时先发 | 落笔发 | 说明 |
|---|---|---|---|
| 笔刷 `B` | `{"SetTool":{"tool":"brush"}}` | `{"Paint":{"x","y"}}` | 拖动时**按格发**，同一条命令在队列里去重 |
| 填充 `F` | `{"SetTool":{"tool":"bucket"}}` | `{"Fill":{"x","y"}}` | 只在按下那一下发 |
| 吸管 `I` | `{"SetTool":{"tool":"pickup"}}` | `{"Pick":{"x","y"}}` | 回执 `note` 里的图号显示在左栏「笔刷」与状态栏 |
| 橡皮擦 `E` | `{"SetTool":{"tool":"eraser"}}` | `{"Erase":{"x","y"}}` | 拖动时按格发；**切回画笔也必须发 `SetTool`**（否则继续擦） |
| **选择 `S`** | **一条都不发**（`toolKey = null`） | 拖动松手发 `{"SetSelection":{"x0","y0","x1","y1"}}` | 选区**不是内核工具**：`SetTool` 只收 `brush/bucket/eraser/pickup` ⇒ 纯前端工具态 |

> **视觉态**：当前工具 = `aria-checked="true"`（CSS 高亮金边）+ 状态栏 `stTool` + 轻提示
> 「当前工具：橡皮擦（eraser）」（后端回执原文）。
> `ME.TOOLS[*].usable === false` 的工具才会置灰 + 「待接入」角标（当前五个全为 `usable`）。

### 4.1 印章放置（2026-11-05 接入）

| 步骤 | 交互 | 发给后端 |
|---|---|---|
| 进入待放置态 | 印章列表每行「放置」按钮 / 单击行体后按 `Enter` / 手机**长按该行**（550 ms） | 无（纯前端态） |
| 待放置态视觉 | 画布光标 `crosshair`（`#mapCanvas.is-stamping`）+ 顶部提示条<br>`点击画布放置：<印章名>，Esc 取消` + 该行高亮 `.is-armed` | 无 |
| 落位 | 点画布某格 | `{"PlaceStamp":{"x","y","stamp":"<id>"}}` |
| 成功 | 重取瓦片（整屏作废缓存）+ 脏标记亮 + toast 后端 `note` 原文；**仍留在待放置态**（可连放） | 200 |
| 失败 | 顶部**错误条**（`data-tone="error"`）显示后端原文：<br>404 `印章 id 不存在：…` / 409 `尚未打开地图` / 422 `.X` 坏了 / 503 目录不可用；<br>**200 + `changed=0`**（整枚越界 / 闸门）也按原文上条 | — |
| 取消 | `Esc`，或点错误条右侧 ×（若仍在待放置态会把提示条接回来） | 无 |

> 点在地图外 ⇒ 前端提示「点在地图外：请点地图范围内的格子（本次未发送）」，**不发请求**。
> 印章列表**没有缩略图**（`/api/stamps` 无 `preview` 字段）⇒ 只显示文字 + 格数 + 范围，不画假图。

### 4.2 矩形选区与剪贴板（2026-11-05 接入）

| 操作 | PC | 手机（底部抽屉里的按钮） | 命令 |
|---|---|---|---|
| 画选区 | 选「选择」工具后拖动（端点夹在图内） | 同左（抽屉里点「选择」） | `{"SetSelection":{…}}` |
| 复制 | `Ctrl+C` | 抽屉「复制」 | `"CopySelection"` |
| 剪切 | `Ctrl+X` | 抽屉「剪切」 | `"CutSelection"` |
| 粘贴 | `Ctrl+V` | 抽屉「粘贴」 | `{"PasteSelection":{"x","y"}}` |
| 清空选区内容 | `Delete` | 抽屉「删除」 | `"DeleteSelection"` |
| 用笔刷填满选区 | 抽屉「填充」 | 抽屉「填充」 | `"FillSelection"` |
| 取消选区 | `Esc` | 抽屉「取消选区」 | `"ClearSelection"` |

- **选区真值以后端回执为准**：`SetSelection` 会越界裁剪，`note` 形如
  `选区：3×3 格（(216,5)..(218,7)，闭区间）` ⇒ 前端解析出矩形再上屏；
  后端说「已取消选区」（整块在图外）就清空；解析不出来则保留本地矩形（不猜）。
- **画布可见**：金色虚线（`#a8842b`，`setLineDash([7,4])`）+ 半透明填充（`rgba(200,160,60,.22)`）；
  拖动中的预览更淡（`.14`）。
- **状态栏**：`选区 W×H` / `剪贴板 W×H`（剪贴板尺寸取自 `CopySelection`/`CutSelection` 回执的
  `W×H 格`），左栏脚注另显「粘贴锚点：指针所在格（当前 x, y）」。
- **粘贴锚点 = 当前指针（鼠标/触摸）所在格**：取 `ME.canvas.hover`，指针不在画布上时退回
  最近一次所在格；两者都没有就提示「把指针移到目标格再按 Ctrl+V」，**不发请求**。
- **触摸点击没有 `pointermove`** ⇒ `pointerdown` 里也会更新悬停格，否则手机上锚点会停在上一次位置。
- 换图/重开图 ⇒ 后端重建会话（选区与剪贴板随之清空），前端 `ME.selection.reset()` 同步清空本地值。

> **整块类命令要整屏作废瓦片**：WS `rerender` 只带**锚点一格**（`touched_cell` 的 `w/h` 恒为 1），
> 而 `Fill`/`PlaceStamp`/`PasteSelection`/`DeleteSelection`/`CutSelection`/`FillSelection` 一次改一片
> ⇒ `app.js` 的 `REGION_CMDS` 在 `edit:result` 里对这些命令做 `ME.tiles.invalidate(null)`（清空缓存重取）。
> ⚠ `Fill` 是 **BFS 连通填充**，一次可能改满整张图，**必须在表里**（漏了它 ⇒ 点一下填充，
> 除锚点那一块外全屏都是填充前的旧图）。

**告警条（黄条）**：`/api/map` 的 `edit_warning` 非空 ⇒ 常驻显示（不是 toast），
左侧加格式标记（`Old12 · 未解区语义未知`）。点 × 可收起，但**再打开地图会重新出现**
（它不是"一次性通知"，是"这张图的属性"）。

**地表闸门**：`ground_paint_supported === false` 且当前编辑层是 `ground`/`low_collision`
⇒ 地表行显示「不可落笔」，点画布**前端先拦**（不请求后端），提示带 `format_label` 原文。

**图层眼睛** → 重新拼 `/api/tiles` 的 `layers` / `collision` **名字列表**；
缓存键含这两个参数 ⇒ 关掉再打开是**零请求**（命中缓存）。

**「开启门」复选框**（图层组**组头**，`#doorsToggle`）→ 换 `/api/tiles` 的 `doors=0|1`，
**不发任何后端命令**（纯视图选项，不改地图数据）。缓存键含 `doors` ⇒ 勾上换一批键、重新取图，
取消勾选**命中旧块**（0 新请求，但画面确实变回闭门帧）。归一化只有 `ME.doorsParam` 一处真源
（`config.js`）：`1`/`'1'`/`true` ⇒ `'1'`，其余 ⇒ `'0'`，URL 与缓存键同源，不会两套口径。

**快捷键**：`Ctrl+Z` 撤销、`Ctrl+Y`（或 `Ctrl+Shift+Z`）重做、`Ctrl+S` 保存、
`B/F/E/I/S/H` 切工具、`Ctrl+C/X/V` 复制/剪切/粘贴选区、`Delete` 清空选区内容、
`Enter` 把选中的印章推进待放置态、`+/-` 缩放、`0` 适应地图、
`Esc`（优先级：撤待放置态 → 撤选区 → 关抽屉/浮层）。

**鼠标/触摸**：左键（单指）= 当前工具；中键 / `Alt`+左键 / **「平移」工具（`H`，单指拖动）** = 平移；
滚轮 / 双指捏合 = 切缩放档（双指**只**缩放，不负责平移）。
⚠ 触摸单指落笔**延迟 120 ms**（`touch.js` 的 `TOOL_DELAY_MS`）：第二根手指在窗口内落下 ⇒
转捏合、**一条命令都不发**；窗口内位移 >10 px ⇒ 判定为拖动，立刻补发；快速点按在抬手时补发。

### 4.3 「打开地图」对话框：列表为主，手打路径为退路（2026-11-05 接入）

| 步骤 | 交互 | 发给后端 |
|---|---|---|
| 打开对话框 | 顶栏「打开地图…」 | `GET /api/maps?offset=0&limit=500`，若 `total > 500` 继续翻页直到拉完（61 张 = 1 次、1097 张 = 3 次） |
| 列表项 | `文件名` + `大小 · 修改时间`（KB/MB 1024 进制；`mtime` 是 Unix 秒，转本地时间） | 无 |
| 搜索 | 输入框 ⇒ **纯客户端子串过滤**（大小写不敏感，空格分词取交集；同时匹配 `name` 与 `path`） | **无请求**（实测输入 `n0` 前后 `/api/maps` 请求数 3 → 3） |
| 点击一项 | 关对话框 → 走与手打路径**同一条**通路 | `POST /api/map/open {"path": <列表里的 path 原样>}` |
| 搜索框回车 | 打开**第一条**匹配项（无匹配 ⇒ 轻提示，不发空路径） | 同上 |
| 「刷新」 | 丢掉本地缓存重新拉全量 | `GET /api/maps` ×N |
| 手打路径（退路） | 展开 `<details>` 填相对路径 → 「按此路径打开」 | 同上（含 `..` / 根外 ⇒ 403 `forbidden_path`） |
| 失败 | 列表位置显示后端原文（如 503「地图列表不可用：地图根读不到…」），**不显示空列表冒充成功** | — |

> **为什么不在输入时请求后端**：`total` 可能上千（1097），每敲一个字符发一次请求等于
> 每秒钟几十次全目录 stat；而且输入抖动会让结果乱序。拉全量后本地过滤是**零延迟**的。
> **为什么渲染上限 300 行**：1097 个 `<button>` 在移动端会明显拖慢首次渲染；超出时计数行
> 明说「仅渲染前 300 条，输入关键词缩小范围」，不假装全都在。
> **触控目标**：桌面 44 px、手机 52 px（`css/maplist.css`；手机断点里 `#mapList` 是
> 底部抽屉内的滚动容器，`clientHeight 388 / scrollHeight 17394`）。

---

## 5. 仍未接的缺口（前端已显式标注）

| 缺口 | 前端现状 | 依据 |
|---|---|---|
| 图块级列表（按库取帧/取图） | 资源库只列到库级；点开写「图块级待接入」 | `/api/palette` 只有 `{lib,name,frames}` |
| 印章缩略图 | 只显示文字 + 格数 + 范围 | `/api/stamps` 无 `preview` |
| 单元格图号 / 阻挡位查询 | 检查器写「待接入（后端无单元格查询端点）」，只显示真实像素色 | 无该端点 |
| 目录浏览（列 .map） | 打开对话框用文本输入相对路径 | 无该端点 |

> ✅ 已收口（2026-11-05）：**橡皮擦**（`SetTool` + `Erase` 全通）、**多格式开图**
> （九种格式 + 宽松尾块，67 张以前 422 的图现在能开）、**印章放置**
> （行内「放置」/ Enter / 手机长按 ⇒ 点画布落位，失败原文上错误条）、
> **选区 / 剪贴板**（拖动框选 + 金色虚线可见 + Ctrl+C/X/V/Delete/Esc + 手机抽屉按钮 +
> 状态栏 `选区 W×H` / `剪贴板 W×H` + 粘贴锚点 = 指针所在格）。

---

## 6. 本地验收证据（2026-11-05 实测，Windows / Edge 150 headless / 1440×900）

| 项 | 实测 |
|---|---|
| **带尾块开图** | `client/热血传奇23周年客户端/Map/EM001.map` → **200**、`30×35`、`tolerant_trailer_bytes=8040`、`format_label=Old12（12B/格，TMapInfo 旧版）`、`editable=false`、`ground_paint_supported=true`、`disk_cell_len=12`、`edit_warning` 非空 |
| **干净 14B 图** | `GodotClient/maps/41.map` → **200**、`200×200`、`format_label=Shanda14（14B/格，TNewMapInfo）`、**`editable=true`**、`edit_warning=null`、`disk_cell_len=14` |
| **黄条上屏** | 前端打开 `EM001.map`：`#warnBar.hidden=false`、标记 `#warnTag = "Old12 · 未解区语义未知"`、`#warnText` = 后端 `edit_warning` **逐字相同**（8040 / 30×35 / Old12 三处都在） |
| **橡皮擦** | 按钮 `disabled=false`；点击后 `aria-checked="true"`、状态栏 `橡皮擦`、toast `当前工具：橡皮擦（eraser）`；点画布 ⇒ `改动 1 格`、脏标记亮起、`canUndo=true` |
| **保存** | toast `已保存 20692 字节`；保存后脏标记熄灭；**文件长度仍 20692**（8040 字节尾块逐字节保留） |
| **撤销** | 撤销后 `canUndo=false` / `canRedo=true`，脏标记重新亮起（撤销点 ≠ 保存点，语义正确） |
| **逐字节取证** | 擦除后保存的副本 vs 原件：**只差 2 字节**（格 index 578 ⇒ `(x=16,y=18)`，格内偏移 0..1 = 地表图号 `0x8000|N → 0x8000`）；**尾部 8040 字节完全一致** |
| **地表闸门（Type5）** | 本机 **1158 张真实 `.map` 里 0 张 Mir3Wemade**（`mir2-format/src/formats/mir3_wemade.rs:79` 已登记）⇒ 用**注入 `ground_paint_supported=false`** 验证 UI 分支：地表行出现「不可落笔」、点画布**未发出任何命令**（`ME.edit.totals.commands` 不变）、toast 给出带 `format_label` 的中文原因 |
| **编辑闭环（旧基线，仍成立）** | `Pick(340,340)` → 图号 772；`Paint(352,352)` → `changed=9`；保存后磁盘逐格比对**恰好 9 格**；`Ctrl+Z` 后 `dirty=false`；再保存后 sha256 与改前**完全一致** |
| **缩放** | 25/50/100/200% 四档全部可用；~~全部 `/api/tiles` 请求 `scale=1`，**切档零新请求**~~ ⚠ **2026-11-05 多倍率改动后已不适用**：现按档位请求 `0.25 / 0.5 / 1 / 1`，切档重新取图、切回原档命中缓存（见 §3.4） |
| **图层** | 关中层：`layers` 由 `ground,middle,front` → `ground,front`，再打开 **0 新请求** |
| **错误** | 404 `not_found`、403 `forbidden_path`、409 `no_map` 三种都有中文提示；未开图发 `SetTool` ⇒ **409** |

**截图**：`layout/refs/web-desktop.png`（1440×900，sha256 `9271f160d067ba362a5c5968e115583d2f76a4b8d6ce17d9781dec56512e6442`）
—— 可见黄条（`Old12 · 未解区语义未知` + 原文）、橡皮擦高亮可选、真实 EM001 瓦片。

---

## 7. 印章放置 / 选区剪贴板 —— 接入实测（2026-11-05 第二轮，Windows / Edge 150 headless）

环境：`MIREDIT_BIND=127.0.0.1:18080`、`MIREDIT_MAP_ROOT=E:\GodM2`、
`I:\miredit-web-target\release\miredit-web.exe`；被测图 = `GodotClient/maps/0.map` 的**副本**
（700×700 格 / `Old12` / 12 B/格 / 列优先 `idx = x*H + y`），原图全程未动。
所有交互都是 CDP 真实鼠标 / 触摸 / 键盘事件（`Input.dispatchMouseEvent` / `dispatchTouchEvent` / `dispatchKeyEvent`）。

| 门 | 实测 | 磁盘逐字节核对 |
|---|---|---|
| **2 印章放置** | 印章页签搜 `mir_00000` → 点「放置」→ 提示条 `点击画布放置：Market stall - mir_00000，Esc 取消` → 点画布 `(100,100)` ⇒ 后端 `changed=8`，toast 原文 `放置印章「mir/mir_00000」8 格；…物体 的库号字节没有对应字节…` → 保存 `5880052` 字节 | 与原件比 **只差 8 格 / 15 字节**：(99,100)(99,101)(99,102)(100,103)(101,103)(102,103)(103,103)(104,103)，全部落在印章包围盒 `[99..104]×[98..103]` 内；每格只写 `fr_img`（7680…7687），`bk/mid` 一字未动；头 52 字节与尾部完全一致 |
| **2 失败分支** | 用不存在的 id（`no/such`）点画布 ⇒ 顶部错误条 `data-tone="error"`、文本 `放置印章失败 · 文件不存在（404）：印章 id 不存在：no/such（用 GET /api/stamps?q=… 检索可用 id）`（后端 `message` 逐字未改） | — |
| **3 复制粘贴** | 「选择」工具拖 3×3 `(216,5)..(218,7)` ⇒ 状态栏 `选区 3×3`；`Ctrl+C` ⇒ `剪贴板 3×3`、toast `已复制 3×3 格（9 格进剪贴板，含空格）`；鼠标移到 `(200,200)`（状态栏 `单元格 200, 200`、脚注 `粘贴锚点…（当前 200, 200）`）→ `Ctrl+V` ⇒ `changed=9`、toast `粘贴 9 格到 (200,200)` → 保存 | **只差 9 格 / 18 字节**：`(200..202)×(200..202)`；每格 `bk_img` 换成源值（1453→51 等，**目标 `bit15` 保留**）、`fr_img` 因源族为 0 而**不写**；其余 0 差异 |
| **4 剪切** | 同一 3×3 选区 `Ctrl+X` ⇒ `changed=9`（`改动 9 格`）→ 保存 | **只差 9 格 / 9 字节**：源 `(216..218)×(5..7)` 的 `bk_img` 由 `0x8033..0x8037` → `0x8000`（清图号、**保留 `bit15`**），其余 0 差异 |
| **5 待放置态** | 进入：`#modeBar.hidden=false`、`tone=hint`、`#modeBarText` = `点击画布放置：Market stall - mir_00000，Esc 取消`、`#mapCanvas.className=is-stamping`、该行 `is-armed`；`Esc` 取消：`#modeBar.hidden=true`、`armed=null`、`className=""`、行高亮消失 | — |
| **6 移动端** | 375×812（`body.layout-mobile`、`innerWidth=375`）：点「面板」→ 浮层里**长按**印章行 750 ms（真触摸）⇒ 待放置态（同上提示条）；关浮层 → 触摸点画布 `(100,100)` ⇒ `changed=8`、`改动 8 格`；保存 | 与原件比 **只差 8 格 / 15 字节**，与门 2 完全同一批格 |
| **附加** | `Enter`（先单击行选中，再按 Enter）⇒ 进待放置态；`Delete` 键 ⇒ `删除选区内容 9 格（三族按层清）`、`changed=9`（磁盘：源 9 格 `bk` 清零）；移动端抽屉「复制 / 粘贴」按钮 ⇒ 锚点 `(200,200)`、`changed=9`（磁盘：目标 9 格） | 见上 |

**截图**（2026-11-05 多倍率轮重拍，**最新**）：`layout/refs/web-desktop.png` 1440×900，1,321,109 B，
sha256 `81bb8ec20b429e0ac2c05e825e1187e10e6b657c9cad2703f2df26475e7b0c1b`
—— 25% 档（`fit` 700×700 到 1440×900 只能选最小档），状态栏 `25% | 瓦片 scale 0.25 · 42125 B`。

**截图**（上一轮，已被替换，留档）：
- ~~`layout/refs/web-desktop.png` 1440×900，1,074,158 B，sha256 `b504b2286d269ba93d577ba02e8413202e8666a7f62d415c3b73d6b9136daabf`~~
  —— 同框可见：黄条（`Old12 · 未解区语义未知`）、金色提示条（`点击画布放置：Market stall - mir_00000，Esc 取消`）、
  画布上的**金色虚线 3×3 选区**、左栏「选区 3×3 · 剪贴板 3×3 / 粘贴锚点（当前 217, 6）」、状态栏 `单元格 217, 6 | 选区 3×3 | 剪贴板 3×3`。
- `layout/refs/web-mobile.png` 375×812，400,963 B，sha256 `beec01d6462bc0737a8041bdd7ba1d25e123e66baacf203b1b85f2b01b16101f`
  —— 待放置提示条仍在，画布上**已落位的摊位印章**可见，状态栏 `改动 8 格 · 累计 8 格`。

### 7.1 本轮修掉的两个真缺陷（都是**遮挡 / 误命中**类）

1. **移动端右栏遮挡（点不到印章行下半截）**
   `375×812` 搜索印章后实测：`.group--fill` 用 `flex: 1 1 auto` ⇒ 内容长的「资源库」组抢走空间，
   「印章」组被压到 **115 px**（内容 257 px），`#stampList` 溢出到下面的「单元格检查器」组底下
   （列表 599…689 vs 检查器 633…804）⇒ 后画的组**盖住并抢走点击**：真鼠标/真触摸在 `y=632`
   命中的是 `group` 而不是印章行（`elementFromPoint` 却说是行 —— 输入管线与布局不一致）。
   修法：`.group--fill { flex: 1 1 0; overflow: hidden; }`（两组平分 + 子元素不许溢出），
   `.listbox { min-height: 48px }`。修后两组各 278 px，列表 436…534 完全在组内，长按恢复正常。
2. **粘贴锚点会粘错格（触摸）**
   ① `pointerleave` 用 `setHover(-1,-1)` 清悬停 ⇒ 视口原点在图内时（`ox=196.09, oy=190.97`）
   反算出来的正是**左上角那一格**，锚点被钉死在 `(196,190)`；改成新增的 `ME.canvas.clearHover()`。
   ② 触摸点击**不产生 `pointermove`** ⇒ 悬停格不更新，锚点停在上一次位置；
   改成 `pointerdown` 里也 `setHover()`。修后实测：点 `(200,200)` ⇒ 锚点 `(200,200)`、`粘贴 9 格到 (200,200)`。

---

## 8. 多倍率渲染接线 —— 实测（2026-11-05 第三轮，Windows / Edge 150 headless + CDP）

被测：`I:\miredit-web-target\release\miredit-web.exe`（`MIREDIT_BIND=127.0.0.1:18080`、
`MIREDIT_MAP_ROOT=E:\GodM2`）、`GodotClient/maps/0.map`（700×700 / Old12）；浏览器 1440×900 / dpr 1。

### 8.1 服务端四档（同一块 `64×48` 格，`layers=ground,middle,front`）

| `scale` | HTTP | `Content-Length` | 像素 | 与 `round(w*48*s) × round(h*32*s)` | `X-Render-Scale` |
|---|---|---|---|---|---|
| `0.25` | 200 | **309,025** | 768×384 | 768×384 ✅ | `0.25` |
| `0.5` | 200 | **1,309,711** | 1536×768 | 1536×768 ✅ | `0.5` |
| `1` | 200 | **4,543,576** | 3072×1536 | 3072×1536 ✅ | `1` |
| `2`（`16×12` 格） | 200 | **353,100** | 1536×768 | 1536×768 ✅ | `2` |

- **`scale=1` 逐字节不变**：sha256 = `5f122cfa3ef4f590a1eaf652b5f73fa935e590cb775a3f5304d2cc8322343ea7`，
  与 `layout/refs/render-0.png` **完全相同**（4,543,576 B 也一致）⇒ 与改动前的 `/api/tiles` 同一个产物。
- **体积比（同一块 64×48 区域）**：4,543,576 ÷ 309,025 = **14.7 倍**（0.5 档是 3.5 倍）。
  注意这**不是** 37 倍：37 倍是按「4.5 MB vs 124 KB」估的，而 124 KB 是**单块 16×16 格**的量级，
  64×48 的 0.25 图（768×384）实测 309 KB。以实测为准。

### 8.2 超限 / 非法（原文照抄）

`scale=2` + `64×48` ⇒ **HTTP 413**：

```json
{"error":{"code":"too_large","message":"出图超限（64×48 格 @ scale=2）：缩放输出需要 75497472 字节（72.0 MiB），超过上限 67108864 字节（64 MiB）：请缩小视口格数或降低倍率（不硬渲，也不静默降级成 1×）"}}
```

`scale=3` ⇒ **HTTP 400**：

```json
{"error":{"code":"bad_request","message":"scale 只支持 0.25 / 0.5 / 1 / 2（也可写 quarter / half / one / two，大小写不敏感），收到：3（不做四舍五入，也不静默降级成 1×）"}}
```

`scale=0.3` / `scale=2x` 同样 400（**不吸附**到 0.25 / 2）。

> ⚠ **诚实缺口**：`IntermediateTooLarge`（1× 中间图 > 128 MiB）在 HTTP 层**打不到** ——
> 它要求 `原宽×原高 > 33,554,432 px`（如 `200×150` 格），而本端点 `w/h ≤ 64`，最大 `64×64`
> 的中间图只有 25.2 MB。该映射已实现并被单测钉住（直接构造 `RenderError`），但**没有真实 HTTP 复现**。

### 8.3 前端四档：`/api/tiles` 的 `scale` 序列与字节数

`ME.tileScaleFor(缩放档)`：25% ⇒ `0.25`、50% ⇒ `0.5`、100% ⇒ `1`、**200% ⇒ `1`**（2× 会 413）。

| 切到 | 请求 `scale` | 累计请求数 | 本档新增 | 本档瓦片字节（`ME.tiles.stats.bytes` 增量） | 单块均值 |
|---|---|---|---|---|---|
| 25%（开图默认） | `0.25` | 48 | +48 | 1,922,775 B | **40,058 B** |
| 50% | `0.5` | 64 | **+16** | 3,722,227 B | 232,639 B |
| 100% | `1` | 68 | **+4** | 3,035,473 B | 758,868 B |
| 200% | `1` | 68 | **+0** | 0 | —（复用 1× 缓存，画布最近邻放大） |
| 回 25% | `0.25` | 68 | **+0** | 0 | —（**48 块全部命中缓存**，`hits` 238→290） |

**单块体积比 25% : 100% = 758,868 ÷ 40,058 ≈ 19 倍**（一个数量级 ✅）。

> ⚠ **口径提醒（别拿它当「整屏省 19 倍」）**：25% 一屏要 48 块（覆盖 192×192 格），
> 100% 一屏只要 4 块（覆盖 64×64 格）。**同一屏**的绝对字节是 1.92 MB vs 3.04 MB（1.6 倍）；
> 折算到**同一块世界面积**才是 14~19 倍（192×192 格在 100% 档需要 9×4=36 块 ≈ 27 MB）。

### 8.4 缓存键 = 图身份 + 出图参数

键 = `<mapToken>|<x,y,w,h,scale,layers,collision,doors>`（`js/tiles.js` 的 `keyOf`）。`scale` 段实测请求数
`48 → 64 → 68 → 68 → 68`（切档 +16/+4，200% 与切回原档 **+0**），
`hits` `190 → 233 → 238 → 242 → 290`。CDP 网络事件数与 URL 去重数**逐档相等**（48/48、64/64、68/68）
⇒ 没有重复请求。LRU 容量 96 → **160**（四档共用一份缓存，太小会把别的档挤掉）。
`mapToken` 段（2026-11-05 加）：`clear()` 自增它 ⇒ 换图后旧键、旧在途请求全部作废；
`invalidate()` 自增该键的序号 ⇒ 撤销/放印章后，在途的旧块回来也会被丢弃（`stats.dropped` 计数）。

### 8.5 画布

`scale<1` 时目标矩形与位图尺寸**逐像素相等**（1:1 贴图，不再 `drawImage` 缩）；
`imageSmoothingEnabled` 恒 `false`（唯一需要缩放的是 200% 放大，像素画放大不许插值）。
状态栏新增 `#stTile`：`瓦片 scale <请求档> · <上一块 PNG 字节数> B`；若响应头档位与请求不符会显式写
`（与请求不符）`（刚切档、上一块还是别的档位时的真实状态，不冒充）。

---

## 9. 「开启门」接线 —— 实测（2026-11-05 第四轮）

被测：`I:\miredit-web-target\release\miredit-web.exe`（17:39:36 / 11,083,672 B）、
`MIREDIT_BIND=127.0.0.1:18081`、`MIREDIT_MAP_ROOT=E:\GodM2`、`GodotClient/maps/0.map`（700×700 / Old12）；
浏览器 Edge 150.0.4078.65 headless + CDP（桌面 1440×900 / dpr 1，移动 375×812 / dpr 2）。

### 9.1 服务端：缺省**逐字节不变**，`doors=1` 真的不同

同一块 `x=106&y=316&w=3&h=3&scale=1`：

| 请求 | HTTP | 字节 | sha256 | `X-Render-Doors` |
|---|---|---|---|---|
| **缺省**（不给 `doors`） | 200 | **32,842** | `e1acd63898d279e2daaf64e18ffa99114a64474622f68eee7aae069416284335` | `0` |
| `doors=0` | 200 | **32,842** | 同上（**逐字节相同**） | `0` |
| `doors=1` | 200 | **32,812** | `3e59dd7355e4df2091749487a4930406450dd74c6910bccc9f9b6fefd1509b8d` | `1` |

- 「缺省」那一行与**改动前的二进制**（本轮开工前先编好、先取的基线）产物**完全相同**：同样
  `32,842 B / e1acd638…4335`。
- 大区域复核：`64×48 @1×` = **4,543,576 B / `5f122cfa…3ea7`**，与 §8.1 上一轮基线**逐字节相同**
  ⇒ 默认出图一个字节都没动。
- `doors=1` 与闭门尺寸相同（144×96 px）、像素不同（同区域 3×3 格内含门格）。

### 9.2 非法值 ⇒ 400（原文照抄）

`doors=2`：

```json
{"error":{"code":"bad_request","message":"doors 只支持 0（闭门，默认）或 1（门全开），收到：2（不静默当成 0）"}}
```

同样 400 的还有 `doors=`（空值，文案写「收到：空值」）、`doors=true`、`doors=01`。
**`?doors=` 与「没给」是两件事**：前者走校验 ⇒ 400，后者才是缺省 `0`
（实现用 `Params::raw_verbatim`，只 trim 不过滤空串；`raw` 的「空串按未给」口径仍用于 `layers`/`collision`）。

### 9.3 跨源头暴露

```
$ curl -s -D - -o NUL -H "Origin: http://localhost:5173" \
    "http://127.0.0.1:18081/api/tiles?x=106&y=316&w=3&h=3&scale=1&doors=1"
HTTP/1.1 200 OK
x-render-scale: 1
x-render-doors: 1
access-control-allow-origin: http://localhost:5173
access-control-expose-headers: x-render-scale, x-render-doors
```

无 `Origin`（同源）时**两条 CORS 头都不出现**（不泄露给非白名单来源）。

### 9.4 前端：勾选 ⇒ 重取 + 画面变；取消 ⇒ 变回（CDP 实测）

| 步骤 | `ME.tiles.stats.requests` | 画布 sha256（`toDataURL`） | `checked` / `ME.canvas.doors` |
|---|---|---|---|
| 开图后（闭门） | 48 | `d11766de4ab27be7` | `false` / `0` |
| **勾选「开启门」** | **96（+48）** | `ed8141b610052f52`（**变了**） | `true` / `1` |
| **取消勾选** | **96（+0，命中旧块）** | `d11766de4ab27be7`（**逐字节变回**） | `false` / `0` |

- 缓存键实测：`2|304,288,16,16,0.25,ground,middle,front,none,1` —— 第 8 段就是 `doors`；
  勾选后 `keys` 48 → 96（两批键并存，取消勾选直接命中旧批）。
- 请求 URL 实测：`…&scale=0.25&layers=ground%2Cmiddle%2Cfront&collision=none&doors=0|1`
  （同几何、同档位，**只差 `doors`**）；`doors=0` 96 条 / `doors=1` 48 条。
- 页面异常 0 条（`Runtime.exceptionThrown` 全程空）。

### 9.5 移动端 375×812：复选框真的点得到

| 量 | 值 |
|---|---|
| 「开启门」整行（`#doorsRow`） | **351 × 50 px** |
| 复选框（`#doorsToggle`） | **44 × 44 px**（`@media (max-width: 767.98px)` 里放大，桌面 16×16） |
| `elementFromPoint`（行中心 / 框中心） | `LABEL#doorsRow` / `INPUT#doorsToggle` |
| 真触摸 tap（`Input.dispatchTouchEvent`） | `checked=false → true`、`doors 0 → 1`、画布 `25e53fda…` → `3faba784…` |

### 9.6 既有布局债：左栏内容超出可视区（**不是本功能引入**，也**没在本次修**）

**现象**：桌面 1440×900 下左栏 `#panelLeft` 的 **`scrollHeight 935` / `clientHeight 820`**
（`overflow-y: auto`）⇒ 内容比可视区高 **115 px**。

**根因**：这是**既有布局债**，与本功能无关 —— **加「开启门」这一行之前就已经超 73 px**
（内容 893 / 可视 820）；本行占 42 px（36 行高 + 6 外边距），加完 935（超 115 px）。

**本次怎么处理的**：把「开启门」从**组尾**挪到图层组**组头**（`#layerList` 之前）——
组尾时它落在 `y≈858..894`，被底部状态栏（`y=872..900`）盖住，未滚动时
`elementFromPoint` 返回 `FOOTER#statusbar`（**标准桌面分辨率下一个控件初始点不到，等于坏了**）。
挪到组头后（**不滚动**，`scrollTop=0`）实测：

```
DOM 顺序：H2.group__title → LABEL#doorsRow.doors → UL#layerList → P#layerFoot → P.group__foot
#doorsRow  y=654..690（状态栏 872..900，不再重叠）
elementFromPoint(行中心) = LABEL#doorsRow     elementFromPoint(框中心) = INPUT#doorsToggle
未滚动真鼠标点击 ⇒ checked=true / ME.canvas.doors='1' / 瓦片请求 48 → 96
左栏 scrollHeight 仍是 935（**挪位置不改变总高度**，只改变哪一块被裁掉）
```

> **待办：左栏内容超出可视区 115 px，需一次布局收敛**（减条目 / 分组折叠 / 加滚动提示），
> **不属本次范围**。挪位置只是让「开启门」可达，**不解决溢出**；
> 也**不许**为了让这个数字好看去删别的控件或缩小触控目标（移动端 44×44 是硬底线）。




---

## 10. 地图光照接线 —— 实测（2026-11-06，Windows / HTTP 字节验收）

> 被测：`I:\miredit-web-target\release\miredit-web.exe`、`MIREDIT_BIND=127.0.0.1:18082`、
> `MIREDIT_MAP_ROOT=E:\GodM2`、真图 `GodotClient/maps/0.map`（700×700 / Old12）。
> 哈希一律 `Get-FileHash -Algorithm SHA256`，PNG 由 `curl.exe -o` 原样落盘。

### 10.1 默认逐字节不变 / 三档互异 / 门态不丢

| 请求 | HTTP | 字节 | sha256 | 实测响应头 |
|---|---:|---:|---|---|
| `x=106&y=316&w=3&h=3&scale=1&doors=0`（不给 lighting） | 200 | **32,842** | `e1acd63898d279e2daaf64e18ffa99114a64474622f68eee7aae069416284335` | `x-render-lighting: day` |
| `x=0&y=0&w=64&h=48&scale=1&doors=0`（不给 lighting） | 200 | **4,543,576** | `5f122cfa3ef4f590a1eaf652b5f73fa935e590cb775a3f5304d2cc8322343ea7` | `x-render-lighting: day` |
| 同 3×3 区域 `lighting=day` | 200 | **32,842** | 同基线 `e1acd638…4335` | `day` |
| 同 3×3 区域 `lighting=dawn_dusk` | 200 | **26,426** | `072ecaedb33c8cd80fde32f41758d300e880433c8f1bbf2eb1518f358a1428c2` | `dawn_dusk` |
| 同 3×3 区域 `lighting=night` | 200 | **301** | `3a8614515d9c4d3dc97bb4a0a5bbd201ec34fa75dbbfdd5ba919c99084a3b4bc` | `night` |
| 同 3×3 区域 `lighting=DAWN_DUSK` | 200 | **26,426** | 同小写档 `072ecaed…28c2` | `dawn_dusk` |

⚠ `x=106&y=316` 的 3×3 视口**没有发光格**：夜晚是纯黑遮罩，不能拿它证明门态不丢。
改用有发光格的 `(240,250)` 64×48 视口，三份 PNG 互不相同：

| 请求 | 字节 | sha256 | 响应头 |
|---|---:|---|---|
| `lighting=night&doors=1` | **3,251,423** | `fcdb843cc30338a3e10e9282e3de338e43cfc78a408a197718c49984c1e861b2` | `doors: 1 / lighting: night` |
| `lighting=night`（只夜间） | **3,251,937** | `70e5f5a731a73f846093bf278d6f6dc81cfb2e33902d70cdecdd81db5532e343` | `doors: 0 / lighting: night` |
| `doors=1`（只门全开） | **9,144,728** | `e516d0a0d4819d25ea24a80b0b929cedffee5c40370b867b0ac0edb0563abd05` | `doors: 1 / lighting: day` |

⇒ `doors` 与 `lighting` 由 `RenderOptions::with_doors_and_lighting` 一次组合，两个开关互不覆盖。

### 10.2 非法值 400 / 跨源头暴露

`?lighting=xyz` 的 400 原文：

```json
{"error":{"code":"bad_request","message":"lighting 只支持 day（白天，默认）/ dawn_dusk（黎明·黄昏）/ night（夜晚），收到：xyz（不静默当成 day；大小写不敏感）"}}
```

`curl -H "Origin: http://localhost:5173"` 实测：

```text
HTTP/1.1 200 OK
access-control-allow-origin: http://localhost:5173
access-control-expose-headers: x-render-scale, x-render-doors, x-render-lighting
```

### 10.3 前端 CDP：切三档各重取、画布哈希互异

> 无头 Edge 150.0.4078.65，桌面 1440×900 / dpr 1，URL `http://127.0.0.1:18082/`。
> 画布 sha256 取 `canvas.toDataURL('image/png')` 的前 16 位；请求数取 `ME.tiles.stats.requests`。

| 步骤 | `/api/tiles` 请求数增量 | 画布 sha256 | 缓存键数 | 状态栏实测光照 | 最后一个 URL 的 `lighting` |
|---|---:|---|---:|---|---|
| 开图（缺省白天） | 48（起始） | `d11766de4ab27be7` | 48 | `光照 白天` | `day` |
| 点「白天」（同档） | **0**（命中旧块） | 同上 | 48 | `光照 白天` | `day` |
| 点「黎明·黄昏」 | **+48** | `b88ed85d454840fb` | 96 | `光照 黎明·黄昏` | `dawn_dusk` |
| 点「夜晚」 | **+48** | `b50dff08385bab51` | 144 | `光照 夜晚` | `night` |

- 桌面状态栏高度始终 **28 px**；三档按钮 `aria-pressed` 恰有一个为 `true`。
- 状态栏档位来自响应头真值（`ME.tiles.lastLighting` 依次为 `day / dawn_dusk / night`），
  不是把 `state.lighting` 的请求值直接上屏；读不到时 `describeLighting` 显示「未知（响应头不可读）」。

### 10.4 移动端 375×812：三按钮可点、状态栏高度不变

> CDP `Emulation.setDeviceMetricsOverride(375×812, dpr=2, mobile=true)` +
> `Input.dispatchTouchEvent` 真触摸 tap。抽屉展开后把内容滚到光照组再点（不改 CSS）。

| 量 | 白天 | 黎明·黄昏 | 夜晚 |
|---|---|---|---|
| 按钮实测触控目标 | **94×44 px** | **94×44 px** | **94×44 px** |
| `elementFromPoint(中心)` | `BUTTON` | `BUTTON` | `BUTTON` |
| 点后请求数增量 | 0（同档命中） | **+18** | **+18** |
| 画布 sha256 | `25e53fda6fbc657f` | `0c9b0d78b77edddf` | `c363e4db9a96a5e2` |
| 状态栏高度 | **113.171875 px** | **113.171875 px** | **113.171875 px** |
| 状态栏文字行 `top` | `704/720/737` | `704/720/737` | `704/720/737` |

⇒ 三按钮均 ≥44px 且真触摸能切档；**加一行光照状态后状态栏高度与折行数都没有变化**。

---

## 11. 地图列表（`GET /api/maps`）—— 实测（2026-11-05 第五轮，Windows / Edge 150 headless / CDP）

> 环境：`CARGO_INCREMENTAL=0`、`CARGO_TARGET_DIR=I:\miredit-web-target`、release 二进制、
> `MIREDIT_BIND=127.0.0.1:18080`。耗时由 `curl.exe -w '%{time_total}'` 量（含进程启动前的
> 连接建立，不含 curl 自身启动）。两处目录都是**真实资产**，未做任何拷贝或裁剪。

### 11.1 两处真实目录的响应时间与 `total`

| 根（`MIREDIT_MAP_ROOT`） | 磁盘上的 `.map` | `total` | `/api/maps?limit=500` 三次耗时 | 响应体 |
|---|---|---|---|---|
| `GodotClient/maps` | **61 张 / 37,582,360 B**（0 个子目录） | **61** | **3.785 / 3.137 / 2.371 ms** | 4,208 B |
| `client/热血传奇23周年客户端/Map` | **1,097 张 / 731,026,566 B**（0 个子目录） | **1,097** | **6.613 / 5.617 / 7.468 ms** | 35,958 B |

三页各一次（1097 张）：

| 请求 | 耗时 | `total` | `items` | 首项 |
|---|---|---|---|---|
| `?limit=500&offset=0` | 5.838 ms | 1097 | 500 | `0.map` |
| `?limit=500&offset=500` | 5.273 ms | 1097 | 500 | `EM044.map` |
| `?limit=500&offset=1000` | 5.305 ms | 1097 | **97** | `T315.map` |

> 前端把这三页拼起来即「全量 1097 条」，实测 3 次请求合计 **51.8 ms**（含 3 次 curl 启动）。

### 11.2 「不解析内容」的数字证明

| 动作 | 耗时 | 读了多少字节 |
|---|---|---|
| `/api/maps`（1097 张，全量 3 页） | **51.8 ms** | **0**（只 `read_dir` + 1,097 次 `metadata`） |
| 纯 metadata 基线：`Get-ChildItem -Filter *.map`（同目录 1,097 文件） | **25.7 ms** | 0 |
| 纯 metadata 基线：`Directory.GetFiles` + `FileInfo.Length`（1,097 文件） | **35.8 ms** | 0 |
| 对照：`POST /api/map/open 0.map`（**解析一张** 700×700 / 5.6 MB） | **71.4 ms** | 5,880,052 |

⇒ 列表耗时（51.8 ms）与**纯 stat** 基线同量级（25.7 / 35.8 ms），而**解析一张**就要 71.4 ms
—— 若扫描里混进了解析，1097 张至少要 1097 × 数十毫秒 ≈ **分钟级**。**实测差 3 个数量级**。
代码侧另有一条测试钉住「0 字节 / 3 字节的垃圾 `.map` 也照样列出」（`扫描不解析内容`）。

### 11.3 分页：两页首项不重复

61 张目录：

```
?limit=10&offset=0  -> total=61 items=10 首项=0.map  末项=12.map
?offset=10（缺省 limit=100） -> total=61 items=51 首项=14.map 末项=xc1.map
两页重复项 = 0
页0 全部: 0.map 3.map 4.map 5.map 6.map 7.map 9.map 10.map 11.map 12.map
```

非法参数（**不是静默截断 / 不是当 0**）：

```
GET /api/maps?limit=501  -> 400 {"error":{"code":"bad_request","message":"limit 上限是 500，收到：501（本端点不静默截断，请分页取）"}}
GET /api/maps?offset=abc -> 400 {"error":{"code":"bad_request","message":"offset 必须是非负整数，收到：abc"}}
```

根目录不存在（另起一个实例，`MIREDIT_MAP_ROOT=E:\GodM2\这个目录不存在`）：

```
GET /api/maps -> 503
{"error":{"code":"assets_unavailable","message":"地图列表不可用：地图根读不到：E:\GodM2\这个目录不存在（系统找不到指定的文件。 (os error 2)）（检查 MIREDIT_MAP_ROOT 指向的目录是否存在、是否可读）"}}
```

### 11.4 自然序：`n2` 在 `n10` 之前

1097 张目录里 93 个 `n*.map`，按后端返回顺序（前 22）：

```
n0.map n1.map n2.map n3.map n4.map n5.map n6.map n11.map N63.map
n0100.map n0101.map n101.map n0102.map n102.map n0103.map n103.map
n0104.map n104.map n0105.map n105.map n0106.map n106.map
```

索引实测：`n2.map`=**2**、`n11.map`=**7**、`n101.map`=**11**、`n1001.map`=**84**
⇒ `n2 < n11 < n101 < n1001` 全部成立（**纯字节序**会把 `n0100.map` 排到最前、`n2.map` 排到 `n10` 之后）。

### 11.5 前端（桌面 1440×900，Edge 150 headless + CDP）

| 断言 | 实测 |
|---|---|
| 打开对话框 ⇒ 列表 | `requestsAfterOpen = 3`（1097 张 = 3 页），`renderedItems = 300` |
| 计数行 | `共 1097 张 · 仅渲染前 300 条，输入关键词缩小范围` |
| 首项 | `0.map` / `5.6 MB · 2012-07-05 18:23` |
| 列表可滚 | `clientHeight 372 / scrollHeight 14,997`，`overflow-y: auto`；`scrollTop 0 → 240` 生效 |
| 项高 | **44 px**（`itemW 505`） |
| 搜 `n0` | **64 条**匹配（`n0.map n0100.map …`）；`/api/maps` 请求数 **3 → 3（delta 0）** |
| 点第一项 | 对话框关闭 ⇒ 状态栏 `n0.map / 700×700 格 / Ext36`，标题 `n0.map · GodM2 地图编辑器` |
| 换一张 | 点 `3.map` ⇒ 状态栏 `3.map / 1000×800 格 / New14`，瓦片 `scale 0.25 · 42,224 B`；**同视口 848×728** 下画布 FNV `3471873464 → 2001581044`（真的换了图，不是只改了文字） |
| 手打路径退路 | `EM001.map` ⇒ `30×35 格 / Old12`，顶部告警条原文「本图有 8040 字节未解释尾块…」 |

### 11.6 移动端 375×812（`Emulation.setDeviceMetricsOverride`，dpr 2）

| 量 | 值 |
|---|---|
| 视口 | **375 × 812**，dpr **2** |
| 列表容器 | `337 × 388`（`clientHeight 388` / `scrollHeight 17,394`），`overflow-y: auto` |
| 项高 | **52 px**（≥44 硬底线） |
| 滚动 | `scrollTop 0 → 240` 生效（抽屉内可滚） |
| 搜 `n0` | 64 条，`/api/maps` 请求数 **3 → 3** |
| 点一项 | `n0.map` 打开：`700×700 格 / Ext36`，画布重绘（截图为证） |

### 11.7 门禁

| 门 | 结果 |
|---|---|
| `cargo test -p miredit-web` | **59 passed / 0 failed**（原 47 + 新增 12；含 2 条真 TCP 往返、1 条「列表 path 能直接喂 open」） |
| `cargo clippy -p miredit-web --all-targets -- -D warnings` | **exit 0，0 warning** |
| `cargo build -p miredit-web --release` | **exit 0，0 warning** |
| 单文件行数 | `routes/maps.rs` **306** + `routes/maps/tests.rs` **371**（拆法同 `routes/tiles/tests.rs`）；`css/app.css` **599**（列表样式另立 `css/maplist.css` 61 行） |
