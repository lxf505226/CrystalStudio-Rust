//! **分块渲染 + 拼接**的验收（2026-11-06 从 `miredit-gui` 上移后新增）。
//!
//! # 一、为什么要有这份测试
//!
//! 分块渲染的正确性判据只有一条：**拼出来的整幅必须与「同一窗口整幅一次渲」逐字节相同**
//! （这叫「无接缝」）。上移（GUI → `miredit-render::tiled`）时还多一条：
//! **与上移前的输出逐字节相同**。
//!
//! # 二、三道证据
//!
//! 1. [`分块渲染与上移前的黄金哈希逐字节相同`]：三个档位（`1` / `0.5` / `0.25`）的
//!    输出哈希是**上移前**用 GUI 的旧实现跑出来、抄进这里的常量（含尺寸、块数、最大扩边）。
//!    任何一位像素变了这条就红 —— 这是「GUI 输出不变」的可执行证据；
//! 2. [`分块拼接与整幅一次渲逐字节相同`]：同一场景（70×40 格，块网格 2×1）上
//!    `render_tiled` 与 `try_render_region_scaled` **逐字节比**；
//! 3. 失败路径：整幅超单边 ⇒ [`TiledError::OutputTooLarge`]（**不截断、不降级**），
//!    0 格窗口 ⇒ [`TiledError::Internal`]（调用方 bug，不是环境问题）。
//!
//! 场景是**合成**的（不读真实素材）：`70×40` 格 + 帧号 1 出 `144×96` 大帧
//! ⇒ 帧会跨块边界（块 0 是 0..63 列、块 1 是 64..69 列），扩边算错必改像素。
//! 真实素材的等价性另由 `miredit-gui` 的 448 条测试（含真实 `0.map` 导出门）覆盖。

use mir2_assets::{Frame, LibRef, SpriteSource};
use mir2_map::Map;

use miredit_render::tiled::{render_tiled, scaled_size, TiledError, MAX_MARGIN_CELLS, TILE_CELLS};
use miredit_render::{
    try_render_region_scaled, CollisionOverlay, LayerMask, RenderOptions, RenderScale, Viewport,
};

/// 行优先合成一张 legacy 9 格式地图（与真实 `.map` 的 `y*w + x` 布局一致）。
fn 造图(w: u16, h: u16, f: impl Fn(u16, u16) -> (u16, u16, u16)) -> Map {
    let mut bytes = vec![0u8; 52 + 12 * (w as usize * h as usize)];
    bytes[0..2].copy_from_slice(&w.to_le_bytes());
    bytes[2..4].copy_from_slice(&h.to_le_bytes());
    for y in 0..h {
        for x in 0..w {
            let (bk, mid, fr) = f(x, y);
            let base = 52 + (y as usize * w as usize + x as usize) * 12;
            bytes[base..base + 2].copy_from_slice(&bk.to_le_bytes());
            bytes[base + 2..base + 4].copy_from_slice(&mid.to_le_bytes());
            bytes[base + 4..base + 6].copy_from_slice(&fr.to_le_bytes());
        }
    }
    Map::parse(&bytes).expect("合成地图应能解析")
}

/// 假帧源：`index == 1` 出**大帧**（逼出扩边），其余出小帧；颜色随帧号变 ⇒ 错位必改像素。
struct 假源 {
    小帧: (u32, u32),
    大帧: (u32, u32),
}

impl SpriteSource for 假源 {
    fn frame(&self, _lib: LibRef, index: u32) -> Option<Frame> {
        let (w, h) = if index == 1 { self.大帧 } else { self.小帧 };
        let c = [
            (index.wrapping_mul(17) & 0xFF) as u8,
            (w % 251) as u8,
            (h % 241) as u8,
            255,
        ];
        let mut rgba = Vec::with_capacity((w * h) as usize * 4);
        for _ in 0..(w * h) {
            rgba.extend_from_slice(&c);
        }
        Some(Frame {
            width: w,
            height: h,
            rgba,
            ..Frame::default()
        })
    }
}

/// 黄金场景：`70×40` 格（块网格 2×1），帧最大 3×3 格。
///
/// - 地表：偶偶格图号 1（小帧 `96×64` = 2×2 格）
/// - 中层：每 5 格一个图号 1
/// - 表层：每 4 格一个图号 2（大帧 `144×96` = 3×3 格），另在 `(61,17)` 再放一个压块边界
fn 黄金场景() -> (Map, 假源) {
    let map = 造图(70, 40, |x, y| {
        let bk = if x % 2 == 0 && y % 2 == 0 { 1 } else { 0 };
        let mid = if (x * 7 + y * 13) % 5 == 0 { 1 } else { 0 };
        let fr = if (x * 3 + y) % 4 == 0 || (x == 61 && y == 17) {
            2
        } else {
            0
        };
        (bk, mid, fr)
    });
    (
        map,
        假源 {
            小帧: (96, 64),
            大帧: (144, 96),
        },
    )
}

/// FNV-1a 64（测试用的稳定哈希；不引第三方 crate）。
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// **与上移前的 GUI 输出逐字节相同**（三个档位各一组）。
///
/// 期望值来源：2026-11-06 在 `miredit-gui` 上移**之前**，用当时的 `export::render_tiled`
/// 跑本场景、打印尺寸/块数/最大扩边/FNV 得到（同一次运行里还断言了
/// `分块.rgba == try_render_region_scaled(整幅).rgba`，见下一条）。
/// 把它写死在这里，就能在任何时候回答「上移有没有改输出」。
#[test]
fn 分块渲染与上移前的黄金哈希逐字节相同() {
    let (map, src) = 黄金场景();
    // (档位, 期望宽, 期望高, 期望块数, 期望最大扩边, 期望 FNV-1a)
    let 期望 = [
        (RenderScale::One, 3360u32, 1280u32, 2u32, 2u32, 0xbbf2_acf9_847a_3325u64),
        (RenderScale::Half, 1680, 640, 2, 2, 0xf0c4_a0a7_de3d_e725),
        (RenderScale::Quarter, 840, 320, 2, 2, 0xf53a_c98d_4c63_1425),
    ];
    for (scale, w, h, tiles, margin, want) in 期望 {
        let img = render_tiled(
            &map,
            &src,
            &LayerMask::all(),
            CollisionOverlay::none(),
            scale,
            (0, 0),
            (70, 40),
            RenderOptions::closed(),
        )
        .expect("分块渲染应成功");
        assert_eq!((img.width, img.height), (w, h), "{} 的输出尺寸", scale.label());
        assert_eq!(img.tiles, tiles, "{} 的块数", scale.label());
        assert_eq!(img.margin, margin, "{} 的最大扩边", scale.label());
        assert_eq!(img.rgba.len(), (w as usize) * (h as usize) * 4);
        assert_eq!(
            fnv(&img.rgba),
            want,
            "{} 的像素与上移前不一致（哪怕一位）",
            scale.label()
        );
    }
}

/// **无接缝**：分块拼出来的整幅 == 同窗口「整幅一次渲」。
///
/// 用 70×40 格（1× 只有 `3360×1280×4 = 17.2 MB`，单次调用装得下）⇒ 两条路径都能跑，
/// 才能逐字节比。块边界正好落在第 64 列，而表层大帧（3×3 格）会跨过去
/// ⇒ 扩边少算一格这条就红。
#[test]
fn 分块拼接与整幅一次渲逐字节相同() {
    let (map, src) = 黄金场景();
    for scale in [RenderScale::One, RenderScale::Half, RenderScale::Quarter] {
        let 分块 = render_tiled(
            &map,
            &src,
            &LayerMask::all(),
            CollisionOverlay::none(),
            scale,
            (0, 0),
            (70, 40),
            RenderOptions::closed(),
        )
        .expect("分块渲染");
        let 整幅 = try_render_region_scaled(
            &map,
            &src,
            &Viewport::new(0, 0, 70, 40),
            &LayerMask::all(),
            CollisionOverlay::none(),
            scale,
        )
        .expect("整幅一次渲");
        assert_eq!(
            (分块.width, 分块.height),
            (整幅.width, 整幅.height),
            "{} 的尺寸",
            scale.label()
        );
        assert_eq!(
            分块.rgba, 整幅.rgba,
            "{}：分块拼图与整幅一次渲必须逐字节相同（块边界丢/多像素都会红）",
            scale.label()
        );
    }
}

/// 整幅输出超单边上限 ⇒ [`TiledError::OutputTooLarge`]，**不截断、不降级**。
///
/// `700×700` 格 `@One` = `33600×22400` px（单边 > 32768）；这条与「分块能不能渲」无关，
/// 是**交付物本身**的上限（[`miredit_render::MAX_EXPORT_SIDE`]）。
#[test]
fn 整幅超单边上限明确报错() {
    let map = 造图(700, 700, |_, _| (0, 0, 0));
    let err = render_tiled(
        &map,
        &假源 {
            小帧: (96, 64),
            大帧: (144, 96),
        },
        &LayerMask::all(),
        CollisionOverlay::none(),
        RenderScale::One,
        (0, 0),
        (700, 700),
        RenderOptions::closed(),
    )
    .expect_err("整幅超单边必须报错");
    match &err {
        TiledError::OutputTooLarge(msg) => {
            assert!(msg.contains("单边上限"), "{msg}");
            assert!(msg.contains("33600") && msg.contains("32768"), "{msg}");
        }
        other => panic!("应是 OutputTooLarge，实得 {other:?}"),
    }
    assert!(err.is_capacity(), "容量类失败必须判成 capacity（HTTP 层映射 413）");
    assert!(err.to_string().contains("导出图太大"), "{err}");
}

/// 0 格窗口是**调用方 bug**（不是环境问题）⇒ `Internal`，且不是 capacity。
#[test]
fn 零格窗口明确报错() {
    let (map, src) = 黄金场景();
    let err = render_tiled(
        &map,
        &src,
        &LayerMask::all(),
        CollisionOverlay::none(),
        RenderScale::One,
        (0, 0),
        (0, 5),
        RenderOptions::closed(),
    )
    .expect_err("0 格必须报错");
    assert!(matches!(err, TiledError::Internal(_)), "{err:?}");
    assert!(!err.is_capacity(), "0 格不是容量问题");
    assert!(err.to_string().contains("导出窗口为 0 格"), "{err}");
}

/// 块边长 / 扩边上界 / 尺寸换算的常量口径（与上移前逐值一致）。
#[test]
fn 块常量与尺寸换算不变() {
    assert_eq!(TILE_CELLS, 64);
    assert_eq!(MAX_MARGIN_CELLS, 48);
    assert_eq!(scaled_size(1, 1, RenderScale::Quarter), (12, 8));
    assert_eq!(scaled_size(1, 1, RenderScale::Half), (24, 16));
    assert_eq!(scaled_size(1, 1, RenderScale::One), (48, 32));
    assert_eq!(scaled_size(1, 1, RenderScale::Two), (96, 64));
    // 块边长 × 档位 必须落在整数像素上（拼接的前提）
    for scale in RenderScale::ALL {
        let (w, h) = scaled_size(TILE_CELLS, TILE_CELLS, scale);
        let (rw, rh) = scale.pixel_size(TILE_CELLS, TILE_CELLS).expect("可算");
        assert_eq!((w, h), (rw, rh), "{} 档的整数换算与 pixel_size 必须一致", scale.label());
    }
}
