//! `GET /api/export/minimap` —— **整图小地图导出**（返回 `image/png` + 下载文件名）。
//!
//! # 一、口径与渲染**都不在本文件**
//!
//! 该出多大、这个尺寸合不合法、该用哪一档倍率，全部由 `miredit-render` 的
//! [`miredit_render::minimap`] 裁决；怎么把这么大的图渲出来，由
//! [`miredit_render::tiled`] 裁决（**分块 + 逐块扩边 + 拼接**，与 GUI 的
//! 「导出小地图/选区」共用同一份实现）：
//!
//! ```text
//! ?preset= / ?width=&height=   →  MinimapSize（本文件只做协议→枚举的显式映射）
//! MinimapSize::resolve(W,H)    →  目标像素宽高（官方 W*48/4|2|1 × H*32/4|2|1）
//! valid_minimap_size(w,h)      →  官方 ValidSize + 本仓更严护栏（非法 ⇒ 400 中文原因）
//! pick_official_scale(...)    →  官方 16 级 mip 到本仓 4 档的有据映射
//! tiled::render_tiled(...)    →  整幅 RGBA（64 格一块，块边界按真实帧尺寸扩边）
//! preview_raster::fit_centered_opaque → 等比 + 居中 + 不透明黑底 + 最近邻落到目标宽高
//! encode_rgba_png              →  PNG 字节（与写文件编码器同一个）
//! ```
//!
//! ⚠ **订正（2026-11-08）**：上面第 6 行原文是
//! 「`compare::resize_to` → 面积平均缩到目标宽高（官方 redraw 的等价落点）」——
//! **那句是错的**。面积平均只出现在上游的 **mip 预缩**（`OverviewPixels.cs:7-42`）；
//! 导出成图那一步（`PreviewRaster.cs:35-38`）是**最近邻点采样**，且调用点
//! `MinimapExporter.cs:56-57` 传的是 `padding: 0, opaque: true`（等比 + 居中 + 不透明黑底）。
//! 已按上游改实现，原文保留作历史引用。
//!
//! **为什么不「整幅一次渲」**：渲染库的单次调用会先合成一张 1× 中间图（≤128 MiB）。
//! `700×700` 格的地图 `@Quarter` 的 1× 中间图是 `33600×22400×4 = 3 GB`，一次调用
//! 根本装不下 —— 这不是选档错，是**单次调用的结构性上限**。分块渲染把同一张图切成
//! 64 格见方的块，每块单独渲再拼回来（`One` 档同样是按块调 1× 路径）。
//!
//! **本文件一行倍率都没有**：`grep -rn "48 \*\|12 \*" crates/miredit-web/src/routes/export.rs`
//! 只应命中注释。两份必然漂，而漂移的症状是「网页导出的尺寸与 GUI 面板/官方差一档」。
//!
//! # 二、参数（与 `/api/tiles` 同一套口径）
//!
//! | 参数 | 取值 | 缺省 |
//! |---|---|---|
//! | `preset` | `small` / `medium` / `large`（大小写不敏感） | `small` |
//! | `width` + `height` | 正整数像素，**两个都必填** | — |
//! | `doors` | `0`（闭门）/ `1`（门全开） | `0` |
//! | `lighting` | `day` / `dawn_dusk` / `night` | `day` |
//!
//! - `preset` 与 `width/height` **互斥**：同时给 ⇒ 400，**不静默只认其中一个**
//!   （与 GUI CLI 的 `--minimap-preset` / `--minimap-width` 同一条判据）；
//! - `?preset=`（给了但为空）是**非法值** ⇒ 400，不是「没给」；
//! - `default small` 与官方缺省 `W*12 × H*8` **逐像素相同**（`W*48/4 = W*12`、
//!   `H*32/4 = H*8`），但两者仍是两个口径（`MinimapSize::Default` vs `Preset(Small)`）。
//!
//! # 三、前置条件与错误码（全部走既有 [`ApiError`]，**不写半成品**）
//!
//! | 情形 | 状态码 | `code` |
//! |---|---|---|
//! | 参数形状错（互斥 / 缺一半 / 非整数 / 枚举不认识） | 400 | `bad_request` |
//! | 尺寸非法（`0` / 超单边 / 超字节） | 400 | `bad_request` |
//! | 尚未打开地图 | 409 | `no_map` |
//! | 图库未就绪 | 503 | `assets_unavailable` |
//! | 这一发装不下（整幅超 `MAX_EXPORT_SIDE`/`MAX_EXPORT_BYTES`、某块扩边超上限、单块超渲染库护栏） | 413 | `too_large` |
//! | 内部不一致（0 格 / 拼出尺寸不符 / 拼接越界） | 500 | `internal` |
//! | PNG 编码失败 | 500 | `internal` |
//!
//! **判序**：参数（400）→ 地图（409）→ 尺寸（400）→ 图库（503）→ 渲染（413）。
//! 参数错先于环境错，与 `/api/tiles` 一致；尺寸要按地图格数算，所以排在地图之后。
//!
//! ⚠ **与 `/api/tiles` 的唯一判序差异**：那边先判图库（503）再判地图（409），本端点相反。
//! 理由：小地图的**尺寸完全由当前地图的格数推导**，没有地图时连「该出多大」都不知道，
//! 报 409 比报「图库未就绪」更接近用户实际要解决的第一步；而 `/api/tiles` 的视口参数
//! 与地图无关，先报缺图库更贴近它自己的因果链。两处都**不降级、不造图**，只是错误优先级不同。
//!
//! # 四、响应
//!
//! ```text
//! Content-Type:        image/png
//! Content-Disposition: attachment; filename="<地图名>-minimap-<w>x<h>.png"
//! Cache-Control:       no-store
//! X-Export-Width:      <w>        ← 实际产出宽（取自渲染结果，不是回显参数）
//! X-Export-Height:     <h>
//! ```
//!
//! 两个 `X-Export-*` 头必须列进 `crate::server::EXPOSED_HEADERS`，否则跨源部署时
//! 页面 JS 读到的恒为 `null`（与 `X-Render-*` 同一个理由）。
//!
//! 文件名里的非 ASCII 字符走 RFC 5987 的 `filename*=UTF-8''…` 第二参数，`filename="…"`
//! 保留 ASCII 回落（HTTP 头是 latin-1/ASCII 语义，中文原名不能直接塞进 `filename=`）。
//!
//! # 五、诚实边界（**不静默降档、不截断、不拼缺像素的图**）
//!
//! 渲染走 [`miredit_render::tiled::render_tiled`]（64 格一块 + **按真实帧尺寸逐块扩边**），
//! 所以「地图比单次渲染上限大」**不再是**拒绝理由：`700×700` 格默认 small
//! （`8400×5600`）在本机真实图库上会正常出图（见测试 `真地图700x700默认small导出成功`）。
//!
//! 仍然会拒绝的三类（都属于「这一发装不下」，**换更小尺寸就能成功**）：
//!
//! 1. **整幅交付物**超 [`crate::server`] 背后那两条护栏（`MAX_EXPORT_SIDE = 32768` /
//!    `MAX_EXPORT_BYTES = 512 MiB`）⇒ 413。`Large` 等超大输出更早就被
//!    `valid_minimap_size` 以 **400** 拦下，两者不混；
//! 2. 某块需要的**扩边**超过本档位上限（帧比整块还大，分块拼接无法保证逐字节一致）⇒ 413；
//! 3. 单块被渲染库的容量护栏拒（目标图 64 MiB / 1× 中间图 128 MiB / 1× 画布）⇒ 413。
//!
//! 这三类都**不会**被「偷偷换更小档位」或「拼一张缺像素的图」绕过去 —— 那等于改口径。
//!
//! 门态与光照整份 [`RenderOptions`] 透传（与 `/api/tiles` 同一个构造器），
//! 缺省闭门 + 白天 ⇒ 与「没有这两个开关」时逐字节相同。

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::{header, HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use miredit_render::minimap::{
    pick_official_scale, valid_minimap_size, MinimapPreset, MinimapSize,
};
use miredit_render::tiled::{render_tiled, TiledError};
use miredit_render::{
    encode_rgba_png, preview_raster::fit_centered_opaque, CollisionOverlay, LayerMask, PngImage,
    RenderOptions, RenderScale,
};
use mir2_assets::SpriteSource;
use mir2_map::Map;
use std::collections::HashMap;

use crate::error::{ApiError, ApiResult};
use crate::routes::tiles::{parse_doors_open, parse_lighting};
use crate::routes::{blocking, Params};
use crate::state::Shared;

/// 合法 `preset` 取值的中文枚举 —— 错误消息与模块文档共用这一处措辞。
const PRESET_HINT: &str = "small（¼，默认）/ medium（½）/ large（1:1）";

/// `GET /api/export/minimap`。
pub async fn export_minimap(
    State(st): State<Shared>,
    q: Result<Query<HashMap<String, String>>, QueryRejection>,
) -> ApiResult<Response> {
    let p = Params::from_query(q)?;
    // ① 参数形状先判（互斥 / 必填 / 枚举）：与「有没有开图、有没有图库」无关
    let size = parse_size(&p)?;
    let doors_open = parse_doors_open(p.raw_verbatim("doors"))?;
    let lighting = parse_lighting(p.raw_verbatim("lighting"))?;
    // 两个开关必须**一次**组合：分两步构造会互相覆盖（见 `tiles.rs` 模块文档 §八）
    let options = RenderOptions::with_doors_and_lighting(doors_open, lighting);

    // ② 地图：没开图就没有可导出的东西（409，与 `/api/tiles` 同一个 `no_map` 码）
    let guard = st.read_session();
    let session = guard.as_ref().ok_or_else(ApiError::no_map)?;
    let map = session.map();
    let (cells_w, cells_h) = (u32::from(map.width), u32::from(map.height));
    // ③ 尺寸：官方口径 + 本仓更严护栏（非法 ⇒ 400 中文原因，不截断不降档）
    let (out_w, out_h) = target_size(size, cells_w, cells_h)?;

    // ④ 图库：没有图源只能出一张全透明图，那是假数据 ⇒ 503（文案与 `/api/tiles` 同一句）
    let src = st.need_source()?;
    // 文件名取自当前地图的真实路径（不是客户端传来的字符串 ⇒ 不存在注入面）
    let stem = download_stem(session.document().path.file_stem());

    // ⑤ 渲染 + 编码是同步 CPU 活：`block_in_place` 让出 worker，读锁不跨任何 await
    let out = blocking(|| render_minimap(map, src, (out_w, out_h), options))?;
    drop(guard);

    let (w_name, w_value) = 数值头("x-export-width", out.0)?;
    let (h_name, h_value) = 数值头("x-export-height", out.1)?;
    let disposition = content_disposition(&stem, out.0, out.1)?;
    Ok((
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/png")),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (header::CONTENT_DISPOSITION, disposition),
            (w_name, w_value),
            (h_name, h_value),
        ],
        out.2,
    )
        .into_response())
}

// ---------------------------------------------------------------------------
// 一、参数 → MinimapSize
// ---------------------------------------------------------------------------

/// `?preset=` / `?width=&height=` → [`MinimapSize`]（**互斥**，缺省 `preset=small`）。
///
/// 为什么用 [`Params::raw_verbatim`] 而不是 `raw`：`?preset=`（给了但为空）是**非法值**
/// ⇒ 400，不是「没给」⇒ `small`。前者被静默当成缺省的话，「参数写错」永远查不出来
/// —— 与 `/api/tiles` 的 `doors` 同一条纪律（见 `routes/mod.rs` 的 `raw_verbatim` 文档）。
///
/// 缺省值取 `Preset(Small)` 而不是 `MinimapSize::Default`：本端点对外只承诺
/// `preset=small|medium|large` 三个取值，缺省就是其中最小的那档；两者算出的像素
/// **逐像素相同**（`W*48/4 = W*12`、`H*32/4 = H*8`），差别只在类型语义上。
fn parse_size(p: &Params) -> ApiResult<MinimapSize> {
    let preset_raw = p.raw_verbatim("preset");
    let width_raw = p.raw_verbatim("width");
    let height_raw = p.raw_verbatim("height");

    if preset_raw.is_some() && (width_raw.is_some() || height_raw.is_some()) {
        return Err(ApiError::bad_request(
            "preset 与 width/height 是两种互斥写法：给了 preset 就不要再给 width/height\
             （不静默只认其中一个）",
        ));
    }

    if let Some(raw) = preset_raw {
        let preset = MinimapPreset::parse(raw).ok_or_else(|| {
            ApiError::bad_request(format!(
                "preset 只支持 {PRESET_HINT}，收到：{}（不静默当成 small）",
                if raw.is_empty() { "空值" } else { raw }
            ))
        })?;
        return Ok(MinimapSize::Preset(preset));
    }

    match (width_raw, height_raw) {
        // 两个都没给 ⇒ 缺省 small
        (None, None) => Ok(MinimapSize::Preset(MinimapPreset::Small)),
        // 只给一个 ⇒ 400（不猜另一个，也不按比例推）
        (Some(_), None) | (None, Some(_)) => Err(ApiError::bad_request(
            "width 与 height 必须同时给（自定义尺寸要两个都填；只给一个不猜另一个）",
        )),
        (Some(w), Some(h)) => Ok(MinimapSize::Explicit {
            width: 解析像素("width", w)?,
            height: 解析像素("height", h)?,
        }),
    }
}

/// 像素参数的整数解析（**只做类型转换**，`0` 留给 [`valid_minimap_size`] 给官方原因）。
fn 解析像素(key: &str, raw: &str) -> ApiResult<u32> {
    raw.parse::<u32>().map_err(|_| {
        ApiError::bad_request(format!(
            "{key} 必须是非负整数像素（`0` 会在尺寸校验那一步被拒），收到：{raw}"
        ))
    })
}

/// 尺寸口径的**人话标签**（只用于错误消息里点明「这一发要的是什么」）。
fn size_label(size: MinimapSize) -> String {
    match size {
        MinimapSize::Default => "缺省（W*12 × H*8）".to_string(),
        MinimapSize::Preset(p) => format!("preset={}", p.key()),
        MinimapSize::Explicit { width, height } => format!("width={width}&height={height}"),
    }
}

/// [`MinimapSize`] → 目标像素宽高，并过 [`valid_minimap_size`]（非法一律 400）。
///
/// 错误消息里补上「这一发要的是什么 + 地图多大格」，因为渲染库/校验层的文案只知道自己
/// 算出来的数字，不知道调用方按哪个预设要的。
fn target_size(size: MinimapSize, cells_w: u32, cells_h: u32) -> ApiResult<(u32, u32)> {
    let 上下文 = format!("{}，地图 {cells_w}×{cells_h} 格", size_label(size));
    let (w, h) = size
        .resolve(cells_w, cells_h)
        .map_err(|reason| ApiError::bad_request(format!("{reason}（{上下文}）")))?;
    valid_minimap_size(w, h)
        .map_err(|reason| ApiError::bad_request(format!("{reason}（{上下文}）")))?;
    Ok((w, h))
}

// ---------------------------------------------------------------------------
// 二、渲染（整幅一次 + 等比居中 + 不透明黑底 + 最近邻落到目标宽高）
// ---------------------------------------------------------------------------

/// 出图并按档位分派，返回 `(实际宽, 实际高, PNG 字节)`。
///
/// 走 **[`miredit_render::tiled::render_tiled`]**（分块 + 逐块扩边 + 拼接，与 GUI 的
/// 「导出小地图/选区」**同一份实现**）—— 不是「整幅一次渲」：渲染库对单次调用的
/// 1× 中间图有 128 MiB 上限，`700×700` 格的地图在 `Quarter` 档的 1× 中间图是
/// `33600×22400×4 = 3 GB`，一次渲根本装不下（那才是「700×700 必须成功」的关键）。
///
/// 第二段是**最终合成**：按上游 `PreviewRaster.cs:15-19,35-38` 做
/// `scale = min(目标宽/源宽, 目标高/源高)` + **居中**留边 + **不透明黑底** + **最近邻**取样
/// （[`fit_centered_opaque`]）。分块出的是「最接近且不越界档位」的整幅，再落到 `target`。
///
/// ⚠ **订正（2026-11-08）**：本段原文是
/// 「第二段仍是**面积平均缩到目标宽高**（官方 `PreviewRaster.Create` 的等价落点）：
/// 分块出的是「最接近且不越界档位」的整幅，再用 [`resize_to`] 精确落到 `target`。」
/// —— **那句是错的**，两处都错：
/// ① 采样：上游 `PreviewRaster.cs:35-38` 是**最近邻**（`(int)((x+.5-left)*pixels.Width/w)`），
///    面积平均只在上游 mip 预缩 `OverviewPixels.cs:7-42`；
/// ② 几何：上游 `PreviewRaster.cs:15-17` 是 `scale = min(...)` + 居中偏移（`padding: 0`），
///    不是「目标宽高原样铺满」—— 后者在非等比请求（如 `1000×100`）上会把图**拉伸失真**。
/// 原文保留作历史引用。
///
/// 宽高取自**产物**（`fit_centered_opaque` 之后那张图），不是回显请求参数 —— 与 `/api/tiles` 的
/// `X-Render-Scale` 同一条纪律：响应头要说「实际给了什么」，不是「你要了什么」。
fn render_minimap(
    map: &Map,
    src: &dyn SpriteSource,
    target: (u32, u32),
    options: RenderOptions,
) -> ApiResult<(u32, u32, Vec<u8>)> {
    let (cells_w, cells_h) = (u32::from(map.width), u32::from(map.height));
    let scale = pick_official_scale(cells_w, cells_h, target.0, target.1);

    let 分块 = render_tiled(
        map,
        src,
        &LayerMask::all(),
        CollisionOverlay::none(),
        scale,
        (0, 0),
        (cells_w, cells_h),
        options,
    )
    .map_err(|e| tiled_error_to_api(e, cells_w, cells_h, scale, target))?;

    // 第二段：等比 + 居中 + 不透明黑底 + 最近邻（公开入口，不自己写滤镜）。
    //
    // 不再有「同尺寸就跳过」的短路：`opaque: true` 的黑底本身就是要落的语义
    // （上游 `PreviewRaster.cs:19` 无条件把 alpha 写成 255），跳过会漏掉那一步。
    let 源 = PngImage {
        width: 分块.width,
        height: 分块.height,
        rgba: 分块.rgba,
    };
    let 图 = fit_centered_opaque(&源, target.0, target.1);
    let (w, h) = (图.width, 图.height);
    let png = encode_rgba_png(w, h, &图.rgba)
        .map_err(|e| ApiError::internal(format!("PNG 编码失败（{w}×{h}）：{e}")))?;
    Ok((w, h, png))
}

/// 分块渲染失败 → HTTP 错误。
///
/// 分类**不靠匹配中文子串**，而是看 [`TiledError`] 的语义（`is_capacity()` + 变体）：
///
/// | 变体 | 含义 | 状态码 |
/// |---|---|---|
/// | `OutputTooLarge` | 整幅交付物超 `MAX_EXPORT_SIDE` / `MAX_EXPORT_BYTES` | 413 |
/// | `FrameMarginTooLarge` | 某块扩边超本档位上限（帧比块还大） | 413 |
/// | `RenderCapacity` | 单块被 64 MiB / 128 MiB / 画布护栏拒 | 413 |
/// | `RenderOther` | 单块渲染的参数类错误 | 400 |
/// | `Internal` | 0 格 / 尺寸不符 / 拼接越界（内部不一致） | 500 |
///
/// 前三条都是「请求合法，只是这一发装不下」⇒ 换更小尺寸就能成功；**绝不允许**
/// 静默降档或截断来「凑过去」。
fn tiled_error_to_api(
    e: TiledError,
    cells_w: u32,
    cells_h: u32,
    scale: RenderScale,
    target: (u32, u32),
) -> ApiError {
    let ctx = format!(
        "地图 {cells_w}×{cells_h} 格 @ scale={} → 目标 {}×{} px",
        scale.label(),
        target.0,
        target.1
    );
    match e {
        TiledError::RenderOther { .. } => {
            ApiError::bad_request(format!("小地图渲染参数非法（{ctx}）：{e}"))
        }
        TiledError::Internal(_) => {
            ApiError::internal(format!("小地图渲染内部不一致（{ctx}）：{e}"))
        }
        // OutputTooLarge / FrameMarginTooLarge / RenderCapacity
        other => ApiError::too_large(format!(
            "小地图出图超限（{ctx}）：{other}（不静默降档、不截断；换更小的尺寸或改用 GUI 导出）"
        )),
    }
}

// ---------------------------------------------------------------------------
// 三、响应头
// ---------------------------------------------------------------------------

/// 把 `u32` 装成一个响应头（值恒为十进制 ASCII，装不下就是内部错误）。
fn 数值头(name: &'static str, value: u32) -> ApiResult<(HeaderName, HeaderValue)> {
    let text = value.to_string();
    let v = HeaderValue::from_str(&text)
        .map_err(|_| ApiError::internal(format!("响应头 {name} 的值不是合法 HTTP 头：{text}")))?;
    Ok((HeaderName::from_static(name), v))
}

/// 下载文件名里的**地图名**（当前地图文件的主干名；取不到就退回 `map`）。
///
/// 来源是 `EditSession::document().path`（服务端自己打开的路径），**不是**请求参数 ⇒
/// 没有路径注入面。非 UTF-8 文件名（某些 Linux 挂载）取不到就退回 `map`，不猜。
fn download_stem(stem: Option<&std::ffi::OsStr>) -> String {
    stem.and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("map")
        .to_string()
}

/// `Content-Disposition: attachment; filename="<名>-minimap-<w>x<h>.png"`。
///
/// `filename=` 只放 ASCII（头字段是 ASCII 语义，中文原名直接塞进去会让整个响应头非法）；
/// 原名不是 ASCII 时**额外**带一个 RFC 5987 的 `filename*=UTF-8''<百分号编码>`，
/// 现代浏览器优先用后者 —— 既不下发非法字节，也不丢原名。
fn content_disposition(stem: &str, w: u32, h: u32) -> ApiResult<HeaderValue> {
    let ascii = ascii_fallback(stem);
    let mut value = format!("attachment; filename=\"{ascii}-minimap-{w}x{h}.png\"");
    if !stem.is_ascii() {
        value.push_str(&format!(
            "; filename*=UTF-8''{}-minimap-{w}x{h}.png",
            rfc5987_encode(stem)
        ));
    }
    HeaderValue::from_str(&value)
        .map_err(|_| ApiError::internal(format!("Content-Disposition 不是合法 HTTP 头：{value}")))
}

/// 文件名的 ASCII 回落：字母数字与 `-_.` 原样保留，其余一律换成 `_`（含空格与非 ASCII）。
fn ascii_fallback(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "map".to_string()
    } else {
        out
    }
}

/// RFC 5987 的 `attr-char` 百分号编码（`ALPHA / DIGIT / !#$&+-.^_`|~` 原样，其余 `%XX`）。
fn rfc5987_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let attr = b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            );
        if attr {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

/// 测试段单独一个文件（本文件已接近 600 行上限，与 `routes/tiles.rs` 同一处置）。
#[cfg(test)]
mod tests;
