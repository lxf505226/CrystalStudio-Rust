//! 窗内小地图的**总览图**（`F-R13-1` 渲染层）：在 `1/2^level` 尺度上合成三层。
//!
//! # 一、这条链路在上游怎么走（只做渲染层，见 §五）
//!
//! | 环节 | 上游 | 本模块对应 |
//! |---|---|---|
//! | 窗内小地图取像素 | `_upstream/CrystalStudio/MapMinimap.Avalonia.cs:45` → `:50 source.OverviewScene`（`_upstream/CrystalStudio/MapViewport.Minimap.cs:7`） | [`render_overview`] 的输出 |
//! | 换图时**无条件**建 level 4 总览 | `_upstream/CrystalStudio/MapViewport.cs:145`（`WarmOverview()`）→ `_upstream/CrystalStudio/MapViewport.Overview.cs:42-47`（`StartOverview(settings, 4)`） | 档位由调用方传 `level`（本模块不替产品选档） |
//! | 逐帧预缩 | `_upstream/CrystalStudio/Assets.cs:119` `width = max(1, (w + (1<<level) - 1) >> level)` | [`预缩帧`]（同一算式：向上取整、下限 1） |
//! | 合成 | `_upstream/CrystalStudio/OverviewBatch.cs:25-104`（4 层 + 行带） | 本模块的三层循环（层序 / 层内序照 [`crate::compose`]） |
//!
//! **为什么必须逐帧预缩、不能「先按 1× 合成再整幅缩小」**：`700×700` 格的地图，
//! 1× 画布是 `33600 × 22400 px = 3.0 GiB`，先合成再缩必然把内存顶爆 —— 上游正是为此
//! 才在**解码时**就按 level 缩小（`_upstream/CrystalStudio/Assets.cs:108-140` 的 `DecodeOverview`），本模块同口径。
//!
//! # 二、放置口径与 [`crate::compose`] **同源**（本模块最要紧的一条）
//!
//! 本模块**不重新实现任何落格规则**，只调 `compose` 里的那两个共享纯函数：
//!
//! | 前提 | 唯一实现处 |
//! |---|---|
//! | 地表只铺偶偶格 / 库路由 / 帧号 / 门偏移 / 加性标志 | [`crate::compose::层取帧`] |
//! | 左上锚点 / 底边锚点 / `library==28` 特例 / 加性帧锚点 | [`crate::compose::层放置`] |
//!
//! 1× 合成（[`crate::compose`] 的三层循环）与总览调的是**同一对函数**；本模块只多做
//! 一步：把 1× 位移换算到 level 尺度 —— `floor(px / 2^level)`（[`i32::div_euclid`]，
//! 向负无穷取整，正是「盒式降采样的块下标」的定义，负坐标也一致）。
//!
//! 等价性由单测 `总览与整幅盒式降采样逐字节相同` 钉住：在 16 对齐的假帧下，总览像素
//! **逐字节等于**「1× 合成图按 `16×16` 块面积平均」，并且与「采样网格错开半块」的
//! 基准**必须不同**（反假绿）。
//!
//! # 三、预算护栏（**不分配大内存、不静默降档截断**）
//!
//! 目标缓冲 = `ceil(W*48 / 2^level) × ceil(H*32 / 2^level) × 4` 字节。
//! `level == 4` 时恰是 `3W × 2H × 4`（`48 / 16 = 3`、`32 / 16 = 2`，见 [`crate::CELL_W`] /
//! [`crate::CELL_H`]）。超过 [`OVERVIEW_MAX_BYTES`]（64 MiB）时**逐级抬 level** 重算，
//! 一直到 [`OVERVIEW_MAX_LEVEL`]（16）；**仍超** ⇒ [`OverviewError::BudgetExceeded`]。
//!
//! 三条硬口径：
//!
//! 1. **判定在分配之前**：先定档、再 `Canvas::new`，不存在「先申请再缩」；
//! 2. **不静默降档截断**：抬档是**换分辨率**（不是裁掉/截屏），且用 [`Overview::level`]
//!    如实回报实际档位 —— 调用方看得到图是 `1/32` 还是 `1/16` 的；
//! 3. **到 16 就报错**：16 是上游 mip 上限（`_upstream/CrystalStudio/MinimapExporter.cs:47-48` 的
//!    `clamp(..., 0, 16)`），不再无限抬。
//!
//! # 四、已知差异与不做的事（如实登记，不静默）
//!
//! 1. **非 `2^level` 对齐的帧有相位误差**：预缩尺寸是 `ceil(w / 2^level)`
//!    （上游同一条算式），而「1× 合成后盒式平均」的真正块跨度是
//!    `floor((p+w-1)/s) - floor(p/s) + 1` —— 当帧**起点**不是 `s` 的整数倍、或帧宽不是
//!    `s` 的整数倍时，两者会差一列/一行。`48` / `32` 都是 16 的倍数 ⇒ level ≤ 4 时
//!    所有落格位置的起点都天然对齐，只有帧尺寸本身不是 16 的倍数（例如 `100×100`）
//!    才会触发。**这与上游同源**（上游也是逐帧 `(w + s - 1) >> level` 预缩），不是本模块自定。
//! 2. **只画三层精灵**：不做光照遮罩、不画阻挡标记、不画门/灯标记（上游总览场景同样只铺
//!    地图精灵）。[`RenderOptions::lighting`] 在本模块**不生效**，只消费
//!    [`RenderOptions::doors_open`]（它改变表层帧号 ⇒ 会改变像素）。
//! 3. **不产出统计**：缺帧按「不画」处理（与 [`crate::compose`] 同一条「禁假数据」纪律：
//!    绝不画占位色），但**不累计** `missing_*` —— 要缺帧统计请走 1× 合成
//!    （[`crate::RenderStats`]）。总览是「看一眼」的图，不是验收口。
//! 4. **层内顺序与 [`crate::compose`] 一致**（**行优先**，`dy` 外 / `dx` 内），没有跟上游
//!    `OverviewBatch.cs` 的行带批处理：本仓的对照基准是 1× 合成图，两者顺序必须一致，
//!    否则等价门失去意义。只在「同一块内互相覆盖」的帧上，两种批处理顺序才看得出差别。
//!
//!    ⚠ **口径订正记录（2026-11-08）**——原文保留：
//!    > 4. **层内顺序照 [`crate::compose`] 的列优先**（`dx` 外 / `dy` 内），没有跟上游
//!    >    `OverviewBatch.cs` 的行带批处理：本仓的对照基准是 1× 合成图，两者顺序必须一致，
//!    >    否则等价门失去意义。只在「同一块内互相覆盖」的帧上，两种批处理顺序才看得出差别。
//!
//!    原为「照 `compose` 的列优先」（`dx` 外 / `dy` 内）；`compose` 于 `d66237b` 改行优先后
//!    本模块**同步**（`render_overview` 的三层循环改成 `dy` 外 / `dx` 内）—— 当初照
//!    `compose` 写就是为了「两者顺序一致」，那个前提变了，这里必须跟着变，否则等价门的
//!    前提反而被自己破坏。层序（地表 → 中层 → 表层）与落格算式一个字节没动。
//! 5. **同一帧只预缩一次**：缓存键 `(库, 帧号)`（与上游按帧缓存 `DecodeOverview` 同思路），
//!    缓存里存的是**预缩后**的帧 + 原帧元数据（尺寸/锚点），1× 像素用完即弃 ⇒
//!    不会把整幅原帧留在内存里；条目数上限 [`MAX_CACHE_ENTRIES`]，超了停止新增（只慢不错）。
//!    命中与否**不影响像素**，只影响耗时。
//!
//! # 五、本模块的边界
//!
//! 只出 RGBA（[`Overview::rgba`]），**不碰 GUI / 网页 / 版本号**，也不改任何既有像素语义：
//! `compose` 的三层循环只是改调 [`crate::compose::层取帧`] / [`crate::compose::层放置`]，
//! 输出逐字节不变（原有 82 条单测一条断言都没改）。

use std::collections::HashMap;
use std::rc::Rc;

use mir2_assets::{Frame, LibRef, SpriteSource};
use mir2_map::Map;

use crate::canvas::Canvas;
use crate::compare::resize_to;
use crate::compose::{取格, 层别, 层取帧, 层放置};
use crate::frames::MAX_CACHE_ENTRIES;
use crate::png_io::PngImage;
use crate::types::{LayerMask, RenderOptions, CELL_H, CELL_W};

/// 总览级别的**上游上限**（16 级 mip）。
///
/// 出处：`_upstream/CrystalStudio/MinimapExporter.cs:47-48` 的 `level = clamp((int)floor(log2(1/scale)), 0, 16)`。
/// 超过它一律报 [`OverviewError::LevelOutOfRange`]，不再无限抬档。
pub const OVERVIEW_MAX_LEVEL: u8 = 16;

/// 单次总览渲染的**目标缓冲**字节上限（64 MiB）。
///
/// 与 [`crate::SCALED_MAX_BYTES`] 同为 64 MiB，但管的是**另一个缓冲**：
/// 那条管「1× 合成后缩放」的目标图，这条管「总览图」本身。两条护栏都只允许
/// 在分配之前判定，超了抬档（见模块文档 §三），不硬渲、不截断。
pub const OVERVIEW_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// 一张总览图：`1/2^level` 尺度的三层合成结果。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Overview {
    /// 像素宽 = `ceil(地图宽 * 48 / 2^level)`。
    pub width: u32,
    /// 像素高 = `ceil(地图高 * 32 / 2^level)`。
    pub height: u32,
    /// **实际采用**的级别（预算不够时被抬过，可能大于调用方传入的 `level`）。
    ///
    /// 调用方（小地图面板）必须按它换算坐标：`1 px = 2^level` 个世界像素。
    pub level: u8,
    /// 行优先 RGBA8，长度 = `width * height * 4`；底色**全透明黑** `(0,0,0,0)`。
    pub rgba: Vec<u8>,
}

impl Overview {
    /// 取某像素 RGBA；越界返回 `None`。
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let s = self.rgba.get(i..i + 4)?;
        Some([s[0], s[1], s[2], s[3]])
    }
}

/// 总览渲染失败的原因（**能明确说清的三类**；缺帧不是错误，按「不画」处理）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OverviewError {
    /// 地图宽或高为 0：没有可合成的格。
    ///
    /// 与 [`crate::RenderError::ZeroCells`] 同一条纪律（`Map::parse` 本来就拒收 0 尺寸，
    /// 这条是防御性护栏，不是常态路径）。
    ZeroCells,
    /// `level` 超过上游 16 级上限（[`OVERVIEW_MAX_LEVEL`]）。
    LevelOutOfRange {
        /// 调用方传入的级别。
        level: u8,
    },
    /// 抬到 [`OVERVIEW_MAX_LEVEL`] 仍超过 [`OVERVIEW_MAX_BYTES`]。
    BudgetExceeded {
        /// 抬到头时采用的级别（恒为 [`OVERVIEW_MAX_LEVEL`]）。
        level: u8,
        /// 该级别下的目标宽（像素）。
        width: u32,
        /// 该级别下的目标高（像素）。
        height: u32,
        /// 该级别下目标缓冲需要的字节数（`width * height * 4`）。
        want: u64,
        /// 上限（= [`OVERVIEW_MAX_BYTES`]）。
        limit: u64,
    },
}

impl core::fmt::Display for OverviewError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OverviewError::ZeroCells => f.write_str("地图宽或高为 0：没有可合成总览的格"),
            OverviewError::LevelOutOfRange { level } => write!(
                f,
                "总览级别 level = {level} 超出上游上限（只支持 0..={OVERVIEW_MAX_LEVEL}，\
                 即 16 级 mip，`MinimapExporter.cs:47-48` 的 clamp 上界）"
            ),
            OverviewError::BudgetExceeded {
                level,
                width,
                height,
                want,
                limit,
            } => write!(
                f,
                "总览目标缓冲需要 {want} 字节（{:.1} MiB，{width}×{height} px @ level {level}），\
                 已抬到 level {level} 仍超过上限 {limit} 字节（{:.0} MiB）：\
                 请缩小地图或换更粗的总览级别（不分配大内存，也不静默降档截断）",
                *want as f64 / (1024.0 * 1024.0),
                *limit as f64 / (1024.0 * 1024.0)
            ),
        }
    }
}

impl std::error::Error for OverviewError {}

/// 在 `1/2^level` 尺度上合成整张地图的三层总览图。
///
/// # 参数
///
/// - `map`：整张地图（**不是**视口窗口：总览恒从 `(0,0)` 起铺满全图）；
/// - `src`：图库帧源（`mir2_assets::SpriteSource`，与 1× 合成同一个）；
/// - `level`：起始级别，`1 px = 2^level` 世界像素；超出 [`OVERVIEW_MAX_LEVEL`] 报
///   [`OverviewError::LevelOutOfRange`]，预算不够时会被**上调**（看 [`Overview::level`]）；
/// - `mask`：三层开关，口径与 [`crate::try_render_region`] 完全一致（关掉的层不铺）；
/// - `options`：只消费 [`RenderOptions::doors_open`]（改变表层帧号）；光照不生效
///   （模块文档 §四.2）。
///
/// # 输出
///
/// `ceil(W*48/2^level) × ceil(H*32/2^level)` 的 RGBA8 图，底色全透明黑。
/// `700×700` 格 @ `level = 4` ⇒ `2100 × 1400 px`。
///
/// # 缺帧
///
/// 取不到帧 ⇒ 该帧**不画**（不画占位色、不计数），与 [`crate::compose`] 的「禁假数据」
/// 纪律一致；该层图号为 0 ⇒ 本来就没图，同样不画。
///
/// # 例子
///
/// ```
/// use miredit_render::{render_overview, LayerMask, RenderOptions};
/// use mir2_map::Map;
/// use mir2_assets::{Frame, LibRef, SpriteSource};
///
/// /// 空源：取不到任何帧（缺帧按「不画」处理）。
/// struct 空源;
/// impl SpriteSource for 空源 {
///     fn frame(&self, _lib: LibRef, _index: u32) -> Option<Frame> { None }
/// }
///
/// // 4×2 格的全空地图：52 B 文件头 + 12 B × 8 格 = 148 B
/// let mut bytes = vec![0u8; 52 + 12 * 8];
/// bytes[0..2].copy_from_slice(&4u16.to_le_bytes());
/// bytes[2..4].copy_from_slice(&2u16.to_le_bytes());
/// let map = Map::parse(&bytes)?;
///
/// let ov = render_overview(&map, &空源, 4, &LayerMask::all(), &RenderOptions::default())?;
/// // 4*48/16 = 12，2*32/16 = 4
/// assert_eq!((ov.width, ov.height), (12, 4));
/// assert_eq!(ov.level, 4);
/// assert!(ov.rgba.iter().all(|b| *b == 0), "全空图必须全透明");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn render_overview(
    map: &Map,
    src: &dyn SpriteSource,
    level: u8,
    mask: &LayerMask,
    options: &RenderOptions,
) -> Result<Overview, OverviewError> {
    if map.width == 0 || map.height == 0 {
        return Err(OverviewError::ZeroCells);
    }
    if level > OVERVIEW_MAX_LEVEL {
        return Err(OverviewError::LevelOutOfRange { level });
    }
    let (level, width, height) = 定档(
        u32::from(map.width),
        u32::from(map.height),
        level,
        OVERVIEW_MAX_BYTES,
    )?;
    let 缩放比 = 1u32 << level;
    let mut canvas = Canvas::new(width, height);
    // 预缩结果缓存：同一 `(库, 帧号)` 只取帧 + 预缩一次（模块文档 §四.5）；
    // `None` 也缓存（缺帧不打第二次库）。**存的是预缩后的帧**，1× 像素用完即弃
    // ⇒ 不会把整幅原帧留在内存里（见 [`缩帧`]）。
    let mut 缩过: HashMap<(LibRef, u32), Option<Rc<缩帧>>> = HashMap::new();

    let cells_w = i32::from(map.width);
    let cells_h = i32::from(map.height);
    // 层序与层内顺序照 `compose`（地表 → 中层 → 表层；`dy` 外 / `dx` 内 = 行优先）。
    // ⚠ 层内序原为列优先（`dx` 外 / `dy` 内），`compose` 于 `d66237b` 改行优先后此处同步
    // （模块文档 §四.4 有订正记录）—— 否则「总览 = 1× 合成图的盒式降采样」这条等价门
    // 的前提（两者层内序一致）就断了。
    let 层们 = [
        (层别::地表, mask.ground),
        (层别::中层, mask.middle),
        (层别::表层, mask.front),
    ];
    for (层, 开) in 层们 {
        if !开 {
            continue;
        }
        for dy in 0..cells_h {
            for dx in 0..cells_w {
                let Some(cell) = 取格(map, dx, dy) else {
                    continue;
                };
                // 偶偶格 / 库路由 / 帧号 / 门偏移 / 加性标志：与 1× 合成同一份（模块文档 §二）
                let Some(取样) = 层取帧(层, dx, dy, cell, *options) else {
                    continue;
                };
                let Some(缩帧) = 取缩帧(&mut 缩过, src, 取样.库, 取样.索引, level)
                else {
                    continue; // 缺帧：不画（不画占位色），总览不计数
                };
                // 落格锚点也是同一份（用**原帧**的尺寸/锚点，不是预缩后的）；
                // 随后把 1× 位移换算到 level 尺度：`floor(px / 2^level)`。
                let (px, py) = 层放置(
                    层,
                    dx,
                    dy,
                    map.format,
                    cell.area,
                    &取样,
                    缩帧.原宽,
                    缩帧.原高,
                    缩帧.锚点,
                );
                let (ax, ay) = (px.div_euclid(缩放比 as i32), py.div_euclid(缩放比 as i32));
                // 混合方式与 1× 合成一致：加性帧走 `blit_additive`，其余走 `blit`
                if 取样.加性 {
                    canvas.blit_additive(&缩帧.图, ax, ay);
                } else {
                    canvas.blit(&缩帧.图, ax, ay);
                }
            }
        }
    }

    Ok(Overview {
        width,
        height,
        level,
        rgba: canvas.into_rgba(),
    })
}

/// 目标尺寸：`ceil(cells_w*48 / 2^level) × ceil(cells_h*32 / 2^level)`（`u64`，不 cast）。
fn 目标尺寸(cells_w: u32, cells_h: u32, level: u8) -> (u64, u64) {
    let 缩放比 = 1u64 << level;
    let 宽 = u64::from(cells_w) * u64::from(CELL_W);
    let 高 = u64::from(cells_h) * u64::from(CELL_H);
    (宽.div_ceil(缩放比), 高.div_ceil(缩放比))
}

/// 预算定档：从 `起始` 起逐级抬 `level`，直到目标缓冲 ≤ `预算`；抬到
/// [`OVERVIEW_MAX_LEVEL`] 仍超 ⇒ [`OverviewError::BudgetExceeded`]。
///
/// **在分配任何缓冲之前调用**（模块文档 §三.1）。`u32` 转换是安全的：`Map` 的宽高是
/// `u16` ⇒ `w ≤ 65535*48 = 3,145,680 < u32::MAX`、`h ≤ 65535*32 = 2,097,120 < u32::MAX`。
fn 定档(
    cells_w: u32,
    cells_h: u32,
    起始: u8,
    预算: u64,
) -> Result<(u8, u32, u32), OverviewError> {
    // 防御性收敛：本函数只在 `render_overview` 校验过 `level ≤ 16` 之后被调，
    // 这里再夹一次是为了让「1u64 << level」在任何调用下都不会溢出。
    let mut level = 起始.min(OVERVIEW_MAX_LEVEL);
    loop {
        let (w, h) = 目标尺寸(cells_w, cells_h, level);
        let want = w * h * 4;
        if want <= 预算 {
            return Ok((level, w as u32, h as u32));
        }
        if level >= OVERVIEW_MAX_LEVEL {
            return Err(OverviewError::BudgetExceeded {
                level,
                width: w as u32,
                height: h as u32,
                want,
                limit: 预算,
            });
        }
        level += 1;
    }
}

/// 预缩后的一帧 **+ 落格需要的原帧元数据**。
///
/// 为什么要带原帧元数据：落格锚点（[`crate::compose::层放置`]）要的是 **1× 尺寸与锚点**
/// （`底边锚点` 的 `32 - 帧高`、`library == 28` 特例的 `X/Y`），不是预缩后的尺寸
/// ⇒ 预缩完就能把 1× 像素丢掉（本结构只留元数据），避免整幅原帧常驻内存。
struct 缩帧 {
    /// 已按 `2^level` 预缩的帧（尺寸 = `max(1, (w + s - 1) >> level)`，上游 `_upstream/CrystalStudio/Assets.cs:119`）。
    图: Rc<Frame>,
    /// 原帧宽（1×，落格用）。
    原宽: u32,
    /// 原帧高（1×，落格用）。
    原高: u32,
    /// 原帧锚点（1×，落格用）。
    锚点: (i16, i16),
}

/// 取一帧并按 `2^level` 盒式预缩，按 `(库, 帧号)` 缓存（`None` 也缓存）。
///
/// - **尺寸**：`max(1, (w + (1<<level) - 1) >> level)`，逐字照上游 `_upstream/CrystalStudio/Assets.cs:119`；
/// - **采样**：复用 [`resize_to`]（面积平均），**本模块不另写采样器**；
/// - **条目上限**：[`MAX_CACHE_ENTRIES`]（与 `frames` 的 1× 缓存同一条纪律）——
///   超出后**停止新增**，宁可重复取帧/预缩也不无限吃内存（正确性不受影响）。
fn 取缩帧(
    缓存: &mut HashMap<(LibRef, u32), Option<Rc<缩帧>>>,
    src: &dyn SpriteSource,
    库: LibRef,
    索引: u32,
    level: u8,
) -> Option<Rc<缩帧>> {
    if let Some(命中) = 缓存.get(&(库, 索引)) {
        return 命中.clone();
    }
    let 缩放比 = 1u32 << level;
    let 结果 = src.frame(库, 索引).map(|帧| {
        let (原宽, 原高, 锚点) = (帧.width, 帧.height, 帧.anchor);
        let 宽 = ((原宽 + 缩放比 - 1) >> level).max(1);
        let 高 = ((原高 + 缩放比 - 1) >> level).max(1);
        // `resize_to` 的入参是 `PngImage`（自有缓冲）⇒ 每帧一次整幅移交（不做额外拷贝）。
        let 源 = PngImage {
            width: 原宽,
            height: 原高,
            rgba: 帧.rgba,
        };
        let 缩 = resize_to(&源, 宽, 高);
        Rc::new(缩帧 {
            图: Rc::new(Frame {
                width: 宽,
                height: 高,
                rgba: 缩.rgba,
                anchor: 锚点,
            }),
            原宽,
            原高,
            锚点,
        })
    });
    if 缓存.len() < MAX_CACHE_ENTRIES {
        缓存.insert((库, 索引), 结果.clone());
    }
    结果
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{try_render_region_with_options, CollisionOverlay, Viewport};

    /// 造一张 `w×h` 的 Old12 地图：52 B 文件头 + 每格 12 B（**列优先**，与
    /// [`mir2_map::Map::cells`] 的排布一致：`cells[x * height + y]`）。
    fn 造图(w: u16, h: u16, 填: impl Fn(u16, u16) -> mir2_map::Cell) -> Map {
        let mut bytes = vec![0u8; 52 + 12 * (w as usize * h as usize)];
        bytes[0..2].copy_from_slice(&w.to_le_bytes());
        bytes[2..4].copy_from_slice(&h.to_le_bytes());
        for x in 0..w {
            for y in 0..h {
                let 格 = 填(x, y);
                let base = 52 + (x as usize * h as usize + y as usize) * 12;
                bytes[base..base + 2].copy_from_slice(&格.bk_img.to_le_bytes());
                bytes[base + 2..base + 4].copy_from_slice(&格.mid_img.to_le_bytes());
                bytes[base + 4..base + 6].copy_from_slice(&格.fr_img.to_le_bytes());
            }
        }
        Map::parse(&bytes).expect("合成地图应能解析")
    }

    /// 空源：取不到任何帧（用来验证「缺帧 = 不画」，以及纯尺寸/预算用例）。
    struct 空源;

    impl SpriteSource for 空源 {
        fn frame(&self, _lib: LibRef, _index: u32) -> Option<Frame> {
            None
        }
    }

    /// 16×16 块内恒定的假帧源（**等价门专用**）。
    ///
    /// 每个 `16×16` 块内 RGBA 完全一致，块与块之间（以及帧与帧、族与族之间）都不同，
    /// 透明度也逐块在 `0 / 128 / 192 / 255` 之间循环（`0` 档走「不碰目标」路径）。
    ///
    /// **为什么必须块内恒定**：`over(over(平均A, 平均B), 平均C)` 一般**不等于**
    /// `平均(over(over(A, B), C))` —— 逐帧预缩与整幅盒式平均只在「每块内容是常量」
    /// 时可交换。构造出这个前提后，本用例比的是**放置**（错块/错像素/漏层），
    /// 而不是采样器的舍入。
    struct 块源;

    /// 各族的帧尺寸（16 的倍数）：地表 96×64（6×4 块），中层/表层 48×32（3×2 块）。
    fn 块面尺寸(lib: LibRef) -> (u32, u32) {
        match lib.family {
            mir2_assets::Family::Ground => (96, 64),
            _ => (48, 32),
        }
    }

    /// 某个 `16×16` 块的 RGBA：族 / 帧号 / 块坐标三者都参与混淆 ⇒ 相邻格、相邻块必然不同。
    fn 块色(族: u32, 帧号: u32, bx: u32, by: u32) -> [u8; 4] {
        let h = 族.wrapping_mul(0x9E37_79B9)
            ^ 帧号.wrapping_mul(0x85EB_CA6B)
            ^ bx.wrapping_mul(0xC2B2_AE35)
            ^ by.wrapping_mul(0x27D4_EB2F);
        [
            (h & 0xFF) as u8,
            ((h >> 8) & 0xFF) as u8,
            ((h >> 16) & 0xFF) as u8,
            [0u8, 128, 192, 255][((h >> 24) & 0x3) as usize],
        ]
    }

    impl SpriteSource for 块源 {
        fn frame(&self, lib: LibRef, index: u32) -> Option<Frame> {
            let (w, h) = 块面尺寸(lib);
            let 族 = lib.family.index() as u32;
            let mut rgba = Vec::with_capacity((w * h) as usize * 4);
            for py in 0..h {
                for px in 0..w {
                    rgba.extend_from_slice(&块色(族, index, px / 16, py / 16));
                }
            }
            Some(Frame {
                width: w,
                height: h,
                rgba,
                anchor: (0, 0),
            })
        }
    }

    /// 逐像素变化的假帧源（**面积平均证据**）：96×64 地面帧，RGB 按 `(x, y)` 变化
    /// 且周期互不相同 ⇒ 任何「丢点采样」都会取到与块均值不同的值。
    struct 逐像素源;

    impl SpriteSource for 逐像素源 {
        fn frame(&self, _lib: LibRef, index: u32) -> Option<Frame> {
            let (w, h) = (96u32, 64u32);
            let 帧号 = index;
            let mut rgba = Vec::with_capacity((w * h) as usize * 4);
            for y in 0..h {
                for x in 0..w {
                    rgba.extend_from_slice(&[
                        ((x * 7 + 帧号 * 3) % 251) as u8,
                        ((y * 11 + 帧号 * 5) % 253) as u8,
                        ((x * y + 帧号) % 249) as u8,
                        255,
                    ]);
                }
            }
            Some(Frame {
                width: w,
                height: h,
                rgba,
                anchor: (0, 0),
            })
        }
    }

    /// 三层都带图号的满图：**每一格的三层帧号都不同**（相邻格不同 ⇒ 错位必然可测）。
    ///
    /// 地表在奇格上也有图号 —— 专门用来证明「只铺偶偶格」这条前提真的生效了
    /// （若总览漏了偶偶格过滤，奇格会多画出地表色，等价门立刻红）。
    fn 三层满图(w: u16, h: u16) -> Map {
        造图(w, h, |x, y| {
            let (x, y) = (u32::from(x), u32::from(y));
            let 高 = u32::from(h);
            mir2_map::Cell {
                bk_img: 1 + ((x / 2) * 高 + (y / 2)) as u16,
                mid_img: 1 + (x * 高 + y) as u16,
                fr_img: 1 + (x * 高 + y) as u16,
                ..Default::default()
            }
        })
    }

    /// **流式盒式降采样（测试基准，不是产品代码）**：每个输出像素 = 对应
    /// `s×s` 块的四通道算术平均，舍入口径与 [`crate::compare::resize_to`] 逐字相同
    /// （`(累加 + n/2) / n`）。内存只有一行累加器（`O(宽)`），按源行带推进 ——
    /// 这就是 `scale.rs` 里「盒式_流式」那套手法在测试侧的复现。
    ///
    /// `偏x` / `偏y` 是**采样网格偏移**（反假绿用）：输出尺寸仍取
    /// `img.width / s`，最后一列/行的窗口按源边缘裁剪。
    fn 盒式块(img: &PngImage, s: u32, 偏x: u32, 偏y: u32) -> PngImage {
        assert_eq!(img.width % s, 0, "基准要求源宽能被 {s} 整除");
        assert_eq!(img.height % s, 0, "基准要求源高能被 {s} 整除");
        let dw = img.width / s;
        let dh = img.height / s;
        let mut out = vec![0u8; (dw * dh * 4) as usize];
        // [r, g, b, a, 计数]
        let mut 累加 = vec![[0u64; 5]; dw as usize];
        for oy in 0..dh {
            for 槽 in 累加.iter_mut() {
                *槽 = [0; 5];
            }
            let y0 = (偏y + oy * s).min(img.height);
            let y1 = (偏y + (oy + 1) * s).min(img.height);
            if y0 >= y1 {
                continue;
            }
            for sy in y0..y1 {
                for (ox, 槽) in 累加.iter_mut().enumerate() {
                    let 目标x = ox as u32;
                    let x0 = (偏x + 目标x * s).min(img.width);
                    let x1 = (偏x + (目标x + 1) * s).min(img.width);
                    for sx in x0..x1 {
                        let i = (sy as usize * img.width as usize + sx as usize) * 4;
                        for (c, 通道) in 槽.iter_mut().take(4).enumerate() {
                            *通道 += u64::from(img.rgba[i + c]);
                        }
                        槽[4] += 1;
                    }
                }
            }
            for (ox, 槽) in 累加.iter().enumerate() {
                let n = 槽[4];
                if n == 0 {
                    continue;
                }
                let di = (oy as usize * dw as usize + ox) * 4;
                for (c, 值) in out[di..di + 4].iter_mut().enumerate() {
                    *值 = ((槽[c] + n / 2) / n) as u8;
                }
            }
        }
        PngImage {
            width: dw,
            height: dh,
            rgba: out,
        }
    }

    /// T1：`700×700` 格 @ `level = 4` ⇒ `2100×1400`（`48/16 = 3`、`32/16 = 2`）。
    #[test]
    fn 尺寸_七百乘七百_level4是2100乘1400() {
        let map = 造图(700, 700, |_, _| mir2_map::Cell::default());
        let ov = render_overview(&map, &空源, 4, &LayerMask::all(), &RenderOptions::default())
            .expect("全空 700×700 应能出总览");
        assert_eq!((ov.width, ov.height), (2100, 1400));
        assert_eq!(
            ov.level, 4,
            "700×700 @ level4 = 11.76 MiB，在 64 MiB 预算内，不该抬档"
        );
        assert_eq!(ov.rgba.len(), 2100 * 1400 * 4);
        assert!(ov.rgba.iter().all(|b| *b == 0), "全空图必须全透明");
    }

    /// T1 补充：`level > 4` 时 `48 / 32 = 1.5`、`32 / 32 = 1` ⇒ **向上取整真的会生效**
    /// （`37*48 = 1776`：`1776/32 = 55.5 ⇒ 56`、`1776/64 = 27.75 ⇒ 28`）。
    #[test]
    fn 尺寸_level大于四时向上取整() {
        let map = 造图(37, 23, |_, _| mir2_map::Cell::default());
        let l5 = render_overview(&map, &空源, 5, &LayerMask::all(), &RenderOptions::default())
            .expect("应能出总览");
        assert_eq!(
            (l5.width, l5.height),
            (56, 23),
            "1776/32 向上取整 = 56，736/32 = 23"
        );
        assert_eq!(l5.rgba.len(), 56 * 23 * 4);

        let l6 = render_overview(&map, &空源, 6, &LayerMask::all(), &RenderOptions::default())
            .expect("应能出总览");
        assert_eq!(
            (l6.width, l6.height),
            (28, 12),
            "1776/64 向上取整 = 28，736/64 = 11.5 向上取整 = 12"
        );
    }

    /// T2（本单核心）：16 对齐假帧下，总览像素**逐字节等于**
    /// 「1× 合成图按 `16×16` 块面积平均」。
    #[test]
    fn 总览与整幅盒式降采样逐字节相同() {
        const 边: u32 = 32;
        let map = 三层满图(边 as u16, 边 as u16);
        let mask = LayerMask::all();
        let opts = RenderOptions::default();

        // 1× 基准：走公开入口（同一份 `层取帧` / `层放置`），整张图 = 视口
        let 一 = try_render_region_with_options(
            &map,
            &块源,
            &Viewport::new(0, 0, 边, 边),
            &mask,
            CollisionOverlay::none(),
            opts,
        )
        .expect("1× 合成应能渲染");
        assert_eq!(
            (一.width, 一.height),
            (边 * 48, 边 * 32),
            "1× 尺寸 = 格数 × 世界格"
        );
        let 一图 = PngImage {
            width: 一.width,
            height: 一.height,
            rgba: 一.rgba,
        };

        let 总览 = render_overview(&map, &块源, 4, &mask, &opts).expect("总览应能渲染");
        assert_eq!(
            (总览.width, 总览.height),
            (96, 64),
            "32*48/16 = 96，32*32/16 = 64"
        );
        assert_eq!(总览.level, 4, "96×64×4 = 24 KiB，远在预算内");

        let 基准 = 盒式块(&一图, 16, 0, 0);
        assert_eq!((基准.width, 基准.height), (96, 64));
        let 字节数 = 总览.rgba.len();
        assert_eq!(字节数, 96 * 64 * 4, "逐字节比对的总字节数");
        assert_eq!(
            总览.rgba, 基准.rgba,
            "总览必须逐字节等于 1× 合成的 16×16 盒式平均（共 {字节数} 字节）"
        );

        // 反假绿：把采样网格整体错开**半个块**（8 px）⇒ 必须不同，
        // 否则说明本用例的夹具根本分辨不出「错位一个像素」。
        let 错位 = 盒式块(&一图, 16, 8, 8);
        assert_ne!(
            总览.rgba, 错位.rgba,
            "夹具失效：错位采样与正确采样给出同一结果"
        );

        // 另一个反假绿方向：只关一层，结果也必须变（否则「层序/开关」没被真渲染）
        let 只地表 = render_overview(
            &map,
            &块源,
            4,
            &LayerMask {
                ground: true,
                middle: false,
                front: false,
            },
            &opts,
        )
        .expect("只开地表应能渲染");
        assert_ne!(总览.rgba, 只地表.rgba, "关掉中层/表层后像素必须不同");
    }

    /// T2 补充：**单层、逐像素变化**的帧也必须逐字节等于面积平均 ⇒ 证明这里用的是
    /// 盒式面积平均（而不是丢点采样）。
    #[test]
    fn 单层任意内容也是面积平均() {
        const 宽: u32 = 24;
        const 高: u32 = 18;
        // 偶偶格铺 96×64 地表帧（覆盖 2×2 格，互不重叠、无遮挡 ⇒ 与「平均后可交换」无关，
        // 这一条专测采样器本身）
        let map = 造图(宽 as u16, 高 as u16, |x, y| mir2_map::Cell {
            bk_img: if x % 2 == 0 && y % 2 == 0 {
                1 + (u32::from(x / 2) * 高 + u32::from(y / 2)) as u16
            } else {
                0
            },
            ..Default::default()
        });
        let 只地表 = LayerMask {
            ground: true,
            middle: false,
            front: false,
        };
        let opts = RenderOptions::default();
        let 一 = try_render_region_with_options(
            &map,
            &逐像素源,
            &Viewport::new(0, 0, 宽, 高),
            &只地表,
            CollisionOverlay::none(),
            opts,
        )
        .expect("1× 合成应能渲染");
        let 一图 = PngImage {
            width: 一.width,
            height: 一.height,
            rgba: 一.rgba,
        };
        let 总览 = render_overview(&map, &逐像素源, 4, &只地表, &opts).expect("总览应能渲染");
        assert_eq!(
            (总览.width, 总览.height),
            (72, 36),
            "24*48/16 = 72，18*32/16 = 36"
        );
        let 基准 = 盒式块(&一图, 16, 0, 0);
        assert_eq!(总览.rgba, 基准.rgba, "逐像素变化的帧也必须等于面积平均");

        // 反假绿：丢点采样（每块左上角那个像素）与面积平均**必然不同**
        let mut 最近邻 = Vec::with_capacity(总览.rgba.len());
        for oy in 0..基准.height {
            for ox in 0..基准.width {
                let p = 一图.pixel(ox * 16, oy * 16).expect("块左上角必在源图内");
                最近邻.extend_from_slice(&p);
            }
        }
        assert_ne!(
            总览.rgba, 最近邻,
            "若相等说明实现退化成丢点采样，不是 `resize_to` 的面积平均"
        );
    }

    /// T3：`1700×1700` 格 @ level 4 的目标缓冲 `5100×3400×4 = 69,360,000 B > 64 MiB`
    /// ⇒ 必须**抬档**而不是超配；抬到 level 5 后 `2550×1700×4 = 17.34 MB` 在限内。
    #[test]
    fn 预算超限时升档而不是超配() {
        const 边: u16 = 1700;
        let level4 = 5100u64 * 3400 * 4;
        assert!(
            level4 > OVERVIEW_MAX_BYTES,
            "夹具失效：level4 目标 {level4} B 必须超过上限 {OVERVIEW_MAX_BYTES} B"
        );
        let map = 造图(边, 边, |_, _| mir2_map::Cell::default());
        // 层全关：本用例只验「定档 + 缓冲尺寸」，不必真扫 289 万格
        let ov = render_overview(
            &map,
            &空源,
            4,
            &LayerMask::none(),
            &RenderOptions::default(),
        )
        .expect("抬档后应能成功");
        assert_eq!(
            ov.level, 5,
            "level 4 超限 ⇒ 必须抬到 5（level 5 才在 64 MiB 内）"
        );
        assert_eq!(
            (ov.width, ov.height),
            (2550, 1700),
            "1700*48/32，1700*32/32"
        );
        assert_eq!(ov.rgba.len(), 2550 * 1700 * 4);
        assert!(
            ov.rgba.len() as u64 <= OVERVIEW_MAX_BYTES,
            "实际分配 {} B 必须 ≤ 上限 {} B",
            ov.rgba.len(),
            OVERVIEW_MAX_BYTES
        );
        assert!(ov.rgba.iter().all(|b| *b == 0), "层全关 ⇒ 全透明");
    }

    /// T4：抬到 16 级**仍超**必须报中文错误（用内部定档函数把预算压到 1 字节）。
    #[test]
    fn 十六级仍超时返回中文原因() {
        // 1700×1700 @ level16：ceil(81600/65536) = 2、ceil(54400/65536) = 1 ⇒ 2*1*4 = 8 B
        let 错 = 定档(1700, 1700, 4, 1).expect_err("预算 1 字节时必然失败");
        match &错 {
            OverviewError::BudgetExceeded {
                level,
                width,
                height,
                want,
                limit,
            } => {
                assert_eq!(*level, OVERVIEW_MAX_LEVEL, "必须抬到 16 级才放弃");
                assert_eq!((*width, *height), (2, 1));
                assert_eq!(*want, 8);
                assert_eq!(*limit, 1);
            }
            其它 => panic!("应报 BudgetExceeded，实际 {其它:?}"),
        }
        let 文案 = 错.to_string();
        assert!(
            文案.contains("level 16"),
            "错误文案必须写明抬到了 16 级：{文案}"
        );
        assert!(
            文案.contains("不分配大内存"),
            "错误文案必须说明不分配大内存：{文案}"
        );
        assert!(
            文案.contains("静默降档"),
            "错误文案必须说明不静默降档：{文案}"
        );
    }

    /// 额外：全空图 ⇒ 全透明但尺寸正确（不崩）。
    #[test]
    fn 全空图输出全透明且尺寸正确() {
        let map = 造图(5, 3, |_, _| mir2_map::Cell::default());
        // 帧源本身有帧（块源），但图号全 0 ⇒ 一帧都不会被请求
        let ov = render_overview(&map, &块源, 4, &LayerMask::all(), &RenderOptions::default())
            .expect("全空图必须能出总览");
        assert_eq!((ov.width, ov.height), (15, 6), "5*48/16 = 15，3*32/16 = 6");
        assert_eq!(ov.rgba.len(), 15 * 6 * 4);
        assert!(ov.rgba.iter().all(|b| *b == 0), "全空图必须全透明");
    }

    /// 额外：零格地图（`Map::parse` 本来就不产，这里用字面量造）⇒ 明确报错而不是崩。
    #[test]
    fn 零格地图明确报错而不是崩() {
        let 空 = Map {
            header_raw: [0u8; 52],
            width: 0,
            height: 0,
            format: mir2_map::CellFormat::Old12,
            cells: Vec::new(),
        };
        assert_eq!(
            render_overview(&空, &空源, 4, &LayerMask::all(), &RenderOptions::default())
                .unwrap_err(),
            OverviewError::ZeroCells
        );
    }

    /// 额外：`level > 16` ⇒ 明确报错（不静默 clamp，免得调用方按错的 `2^level` 换算）。
    #[test]
    fn 级别超上游上限明确报错() {
        let map = 造图(2, 2, |_, _| mir2_map::Cell::default());
        let 错 = render_overview(
            &map,
            &空源,
            17,
            &LayerMask::all(),
            &RenderOptions::default(),
        )
        .unwrap_err();
        assert_eq!(错, OverviewError::LevelOutOfRange { level: 17 });
        assert!(
            错.to_string().contains("0..=16"),
            "文案要给出合法区间：{错}"
        );
    }
}
