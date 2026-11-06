//! PNG 读写（**非渲染核心**：只用于导出参考图与读客户端截图做比对）。
//!
//! 之所以单独一模块：渲染核心（`canvas` / `compose` / `frames`）只碰 `Vec<u8>` RGBA，
//! 不依赖任何图像编解码库；这里才把 `png` 依赖收进来，方便以后换实现（GUI 侧要直接
//! 上纹理时甚至可以不要本模块）。

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// 一张读回来的 RGBA8 图（行优先，`rgba.len() == width*height*4`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PngImage {
    /// 像素宽。
    pub width: u32,
    /// 像素高。
    pub height: u32,
    /// 行优先 RGBA8。
    pub rgba: Vec<u8>,
}

impl PngImage {
    /// 取某像素 RGBA；越界返回 `None`。
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let s = self.rgba.get(i..i + 4)?;
        Some([s[0], s[1], s[2], s[3]])
    }

    /// 导出为 PNG（RGBA8）。
    pub fn save_png(&self, path: &Path) -> std::io::Result<()> {
        write_rgba_png(path, self.width, self.height, &self.rgba)
    }

    /// 从 `x, y` 起裁一块 `w × h`（越界部分用全透明补齐，不 panic）。
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> PngImage {
        let mut out = vec![0u8; w as usize * h as usize * 4];
        for row in 0..h {
            let sy = y + row;
            if sy >= self.height {
                break;
            }
            for col in 0..w {
                let sx = x + col;
                if sx >= self.width {
                    break;
                }
                let si = (sy as usize * self.width as usize + sx as usize) * 4;
                let di = (row as usize * w as usize + col as usize) * 4;
                out[di..di + 4].copy_from_slice(&self.rgba[si..si + 4]);
            }
        }
        PngImage {
            width: w,
            height: h,
            rgba: out,
        }
    }
}

/// 把行优先 RGBA8 写成 PNG。目录不存在会直接报错（不隐式建目录，避免到处乱造路径）。
pub fn write_rgba_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let file = File::create(path)?;
    编码(BufWriter::new(file), width, height, rgba)
}

/// 把行优先 RGBA8 编码成 **PNG 字节**（内存形态，不碰磁盘）。
///
/// 与 [`write_rgba_png`] 是**同一个编码器**（同一组 `png::Encoder` 参数、同一个字节校验），
/// 唯一差别是输出落到 `Vec<u8>` 而不是文件 ⇒ 同尺寸同像素的两次编码**逐字节相同**。
///
/// 为什么要有它：HTTP 端点（`miredit-web` 的 `/api/tiles`）为拿字节曾走
/// 「写系统临时文件 → 读回 → 删掉」，多两次磁盘 IO 且留下一个失败窗口。
/// 需要字节的调用方一律走本函数；**`png` 依赖仍然只住在 png_io.rs 一个文件里**
/// （渲染核心只吃 `Vec<u8>`，见模块文档）。
pub fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    编码(std::io::Cursor::new(&mut out), width, height, rgba)?;
    Ok(out)
}

/// 编码器本体（[`write_rgba_png`] / [`encode_rgba_png`] 共用的**唯一**实现）。
///
/// 字节校验在这里只做一次：`rgba.len()` 与 `width*height*4` 不符 ⇒ `InvalidInput`
/// （不做静默补齐，也不按行截断 —— 那会编出一张尺寸对、内容错的图）。
fn 编码<W: std::io::Write>(w: W, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let expect = width as usize * height as usize * 4;
    if rgba.len() != expect {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "RGBA 字节数 {} 与 {width}×{height} 不符（应为 {expect}）",
                rgba.len()
            ),
        ));
    }
    let mut enc = png::Encoder::new(w, width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    Ok(())
}

/// 读一张 PNG，统一转成 RGBA8（RGB / 灰度 / 带 alpha 的灰度都会展开成 RGBA）。
pub fn read_png(path: &Path) -> std::io::Result<PngImage> {
    let file = BufReader::new(File::open(path)?);
    解码(file)
}

/// 从**内存字节**读一张 PNG，口径与 [`read_png`] 完全相同（同一个解码器）。
///
/// 用途：光罩贴图用 `include_bytes!` 在**编译期**嵌进二进制（见 `crate::lighting`），
/// 运行期不碰磁盘 —— 这样「贴图缺失」是编译错误而不是运行期静默降级。
///
/// ⚠ 本函数仍然只住在本模块：`png` 依赖**只出现在 `png_io.rs` 一个文件**，
/// 渲染核心（`canvas` / `compose` / `lighting`）只吃 `Vec<u8>`（模块文档口径不变）。
pub fn read_png_from_memory(bytes: &[u8]) -> std::io::Result<PngImage> {
    解码(std::io::Cursor::new(bytes))
}

/// 解码器本体（`read_png` / `read_png_from_memory` 共用的唯一实现）。
fn 解码<R: std::io::BufRead + std::io::Seek>(input: R) -> std::io::Result<PngImage> {
    let mut dec = png::Decoder::new(input);
    // EXPAND：把调色板/低位深展开成 8bit；ALPHA：把 tRNS 变成真 alpha。
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = dec.read_info()?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "PNG 尺寸无法确定"))?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf)?;
    let src = &buf[..info.buffer_size()];
    let (w, h) = (info.width, info.height);
    let n = w as usize * h as usize;

    let rgba = match info.color_type {
        png::ColorType::Rgba => src.to_vec(),
        png::ColorType::Rgb => {
            let mut out = vec![255u8; n * 4];
            for i in 0..n {
                out[i * 4] = src[i * 3];
                out[i * 4 + 1] = src[i * 3 + 1];
                out[i * 4 + 2] = src[i * 3 + 2];
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = vec![255u8; n * 4];
            for i in 0..n {
                let g = src[i];
                out[i * 4] = g;
                out[i * 4 + 1] = g;
                out[i * 4 + 2] = g;
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = vec![0u8; n * 4];
            for i in 0..n {
                let g = src[i * 2];
                let a = src[i * 2 + 1];
                out[i * 4] = g;
                out[i * 4 + 1] = g;
                out[i * 4 + 2] = g;
                out[i * 4 + 3] = a;
            }
            out
        }
        png::ColorType::Indexed => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "调色板 PNG 未按 EXPAND 展开（不该发生）",
            ))
        }
    };

    Ok(PngImage {
        width: w,
        height: h,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **同一个编码器**：内存形态与写文件形态必须逐字节相同，且能原样解回来。
    ///
    /// 这条就是 `miredit-web` 敢把「写临时文件再读回」换成 [`encode_rgba_png`] 的判据。
    #[test]
    fn 内存编码与文件编码逐字节相同() {
        let (w, h) = (3u32, 2u32);
        let rgba: Vec<u8> = (0..(w as usize * h as usize * 4)).map(|i| i as u8).collect();

        let mem = encode_rgba_png(w, h, &rgba).expect("内存编码");
        assert_eq!(&mem[..8], b"\x89PNG\r\n\x1a\n", "必须是 PNG 魔数");

        let path = std::env::temp_dir().join(format!(
            "miredit-render-png-编码比对-{}.png",
            std::process::id()
        ));
        write_rgba_png(&path, w, h, &rgba).expect("写文件编码");
        let disk = std::fs::read(&path).expect("读回文件");
        let _ = std::fs::remove_file(&path);
        assert_eq!(mem, disk, "两条路径必须是同一个编码器（逐字节相同）");

        let img = read_png_from_memory(&mem).expect("解码");
        assert_eq!((img.width, img.height), (w, h));
        assert_eq!(img.rgba, rgba);

        // 长度不符 ⇒ InvalidInput（不静默补齐、不按行截断）
        let e = encode_rgba_png(w, h, &rgba[..rgba.len() - 4]).expect_err("长度不符必须报错");
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
    }
}
