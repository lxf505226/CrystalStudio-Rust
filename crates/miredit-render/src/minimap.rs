//! 小地图导出的**尺寸口径**（跨 crate 唯一一份：GUI 与网页后端共用）。
//!
//! 本模块是 2026-11-06 从 `miredit-gui/src/export.rs` **整体搬来**的纯尺寸/校验部分
//! （`MinimapPreset` / `MinimapSize` / `resolve` / `valid_minimap_size` /
//! `MINIMAP_MAX_PIXELS` / `MAX_EXPORT_SIDE` / `MAX_EXPORT_BYTES` / `pick_official_scale`）。
//! 搬的理由不是「顺手开放 API」，而是网页端 `GET /api/export/minimap` 也要同一套口径：
//! **两份必然漂**，而漂移的症状是「GUI 导出的与小地图面板写的尺寸差一档」。
//! 搬完后 `miredit-gui` 那边只剩 `pub use`（API 与行为一个字节都没变，测试不动语义）。
//!
//! 本模块**不含渲染流程**：它只回答「该出多大」「这个尺寸合不合法」「该用哪一档倍率」。
//! 真正出图仍走 [`crate::tiled::render_tiled`] + [`crate::preview_raster::fit_centered_opaque`]。
//!
//! ⚠ **订正（2026-11-08）**：本行原文是「真正出图仍走 [`crate::try_render_region_scaled_with`]
//! \+ [`crate::compare::resize_to`]」—— 那句在导出链上已过期：第二段不再是面积平均的
//! `resize_to`，而是上游 `PreviewRaster.Create(..., padding: 0, opaque: true)` 的等价落点
//! （等比 + 居中 + 不透明黑底 + 最近邻，见 [`crate::preview_raster`]）。`resize_to` 仍被
//! `overview` 的 mip 预缩与 `compare_blocks` 诊断使用，**没有**被替换。原文保留作历史引用。
//!
//! # 一、官方口径（`@acfbbf8a` 回源订正，2026-11-06）
//!
//! 官方尺寸口径见 `I:\cs-new\MinimapExporter.cs` + `MinimapExportWindow.cs`：
//!
//! 1. **默认输出尺寸**：`width = document.Width * 12`、`height = document.Height * 8`
//!    （`MinimapExporter.cs:20`），**不是 2048 长边**；
//! 2. **三档预设**：以 `Width*48` / `Height*32` 为基准除以 `{4, 2, 1}`，选中预设时
//!    `checked(document.Width * 48 / divisor)` / `checked(document.Height * 32 / divisor)`
//!    （`MinimapExportWindow.cs:36-38`）；
//! 3. **合法尺寸判据**：`w > 0 && h > 0 && (long)w*h <= Array.MaxLength / 4`
//!    （`MinimapExporter.cs:15-16`）；本仓等价换算见 [`MINIMAP_MAX_PIXELS`]；
//! 4. **精度/换算公式**：`scale = min(width/(W*48.0), height/(H*32.0))`、
//!    `level = scale >= 1 ? 0 : clamp((int)floor(log2(1/scale)), 0, 16)`
//!    （`MinimapExporter.cs:47-48`）。官方用 16 级 overview mip，本仓只有 4 档
//!    ⇒ [`pick_official_scale`] 按「最接近且不越界」做**有据映射，不是自定值**。
//!
//! # 二、本仓更严的护栏（**保留更严者，不放宽**）
//!
//! [`MINIMAP_MAX_PIXELS`] 是官方 `Array.MaxLength / 4` 的 Rust 等价换算，但本仓另有
//! [`MAX_EXPORT_SIDE`]（单边 32768 px）与 [`MAX_EXPORT_BYTES`]（512 MiB）两条更严的
//! 内存护栏 —— 冲突时取更严的那条，**不静默截断、不偷偷降档**（见 [`valid_minimap_size`]）。

use crate::{RenderScale, CELL_H, CELL_W};

/// 官方 `ValidSize(w,h)` 的**本仓 Rust 等价上限**（像素数，不是字节数）。
///
/// 官方（.NET）：`w > 0 && h > 0 && (long)w*h <= Array.MaxLength / 4`
/// （`I:\cs-new\MinimapExporter.cs:15-16`）。`Array.MaxLength = 0x7FFFFFC7`，
/// `/4 = 536,870,897` 个 `u32`，对应 **RGBA8 恰为 `2,147,483,588 B = 2 GiB - 4 B`**；
/// 本仓 `ExportImage.rgba: Vec<u8>` 按字节分配，`isize::MAX` 在 64 位是
/// `0x7FFF_FFFF_FFFF_FFFF`（9,223,372,036,854,775,807 B），令
/// `n = (isize::MAX as u64) / 4 = 2,305,843,009,213,693,951`
/// ⇒ `n * 4 <= isize::MAX`，这就是 Rust 侧「一次 `Vec<u8>` 能表达的最大 RGBA 像素数」。
/// **等价换算，不是抄 .NET 数字**。
///
/// ⚠ 本仓既有 [`MAX_EXPORT_SIDE`] = 32768 与 [`MAX_EXPORT_BYTES`] = 512 MiB 更严
/// （`32768² = 1,073,741,824` 像素，但字节护栏先卡到 `134,217,728` 像素），
/// 故 [`valid_minimap_size`] 在官方上限之外还会保留这些护栏；**没有放宽内存护栏**。
pub const MINIMAP_MAX_PIXELS: u64 = (isize::MAX as u64) / 4;

/// 单次导出的**输出**单边上限（像素）。超过 ⇒ 报错（不截断）。
pub const MAX_EXPORT_SIDE: u32 = 32_768;

/// 单次导出的**输出**字节上限（512 MiB）。
///
/// 与 render crate 的 [`crate::SCALED_MAX_BYTES`]（64 MiB，管**单次调用**的目标图）
/// 与 [`crate::INTERMEDIATE_MAX_BYTES`]（128 MiB，管 1× 中间图）不是一回事：
/// 那条管的是「一次渲染调用」，这条管的是「用户最终要的那张整幅输出」。
/// `0.map @Quarter` = `8400×5600×4 = 188 MB` ⇒ 在限内；`700×700 @Half` = 752 MB ⇒ 报错。
pub const MAX_EXPORT_BYTES: u64 = 512 * 1024 * 1024;

/// 官方三档小地图预设（基准 `W*48 × H*32`，除以 4 / 2 / 1）。
///
/// @acfbbf8a `MinimapExportWindow.cs:21,36-38`：
/// `Small (¼)` / `Medium (½)` / `Large (1:1)`，选中时写入
/// `checked(document.Width * 48 / divisor)` 与 `checked(document.Height * 32 / divisor)`。
/// 本枚举的 [`MinimapPreset::divisor`] 就是那三个除数，不另造倍率。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinimapPreset {
    /// `Small (¼)`：`W*48/4 × H*32/4` = `W*12 × H*8`（与官方默认同尺寸）。
    Small,
    /// `Medium (½)`：`W*48/2 × H*32/2` = `W*24 × H*16`。
    Medium,
    /// `Large (1:1)`：`W*48 × H*32`。
    Large,
}

impl MinimapPreset {
    /// 官方 `new[] { 4, 2, 1 }[Preset.SelectedIndex]` 的除数
    /// （`MinimapExportWindow.cs:36`；同处 `:37-38` 是
    /// `checked(document.Width * 48 / divisor)` / `checked(document.Height * 32 / divisor)`）。
    pub const fn divisor(self) -> u32 {
        match self {
            MinimapPreset::Small => 4,
            MinimapPreset::Medium => 2,
            MinimapPreset::Large => 1,
        }
    }

    /// CLI / URL 名与 `--minimap-preset`、`?preset=` 的取值逐字对应；不是 UI 显示名。
    pub const fn key(self) -> &'static str {
        match self {
            MinimapPreset::Small => "small",
            MinimapPreset::Medium => "medium",
            MinimapPreset::Large => "large",
        }
    }

    /// `small|medium|large` → 枚举；其它写法一律 `None`（由调用方报中文原因）。
    pub fn parse(s: &str) -> Option<Self> {
        let key = s.trim();
        [Self::Small, Self::Medium, Self::Large]
            .into_iter()
            .find(|p| p.key().eq_ignore_ascii_case(key))
    }
}

/// 一次小地图导出的**尺寸口径**。
///
/// `Default` / `Preset` / `Explicit` 分别对应官方 `MinimapExporter.ExportAsync` 的
/// 「两个可空参数都缺省」/「尺寸面板预设」/「手动填宽高」。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MinimapSize {
    /// 官方默认：`W*12 × H*8`（`MinimapExporter.cs:20`）。
    #[default]
    Default,
    /// 官方三档预设：`W*48/divisor × H*32/divisor`。
    Preset(MinimapPreset),
    /// 官方尺寸面板里手填的宽 / 高（两个都必填；缺一个报中文原因）。
    Explicit {
        /// 目标像素宽。
        width: u32,
        /// 目标像素高。
        height: u32,
    },
}

impl MinimapSize {
    /// 按地图格数算出**精确像素尺寸**（宽、高）。
    ///
    /// 官方预设用 C# `checked(...)`；本仓用 `u64` 算完再 `u32::try_from`，
    /// 溢出返回中文原因，不静默截断。
    pub fn resolve(self, cells_w: u32, cells_h: u32) -> Result<(u32, u32), String> {
        let (width, height) = match self {
            // 官方 `MinimapExporter.cs:20`：`width = outputWidth ?? checked(document.Width * 12)`、
            // `height = outputHeight ?? checked(document.Height * 8)`（缺省值，不是 2048 长边）。
            Self::Default => (u64::from(cells_w) * 12, u64::from(cells_h) * 8),
            Self::Preset(p) => {
                let d = u64::from(p.divisor());
                (
                    u64::from(cells_w) * u64::from(CELL_W) / d,
                    u64::from(cells_h) * u64::from(CELL_H) / d,
                )
            }
            Self::Explicit { width, height } => (u64::from(width), u64::from(height)),
        };
        let to_u32 = |v: u64, side: &str| {
            u32::try_from(v).map_err(|_| {
                format!("小地图导出尺寸溢出：{side} = {v} px 超过 u32 上限（官方 checked 语义）")
            })
        };
        Ok((to_u32(width, "宽")?, to_u32(height, "高")?))
    }
}

/// 官方 `ValidSize` 的等价判据 + 本仓更严的内存护栏（**不静默截断/降级**）。
///
/// @acfbbf8a `MinimapExporter.cs:15-16,21-23`：官方只判 `w>0 && h>0 && w*h <= Array.MaxLength/4`；
/// 本仓 [`MINIMAP_MAX_PIXELS`] 是该 .NET 数字按 `Vec<u8>` / `isize::MAX` 做的等价换算
/// （不是抄数字），同时 [`MAX_EXPORT_SIDE`] / [`MAX_EXPORT_BYTES`] 更严，冲突时**保留更严者**。
/// ⇒ 0 / 超大像素一律给中文原因并拒绝。
pub fn valid_minimap_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err(format!(
            "小地图导出尺寸非法：宽和高都必须是正整数（收到 {width}×{height} px，官方 ValidSize 拒绝 0）"
        ));
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MINIMAP_MAX_PIXELS {
        return Err(format!(
            "小地图导出尺寸过大：{width}×{height} px = {pixels} 像素，超过 Rust RGBA8 可分配上限 {MINIMAP_MAX_PIXELS} 像素（官方 ValidSize 的等价换算）"
        ));
    }
    if width > MAX_EXPORT_SIDE || height > MAX_EXPORT_SIDE {
        return Err(format!(
            "小地图导出尺寸过大：{width}×{height} px 超过本仓单边上限 {MAX_EXPORT_SIDE} px（保留更严的内存护栏，不截断、不降级）"
        ));
    }
    let bytes = pixels * 4;
    if bytes > MAX_EXPORT_BYTES {
        return Err(format!(
            "小地图导出尺寸过大：{width}×{height} px 的 RGBA 需要 {:.1} MB，超过本仓 {:.0} MB 上限（保留更严的内存护栏，不截断、不降级）",
            bytes as f64 / 1_048_576.0,
            MAX_EXPORT_BYTES as f64 / 1_048_576.0
        ));
    }
    Ok(())
}

/// 官方 16 级 overview mip → 本仓 4 档的**有据映射**（不是自定值）。
///
/// @acfbbf8a `MinimapExporter.cs:47-48`：
/// `scale = min(width/(W*48.0), height/(H*32.0))`；
/// `level = scale >= 1 ? 0 : clamp(floor(log2(1/scale)), 0, 16)`；
/// 官方 `Assets.cs:119` 的 `1 << level` 把源图缩小 `2^level` 倍。
/// 本仓 [`RenderScale`] 只有 `Quarter(1/4) / Half(1/2) / One(1) / Two(2)`，
/// **没有 16 级 mip** ⇒ 取「最接近且不越过目标尺寸的档位」：
///
/// ```text
/// scale >= 2            → Two      （官方 scale>=1 本可 level=0；本仓 2× 不越过目标）
/// 1 <= scale < 2        → One
/// 0.5 <= scale < 1      → Half     （官方 level=1）
/// 0 < scale < 0.5       → Quarter  （官方 level>=2 统一夹到本仓最小档）
/// ```
///
/// ⚠ **官方用 16 级 overview mip，本仓只有 4 档 ⇒ 有据映射，不是自定值**。
/// 目标像素尺寸最终仍由 [`crate::preview_raster::fit_centered_opaque`] 精确落到
/// [`valid_minimap_size`] 判过的宽高（等比 + 居中 + 不透明黑底 + 最近邻，上游
/// `PreviewRaster.cs:15-19,35-38`），不放大采样核、不改任何 render 常量。
///
/// ⚠ **订正（2026-11-08）**：本行原文写的是「最终仍由 [`crate::compare::resize_to`]
/// 精确落到 …」，那是**旧口径**（面积平均、且目标宽高原样铺满 ⇒ 非等比请求会拉伸失真）。
/// 按上游 `MinimapExporter.cs:56-57` → `PreviewRaster.cs:15-19,35-38` 改为上面这条。
/// 原文保留作历史引用；`resize_to` 本身**没动**（仍服务 mip 预缩与诊断）。
pub fn pick_official_scale(
    cells_w: u32,
    cells_h: u32,
    width: u32,
    height: u32,
) -> RenderScale {
    let native_w = f64::from(cells_w) * f64::from(CELL_W);
    let native_h = f64::from(cells_h) * f64::from(CELL_H);
    let scale = (f64::from(width) / native_w).min(f64::from(height) / native_h);
    if scale >= 2.0 {
        RenderScale::Two
    } else if scale >= 1.0 {
        RenderScale::One
    } else if scale >= 0.5 {
        RenderScale::Half
    } else {
        RenderScale::Quarter
    }
}

/// 该档位下「整幅输出」是否还过得了本模块自己的护栏
/// （[`MAX_EXPORT_SIDE`] / [`MAX_EXPORT_BYTES`]）。
///
/// 为什么选档要看这个：用户给的目标可能比护栏大得多。若只看目标尺寸，
/// `700×700` 的 `Large` 会选 `One` ⇒ `33600×22400`（单边超 32768、整幅 3 GB）
/// ⇒ 渲染层当场报错。选档必须只在**真渲得出来**的档位里挑，否则「目标调大」反而变成失败。
///
/// 与 [`crate::RenderScale::pixel_size`] 的分工：那条返回 `Option`（算不出就是 `None`），
/// 本条把 `None` 也算成「不可行」，供调用方一条判据用尽。
pub fn minimap_scale_fits(cells_w: u32, cells_h: u32, scale: RenderScale) -> bool {
    let Some((w, h)) = scale.pixel_size(cells_w, cells_h) else {
        return false;
    };
    w <= MAX_EXPORT_SIDE
        && h <= MAX_EXPORT_SIDE
        && u64::from(w) * u64::from(h) * 4 <= MAX_EXPORT_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 三档除数就是官方的 `{4, 2, 1}`，且 `resolve` 的分母只来自
    /// [`MinimapPreset::divisor`]（**不另写倍率**）。
    #[test]
    fn 三档除数与像素尺寸() {
        assert_eq!(MinimapPreset::Small.divisor(), 4);
        assert_eq!(MinimapPreset::Medium.divisor(), 2);
        assert_eq!(MinimapPreset::Large.divisor(), 1);

        // 700×700：Small = 8400×5600（W*12 × H*8）、Medium = 16800×11200、Large = 33600×22400
        for (preset, want) in [
            (MinimapPreset::Small, (8400u32, 5600u32)),
            (MinimapPreset::Medium, (16_800, 11_200)),
            (MinimapPreset::Large, (33_600, 22_400)),
        ] {
            assert_eq!(
                MinimapSize::Preset(preset).resolve(700, 700).expect("可算"),
                want,
                "{}",
                preset.key()
            );
            // 除数只从 divisor() 来：手算一遍必须逐像素相同
            let d = preset.divisor();
            assert_eq!(want, (700 * 48 / d, 700 * 32 / d), "{}", preset.key());
        }
        // 小图同样整数：2×2 格 ⇒ 24×16 / 48×32 / 96×64
        assert_eq!(
            MinimapSize::Preset(MinimapPreset::Small)
                .resolve(2, 2)
                .expect("可算"),
            (24, 16)
        );
        assert_eq!(
            MinimapSize::Preset(MinimapPreset::Medium)
                .resolve(2, 2)
                .expect("可算"),
            (48, 32)
        );
        assert_eq!(
            MinimapSize::Preset(MinimapPreset::Large)
                .resolve(2, 2)
                .expect("可算"),
            (96, 64)
        );
    }

    /// 官方缺省 = `W*12 × H*8`（**不是** Small 的别名，但两者算出的像素恰好相同）。
    #[test]
    fn 缺省尺寸是十二乘八() {
        assert_eq!(MinimapSize::default(), MinimapSize::Default);
        assert_eq!(
            MinimapSize::Default.resolve(700, 700).expect("可算"),
            (8400, 5600)
        );
        assert_eq!(MinimapSize::Default.resolve(1, 1).expect("可算"), (12, 8));
        assert_eq!(
            MinimapSize::Default.resolve(700, 700).expect("可算"),
            MinimapSize::Preset(MinimapPreset::Small)
                .resolve(700, 700)
                .expect("可算")
        );
    }

    /// 显式宽高原样返回（不吸附、不按格换算），溢出给中文原因。
    #[test]
    fn 显式尺寸原样且溢出报错() {
        assert_eq!(
            MinimapSize::Explicit {
                width: 321,
                height: 123
            }
            .resolve(700, 700)
            .expect("可算"),
            (321, 123)
        );
        // 1 格图也给 1×1（不偷偷夹到最小 64）
        assert_eq!(
            MinimapSize::Explicit {
                width: 1,
                height: 1
            }
            .resolve(700, 700)
            .expect("可算"),
            (1, 1)
        );
        // u32::MAX 本身仍是合法显式宽度（不夹、不吸附）
        assert_eq!(
            MinimapSize::Explicit {
                width: u32::MAX,
                height: 1
            }
            .resolve(1, 1)
            .expect("u32::MAX 合法"),
            (u32::MAX, 1)
        );
        // u64 算完再 try_from：预设在大格数下溢出 u32 ⇒ 中文原因，不静默截断
        let e = MinimapSize::Preset(MinimapPreset::Large)
            .resolve(u32::MAX, 1)
            .expect_err("Large 在 u32::MAX 格下必须溢出报错");
        assert!(e.contains("溢出"), "{e}");
    }

    /// `valid_minimap_size`：官方 ValidSize + 本仓两条更严护栏的边界。
    #[test]
    fn 合法尺寸边界() {
        // 官方默认尺寸合法
        assert!(valid_minimap_size(8400, 5600).is_ok());
        // 单边上限边界：32768 可以，32769 不行
        assert!(valid_minimap_size(32_768, 1).is_ok());
        let e = valid_minimap_size(32_769, 1).expect_err("单边超限必须拒绝");
        assert!(e.contains("单边上限"), "{e}");
        // 0 宽 / 0 高（官方 ValidSize 的第一条）
        for (w, h) in [(0u32, 5600u32), (8400, 0)] {
            let e = valid_minimap_size(w, h).expect_err("0 必须拒绝");
            assert!(e.contains("正整数"), "{e}");
        }
        // 字节护栏：30000×30000×4 = 3.6 GB > 512 MiB（两边都 ≤ 32768）
        let e = valid_minimap_size(30_000, 30_000).expect_err("超字节必须拒绝");
        assert!(e.contains("512") || e.contains("MB"), "{e}");
        // 512 MiB 的精确边界：134,217,728 像素可以（= 512 MiB RGBA），多 1 像素不行
        assert!(valid_minimap_size(16_384, 8_192).is_ok());
        let e = valid_minimap_size(16_384, 8_193).expect_err("超一像素必须拒绝");
        assert!(e.contains("MB"), "{e}");
        // 像素数护栏（官方 Array.MaxLength/4 的等价换算）优先于单边判断
        let e = valid_minimap_size(2_000_000_000, 2_000_000_000).expect_err("超像素必须拒绝");
        assert!(e.contains("可分配上限"), "{e}");
        assert_eq!(MINIMAP_MAX_PIXELS, (isize::MAX as u64) / 4);
    }

    /// 官方 `scale = min(w/(W*48), h/(H*32))` 的 4 档映射（**有据映射不是自定值**）。
    #[test]
    fn 档位映射与官方scale一致() {
        assert_eq!(pick_official_scale(700, 700, 8400, 5600), RenderScale::Quarter);
        assert_eq!(
            pick_official_scale(700, 700, 16_800, 11_200),
            RenderScale::Half
        );
        assert_eq!(
            pick_official_scale(700, 700, 33_600, 22_400),
            RenderScale::One
        );
        assert_eq!(
            pick_official_scale(700, 700, 70_000, 45_000),
            RenderScale::Two
        );
        // 一边拖后腿 ⇒ 按更小的那个比（`h` 只有 1 px）
        assert_eq!(pick_official_scale(700, 700, 33_600, 1), RenderScale::Quarter);
        // 精确边界：0.5 / 1 / 2
        assert_eq!(
            pick_official_scale(1, 1, 24, 16), // native 48×32 ⇒ scale 0.5
            RenderScale::Half
        );
        assert_eq!(pick_official_scale(1, 1, 48, 32), RenderScale::One);
        assert_eq!(pick_official_scale(1, 1, 96, 64), RenderScale::Two);
        // 略低于边界 ⇒ 落到下一档（不四舍五入；`min` 先取更小的那一侧）
        assert_eq!(pick_official_scale(1, 1, 23, 16), RenderScale::Quarter); // min(23/48, 16/32) = 0.479
        assert_eq!(pick_official_scale(1, 1, 95, 64), RenderScale::One); // min(95/48, 64/32) = 1.979
    }

    /// 档位可行性：整幅输出过得了 `MAX_EXPORT_SIDE` / `MAX_EXPORT_BYTES` 才算可行。
    #[test]
    fn 档位可行性() {
        // 700×700：Quarter = 8400×5600×4 = 188 MB ✅
        assert!(minimap_scale_fits(700, 700, RenderScale::Quarter));
        // Half = 16800×11200×4 = 752 MB ❌（超 512 MiB）
        assert!(!minimap_scale_fits(700, 700, RenderScale::Half));
        // One = 33600×22400 ❌（单边 33600 > 32768）
        assert!(!minimap_scale_fits(700, 700, RenderScale::One));
        // 2×2 格四档都可行
        for s in RenderScale::ALL {
            assert!(minimap_scale_fits(2, 2, s), "{}", s.label());
        }
    }
}
