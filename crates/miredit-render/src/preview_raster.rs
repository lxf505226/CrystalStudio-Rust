//! 导出成图的**最终合成**：对齐上游 `PreviewRaster.Create(..., padding: 0, opaque: true)`。
//!
//! # 一、上游真值（`I:\miredit\_upstream\CrystalStudio\`）
//!
//! | 口径 | 上游位置 | 原式 |
//! |---|---|---|
//! | 等比缩放比 | `PreviewRaster.cs:15` | `double scale = Math.Min((width - padding*2)/area.Width, (height - padding*2)/area.Height);` |
//! | 居中偏移 | `PreviewRaster.cs:16-17` | `offsetX = (width - area.Width*scale)/2 - area.X*scale;`（`offsetY` 同形）|
//! | 不透明黑底 | `PreviewRaster.cs:19` | `if (opaque) for (int i = 3; i < output.Length; i += 4) output[i] = 255;`（RGB 保持 0）|
//! | 采样区间 | `PreviewRaster.cs:31-32` | `x0 = Math.Max(0, (int)Math.Ceiling(left - .5)); x1 = Math.Min(width, (int)Math.Ceiling(left + w - .5));`（`y` 同形）|
//! | **最近邻**采样 | `PreviewRaster.cs:35,38` | `sy = Math.Clamp((int)((y + .5 - top) * pixels.Height / h), 0, pixels.Height - 1);`、`sx = Math.Clamp((int)((x + .5 - left) * pixels.Width / w), 0, pixels.Width - 1);` |
//! | 合成 | `PreviewRaster.cs:45-46` | `a == 255` ⇒ 整像素覆盖；`a != 0` ⇒ `out[c] = Math.Min(255, src[c] + (out[c]*(255-a) + 127)/255)` |
//! | 调用点 | `MinimapExporter.cs:56-57` | `PreviewRaster.Create(drawings, new Rect(0, 0, document.Width*48.0, document.Height*32.0), width, height, default, padding: 0, opaque: true);` |
//!
//! # 二、本仓为什么只对应「第二段」
//!
//! 上游第一步是按 mip 级别**逐帧解码**（`Assets.PrepareOverviewAsync` → `Assets.cs:108-140`
//! 的 `DecodeOverview` → `OverviewPixels.Reduce`），本仓没有 mip 精灵缓存，第一步由
//! [`crate::tiled::render_tiled`] 按「最接近且不越界的档位」出整幅（`pick_official_scale`
//! 已经用上游同一条 `scale = min(...)` 选档，见 `minimap.rs`）。**第二段**（本模块）才是
//! `PreviewRaster.Create` 的落点：把整幅按 `scale = min(...)` 等比缩放、居中、铺不透明
//! 黑底、逐像素**最近邻**取样。
//!
//! # 三、与 mip 预缩的分工（**不许合并**）
//!
//! 面积平均只出现在上游的 **mip 预缩** `OverviewPixels.cs:7-42`（`Reduce`：预乘后按
//! `x*width/targetWidth` 的整块求均值），作用在**逐帧解码**阶段。导出成图那一步
//! （`PreviewRaster.cs:35-38`）是**最近邻**，两件事不能互相替代。本仓的对应物：
//!
//! | 环节 | 本仓落点 | 采样 |
//! |---|---|---|
//! | mip 预缩（逐帧/整幅缩小到档位） | [`crate::overview`] 的 `取缩帧`、[`crate::scale`] 的缩小档（`resample::盒式_流式`） | **面积平均，保持不动**（画布上屏与导出共用）|
//! | 导出成图（落到任意目标宽高） | **本模块** `fit_centered_opaque` | **最近邻**（只给导出链）|
//!
//! 所以本模块**不**替换 `compare::resize_to`：那个仍是面积平均，被 `overview`（上屏总览的
//! mip 预缩）与 `compare_blocks`（诊断）共用，改它会把上屏质量与 `--shot` 一起动掉。
//!
//! # 四、等比时「几何」不变（可执行判据）
//!
//! 目标尺寸等于源尺寸时 `scale == 1`、`left == top == 0` ⇒ 采样映射是恒等
//! （`sx == x`、`sy == y`）。因此**全不透明源**的输出与输入**逐字节相同**
//! （单测 `等比同尺寸且源全不透明则逐字节恒等`）。源里若有半透明/全透明像素，输出必然
//! 不同 —— 那是 `opaque: true`（黑底）的语义，**不是几何漂移**（单测 `等比同尺寸时几何不动`
//! 用「旧图 alpha==255 的像素 RGB 全等」把这两件事分开）。

use crate::canvas::over;
use crate::png_io::PngImage;

/// 把 `源` 等比缩放并**居中**放进 `目标宽 × 目标高` 的画布，底色是**不透明黑**
/// `RGB(0,0,0) A255`，采样是**最近邻**。
///
/// 这就是上游 `PreviewRaster.Create(tiles, area, width, height, …, padding: 0, opaque: true)`
/// 在「tiles 已经合成为一张整幅」时的等价落点（`MinimapExporter.cs:56-57` 是那条调用）。
///
/// # 参数
///
/// - `源`：第一步（[`crate::tiled::render_tiled`]）出的整幅，尺寸即「上游的 `area`」；
/// - `目标宽` / `目标高`：用户要的像素尺寸（`> 0` 由调用方保证，`0` 会得到空图而不 panic）。
///
/// # 行为（逐条对齐上游，见模块文档 §一）
///
/// 1. 画布先铺 `A = 255`（RGB 保持 0 ⇒ 黑底）；
/// 2. `scale = min(目标宽/源宽, 目标高/源高)`，两个方向**同一个** `scale` ⇒ 不会拉伸失真；
/// 3. 内容左上角落在 `((目标宽 - 源宽*scale)/2, (目标高 - 源高*scale)/2)` ⇒ 居中留边；
/// 4. 内容区外一个字节都不碰 ⇒ 留边是纯黑不透明；
/// 5. 内容区逐像素**最近邻**取样，再按上游 `over` 口径压到黑底上。
pub fn fit_centered_opaque(源: &PngImage, 目标宽: u32, 目标高: u32) -> PngImage {
    let (sw, sh) = (源.width, 源.height);
    let mut rgba = vec![0u8; 目标宽 as usize * 目标高 as usize * 4];
    // 上游 `PreviewRaster.cs:19`：`opaque` ⇒ 只把 alpha 写成 255，RGB 保持 0（黑底）。
    for px in rgba.as_chunks_mut::<4>().0 {
        px[3] = 255;
    }
    if sw == 0 || sh == 0 || 目标宽 == 0 || 目标高 == 0 {
        return PngImage {
            width: 目标宽,
            height: 目标高,
            rgba,
        };
    }

    // 上游 `PreviewRaster.cs:15-17`：本仓的 `area` 恒为 `(0, 0, sw, sh)`（导出整图，不裁子矩形）
    // ⇒ `area.X == area.Y == 0`、`padding == 0`（`MinimapExporter.cs:56-57` 传的就是 0）。
    let scale = (f64::from(目标宽) / f64::from(sw)).min(f64::from(目标高) / f64::from(sh));
    let left = (f64::from(目标宽) - f64::from(sw) * scale) / 2.0;
    let top = (f64::from(目标高) - f64::from(sh) * scale) / 2.0;
    let w = f64::from(sw) * scale;
    let h = f64::from(sh) * scale;

    // 上游 `PreviewRaster.cs:31-32`：内容区在目标里的半开区间（含 .5 补偿，别改成四舍五入）
    let x0 = (left - 0.5).ceil().max(0.0) as u32;
    let x1 = (left + w - 0.5).ceil().min(f64::from(目标宽)) as u32;
    let y0 = (top - 0.5).ceil().max(0.0) as u32;
    let y1 = (top + h - 0.5).ceil().min(f64::from(目标高)) as u32;
    if x0 >= x1 || y0 >= y1 {
        // 源有效但内容区被浮点边界夹成空：只剩黑底（不 panic、不猜像素）。
        return PngImage {
            width: 目标宽,
            height: 目标高,
            rgba,
        };
    }

    // 上游 `PreviewRaster.cs:38`：`sx = clamp((int)((x + .5 - left) * sw / w), 0, sw-1)`。
    // 横向只与 `x` 有关 ⇒ 先算成表（同一表达式、同一求值顺序，结果与逐像素重算逐字节相同）。
    let mut xs: Vec<u32> = Vec::with_capacity((x1 - x0) as usize);
    for x in x0..x1 {
        let sx = ((f64::from(x) + 0.5 - left) * f64::from(sw) / w) as i64;
        xs.push(sx.clamp(0, i64::from(sw) - 1) as u32);
    }

    for y in y0..y1 {
        // 上游 `PreviewRaster.cs:35`：`sy = clamp((int)((y + .5 - top) * sh / h), 0, sh-1)`
        let sy = (((f64::from(y) + 0.5 - top) * f64::from(sh) / h) as i64).clamp(0, i64::from(sh) - 1);
        let 源行 = sy as usize * sw as usize * 4;
        let 目标行 = y as usize * 目标宽 as usize * 4;
        for (i, &sx) in xs.iter().enumerate() {
            let si = 源行 + sx as usize * 4;
            let di = 目标行 + (x0 as usize + i) * 4;
            let src = [
                源.rgba[si],
                源.rgba[si + 1],
                源.rgba[si + 2],
                源.rgba[si + 3],
            ];
            let dst = [rgba[di], rgba[di + 1], rgba[di + 2], rgba[di + 3]];
            // 与 `Canvas::blit` 同一条 `over`（`a == 255` 覆盖、`a == 0` 不碰、
            // 其余按 `(src*a + dst*dst_a_keep)/out_a` 整型合成）。底色是 A=255 的黑 ⇒
            // 结果与上游 `PreviewRaster.cs:45-46` 那条预乘式逐字节相同。
            let out = over(src, dst);
            rgba[di..di + 4].copy_from_slice(&out);
        }
    }

    PngImage {
        width: 目标宽,
        height: 目标高,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一张 `w×h` 图：颜色由 `f(x, y)` 给。
    fn 造图(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> PngImage {
        let mut rgba = Vec::with_capacity((w * h) as usize * 4);
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        PngImage {
            width: w,
            height: h,
            rgba,
        }
    }

    /// 纯色不透明图。
    fn 纯色(w: u32, h: u32, c: [u8; 4]) -> PngImage {
        造图(w, h, |_, _| c)
    }

    /// 等比同尺寸 + 源全不透明 ⇒ **逐字节恒等**（这是「等比不改坏」的硬判据）。
    #[test]
    fn 等比同尺寸且源全不透明则逐字节恒等() {
        let src = 造图(9, 7, |x, y| {
            [
                (x * 17 % 256) as u8,
                (y * 29 % 256) as u8,
                ((x + y) * 7 % 256) as u8,
                255,
            ]
        });
        let out = fit_centered_opaque(&src, 9, 7);
        assert_eq!((out.width, out.height), (9, 7));
        assert_eq!(out.rgba, src.rgba, "等比同尺寸必须逐像素原样（几何不许动）");
    }

    /// 等比同尺寸 + 源含透明像素 ⇒ **几何不动、只有底色生效**。
    ///
    /// 判据：旧图 `alpha == 255` 的像素，新图 RGB 必须逐字节相同（采样点没漂）；
    /// `alpha == 0` 的像素必须变成不透明黑；`0 < alpha < 255` 必须按黑底 over 压暗。
    #[test]
    fn 等比同尺寸时几何不动() {
        // 4×2：左 2 列不透明、第 3 列半透明（RGB 全 200）、第 4 列全透明
        let src = 造图(4, 2, |x, _| match x {
            0 | 1 => [10, 20, 30, 255],
            2 => [200, 200, 200, 128],
            _ => [0, 0, 0, 0],
        });
        let out = fit_centered_opaque(&src, 4, 2);
        for y in 0..2 {
            for x in 0..4 {
                let s = src.pixel(x, y).expect("源像素");
                let o = out.pixel(x, y).expect("输出像素");
                match s[3] {
                    255 => assert_eq!(o, s, "({x},{y})：不透明像素必须原样"),
                    0 => assert_eq!(o, [0, 0, 0, 255], "({x},{y})：全透明必须是黑底"),
                    a => {
                        // over(src, 黑)：C = (src*255 + 0*0)/255 = src*a/255（四舍五入）
                        let 期望 = |c: u8| ((u32::from(c) * u32::from(a) + 127) / 255) as u8;
                        assert_eq!(
                            o,
                            [期望(s[0]), 期望(s[1]), 期望(s[2]), 255],
                            "({x},{y})：半透明必须压到黑底上"
                        );
                    }
                }
            }
        }
    }

    /// **不拉伸**：非等比目标（宽方向富余）⇒ 内容等比缩放后**水平居中**、两侧留纯黑边。
    ///
    /// 源 8×4、目标 8×4 的两倍宽（16×4）⇒ `scale = min(16/8, 4/4) = 1`
    /// ⇒ 内容仍是 8×4，`left = (16-8)/2 = 4`，左右各 4 列纯黑。
    #[test]
    fn 非等比目标不拉伸且居中() {
        let src = 纯色(8, 4, [1, 2, 3, 255]);
        let out = fit_centered_opaque(&src, 16, 4);
        assert_eq!((out.width, out.height), (16, 4));
        for y in 0..4 {
            for x in 0..16 {
                let px = out.pixel(x, y).expect("像素");
                if (4..12).contains(&x) {
                    assert_eq!(px, [1, 2, 3, 255], "({x},{y})：内容区应是源像素");
                } else {
                    assert_eq!(px, [0, 0, 0, 255], "({x},{y})：留边必须是纯黑不透明");
                }
            }
        }
    }

    /// 竖直方向同理（高方向富余 ⇒ 上下留边），且**横向不留边**。
    #[test]
    fn 竖直留边同样居中() {
        let src = 纯色(4, 2, [9, 8, 7, 255]);
        let out = fit_centered_opaque(&src, 4, 6);
        // scale = min(4/4, 6/2) = 1 ⇒ 内容 4×2，top = (6-2)/2 = 2
        for y in 0..6 {
            for x in 0..4 {
                let px = out.pixel(x, y).expect("像素");
                if (2..4).contains(&y) {
                    assert_eq!(px, [9, 8, 7, 255], "({x},{y})");
                } else {
                    assert_eq!(px, [0, 0, 0, 255], "({x},{y})");
                }
            }
        }
    }

    /// 缩小走**最近邻**：2×2 四色放大到 4×4 只许出现这四种颜色（面积平均会插出新色）。
    #[test]
    fn 放大只取源像素不插值() {
        let src = 造图(2, 2, |x, y| match (x, y) {
            (0, 0) => [255, 0, 0, 255],
            (1, 0) => [0, 255, 0, 255],
            (0, 1) => [0, 0, 255, 255],
            _ => [255, 255, 0, 255],
        });
        let out = fit_centered_opaque(&src, 4, 4);
        允许四色(&out, 4, 4);
    }

    /// 缩小也走**最近邻**：4×4 四色块缩到 2×2，颜色必须仍是那四色之一（不许出现均值色）。
    #[test]
    fn 缩小只取源像素不插值() {
        let src = 造图(4, 4, |x, y| match (x < 2, y < 2) {
            (true, true) => [200, 0, 0, 255],
            (false, true) => [0, 200, 0, 255],
            (true, false) => [0, 0, 200, 255],
            (false, false) => [200, 200, 0, 255],
        });
        let out = fit_centered_opaque(&src, 2, 2);
        允许四色(&out, 2, 2);
    }

    fn 允许四色(img: &PngImage, w: u32, h: u32) {
        let 允许: [[u8; 4]; 8] = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
            [200, 0, 0, 255],
            [0, 200, 0, 255],
            [0, 0, 200, 255],
            [200, 200, 0, 255],
        ];
        for y in 0..h {
            for x in 0..w {
                let px = img.pixel(x, y).expect("像素");
                assert!(允许.contains(&px), "({x},{y}) 出现了插值色 {px:?}（应最近邻）");
            }
        }
    }

    /// 采样点就是上游那条式子（手算对拍，钉死「含 .5 补偿、截断而不是四舍五入」）。
    ///
    /// 源 3×1 三色、目标 6×1：`scale = min(6/3, 1/1) = 1` ⇒ 内容 3×1 居中在 `left = 1.5`。
    /// 内容区 `x0 = ceil(1.5-.5) = 1`、`x1 = ceil(1.5+3-.5) = 4` ⇒ 目标列 1..4 各取
    /// `sx = (int)((x+.5-1.5)*3/3) = (int)(x-1)` ⇒ `x=1 → 0`、`x=2 → 1`、`x=3 → 2`。
    #[test]
    fn 采样点与上游式子逐列对拍() {
        let src = 造图(3, 1, |x, _| match x {
            0 => [11, 0, 0, 255],
            1 => [22, 0, 0, 255],
            _ => [33, 0, 0, 255],
        });
        let out = fit_centered_opaque(&src, 6, 1);
        let 行: Vec<u8> = (0..6)
            .map(|x| out.pixel(x, 0).expect("像素")[0])
            .collect();
        assert_eq!(行, vec![0, 11, 22, 33, 0, 0], "逐列必须与上游式子一致");
    }

    /// 空输入不 panic：源 0 尺寸 / 目标 0 尺寸 ⇒ 空图或纯黑底。
    #[test]
    fn 空尺寸不panic() {
        let 空 = PngImage {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        let a = fit_centered_opaque(&空, 3, 2);
        assert_eq!((a.width, a.height), (3, 2));
        assert_eq!(a.rgba, [0, 0, 0, 255].repeat(6), "无内容 ⇒ 整张纯黑不透明");

        let src = 纯色(2, 2, [1, 2, 3, 255]);
        let b = fit_centered_opaque(&src, 0, 5);
        assert_eq!((b.width, b.height), (0, 5));
        assert!(b.rgba.is_empty());
    }
}
