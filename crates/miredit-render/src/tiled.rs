//! **分块渲染 + 拼接**（导出大图/小地图的唯一实现；2026-11-06 从 `miredit-gui` 上移）。
//!
//! # 一、为什么需要「分块渲染」，而不是一次 `try_render_region_scaled_with`
//!
//! 渲染库对**单次调用**有两条字节护栏 + 一条画布护栏：
//!
//! | 护栏 | 管什么 | 上限 |
//! |---|---|---|
//! | [`crate::SCALED_MAX_BYTES`] | 目标图（要交出去的那张） | 64 MiB |
//! | [`crate::INTERMEDIATE_MAX_BYTES`] | 1× 中间图（内部先合成的那张） | 128 MiB |
//! | `MAX_SIDE` / `MAX_PIXELS`（`types.rs`） | 1× 画布 | 单边 32768 px / 256 M 像素 |
//!
//! 而 `try_render_region_scaled_with` 的流程是「**先按 1× 合成整幅，再重采样**」——
//! 一张 `700×700` 格的地图（官方小地图默认要 `8400×5600`）在 1× 下是 `33600×22400`：
//! 单边就超了 32768、字节更是 3 GB。**一次调用根本渲不出来**，不是选档选错。
//!
//! 所以本模块把窗口切成边长 [`TILE_CELLS`] 格的块：每块单独调
//! [`crate::try_render_region_scaled_with`]（**采样 100% 由渲染库做，本模块一行滤镜都没有**），
//! 再把结果按档位整数倍拼回整幅。`One` 档更直接：`try_render_region_scaled_with(.., One)`
//! 内部 `return compose::render(..)`，本模块只是把它按块调。
//!
//! # 二、块边界的**扩边**：按每一帧的真实尺寸算，不是拍脑袋的常量
//!
//! [`compose`](crate::compose) 只遍历窗口内的格、且把帧按**原始尺寸**贴上去：
//!
//! | 层 | 锚点 | 覆盖范围（世界格 `(x,y)`）|
//! |---|---|---|
//! | 地表 | 左上 `(x*48, y*32)`，只偶偶格 | 向右 `帧宽`、向下 `帧高` |
//! | 中层 | 左上 `(x*48, y*32)` | 向右 `帧宽`、向下 `帧高` |
//! | 表层 | **底边** `(x*48, (y+1)*32 - 帧高)` | 向右 `帧宽`、向**上** `帧高` |
//!
//! 一帧若比一格大，就会画出自己那一格：块边界上会**丢像素**（块内宽帧被画布裁掉）
//! 或**少画**（块外大帧本该溢进来）。把块向四周多渲 `m` 格再裁回来即可 ——
//! 只要 `m` 覆盖「所有能溢进本块的格到本块的距离」。这个 `m` 由 [`tile_margins`]
//! 按**真实帧尺寸**逐块算出来（含开门态帧号，见 [`crate::RenderOptions`]），
//! 上限由渲染库护栏反算（[`margin_cap`]），**任何一格都不是猜的**。
//!
//! 为什么逐块算、而不是全窗口取一个最大值：实测 `0.map` 里存在外溢 **30 格**的物件
//! （≈1488 px 宽）。全窗口取最大值会让每一块都扩 30 格（单块 `64+60=124` 格、
//! 重复渲染系数 `(124/64)² ≈ 3.8`），而逐块算之后只有真正挨着那个物件的一两块扩边。
//!
//! # 三、与 GUI 的历史关系（等价性判据）
//!
//! 本文件是 `miredit-gui/src/export.rs` 里那套分块实现的**唯一副本**（2026-11-06 上移），
//! GUI 现在只做转调 + 类型转换。等价性有三道证据：
//!
//! 1. `tests/tiled.rs::分块渲染与上移前的黄金哈希逐字节相同` —— 上移**前**从 GUI 实现
//!    抓的 1× / 0.5× / 0.25× 三组输出哈希，逐字节相同；
//! 2. `tests/tiled.rs::分块拼接与整幅一次渲逐字节相同` —— 小图（单次渲染装得下的窗口）
//!    上分块结果与整幅一次渲**逐字节相同**（这是「无接缝」的判据）；
//! 3. `miredit-gui` 的 448 条测试原样通过（含真实 `0.map` 的导出门）。
//!
//! # 四、诚实边界
//!
//! - 整幅输出超 [`MAX_EXPORT_SIDE`] / [`MAX_EXPORT_BYTES`] ⇒ [`TiledError::OutputTooLarge`]
//!   （**报错，不截断、不降级**）；
//! - 某块需要的扩边超过本档位上限 ⇒ [`TiledError::FrameMarginTooLarge`]（不拼缺像素的图）；
//! - 单块被渲染库的容量护栏拒 ⇒ [`TiledError::RenderCapacity`]（带内层 [`RenderError`]）；
//! - 0 格窗口 / 拼出尺寸不符 / 拼接越界 ⇒ [`TiledError::Internal`]（调用方 bug，不是环境问题）。

use std::collections::HashMap;

use mir2_assets::{LibRef, SpriteSource};
use mir2_map::Map;

use crate::minimap::{MAX_EXPORT_BYTES, MAX_EXPORT_SIDE};
use crate::types::{
    CollisionOverlay, LayerMask, RenderError, RenderOptions, RenderScale, Viewport, CELL_H, CELL_W,
    INTERMEDIATE_MAX_BYTES, MAX_PIXELS, MAX_SIDE, SCALED_MAX_BYTES,
};
use crate::{try_render_region_scaled_with, 门偏移, 帧号};

/// 分块边长（格）。**必须能被缩小档的 4 / 2 整除**（一块 = 整数个输出像素块）。
///
/// 且单块的 1× 中间图 / 目标图都要留在渲染库的护栏内：
///
/// | 档位 | 块边长 | 1× 中间图 | 目标图 |
/// |---|---|---|---|
/// | `One` / `Half` / `Quarter` | 64 格 | 3072×2048×4 = 24 MB ≤ 128 MiB ✅ | `Half` 1536×1024 ✅ |
/// | `Two` | 32 格 | 1536×1024×4 = 6 MB ✅ | 3072×2048×4 = 24 MB ≤ 64 MiB ✅ |
///
/// `Two` 必须更小块：64 格 `@2×` 的目标是 `6144×4096×4 = 100 MB > 64 MiB` ⇒ 会被拒。
pub const TILE_CELLS: u32 = 64;

/// `Two` 档专用的块边长（理由见 [`TILE_CELLS`] 的表格）。
pub const TILE_CELLS_TWO: u32 = 32;

/// 扩边格数的**搜索上界**（真上限由渲染库护栏反算，见 [`margin_cap`]）。
///
/// 为什么不是一个写死的"安全阀"：实测被写死的数打脸两次 ——
/// 第一版按 8 格设，`0.map` 的 3×3 选区里就有 **13 格**外溢的物件（一帧高 ≈ 448 px）；
/// 改成 24 格后整图导出又撞到 **27 格**（第 17 块）。
/// 正确的做法是**让护栏自己算上限**：只要「块 + 2×扩边」还过得了
/// [`crate::try_render_region_scaled_with`] 的检查就允许；过不去才报错
/// （不硬渲、不静默拼缺像素的图）。
///
/// 48 只是**搜索起点**：实际有效上限对 `Quarter` / `Half` 是 **41 格**、对 `One` 是 48 格、
/// 对 `Two`（块 32）只有 5 格 —— 因为前三者还要受 1× 中间图 / 目标图字节上限约束。
pub const MAX_MARGIN_CELLS: u32 = 48;

/// 一张拼好的整幅 RGBA8 位图（行优先，`rgba.len() == width*height*4`）。
///
/// 为什么不直接复用 [`crate::Rendered`]：那个结构标了 `#[non_exhaustive]`，
/// 外部 crate 不能构造；拼接结果需要「尺寸 + 像素 + 分了几块 + 最大扩边」这四样，
/// 单独一个类型比硬塞进渲染统计更清楚。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiledImage {
    /// 像素宽。
    pub width: u32,
    /// 像素高。
    pub height: u32,
    /// 行优先 RGBA8。
    pub rgba: Vec<u8>,
    /// 实际渲染的块数（单块窗口为 1）。
    pub tiles: u32,
    /// 本次所有块里最大的扩边格数（0 = 所有块都不需要扩边）。
    pub margin: u32,
}

/// 分块渲染失败的原因。
///
/// `Display` 输出的是**与上移前逐字相同的中文文案**（GUI 多年排障用的原文，
/// 见 `miredit-gui/src/export.rs` 的历史注释），所以这里把已格式化好的句子存在变体里，
/// 只额外保留机器可判的语义（哪一种失败、内层 [`RenderError`] 是什么）。
/// 上层据此决定 HTTP 状态码（见 `miredit-web` 的 `/api/export/minimap`），
/// 不必去匹配中文子串。
#[derive(Debug)]
pub enum TiledError {
    /// 整幅输出过不了 [`MAX_EXPORT_SIDE`] / [`MAX_EXPORT_BYTES`]（请求合法，装不下）。
    OutputTooLarge(
        /// 逐字保留的中文原因。
        String,
    ),
    /// 某块需要的扩边超过本档位上限（帧比块还大，拼接无法保证逐字节一致）。
    FrameMarginTooLarge(
        /// 逐字保留的中文原因。
        String,
    ),
    /// 单块被渲染库的**容量**护栏拒（目标图 / 1× 中间图 / 1× 画布）。
    RenderCapacity {
        /// 块的上下文（`分块渲染失败（块 …）` 那句）。
        context: String,
        /// 渲染库的原始错误。
        source: RenderError,
    },
    /// 单块渲染的其它错误（参数非法一类的调用方问题）。
    RenderOther {
        /// 块的上下文（同上）。
        context: String,
        /// 渲染库的原始错误。
        source: RenderError,
    },
    /// 调用方 bug 或内部不一致（0 格 / 尺寸不符 / 拼接越界）。
    Internal(
        /// 逐字保留的中文原因。
        String,
    ),
}

impl std::fmt::Display for TiledError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutputTooLarge(m) | Self::FrameMarginTooLarge(m) | Self::Internal(m) => {
                f.write_str(m)
            }
            Self::RenderCapacity { context, source }
            | Self::RenderOther { context, source } => write!(f, "{context}：{source}"),
        }
    }
}

impl std::error::Error for TiledError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RenderCapacity { source, .. } | Self::RenderOther { source, .. } => {
                Some(source)
            }
            _ => None,
        }
    }
}

impl TiledError {
    /// 是否属于「**容量**类」失败（服务端装不下这一发，换更小的尺寸就能成功）。
    ///
    /// 判据：整幅超护栏 / 扩边超上限 / 内层渲染错误是容量档
    /// （[`RenderError::TooLarge`] / [`RenderError::IntermediateTooLarge`] /
    /// [`RenderError::DimensionOverflow`]）。HTTP 层把这一类映射成 **413**，
    /// 其余（0 格、尺寸不符、拼接越界、参数非法）不是「装不下」。
    pub fn is_capacity(&self) -> bool {
        match self {
            Self::OutputTooLarge(_) | Self::FrameMarginTooLarge(_) => true,
            Self::RenderCapacity { .. } => true,
            Self::RenderOther { source, .. } => matches!(
                source,
                RenderError::TooLarge { .. }
                    | RenderError::IntermediateTooLarge { .. }
                    | RenderError::DimensionOverflow { .. }
            ),
            Self::Internal(_) => false,
        }
    }
}

/// 档位的分数表示 `(分子, 分母)`：`0.25 = 1/4`、`0.5 = 1/2`、`1 = 1/1`、`2 = 2/1`。
///
/// 用整数而不是 `f32`：拼接要算「第几格落在第几个输出像素」，浮点一旦有
/// `0.30000000000000004` 这类尾巴，块与块之间就会错一列。
const fn ratio(scale: RenderScale) -> (u64, u64) {
    match scale {
        RenderScale::Quarter => (1, 4),
        RenderScale::Half => (1, 2),
        RenderScale::One => (1, 1),
        RenderScale::Two => (2, 1),
    }
}

/// `cells` 个格 × 每格 `cell_px` 像素，在 `scale` 档下的**整数**像素数。
///
/// 恒等式：`48 % 4 == 0`、`32 % 4 == 0` ⇒ 四个档位的乘积都是整数（不存在四舍五入），
/// 与 [`RenderScale::pixel_size`] 的 `round(...)` 结果相同（有测试钉住）。
fn px_of(cells: u32, cell_px: u32, scale: RenderScale) -> u32 {
    let (num, den) = ratio(scale);
    (cells as u64 * cell_px as u64 * num / den) as u32
}

/// 一块 `cells_w × cells_h` 格在 `scale` 档下的输出像素尺寸。
pub fn scaled_size(cells_w: u32, cells_h: u32, scale: RenderScale) -> (u32, u32) {
    (
        px_of(cells_w, CELL_W, scale),
        px_of(cells_h, CELL_H, scale),
    )
}

/// 该档位下块边长取多少格（见 [`TILE_CELLS`] / [`TILE_CELLS_TWO`]）。
const fn tile_cells_for(scale: RenderScale) -> u32 {
    match scale {
        RenderScale::Two => TILE_CELLS_TWO,
        _ => TILE_CELLS,
    }
}

/// 一帧的像素尺寸（只为算外溢，不带像素数据）。
type FrameSize = (u32, u32);

/// 帧尺寸缓存：同一个 `(库, 帧号)` 只问一次图库。
///
/// 为什么必须有：算外溢要扫窗口里**每一格的三层帧**（700×700 = 147 万次），
/// 而 [`SpriteSource::frame`] 的契约是**每次返回一份新解码的像素**
/// ⇒ 不缓存的话为了拿一个宽高会把同一帧解码上千遍。
///
/// 公开的理由：`miredit-gui` 的导出扩边测试要用它验「同一 (库, 帧号) 只问一次」
/// （那条测试随实现一起上移会丢覆盖率，所以这里开放类型本体，语义与上移前一致）。
#[derive(Default)]
pub struct SizeCache<'a> {
    src: Option<&'a dyn SpriteSource>,
    slots: HashMap<(LibRef, u32), Option<FrameSize>>,
}

impl<'a> SizeCache<'a> {
    /// 绑定一个帧源。
    pub fn new(src: &'a dyn SpriteSource) -> Self {
        SizeCache {
            src: Some(src),
            slots: HashMap::new(),
        }
    }

    /// 取 `(库, 帧号)` 的帧尺寸；取不到返回 `None`（**缺帧不是外溢**，按 0 算）。
    pub fn size(&mut self, lib: LibRef, index: u32) -> Option<FrameSize> {
        if let Some(hit) = self.slots.get(&(lib, index)) {
            return *hit;
        }
        let got = self
            .src
            .and_then(|s| s.frame(lib, index))
            .map(|f| (f.width, f.height));
        self.slots.insert((lib, index), got);
        got
    }
}

/// **每块各需要向外扩多少格**（按块网格返回，`index = tj*nx + ti`）。
///
/// # 怎么算（一遍扫完窗口，不重复扫）
///
/// 对窗口里每一格、每一层的帧，算出它的**像素矩形** `R`（世界格内相对窗口原点），
/// 再算出 `R` 与哪些块相交（块是规则的网格 ⇒ 直接整除即可，不需要遍历所有块），
/// 对这些块取「该格到块矩形」的切比雪夫距离作为所需扩边，逐块取最大值。
/// 一格能碰到的块通常只有 1～4 个，所以整趟是 `O(窗口格数)`。
///
/// **只看窗口内的格**（这是正确性的必要条件，不是省事）：窗口外的格在整图渲染里
/// 本来就不画（`compose` 的 `for dx in 0..cells_w`），而 [`expand`] 会把扩边区
/// **夹回窗口** ⇒ 窗口外的格永远不会被画进来。
///
/// 本函数**不判上限**：上限由 [`render_tiled`] 按「这次到底分不分块」决定
/// （单块时扩边被夹回窗口，多大都无害）。
///
/// # `options` 为什么是必填参数
///
/// 表层帧号在开门态下会 `+= 门偏移(cell, options)`（见 [`crate::门偏移`]），
/// **而门开帧与闭门帧的尺寸可以不同** ⇒ 扩边必须按**将要真正画出去的那一帧**算，
/// 否则块边界上会丢像素/少画。传错这个参数不会报错、只会悄悄拼错图，
/// 所以它**没有缺省值**：调用方必须显式表态。
pub fn tile_margins(
    map: &Map,
    src: &dyn SpriteSource,
    mask: &LayerMask,
    origin: (i32, i32),
    cells: (u32, u32),
    tile: u32,
    options: RenderOptions,
) -> Vec<u32> {
    let nx = cells.0.div_ceil(tile) as usize;
    let ny = cells.1.div_ceil(tile) as usize;
    let mut out = vec![0u32; nx * ny];
    if nx == 0 || ny == 0 {
        return out;
    }
    let mut cache = SizeCache::new(src);

    // 块网格的像素步长（一块 `tile` 格 = `tile*48` × `tile*32` px）
    let step_x = i64::from(tile) * i64::from(CELL_W);
    let step_y = i64::from(tile) * i64::from(CELL_H);

    // 一层的帧矩形 `(x0, y0, x1, y1)`（相对窗口原点的像素坐标，半开区间）
    let bump = |rect: (i64, i64, i64, i64), dx: u32, dy: u32, out: &mut [u32]| {
        let (x0, y0, x1, y1) = rect;
        if x1 <= x0 || y1 <= y0 {
            return; // 0×0 空帧：什么也不画，也就没有外溢
        }
        // 与哪些块相交：块 `ti` 覆盖像素 `[ti*step_x, (ti+1)*step_x)`
        let ti0 = (x0 / step_x).max(0) as usize;
        let ti1 = (((x1 - 1) / step_x).max(0) as usize).min(nx - 1);
        let tj0 = (y0 / step_y).max(0) as usize;
        let tj1 = (((y1 - 1) / step_y).max(0) as usize).min(ny - 1);
        for tj in tj0..=tj1 {
            for ti in ti0..=ti1 {
                // 该块在窗口内的格矩形（闭区间）
                let bx0 = (ti as u32) * tile;
                let by0 = (tj as u32) * tile;
                let bx1 = (bx0 + tile).min(cells.0) - 1;
                let by1 = (by0 + tile).min(cells.1) - 1;
                // 该格到块矩形的切比雪夫距离（在块内 ⇒ 0）。
                //
                // 一维「点到区间 `[lo, hi]` 的距离」= `lo.saturating_sub(v)` 与
                // `v.saturating_sub(hi)` 取大者：点在区间左侧 ⇒ 前者是 `lo - v`、后者 0；
                // 点在区间内 ⇒ 两个都是 0；点在右侧 ⇒ 前者 0、后者是 `v - hi`。
                let mx = bx0.saturating_sub(dx).max(dx.saturating_sub(bx1));
                let my = by0.saturating_sub(dy).max(dy.saturating_sub(by1));
                let m = mx.max(my);
                let slot = &mut out[tj * nx + ti];
                *slot = (*slot).max(m);
            }
        }
    };

    for dy in 0..cells.1 {
        let y = origin.1 + dy as i32;
        let Ok(cy) = u16::try_from(y) else {
            continue; // 图外（负坐标）：不画，也不参与外溢
        };
        for dx in 0..cells.0 {
            let x = origin.0 + dx as i32;
            let Ok(cx) = u16::try_from(x) else {
                continue;
            };
            let Some(cell) = map.cell(cx, cy) else {
                continue;
            };
            let px = i64::from(dx) * i64::from(CELL_W);
            let py = i64::from(dy) * i64::from(CELL_H);
            // 地表：只铺偶偶格，左上锚点
            if mask.ground && cx & 1 == 0 && cy & 1 == 0 {
                if let Some(i) = 帧号(cell.bk_img) {
                    if let Some((fw, fh)) = cache.size(LibRef::ground(u16::from(cell.tiles)), i) {
                        bump(
                            (px, py, px + i64::from(fw), py + i64::from(fh)),
                            dx,
                            dy,
                            &mut out,
                        );
                    }
                }
            }
            // 中层：逐格左上锚点
            if mask.middle {
                if let Some(i) = 帧号(cell.mid_img) {
                    if let Some((fw, fh)) = cache.size(LibRef::middle(u16::from(cell.sm_tiles)), i) {
                        bump(
                            (px, py, px + i64::from(fw), py + i64::from(fh)),
                            dx,
                            dy,
                            &mut out,
                        );
                    }
                }
            }
            // 表层：**底边**钉在格底 ⇒ 帧向上长
            if mask.front {
                if let Some(i) = 帧号(cell.fr_img) {
                    // 门开帧偏移：与 `compose::render` 的表层分支算的是同一个帧号
                    let i = i + 门偏移(cell, options);
                    if let Some((fw, fh)) = cache.size(LibRef::surface(u16::from(cell.area)), i) {
                        let 底 = py + i64::from(CELL_H);
                        bump(
                            (px, 底 - i64::from(fh), px + i64::from(fw), 底),
                            dx,
                            dy,
                            &mut out,
                        );
                    }
                }
            }
        }
    }

    out
}

/// 一块（窗口内相对格坐标）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tile {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// 把块向四周扩 `margin` 格，并**夹回窗口**（夹回是必须的：窗口外的格
/// 在整图渲染里本来就不画，扩出去会把它们画进来 ⇒ 反而多像素）。
fn expand(t: Tile, margin: u32, win: (u32, u32)) -> Tile {
    let x0 = t.x.saturating_sub(margin);
    let y0 = t.y.saturating_sub(margin);
    let x1 = (t.x + t.w + margin).min(win.0);
    let y1 = (t.y + t.h + margin).min(win.1);
    Tile {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

/// 按渲染库的护栏把块边长缩到能过（真的过不了就报错，不硬渲）。
fn fit_tile(base: u32, margin: u32, scale: RenderScale) -> Result<u32, TiledError> {
    let mut t = base.max(1);
    loop {
        let e = t + 2 * margin;
        let 中间宽 = u64::from(e) * u64::from(CELL_W);
        let 中间高 = u64::from(e) * u64::from(CELL_H);
        let 中间字节 = 中间宽 * 中间高 * 4;
        let (tw, th) = scaled_size(e, e, scale);
        let 目标字节 = u64::from(tw) * u64::from(th) * 4;

        // 与渲染库**同一批常量**（`types.rs` 的 `MAX_SIDE` / `MAX_PIXELS`，crate 内可见）。
        // 上移前这三条是「照抄同值」的副本，现在直接读真值 ⇒ 常量一改两边同时变。
        let ok_side = 中间宽 <= MAX_SIDE && 中间高 <= MAX_SIDE && 中间宽 * 中间高 <= MAX_PIXELS;
        // `One` 档不产生中间图、也不受目标图字节上限约束（`scale.rs` 直接转调 1× 路径）。
        let ok_bytes = if scale == RenderScale::One {
            true
        } else {
            中间字节 <= INTERMEDIATE_MAX_BYTES && 目标字节 <= SCALED_MAX_BYTES
        };
        if ok_side && ok_bytes {
            return Ok(t);
        }
        if t <= 1 {
            return Err(TiledError::Internal(format!(
                "分块尺寸降到 1 格仍超 render 护栏（扩边 {margin} 格、档位 {}）",
                scale.label()
            )));
        }
        t = (t / 2).max(1);
    }
}

/// 该档位下**允许的最大扩边格数** = 最大的那个仍过护栏的 `m`（用 [`fit_tile`] 反算）。
///
/// `TILE_CELLS = 64` 时的实际值（算式见 [`fit_tile`] 的护栏）：
///
/// | 档位 | 卡在哪一条 | 最大 `m` |
/// |---|---|---|
/// | `Quarter` / `Half` | 1× 中间图 `(64+2m)²×1536 ≤ 128 MiB` | **41** |
/// | `One` | 单边 `(64+2m)×48 ≤ 32768` | **48**（顶到搜索起点）|
/// | `Two`（块 32） | 目标图 `(32+2m)×96 ≤ 4096` | **5** |
fn margin_cap(tile: u32, scale: RenderScale) -> u32 {
    for m in (0..=MAX_MARGIN_CELLS).rev() {
        if fit_tile(tile, m, scale).is_ok() {
            return m;
        }
    }
    0
}

/// 把 `src` 里从 `off` 格开始的一块，拷进 `dst` 的 `at` 格位置（像素偏移按档位整数换算）。
#[allow(clippy::too_many_arguments)] // 8 个参数就是「一次拼接」的 8 个输入，打包只会更绕
fn blit_crop(
    dst: &mut [u8],
    dst_w: u32,
    src: &[u8],
    src_w: u32,
    off: (u32, u32),
    at: (u32, u32),
    size: (u32, u32),
    scale: RenderScale,
) -> Result<(), TiledError> {
    let sx = px_of(off.0, CELL_W, scale);
    let sy = px_of(off.1, CELL_H, scale);
    let dx = px_of(at.0, CELL_W, scale);
    let dy = px_of(at.1, CELL_H, scale);
    let w = px_of(size.0, CELL_W, scale);
    let h = px_of(size.1, CELL_H, scale);
    let row = w as usize * 4;

    // 越界一律报错（宁可失败也不写错位的图）
    let need_src = (u64::from(sx) + u64::from(w)) <= u64::from(src_w);
    let need_dst = (u64::from(dx) + u64::from(w)) <= u64::from(dst_w);
    if !need_src || !need_dst {
        return Err(TiledError::Internal(format!(
            "拼接越界：块源偏移 {sx}+{w} 宽 {src_w} / 目标偏移 {dx}+{w} 宽 {dst_w}"
        )));
    }
    for j in 0..h {
        let s = ((u64::from(sy) + u64::from(j)) * u64::from(src_w) + u64::from(sx)) as usize * 4;
        let d = ((u64::from(dy) + u64::from(j)) * u64::from(dst_w) + u64::from(dx)) as usize * 4;
        let (Some(ss), Some(dd)) = (src.get(s..s + row), dst.get_mut(d..d + row)) else {
            return Err(TiledError::Internal("拼接越界：行切片取不到".to_string()));
        };
        dd.copy_from_slice(ss);
    }
    Ok(())
}

/// 渲染 `origin` 起 `cells` 格的一块窗口，**内部分块**，返回拼好的整幅 RGBA。
///
/// 这是导出（整图 / 选区 / 小地图）的**唯一**渲染入口：三块功能只差
/// `origin/cells/scale/options`。
///
/// `options` = **导出时的门态 + 光照档位**（[`RenderOptions`]）。它同时喂给两条链路，
/// 缺一不可：
///
/// 1. [`tile_margins`]（按**真正会画出去的那一帧**的尺寸算扩边）；
/// 2. 每块的 [`crate::try_render_region_scaled_with`]（同一份 `RenderOptions`）。
///
/// 只做②不做① ⇒ 开门态下大尺寸的门开帧跨块时拿不到扩边，拼接与整图不一致；
/// 只做①不做② ⇒ 扩边按开门帧算、画出来还是闭门帧，白扩。
///
/// 缺省 [`RenderOptions::default()`] = 闭门 + 白天 ⇒ 与引入这两个开关前逐字节相同。
#[allow(clippy::too_many_arguments)] // 8 个参数就是「一次导出请求」的 8 个输入，打包成结构体只会更绕（与本 crate 其它绘制入口同口径）
pub fn render_tiled(
    map: &Map,
    src: &dyn SpriteSource,
    mask: &LayerMask,
    overlay: CollisionOverlay,
    scale: RenderScale,
    origin: (i32, i32),
    cells: (u32, u32),
    options: RenderOptions,
) -> Result<TiledImage, TiledError> {
    if cells.0 == 0 || cells.1 == 0 {
        return Err(TiledError::Internal(
            "导出窗口为 0 格（没有可导出的范围）".to_string(),
        ));
    }
    let (out_w, out_h) = scaled_size(cells.0, cells.1, scale);
    // 整幅输出的护栏（与渲染库的单次护栏是两回事，见模块文档 §四）
    if out_w > MAX_EXPORT_SIDE || out_h > MAX_EXPORT_SIDE {
        return Err(TiledError::OutputTooLarge(format!(
            "导出图太大：{out_w}×{out_h} px 超过单边上限 {MAX_EXPORT_SIDE} px \
             （缩小选区，或换更小的图）"
        )));
    }
    let 整幅字节 = u64::from(out_w) * u64::from(out_h) * 4;
    if 整幅字节 > MAX_EXPORT_BYTES {
        return Err(TiledError::OutputTooLarge(format!(
            "导出图太大：{out_w}×{out_h} px = {:.1} MB，超过上限 {:.0} MB",
            整幅字节 as f64 / 1_048_576.0,
            MAX_EXPORT_BYTES as f64 / 1_048_576.0
        )));
    }

    let tile = tile_cells_for(scale);
    // 扩边的**真实上限**由渲染库护栏反算（不是写死的数，见 [`margin_cap`]）
    let cap = margin_cap(tile, scale);
    let nx = cells.0.div_ceil(tile).max(1);
    let ny = cells.1.div_ceil(tile).max(1);
    let 单块 = nx == 1 && ny == 1;

    // 每块各需要扩多少格（逐块算，见 [`tile_margins`] 的文档）
    let mut margins = tile_margins(map, src, mask, origin, cells, tile, options);
    if 单块 {
        // 单块：扩边区被 [`expand`] 夹回窗口，**多大都无害**（结果与整图一次渲完全相同），
        // 所以不在这里卡上限 —— 否则「3×3 选区 + 图里恰好有个大物件」会被无谓地拒掉。
        for m in margins.iter_mut() {
            *m = (*m).min(cap);
        }
    } else if let Some((i, &bad)) = margins.iter().enumerate().find(|(_, &m)| m > cap) {
        return Err(TiledError::FrameMarginTooLarge(format!(
            "帧尺寸过大：第 {} 块需要外扩 {bad} 格，超过本档位上限 {cap} 格 \
             （宽 > {} px 或高 > {} px 的帧无法保证分块拼接逐字节一致；\
               缩小导出范围，或先把这些格里的物件挪走）",
            i + 1,
            CELL_W + cap * CELL_W,
            CELL_H + cap * CELL_H
        )));
    }

    let mut buf = vec![0u8; 整幅字节 as usize];
    let mut tiles = 0u32;
    let mut 最大扩边 = 0u32;
    // `options` 由调用方直接递进来（门态 + 光照档位同在一份 `RenderOptions` 里），
    // 逐块传值不会重复构造。为什么不用 `try_render_region_scaled`：那个入口内部写死
    // `RenderOptions::default()` ⇒ 门恒闭 + 恒白天。

    for tj in 0..ny {
        for ti in 0..nx {
            let tx = ti * tile;
            let ty = tj * tile;
            let t = Tile {
                x: tx,
                y: ty,
                w: tile.min(cells.0 - tx),
                h: tile.min(cells.1 - ty),
            };
            let margin = margins[(tj * nx + ti) as usize];
            最大扩边 = 最大扩边.max(margin);
            let e = expand(t, margin, cells);
            let vp = Viewport::new(origin.0 + e.x as i32, origin.1 + e.y as i32, e.w, e.h);
            let r = try_render_region_scaled_with(map, src, &vp, mask, overlay, scale, options)
                .map_err(|err| {
                    let context = format!(
                        "分块渲染失败（块 {}×{} 格 @({tx},{ty})，扩边 {margin} 格 → \
                         实际渲 {}×{} 格，档位 {}）",
                        t.w,
                        t.h,
                        e.w,
                        e.h,
                        scale.label()
                    );
                    // 容量档（目标图 / 1× 中间图 / 1× 画布）与其它错误分开，
                    // 上层据此决定 413 还是 400 —— 不去匹配中文子串。
                    match err {
                        RenderError::TooLarge { .. }
                        | RenderError::IntermediateTooLarge { .. }
                        | RenderError::DimensionOverflow { .. } => TiledError::RenderCapacity {
                            context,
                            source: err,
                        },
                        other => TiledError::RenderOther {
                            context,
                            source: other,
                        },
                    }
                })?;
            // 尺寸自证：render 出的必须正好是「扩边块」在本次档位下的像素尺寸
            let (ew, eh) = scaled_size(e.w, e.h, scale);
            if r.width != ew || r.height != eh {
                return Err(TiledError::Internal(format!(
                    "分块尺寸不符：期望 {ew}×{eh}，实际 {}×{}",
                    r.width, r.height
                )));
            }
            blit_crop(
                &mut buf,
                out_w,
                &r.rgba,
                r.width,
                (t.x - e.x, t.y - e.y),
                (t.x, t.y),
                (t.w, t.h),
                scale,
            )?;
            tiles += 1;
        }
    }

    Ok(TiledImage {
        width: out_w,
        height: out_h,
        rgba: buf,
        tiles,
        margin: 最大扩边,
    })
}
