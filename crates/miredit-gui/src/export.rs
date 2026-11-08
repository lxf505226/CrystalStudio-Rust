//! 导出：**选区 PNG**（文件 → 导出选区(E)…）与**小地图 PNG**（文件 → 导出小地图(M)…）。
//!
//! # 一、为什么需要「分块渲染」，而不是一次 `try_render_region_scaled_with`
//!
//! ⚠ **2026-11-06 起，分块渲染的实现整体上移到 `miredit_render::tiled`**
//! （网页端 `/api/export/minimap` 也走它）。本模块只保留：`render_tiled` 的薄转调
//! （`TiledError` → `String`、`TiledImage` → [`ExportImage`]）、选档、两段式缩放、
//! 原子写与 CLI/菜单入口。**下面 §一/§二 描述的是那份共享实现的判据**（保留原文，
//! 因为它就是这个设计的存在理由），代码位置见 [`miredit_render::tiled::render_tiled`]。
//!
//! `miredit-render` 的两条护栏（`types.rs:32,35,48,95`，**本 crate 一个字都不改**）：
//!
//! | 常量 | 值 | 管什么 |
//! |---|---|---|
//! | `MAX_SIDE` | 32768 px | 1× 合成画布单边 |
//! | `MAX_PIXELS` | 256,000,000 px | 1× 合成画布总像素 |
//! | `SCALED_MAX_BYTES` | 64 MiB | **目标图**字节数（缩放档） |
//! | `INTERMEDIATE_MAX_BYTES` | 128 MiB | **1× 中间图**字节数（缩放档） |
//!
//! 而 `try_render_region_scaled_with` 的流程是「**先按 1× 合成整幅，再重采样**」
//! （`scale.rs` 模块文档 §二）⇒ 整图 1× 中间图是绕不过去的。`0.map` 实测 **700×700 格**：
//!
//! ```text
//! 1× 中间图 = 700*48 × 700*32 = 33600 × 22400
//!           33600 > MAX_SIDE(32768)         ⇒ DimensionOverflow
//!           33600*22400 = 752,640,000 > MAX_PIXELS(256M) ⇒ DimensionOverflow
//! @0.25 目标 = 8400 × 5600 × 4 = 188,160,000 B > SCALED_MAX_BYTES(64 MiB)
//! ```
//!
//! ⇒ **任何档位单次调用都渲不出 700×700 整图**。所以本模块把窗口切成若干块，
//! 每块单独调 [`miredit_render::try_render_region_scaled_with`]（**采样 100% 由 render crate 做，
//! 本文件一行滤镜都没有**），再把各块按整数像素偏移拼回整幅。
//!
//! # 二、拼接为什么是**逐字节精确**的（两条依据）
//!
//! **① 缩放核按整块对齐**：`盒式_流式` 把输出像素 `j` 映到源像素 `[j*sw/dw, (j+1)*sw/dw)`；
//! 本模块的块边长恒为 `TILE_CELLS`（64 或 32，都能被缩小档的 4 / 2 整除），
//! 而世界格像素 `48 / 32` 也都能被 4 整除 ⇒ 每块内第 `j` 个输出像素覆盖的源矩形
//! **恰好**是整幅图里第 `(块起点/倍率 + j)` 个输出像素覆盖的同一块源像素。
//! ⇒ 缩小档（`0.25` / `0.5`）拼出来与「整图一次渲」逐字节相同。
//! `One` 档更直接：`try_render_region_scaled_with(.., One)` 内部 `return compose::render(..)`
//! （`scale.rs:174-176`），压根不进重采样。
//!
//! **② 精灵可能跨块**：`compose::render` 的三层循环**只遍历窗口内的格**
//! （`compose.rs:115/149/176` 的 `for dx in 0..cells_w`，没有任何外扩边距），
//! 而单帧可能比一格大得多 —— 地表 96×64（2×2 格）、表层帧宽可任意。
//! ⇒ 若直接按块切，块边界上会**丢像素**（块内最后一列的宽帧被画布裁掉）
//! 或**少画**（块外第一列的大帧本该溢进来）。
//! 本模块的处理：**每块各算一个扩边格数**，向四周多渲那么多格，再裁回块本身。
//! 扩边不是拍脑袋的常量，而是**按窗口里每一格的真实帧尺寸算出来的**
//! （见 [`tile_margins`]）；上限也由 render crate 的护栏反算（`margin_cap`，同模块内私有）。
//!
//! # 三、原子写（不许留半个文件）
//!
//! PNG 编码复用 [`miredit_render::png_io::write_rgba_png`]（与 `miredit-web` 的
//! `/api/tiles` **同一个编码器**，见 `miredit-web/src/routes/mod.rs:78-108`），
//! 但那个 API 只有「写文件」形态、没有「返回 `Vec<u8>`」形态 ⇒ 先写到系统临时目录、
//! 读回字节、立刻删掉，再把字节交给 [`crate::save::save_atomic`]
//! （`tmp → write_all → sync_all → 读回逐字节核对 → rename → 尽力 fsync 父目录`）。
//! ⇒ 失败时目标路径上**不会出现半个文件**，临时文件也由 `save_atomic` 自己清掉。
//!
//! # 四、选档 + 两段式（小地图）
//!
//! **回源订正（2026-11-06）**：上游新增小地图尺寸面板，官方尺寸口径已可得
//! ⇒ 小地图**缺省**改走 [`export_minimap_sized`] + [`MinimapSize::Default`]
//! （`W*12 × H*8`，官方 `_upstream/CrystalStudio/MinimapExporter.cs:20`）；三档预设与显式像素见
//! [`MinimapPreset`] / [`MinimapSize`]，官方 `ValidSize` 的 Rust 等价换算见
//! [`MINIMAP_MAX_PIXELS`]，官方 16 级 mip 到本仓 4 档的有据映射见 [`pick_official_scale`]。
//!
//! 本节下面的「长边两段式」是**本仓兼容链**（`--minimap-max-side`），
//! 由 [`export_minimap`] 原样保留，护栏与逐字节输出都不变；它**不再是官方缺省口径**。
//!
//! ⚠ **订正（2026-11-08）**：上一句里的「逐字节输出都不变」**只对护栏与选档成立**，
//! 对**走了第二段**的发次已不成立 —— 第二段按上游 `_upstream/CrystalStudio/PreviewRaster.cs:15-19,35-38`
//! 从面积平均换成了「等比 + 居中 + 不透明黑底 + 最近邻」，见下一小节。
//! 原文保留作历史引用（它说的是 2026-11-06 那次上移/接线没有改字节，与本次无关）。
//!
//! 见 [`pick_minimap_scale`]：从大到小试 `One → Half → Quarter`，取**第一个**同时
//! 满足「两边 ≤ [`MINIMAP_MAX_SIDE`]」且「整幅输出过得了 [`MAX_EXPORT_SIDE`] /
//! [`MAX_EXPORT_BYTES`]」的档位。`Two` 永远不选 —— 小地图放大没有意义。
//!
//! 最小档仍超目标长边时，**不报错、也不超限交付**，而是走**第二段最终合成**
//! （[`downscale_to_fit`] → [`fit_ratio`]），把整幅落到目标尺寸，
//! 且**长宽比与 1× 渲染逐位相等**（判据 `w*ch*32 == h*cw*48`）。
//!
//! ## 第二段用哪个入口：`preview_raster::fit_centered_opaque`
//!
//! ⚠ **订正（2026-11-08）**：本节原文标题是
//! 「## 为什么第二段用 `compare::resize_to`，而不是 `resample::盒式_流式`」，
//! 原文如下（**保留作历史引用**）：
//!
//! > 两条都是 render crate 自己的面积平均实现，但**只有前者是公开的**：
//! >
//! > | 入口 | 可见性 | 能不能从本 crate 调 |
//! > |---|---|---|
//! > | `resample::盒式_流式` | `lib.rs:162` `mod resample;` + `resample.rs:165` `pub(crate)` | **不能**（私有模块 + crate 内可见）|
//! > | `compare::resize_to` | `lib.rs:166` `pub mod compare;` + `compare.rs:154` `pub fn` | **能** |
//! >
//! > 两者语义同类（面积平均、确定性、不预乘）；内存上 `resize_to` 更省
//! > （额外只有每个目标像素一个 `[u64; 4]` 累加器，峰值 ≈ 源图 + 目标图），
//! > 而 `resample::缩放`（整幅预乘版）在 `8400×5600` 上要 `47 MP × 32 B ≈ 1.5 GB` —— 那条不能碰。
//! > ⇒ 本模块固定用 `compare::resize_to`，**不**去动 `miredit-render` 的可见性
//! > （为一个已有等价公开入口再加一条 API 表面，只会多一个无人消费的契约）。
//!
//! **实为**：那段论证只回答了「哪个**面积平均**入口能调」，
//! 而「第二段该不该是面积平均」这个**前提本身就是错的**。上游导出成图的最终合成是
//! `PreviewRaster.Create(drawings, area, width, height, …, padding: 0, opaque: true)`
//! （调用点 `_upstream/CrystalStudio/MinimapExporter.cs:56-57`），它是**等比 + 居中 + 不透明黑底 + 最近邻**：
//!
//! | 口径 | 上游位置 | 原式 |
//! |---|---|---|
//! | 等比缩放比 | `_upstream/CrystalStudio/PreviewRaster.cs:15` | `scale = Math.Min((width-padding*2)/area.Width, (height-padding*2)/area.Height)` |
//! | 居中偏移 | `_upstream/CrystalStudio/PreviewRaster.cs:16-17` | `offsetX=(width-area.Width*scale)/2-area.X*scale`（`offsetY` 同形）|
//! | 不透明黑底 | `_upstream/CrystalStudio/PreviewRaster.cs:19` | `if(opaque) for(int i=3;i<output.Length;i+=4) output[i]=255;`（RGB 保 0）|
//! | 最近邻取样 | `_upstream/CrystalStudio/PreviewRaster.cs:35,38` | `sy=(int)((y+.5-top)*pixels.Height/h)`、`sx=(int)((x+.5-left)*pixels.Width/w)` |
//!
//! 面积平均只出现在上游的 **mip 预缩**（`_upstream/CrystalStudio/OverviewPixels.cs:7-42` 的 `Reduce`），
//! 作用在**逐帧解码**阶段 —— 本仓对应物是第一段 `try_render_region_scaled_with` 的
//! 缩小档（`resample::盒式_流式`），**保持不动**。
//! 第二段（导出成图）已按上游改为
//! [`miredit_render::preview_raster::fit_centered_opaque`]
//! （crate 根也 re-export 成 [`miredit_render::fit_centered_opaque`]：`lib.rs` 的
//! `pub mod preview_raster;` + `pub use preview_raster::fit_centered_opaque;`
//! —— **这里不写行号**：render crate 正被并行改动，行号会漂，符号名不会），
//! 与 `miredit-web` 的 `GET /api/export/minimap` **同一个入口**。
//!
//! `compare::resize_to`（`compare.rs:154`）**保留未删、签名不变**：它仍是
//! `overview`（画布上屏的总览 mip 预缩）与 `compare_blocks`（诊断）的实现，
//! 只是本模块**不再用它**。
//!
//! # 五、诚实边界
//!
//! - 整幅输出超 [`MAX_EXPORT_BYTES`] / [`MAX_EXPORT_SIDE`] ⇒ **报错**，不截断、不降级；
//! - 官方尺寸链（[`export_minimap_sized`]）的 `0` / 超大宽高 ⇒ **中文原因报错**，
//!   不静默截断、不偷偷降级到别的档位（见 [`valid_minimap_size`]）；
//! - 官方预设要求的档位过不了本仓更严的内存护栏 ⇒ **中文原因报错**，不静默换档；
//! - 帧大到某一块需要的外扩超过 `margin_cap` ⇒ **报错**（那时拼接不再精确）；
//! - 没有选区 / 没有地图 / 没有图库 ⇒ 各自的**中文原因**，一个文件都不写。
//!
//! # 六、门态（`doors_open`）**透传**（2026-11-05 补：闭门边界收口）
//!
//! 本模块曾经**只出闭门图**（走 `try_render_region_scaled`，它内部写死
//! `RenderOptions::default()`）⇒ 「画布开着门、导出是闭门」，同一张图两副样子，
//! 用户会以为导出坏了。现在门态**从调用方一路透到底**：
//!
//! ```text
//! 菜单「视图 → 开启门」/ --open-doors   →  宿主真源（app.doors_open）
//!                                          ↓
//!                          export_selection(.., doors_open) / export_minimap(.., doors_open)
//!                                          ↓
//!                    render_tiled(.., doors_open)  →  try_render_region_scaled_with(
//!                                                        .., RenderOptions::with_doors_open(doors_open))
//! ```
//!
//! 缺省 **`false`**（= 闭门，`RenderOptions::default()`）⇒ 与接线前**逐字节相同**；
//! 判据：`0.map` 的 `(106,316)` 起 3×3 导出仍是 `32,842 B / sha256 e1acd638…4335`。
//!
//! ## 为什么门态还必须进 [`tile_margins`]（否则分块拼接会破）
//!
//! 门开合**只改前景层帧号**（`帧号 += door_offset & 0x7F`，见 `miredit-render` 的
//! `RenderOptions` 文档 §三/§四），而**门开帧与闭门帧的尺寸可以不同**（实测 `0.map`
//! 就有这种格）。本模块的扩边是按「每一格真实帧的像素矩形」算出来的（§二 ②）——
//! 若扩边仍按**闭门**帧尺寸算，开门态下大出好几格的门开帧就会跨过块边界却拿不到扩边
//! ⇒ 块边界上丢像素/少画，「分块拼接 == 整图一次渲」当场不成立。
//! ⇒ [`tile_margins`] 与 [`render_tiled`] 收**同一个** `doors_open`，两边算的是同一批帧。
//!
//! ## 门偏移 / 帧号**只有一份实现**（2026-11-05 收口）
//!
//! 本模块**不再自带** `帧号` / `门偏移`：两者直接调 [`miredit_render::帧号`] /
//! [`miredit_render::门偏移`]（render 侧已从 `pub(crate)` 提成 `pub` 并再导出）。
//! 起因就是这里曾各抄一份（跨 crate 取不到），抄件注释写着「改口径两边一起改」——
//! **两份必然漂，而漂移的症状正是「画布开门、导出闭门」**。
//! 判据：`grep -rn "0x7F" crates/miredit-gui/src/export.rs` 只剩注释，一行实现都没有。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use miredit_core::Selection;
use miredit_render::{
    png_io::write_rgba_png, CollisionOverlay, LayerMask, MapLighting, RenderOptions, RenderScale,
    CELL_H, CELL_W,
};
use mir2_assets::SpriteSource;
use mir2_map::Map;

use crate::mapdoc::DocState;
use crate::save;

// 小地图**尺寸口径**：整体搬到 `miredit-render::minimap`（跨 crate 唯一一份），这里只再导出。
// 为什么不留一份本地实现：网页端 `/api/export/minimap` 也要同一套 `W*12 × H*8` / `{4,2,1}`，
// 两份必然漂，而漂移的症状是「GUI 导出的尺寸与网页/官方面板差一档」。
pub use miredit_render::minimap::{
    pick_official_scale, valid_minimap_size, MinimapPreset, MinimapSize, MAX_EXPORT_BYTES,
    MAX_EXPORT_SIDE, MINIMAP_MAX_PIXELS,
};
// 本模块原来的私有 `档位输出可行`，同样搬去共享（选档/预设护栏只看这一条判据）。
use miredit_render::minimap::minimap_scale_fits;

// 分块渲染（块边长 / 扩边搜索上界 / 尺寸换算 / 逐块扩边 / 拼图）自 2026-11-06 起
// **只有一份实现**，住在 `miredit-render/src/tiled.rs`（网页端 `/api/export/minimap`
// 也走它）。这里只再导出同名 API ⇒ `crate::export::{TILE_CELLS, scaled_size, …}`
// 这些路径、GUI 测试的调用与真实导出的逐字节输出**一个字节都没变**。
pub use miredit_render::tiled::{
    scaled_size, tile_margins, SizeCache, MAX_MARGIN_CELLS, TILE_CELLS, TILE_CELLS_TWO,
};

/// 小地图**本仓兼容链**的目标长边（像素）。选档时要求输出两边都不超过它；
/// 最小档仍超 ⇒ 再走一次**第二段最终合成**（[`downscale_to_fit`]：等比 + 居中 +
/// 不透明黑底 + 最近邻，对齐上游 `_upstream/CrystalStudio/PreviewRaster.cs:15-19,35-38`）把整幅落到它以内。
///
/// ⚠ **原判断「原版不可得 ⇒ 自定值」已失效，回源订正（2026-11-06）**：
/// 官方尺寸口径见 `I:\cs-new\MinimapExporter.cs` + `MinimapExportWindow.cs`：
///
/// 1. **默认输出尺寸**：`width = document.Width * 12`、`height = document.Height * 8`
///    （`_upstream/CrystalStudio/MinimapExporter.cs:20`），**不是 2048 长边**；
/// 2. **三档预设**：以 `Width*48` / `Height*32` 为基准除以 `{4, 2, 1}`，选中预设时
///    `checked(document.Width * 48 / divisor)` / `checked(document.Height * 32 / divisor)`
///    （`_upstream/CrystalStudio/MinimapExportWindow.cs:36-38`）；
/// 3. **合法尺寸判据**：`w > 0 && h > 0 && (long)w*h <= Array.MaxLength / 4`
///    （`_upstream/CrystalStudio/MinimapExporter.cs:15-16`）；本仓等价换算、并叠加更严的既有护栏，见 [`MINIMAP_MAX_PIXELS`]；
/// 4. **精度/换算公式**：`scale = min(width/(W*48.0), height/(H*32.0))`、
///    `level = scale >= 1 ? 0 : clamp((int)floor(log2(1/scale)), 0, 16)`
///    （`_upstream/CrystalStudio/MinimapExporter.cs:47-48`）。**官方用 16 级 overview mip，本仓只有 4 档**
///    ⇒ [`pick_official_scale`] 按「最接近且不越界」做**有据映射，不是自定值**。
///
/// ⇒ 2048 仅保留给本仓旧开关 `--minimap-max-side` 的兼容链，**官方缺省不再经过它**；
/// 新 CLI / 菜单默认走 [`export_minimap_sized`] + [`MinimapSize::Default`]。
/// 改这个数**不会**改变「采样由谁做」：第一段仍是 `try_render_region_scaled_with`，
/// 第二段是 render crate 的最终合成入口 —— 本模块一行滤镜都没有。
///
/// ⚠ **订正（2026-11-08）**：原文这句是「第二段是 render crate 的**面积平均缩放**」。
/// 实为：第二段按上游 `_upstream/CrystalStudio/PreviewRaster.cs:15-19,35-38` 是
/// **等比 + 居中 + 不透明黑底 + 最近邻**（[`miredit_render::fit_centered_opaque`]），
/// 面积平均只在上游 mip 预缩 `_upstream/CrystalStudio/OverviewPixels.cs:7-42`。原文保留作历史引用。
pub const MINIMAP_MAX_SIDE: u32 = 2048;

/// `--minimap-max-side` 允许的最小值。
///
/// ⚠ **原判断「自定值」已失效，回源订正（2026-11-06）**：
/// 官方 `MinimapExporter.ValidSize` 的下界条件就是 `width > 0 && height > 0`；
/// **64 不再是官方判据**，只保留为本仓旧「长边两段式」开关的局部下限：
/// 该路径要求保持 `cw*48 : ch*32` 的精确整数比，64 能确保 `fit_ratio` 不退化成 0 宽/0 高
/// （最简比 `3:2` 时 k=21 ⇒ 63×42）。官方尺寸链 [`export_minimap_sized`] 不读这个数，
/// 其合法性由 [`MINIMAP_MAX_PIXELS`] + 本仓更严的内存护栏共同裁决。
pub const MINIMAP_MAX_SIDE_MIN: u32 = 64;

// 小地图**尺寸口径**（`MinimapPreset` / `MinimapSize` / `resolve` / `valid_minimap_size` /
// `MINIMAP_MAX_PIXELS` / `MAX_EXPORT_SIDE` / `MAX_EXPORT_BYTES` / `pick_official_scale` /
// `minimap_scale_fits`）自 2026-11-06 起**只有一份实现**，住在
// `miredit-render/src/minimap.rs`（网页端 `GET /api/export/minimap` 用的是同一份）。
// 本模块只保留同名 `pub use`（见文件头的 use 段）⇒ `crate::export::MinimapSize` 这类
// 路径、CLI 解析、面板行为与测试语义**一个字节都没变**。

/// 一张导出的 RGBA8 位图（行优先，`rgba.len() == width*height*4`）。
///
/// 为什么不直接复用 [`miredit_render::Rendered`]：它标了 `#[non_exhaustive]`
/// （`types.rs:457`），外部 crate **不能**用结构体字面量构造 ⇒ 拼接结果只能自带一个类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportImage {
    /// 像素宽。
    pub width: u32,
    /// 像素高。
    pub height: u32,
    /// 行优先 RGBA8。
    pub rgba: Vec<u8>,
    /// 实际用了几块（`1` = 一次渲完，没分块）。
    pub tiles: u32,
    /// 实际用的扩边格数（`0` = 窗口里所有帧都不溢出自己那一格）。
    pub margin: u32,
}

/// 一次导出的回执（状态栏 / 命令行诊断都读它）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    /// 真正落盘的路径。
    pub path: PathBuf,
    /// 像素宽。
    pub width: u32,
    /// 像素高。
    pub height: u32,
    /// PNG 文件字节数。
    pub bytes: u64,
    /// 用的档位。
    pub scale: RenderScale,
    /// 分块数。
    pub tiles: u32,
    /// 扩边格数（逐块不同，这里报**最大**的那个）。
    pub margin: u32,
    /// 第二段是否**真的改了尺寸**（小地图才可能 `true`）。
    ///
    /// ⚠ **订正（2026-11-08）**：原文档写的是「是否走了第二段**面积平均缩放**」。
    /// 第二段已按上游 `_upstream/CrystalStudio/PreviewRaster.cs:15-19,35-38` 换成
    /// [`miredit_render::fit_centered_opaque`]（等比 + 居中 + 不透明黑底 + 最近邻），
    /// 且**只要给了目标尺寸就一定会跑**（同尺寸时 `scale == 1`、`left == top == 0`，
    /// 但 `opaque: true` 的 alpha 覆盖照样落）⇒ 本字段不能再表示「第二段跑没跑」，
    /// 只表示「第二段有没有真的把图缩小」。
    /// ⇒ `false` **不等于**输出与 [`render_tiled`] 逐字节相同
    /// （同尺寸时源里 alpha < 255 的像素仍会被铺成不透明黑）。
    pub two_stage: bool,
    /// 小地图是否走官方尺寸链（[`export_minimap_sized`]）；
    /// `false` = 本仓旧「`--minimap-max-side` 长边两段式」兼容链。
    pub official_size: bool,
    /// 这次导出用的**门态**（`false` = 闭门）。
    ///
    /// 为什么回执要带它：这是「画布开着门、导出是闭门」那个不一致收口后的**自证字段** ——
    /// 命令行回执里打出 `开门=1` 就能与画布截图对上；不带的话，两张导出图
    /// （`32,842 B` vs `32,812 B`）只能靠肉眼猜哪张是开门的。
    pub doors_open: bool,
    /// 这次导出用的**地图光照档位**（缺省 [`MapLighting::Day`] = 不画遮罩 ⇒ 与接线前逐字节相同）。
    pub lighting: MapLighting,
}

// ---------------------------------------------------------------------------
// 一、选档
// ---------------------------------------------------------------------------


/// 小地图导出选哪一档：`One → Half → Quarter` 里**第一个**同时满足
///
/// 1. 两边都 ≤ `max_side`（用户给的目标长边），且
/// 2. 整幅输出过得了 [`MAX_EXPORT_SIDE`] / [`MAX_EXPORT_BYTES`]（[`minimap_scale_fits`]）
///
/// 的档位；都不行就用 `Quarter`（最小档，之后由 [`downscale_to_fit`] 走第二段最终合成）。
///
/// `Two` 不参与：小地图是「整图缩略」，放大没有意义（而且 `Two` 只会更快撞护栏）。
///
/// 举例（`0.map` 实测 700×700）：
///
/// | `max_side` | 选档 | 理由 |
/// |---|---|---|
/// | `2048` | `Quarter` | 三档都超 2048；`Quarter` 是最小档 |
/// | `99999` | `Quarter` | `One` 33600 px 超 `MAX_EXPORT_SIDE`；`Half` 752 MB 超 `MAX_EXPORT_BYTES` ⇒ 只剩 `Quarter` |
///
/// ⚠ 回源订正后，这条**只服务本仓旧开关 `--minimap-max-side`**；
/// 官方缺省 / 预设 / 指定像素走 [`pick_official_scale`] + [`export_minimap_sized`]。
pub fn pick_minimap_scale(cells_w: u32, cells_h: u32, max_side: u32) -> RenderScale {
    for s in [RenderScale::One, RenderScale::Half, RenderScale::Quarter] {
        if !minimap_scale_fits(cells_w, cells_h, s) {
            continue;
        }
        if let Some((w, h)) = s.pixel_size(cells_w, cells_h) {
            if w <= max_side && h <= max_side {
                return s;
            }
        }
    }
    RenderScale::Quarter
}

/// 最大公约数（[`fit_ratio`] 用；只对非零入参调用）。
const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// 把 `src_w × src_h` 按「**精确**保持地图格比例 `cw : ch`」缩到两边都 ≤ `max_side`。
///
/// # 返回
///
/// - `Ok(None)`：已经装得下（不缩、**也绝不放大**）；
/// - `Ok(Some((w, h)))`：要缩到这个尺寸，且 `w*ch*32 == h*cw*48` **逐位成立**；
/// - `Err(_)`：`max_side` 小到连最简比都放不下（此时无法在保持比例的前提下满足它）。
///
/// # 为什么不能简单按「长边缩到 max_side」算
///
/// 那样算出来的两个整数会**破坏长宽比**：`8400×5600` 按长边缩到 2048 得到
/// `2048×1365`，而 `2048/1365 = 1.500366…` ≠ `3/2` —— 验收判据
/// `w*ch*32 == h*cw*48` 当场就红。
///
/// 正确做法：先把 `cw*48 : ch*32` 化成最简整数比 `a : b`，再取**最大整数倍** `k`
/// （受 `max_side` 与源图尺寸双向约束）⇒ 输出 `(a*k, b*k)` 与 1× 渲染同比例。
/// `0.map`（700×700）⇒ `a:b = 3:2`，`k = 682` ⇒ **2046×1364**。
pub fn fit_ratio(
    src_w: u32,
    src_h: u32,
    cw: u32,
    ch: u32,
    max_side: u32,
) -> Result<Option<(u32, u32)>, String> {
    if src_w == 0 || src_h == 0 || cw == 0 || ch == 0 {
        return Ok(None);
    }
    let w0 = u64::from(cw) * u64::from(CELL_W);
    let h0 = u64::from(ch) * u64::from(CELL_H);
    let g = gcd(w0, h0).max(1);
    let (a, b) = (w0 / g, h0 / g);
    if u64::from(max_side) < a.max(b) {
        return Err(format!(
            "目标长边 {max_side} px 太小：这张图的比例是 {a}:{b}，保持比例至少要 {} px",
            a.max(b)
        ));
    }
    // 最大整数倍：两边都不超 max_side，也不超源图（不放大）
    let k = (u64::from(max_side) / a)
        .min(u64::from(max_side) / b)
        .min(u64::from(src_w) / a)
        .min(u64::from(src_h) / b);
    let (tw, th) = (k * a, k * b);
    if tw == u64::from(src_w) && th == u64::from(src_h) {
        return Ok(None);
    }
    Ok(Some((tw as u32, th as u32)))
}

// ---------------------------------------------------------------------------
// 二、分块渲染（共享实现 + 类型适配）
// ---------------------------------------------------------------------------

/// 渲染 `origin` 起 `cells` 格的一块窗口，**内部分块**，返回拼好的整幅 RGBA。
///
/// 实现已上移到 [`miredit_render::tiled::render_tiled`]（分块 + 逐块扩边 + 拼接，
/// 与网页端 `/api/export/minimap` 共用**同一份**）。本函数只做两件事：
///
/// 1. 把 [`miredit_render::TiledError`] 转成 GUI 沿用的 `String`（错误文案**逐字不变**）；
/// 2. 把 [`miredit_render::TiledImage`] 转成本模块的 [`ExportImage`]（字段一一对应）。
///
/// 为什么不直接 `pub use`：本模块的 `ExportImage` 与 `Result<_, String>` 是 GUI/CLI 的
/// 既有契约（[`ExportReport`] / [`finish`] / 448 条测试都按这个写），换类型等于改 API。
#[allow(clippy::too_many_arguments)] // 与本 crate 其它绘制入口同口径：8 个参数就是一次导出请求的 8 个输入
pub fn render_tiled(
    map: &Map,
    src: &dyn SpriteSource,
    mask: &LayerMask,
    overlay: CollisionOverlay,
    scale: RenderScale,
    origin: (i32, i32),
    cells: (u32, u32),
    options: RenderOptions,
) -> Result<ExportImage, String> {
    let img = miredit_render::tiled::render_tiled(
        map, src, mask, overlay, scale, origin, cells, options,
    )
    .map_err(|e| e.to_string())?;
    Ok(ExportImage {
        width: img.width,
        height: img.height,
        rgba: img.rgba,
        tiles: img.tiles,
        margin: img.margin,
    })
}

// ---------------------------------------------------------------------------
// 三、PNG 原子写
// ---------------------------------------------------------------------------

/// 把 RGBA8 编成 PNG 字节（**复用 render crate 的编码器**，不自己写）。
///
/// 为什么绕一圈临时文件：`png_io` 的公开 API 只有「写文件」形态
/// （`write_rgba_png` / `PngImage::save_png`），没有「返回 `Vec<u8>`」形态 ——
/// 与 `miredit-web/src/routes/mod.rs:84-86` 的处理完全一致。
/// 临时文件带 pid + 自增序号，并发不会撞名；**无论成败都删**。
fn encode_png_bytes(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("miredit-gui-{}-{seq}.png", std::process::id()));
    if let Err(e) = write_rgba_png(&tmp, width, height, rgba) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("PNG 编码失败（{width}×{height}）：{e}"));
    }
    let bytes = std::fs::read(&tmp)
        .map_err(|e| format!("PNG 读回失败（{}）：{e}", tmp.display()));
    if std::fs::remove_file(&tmp).is_err() {
        eprintln!("[导出] 临时文件清理失败：{}", tmp.display());
    }
    bytes
}

/// 把一张位图**原子地**写成 PNG（`tmp → 写 → rename`，失败不留半个文件）。
///
/// 真正的写盘是 [`crate::save::save_atomic`]（= `miredit_core::save_atomic`）：
/// 写 `<目标名>.miredit-tmp` → `sync_all` → 读回逐字节核对 → `rename` 原子替换 →
/// 尽力 fsync 父目录。目录不存在 ⇒ 第一阶段就失败，**目标目录里一个文件都不会多**。
pub fn write_png_atomic(target: &Path, img: &ExportImage) -> Result<(), String> {
    let bytes = encode_png_bytes(img.width, img.height, &img.rgba)?;
    save::save_atomic(target, &bytes).map(|_| ())
}

// ---------------------------------------------------------------------------
// 四、两个导出入口
// ---------------------------------------------------------------------------

/// 落盘 + 回执（两个导出共用的尾巴）。
fn finish(
    img: ExportImage,
    target: &Path,
    scale: RenderScale,
    two_stage: bool,
    official_size: bool,
    options: RenderOptions,
) -> Result<ExportReport, String> {
    write_png_atomic(target, &img)?;
    let bytes = std::fs::metadata(target).map(|m| m.len()).unwrap_or(0);
    Ok(ExportReport {
        path: target.to_path_buf(),
        width: img.width,
        height: img.height,
        bytes,
        scale,
        tiles: img.tiles,
        margin: img.margin,
        two_stage,
        official_size,
        doors_open: options.doors_open,
        lighting: options.lighting,
    })
}

/// 取「地图 + 图库」，缺哪个报哪个（中文原因，一个文件都不写）。
fn map_and_source(doc: &DocState) -> Result<(&Map, &dyn SpriteSource), String> {
    let d = doc
        .doc
        .as_ref()
        .ok_or_else(|| "需要先开图：当前没有打开地图（文件 → 打开地图…）".to_string())?;
    let src = doc
        .assets
        .source()
        .ok_or_else(|| "没有图库：图库目录不可用，导不出图（不是空图，是取不到帧）".to_string())?;
    Ok((&d.map, src as &dyn SpriteSource))
}

/// **导出选区**：把 `sel`（闭区间）按 `scale = 1` 渲成 PNG 写到 `target`。
///
/// - `sel == None` ⇒ 报「没有选区」的原因，**不写文件**；
/// - 选区先按地图尺寸裁剪（与 `EditSession::set_selection` 同一条 `clamp_to`），
///   整块在图外 ⇒ 报错；
/// - 层口径：**三层全开、不叠阻挡**（导出的是地图内容本身，不是当前视图状态）
///   —— 这正是 `miredit-web` 的 `/api/tiles` 的缺省口径（`layers` 缺省三层全开、
///   `collision` 缺省 0，见 `tiles.rs:23-24`）⇒ 同一区域、**同一 `doors_open`** 下导出的
///   PNG 与 `/api/tiles?...&scale=1&doors=<0|1>` **逐字节相同**（同一套 `compose::render`
///   + 同一个编码器）；
/// - `options`：**渲染选项**（门态 + 地图光照档位）。调用方必须传**当前视图的那一份**
///   （菜单路径 = `RenderOptions::with_doors_and_lighting(*cx.doors_open, *cx.lighting)`，
///   命令行路径 = [`crate::cli::Cli::export_options`]），否则又回到「画布开着门/切了夜晚、
///   导出是闭门白天」那个不一致（模块文档 §六/§七）。
///   缺省 [`RenderOptions::default()`] = 闭门 + 白天 ⇒ 与接线前逐字节相同。
///
/// 选区块本身走 [`render_tiled`]：3×3 时只有一块，与直接调 render 完全等价；
/// 用户框了一大片（比如 200×200 格 @1× = 9600×6400）时自动分块，不会撞 1× 护栏。
pub fn export_selection(
    doc: &DocState,
    sel: Option<Selection>,
    target: &Path,
    options: RenderOptions,
) -> Result<ExportReport, String> {
    let (map, src) = map_and_source(doc)?;
    let sel = sel.ok_or_else(|| {
        "没有选区：先在画布上用选择工具拖出一个矩形（或给 --select x0,y0,x1,y1）".to_string()
    })?;
    let sel = sel
        .clamp_to(map.width, map.height)
        .ok_or_else(|| {
            format!(
                "选区整块在地图外：({},{})..({},{}) 对不上 {}×{} 的地图",
                sel.x0, sel.y0, sel.x1, sel.y1, map.width, map.height
            )
        })?;
    let img = render_tiled(
        map,
        src,
        &LayerMask::all(),
        CollisionOverlay::none(),
        RenderScale::One,
        (i32::from(sel.x0), i32::from(sel.y0)),
        (u32::from(sel.width()), u32::from(sel.height())),
        options,
    )?;
    finish(img, target, RenderScale::One, false, false, options)
}

/// **第二段：把已渲好的整幅**落到目标尺寸（等比 + 居中 + 不透明黑底 + 最近邻）。
///
/// # 采样归属（这是本模块最容易被误解的一处）
///
/// - **第一段**（决定每个像素取哪一格、哪一帧）**只能**由 `miredit-render` 做 ——
///   即 [`render_tiled`] 里的 `try_render_region_scaled_with`；
/// - **第二段**只是「把一张已经渲好的 RGBA 图整体落到目标宽高」，不再碰任何地图语义 ——
///   这里复用 `miredit_render::preview_raster::fit_centered_opaque`
///   （等比 / 居中 / 不透明黑底 / 最近邻，逐字对齐上游 `_upstream/CrystalStudio/PreviewRaster.cs:15-19,35-38`）。
///   ⇒ 本模块**仍然一行滤镜都没写**。
///
/// # 为什么是 `fit_centered_opaque` 而不是 `compare::resize_to`
///
/// ⚠ **订正（2026-11-08）**：本节原文标题是
/// 「# 为什么是 `compare::resize_to` 而不是 `resample::盒式_流式`」，原文如下
/// （**保留作历史引用**）：
///
/// > `resample` 在 `miredit-render` 里是**私有模块**（`lib.rs:162` `mod resample;`，
/// > 两个函数都是 `pub(crate)`）⇒ 从本 crate **物理上调不到**；要用它必须改
/// > `miredit-render`（不在本单写域）。
/// > 公开可用的面积平均入口只有 `compare::resize_to`（`compare.rs:154`，文档原话
/// > 「面积平均缩放（确定性…）」），语义与 `盒式_流式` 同类。
/// >
/// > 内存上它也更省：`盒式_流式` 每行要一份 `sw × 32 B` 的预乘缓冲（8400 宽 = 269 KB，不大），
/// > 但 `resample::缩放`（整幅预乘版）要 `47 MP × 32 B ≈ 1.5 GB` —— 那才是必须避开的那个。
/// > `resize_to` 的额外内存只有**每个目标像素一个 `[u64; 4]` 累加器**，
/// > 峰值 ≈ 源图 + 目标图（`8400×5600×4 = 188 MB` + `2046×1364×4 = 11 MB`）。
///
/// **实为**：原文只论证了「哪个**面积平均**入口能调」，但「第二段该不该是面积平均」
/// 这个前提是错的 —— 上游导出成图的最终合成（调用点 `_upstream/CrystalStudio/MinimapExporter.cs:56-57` →
/// `_upstream/CrystalStudio/PreviewRaster.cs:15-19,35-38`）是**最近邻**，面积平均只在上游的 mip 预缩
/// `_upstream/CrystalStudio/OverviewPixels.cs:7-42`（作用在逐帧解码阶段，本仓对应物是第一段的缩小档）。
/// 已按上游改为 `preview_raster::fit_centered_opaque`（crate 根也有同名 re-export）。
///
/// 内存上两者同量级：`fit_centered_opaque` 额外只有**目标图 + 一张横向取样表**
/// （`x1-x0` 个 `u32`），峰值 ≈ 源图 + 目标图；`resize_to` 额外只有每个目标像素一个
/// `[u64; 4]` 累加器。两者都不是 `resample::缩放`（整幅预乘版，`8400×5600` 上要
/// `47 MP × 32 B ≈ 1.5 GB`）那条 —— 那条仍然不能碰。
///
/// `compare::resize_to` **保留未删、签名不变**：`overview`（画布上屏的总览 mip 预缩）
/// 与 `compare_blocks`（诊断）仍在用它，本模块只是不再用它。
///
/// `target = Some((w,h))` 时**直接按官方指定像素尺寸**做第二段最终合成
/// （官方 `PreviewRaster.Create(..., width, height)` 的等价落点）；`None` 才是本仓旧
/// 「保持格比例、缩到长边以内」的兼容链。`cw/ch` 只被兼容链使用。
///
/// 返回 `(新图, 是否真的缩了)`。
fn downscale_to_fit(
    img: ExportImage,
    cw: u32,
    ch: u32,
    max_side: u32,
    target: Option<(u32, u32)>,
) -> Result<(ExportImage, bool), String> {
    let 目标 = match target {
        Some(t) => Some(t),
        None => fit_ratio(img.width, img.height, cw, ch, max_side)?,
    };
    let Some((tw, th)) = 目标 else {
        // 兼容链「已经装得下」：**没有目标尺寸可落** ⇒ 原样返回，不做最终合成。
        // （这与「有目标尺寸、只是恰好同尺寸」不是一回事，后者见下一条 ⚠ 订正。）
        return Ok((img, false));
    };
    // ⚠ **订正（2026-11-08）**：这里原本还有一条
    //
    //     if img.width == tw && img.height == th { return Ok((img, false)); }
    //
    // 原文理由是「官方默认 8400×5600 与 Quarter 原生输出恰好同尺寸：resize_to 只会
    // clone，这时回执不能虚报『两段式 yes』」。换成 `fit_centered_opaque` 后**必须删掉**：
    // `compare::resize_to` 在同尺寸时确实 `return img.clone()`（`compare.rs:155-157`），
    // 但上游 `_upstream/CrystalStudio/PreviewRaster.cs:19` 的 `opaque: true` 是**无条件**把 alpha 写成 255
    // （RGB 保 0 ⇒ 不透明黑底）—— 同尺寸时 `scale == 1`、`left == top == 0` 只是
    // **几何恒等**，底色这一步照样要落。跳过它会让 GUI 与 `miredit-web` 的
    // `GET /api/export/minimap`（那边明确没有同尺寸短路，见
    // `routes/export.rs:306-307`「不再有『同尺寸就跳过』的短路」）在同一张图、
    // 同一目标尺寸上产出**不同字节**。
    // 回执的诚实性改由下面 `真的缩了` 承担：`two_stage` 只报「尺寸有没有真的变小」。
    let 真的缩了 = img.width != tw || img.height != th;
    // 交给 render crate 的公开最终合成入口（不自己写滤镜）
    let src = miredit_render::PngImage {
        width: img.width,
        height: img.height,
        rgba: img.rgba,
    };
    let out = miredit_render::fit_centered_opaque(&src, tw, th);
    drop(src);
    Ok((
        ExportImage {
            width: out.width,
            height: out.height,
            rgba: out.rgba,
            tiles: img.tiles,
            margin: img.margin,
        },
        真的缩了,
    ))
}

/// **导出小地图**：整张地图先按 [`pick_minimap_scale`] 选出的档位渲，再（必要时）
/// 落到 `max_side` 以内（第二段 = 等比 + 居中 + 不透明黑底 + 最近邻，见 [`downscale_to_fit`]）。
///
/// `options`：**渲染选项**（门态 + 光照档位）。门偏移与光照遮罩都在 1× 合成阶段
/// 生效、缩放只是纯后处理 ⇒ 缩完的图与「同一组选项的画布」一致（模块文档 §六/§七）。
/// 缺省 [`RenderOptions::default()`] = 闭门 + 白天 ⇒ 与**门态/光照接线前**逐字节相同。
///
/// ⚠ **订正（2026-11-08）**：上面这句「逐字节相同」只对**没走第二段**的发次成立。
/// 第二段已按上游 `_upstream/CrystalStudio/PreviewRaster.cs:15-19,35-38` 换成
/// [`miredit_render::fit_centered_opaque`]（等比 + 居中 + 不透明黑底 + 最近邻），
/// 它会把源里 alpha < 255 的像素铺成不透明黑 ⇒ 走第二段的发次与 2026-11-08 之前
/// **必然不同字节**（那正是对齐上游要改的东西）。原文保留作历史引用。
///
/// 输出尺寸与长宽比：
/// - 一档装得下 ⇒ 尺寸 = `round(宽*48*f) × round(高*32*f)`，比例与 1× 渲染恒等；
/// - 装不下（如 700×700 @`Quarter` = 8400×5600）⇒ 先出 8400×5600，再按
///   [`fit_ratio`] 缩到 `(a*k, b*k)`，**比例仍与 1× 渲染逐位相同**（判据
///   `w*ch*32 == h*cw*48`）。700×700 + `max_side=2048` ⇒ **2046×1364**
///   （不是 2048×1365 —— 那个比例是 1.500366…，不是 3:2）。
pub fn export_minimap(
    doc: &DocState,
    target: &Path,
    max_side: u32,
    options: RenderOptions,
) -> Result<ExportReport, String> {
    let (map, src) = map_and_source(doc)?;
    let (w, h) = (u32::from(map.width), u32::from(map.height));
    let scale = pick_minimap_scale(w, h, max_side);
    let img = render_tiled(
        map,
        src,
        &LayerMask::all(),
        CollisionOverlay::none(),
        scale,
        (0, 0),
        (w, h),
        options,
    )?;
    let (img, two_stage) = downscale_to_fit(img, w, h, max_side, None)?;
    finish(img, target, scale, two_stage, false, options)
}

/// **导出小地图（官方尺寸口径）**：缺省 `W*12 × H*8`，
/// 三档预设 `W*48/{4,2,1} × H*32/{4,2,1}`，或 `--minimap-width/--minimap-height` 指定像素。
///
/// 流程：`valid_minimap_size` 先拒绝非法尺寸（中文原因、不写文件）→
/// `pick_official_scale` 把官方 16 级 mip 有据映射到本仓 4 档 →
/// 复用既有 [`render_tiled`] 分块护栏 →
/// [`miredit_render::fit_centered_opaque`] 等比 + 居中 + 不透明黑底 + 最近邻落到指定宽高。
///
/// ⚠ **订正（2026-11-08）**：本段原文是
/// 「…→ `compare::resize_to` 精确落到指定宽高。」与
/// 「第二段只做公开的**面积平均缩放** —— **有据映射，不是自定值**」。
/// **实为**：上游 `PreviewRaster.Create` 的最终合成是
/// **最近邻 + 等比居中 + 不透明黑底**（`_upstream/CrystalStudio/PreviewRaster.cs:15-19,35-38`，调用点
/// `_upstream/CrystalStudio/MinimapExporter.cs:56-57` 传的就是 `padding: 0, opaque: true`）；
/// 面积平均只在上游的 mip 预缩 `_upstream/CrystalStudio/OverviewPixels.cs:7-42`。
/// 已按上游改入口，原文保留作历史引用。
///
/// 官方 `PreviewRaster.Create` 从 **overview mip 精灵** 重绘到任意 `width×height`
/// （`_upstream/CrystalStudio/MinimapExporter.cs:46-57`）；本仓没有 mip 精灵缓存，故第一段按最接近且不越界的
/// 档位出整幅 RGBA，第二段做上游那条最终合成 —— **有据映射，不是自定值**，
/// 也不改 render crate 的任何常量。门态 + 光照档位整份 `RenderOptions` 透传，
/// 缺省仍是闭门 + 白天。
pub fn export_minimap_sized(
    doc: &DocState,
    target: &Path,
    size: MinimapSize,
    options: RenderOptions,
) -> Result<ExportReport, String> {
    let (map, src) = map_and_source(doc)?;
    let (w, h) = (u32::from(map.width), u32::from(map.height));
    let (out_w, out_h) = size.resolve(w, h)?;
    valid_minimap_size(out_w, out_h)?;
    let scale = pick_official_scale(w, h, out_w, out_h);
    // 目标档位整幅输出的护栏：预设 Large(1:1) 若超 MAX_EXPORT_SIDE / MAX_EXPORT_BYTES，
    // 必须中文报错，不许静默降到 Half（官方预设就是 1:1，降档=偷偷改口径）。
    if !minimap_scale_fits(w, h, scale) {
        return Err(format!(
            "官方预设所需档位 {} 的单次输出过不了本仓分块护栏（地图 {w}×{h} 格、目标 {out_w}×{out_h} px）：请改用 smaller 预设或指定更小尺寸（不静默换档、不截断）",
            scale.label()
        ));
    }
    let img = render_tiled(
        map,
        src,
        &LayerMask::all(),
        CollisionOverlay::none(),
        scale,
        (0, 0),
        (w, h),
        options,
    )?;
    let (img, two_stage) = downscale_to_fit(img, w, h, out_w.min(out_h), Some((out_w, out_h)))?;
    finish(img, target, scale, two_stage, true, options)
}

#[cfg(test)]
mod tests;
