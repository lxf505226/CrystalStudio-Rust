//! `resize` 的单元测试（实现见 `resize.rs`）：对齐公式 / 7 档头部逐字节 / 搬格 / 拒绝 / 撤销。
//!
//! 夹具直接构造 [`RawMap`]（字段全 `pub`），头部宽高的**期望偏移在本文件里按 C# 源码另写一份**
//! （不复用生产代码的 `dims_slot`）—— 否则"表写错了"会被"用同一张表断言"掩盖。

use mir2_format::{format_by_id, Cell as RawCell, FormatId, RawMap};
use mir2_map::CellFormat;

use super::*;
use crate::selection::Selection;

/// 该格式在**测试里**认定的头部宽高位置 `(header_len, w_at, h_at, xor_at, tail_len)`。
///
/// 依据（与生产表**独立**，按上游源码抄）：
/// - `Old12/Shanda14/Shanda2012`：`tmap_dims`（`0/2`）、无 XOR；
/// - `Wemade2010`：C# `:174-181`（`21/23/25`）；
/// - `AntiHack`：C# `:332-340`（`31/33/35`）；
/// - `Heroes34`：C# `:520-522`（`21/25`），无 XOR；
/// - `Mir3Shanda`：C# `:460-464`（`16/18`），无 XOR。
fn 头部布局(format: FormatId) -> (usize, usize, usize, Option<usize>, usize) {
    use FormatId as F;
    let len = format_by_id(format).header_len();
    match format {
        F::Old12 | F::Shanda14 => (len, 0, 2, None, 0),
        // 36 字节格：22 字节尾区（`shanda2012.rs` 的 `TAIL_LEN`）
        F::Shanda2012 => (len, 0, 2, None, 22),
        F::Wemade2010 => (len, 21, 25, Some(23), 1),
        F::AntiHack => (len, 31, 35, Some(33), 0),
        F::Heroes34 => (len, 21, 25, None, 1),
        F::Mir3Shanda => (len, 16, 18, None, 7),
        F::Mir3Wemade => (len, 22, 24, None, 4),
        F::CrystalCs => (len, 4, 6, None, 0),
    }
}

/// 造一张 `w×h` 的 raw：头部宽高按该格式的规矩写好，格的 `bk_img` = 格序 + 1（可区分）。
fn 造raw(format: FormatId, w: u16, h: u16) -> RawMap {
    let (len, w_at, h_at, xor_at, tail_len) = 头部布局(format);
    let mut header = vec![0u8; len];
    let key = 0x1234i16;
    if let Some(at) = xor_at {
        header[at..at + 2].copy_from_slice(&key.to_le_bytes());
    }
    let k = if xor_at.is_some() { key } else { 0 };
    header[w_at..w_at + 2].copy_from_slice(&(w as i16 ^ k).to_le_bytes());
    header[h_at..h_at + 2].copy_from_slice(&(h as i16 ^ k).to_le_bytes());
    let mut cells = Vec::with_capacity(w as usize * h as usize);
    for i in 0..w as usize * h as usize {
        let mut c = RawCell::zeroed(tail_len);
        c.bk_img = (i as u32) + 1;
        cells.push(c);
    }
    RawMap {
        format,
        width: w,
        height: h,
        header_raw: header,
        cells,
        trailer: Vec::new(),
    }
}

fn 文档(raw: RawMap) -> Document {
    // 合成头没有判别魔数 ⇒ 不能走 `from_bytes`（那要先过格式判别），用内部构造器
    let mut doc = Document::from_raw(raw);
    doc.rebuild_layer_counts();
    doc
}

fn 格(raw: &RawMap, x: u16, y: u16) -> u32 {
    raw.cells[x as usize * raw.height as usize + y as usize].bk_img
}

// ───────────────────────────── 对齐公式（Crop） ─────────────────────────────

/// `AlignSelection`：四边偶数对齐、右/下往外圆整、夹在图内。
#[test]
fn 裁剪边界对齐公式() {
    // 1×1 在 (5,5) ⇒ 2×2（左/上都往下取偶、右/下往上取偶）
    assert_eq!(
        crop_bounds(20, 20, Selection::new(5, 5, 5, 5)),
        Ok((4, 4, 6, 6))
    );
    // 4×4 且已偶数对齐（4..=7）⇒ 原样 4×4
    assert_eq!(
        crop_bounds(20, 20, Selection::new(4, 4, 7, 7)),
        Ok((4, 4, 8, 8))
    );
    // 4×4 但起点是奇数（3..=6）⇒ 往外圆成 6×6（2..8）
    assert_eq!(
        crop_bounds(20, 20, Selection::new(3, 3, 6, 6)),
        Ok((2, 2, 8, 8))
    );
    // 贴右下角：右/下夹到图边（W=10 ⇒ (9+2)&~1 = 10）
    assert_eq!(
        crop_bounds(10, 10, Selection::new(9, 9, 9, 9)),
        Ok((8, 8, 10, 10))
    );
    // 反向端点先规范化（x0 > x1）
    assert_eq!(
        crop_bounds(20, 20, Selection::new(6, 6, 3, 3)),
        Ok((2, 2, 8, 8))
    );
}

/// 选区整个在图外 ⇒ 明确的「先框选」原因（上游抛 `Select an area inside the map…`）。
#[test]
fn 空选区报明确原因() {
    let e = crop_bounds(10, 10, Selection::new(10, 10, 12, 12)).unwrap_err();
    assert_eq!(e, ResizeError::EmptySelection);
    assert_eq!(e.to_string(), "先在地图上框选要保留的部分再裁剪");
}

// ───────────────────────────── 7 档头部逐字节 ─────────────────────────────

/// **7 档头部宽高逐字节**：写完之后只动该动的 2 个 i16（XOR 档先与既有键异或），其余字节原样。
#[test]
fn 七档头部宽高逐字节() {
    let 支持 = [
        FormatId::Old12,
        FormatId::Shanda14,
        FormatId::Shanda2012,
        FormatId::Wemade2010,
        FormatId::AntiHack,
        FormatId::Heroes34,
        FormatId::Mir3Shanda,
    ];
    for format in 支持 {
        let (len, w_at, h_at, xor_at, _) = 头部布局(format);
        let mut doc = 文档(造raw(format, 4, 4));
        let 前 = doc.raw().header_raw.clone();
        doc.replace_resized(6, 8, 0, 0)
            .unwrap_or_else(|e| panic!("{format:?} 应能改尺寸：{e}"));

        let 后 = doc.raw().header_raw.clone();
        assert_eq!(后.len(), len, "{format:?}：头部长不变");
        let key = match xor_at {
            Some(at) => i16::from_le_bytes([前[at], 前[at + 1]]),
            None => 0,
        };
        assert_eq!(
            i16::from_le_bytes([后[w_at], 后[w_at + 1]]),
            6i16 ^ key,
            "{format:?}：宽要写成 新值 ^ xor"
        );
        assert_eq!(
            i16::from_le_bytes([后[h_at], 后[h_at + 1]]),
            8i16 ^ key,
            "{format:?}：高要写成 新值 ^ xor"
        );
        // 其余字节**一个都不许动**（含 XOR 键本身）
        for (i, (a, b)) in 前.iter().zip(后.iter()).enumerate() {
            if i == w_at || i == w_at + 1 || i == h_at || i == h_at + 1 {
                continue;
            }
            assert_eq!(a, b, "{format:?}：头部第 {i} 字节被改了（{a} → {b}）");
        }
        // 该格式自己的读侧必须读回新尺寸
        assert_eq!(
            format_by_id(format).dims(&后),
            Some((6, 8)),
            "{format:?}：dims() 读不回新尺寸"
        );
    }
}

/// **Wemade2010 / AntiHack 的 XOR 键不许动**（键在宽高之间，写反了整张图都读不回来）。
#[test]
fn xor键在改尺寸后保持原值() {
    for (format, at) in [(FormatId::Wemade2010, 23usize), (FormatId::AntiHack, 33usize)] {
        let mut doc = 文档(造raw(format, 4, 4));
        let 前 = i16::from_le_bytes([doc.raw().header_raw[at], doc.raw().header_raw[at + 1]]);
        assert_eq!(前, 0x1234, "{format:?}：夹具的键");
        doc.replace_resized(10, 12, 0, 0).expect("应能改尺寸");
        let 后 = i16::from_le_bytes([doc.raw().header_raw[at], doc.raw().header_raw[at + 1]]);
        assert_eq!(后, 前, "{format:?}：XOR 键必须原样保留");
    }
}

/// **to_bytes → 读回**：七档改尺寸后都能按该格式重新读出来，且尺寸 = 新尺寸。
#[test]
fn 七档改尺寸后能读回() {
    for format in [
        FormatId::Old12,
        FormatId::Shanda14,
        FormatId::Shanda2012,
        FormatId::Wemade2010,
        FormatId::AntiHack,
        FormatId::Heroes34,
        FormatId::Mir3Shanda,
    ] {
        let mut doc = 文档(造raw(format, 4, 4));
        doc.replace_resized(6, 2, 0, 0)
            .unwrap_or_else(|e| panic!("{format:?} 应能改尺寸：{e}"));
        let bytes = doc.try_to_bytes().expect("出字节");
        let 回读 = format_by_id(format)
            .read(&bytes)
            .unwrap_or_else(|e| panic!("{format:?} 读回失败：{e}"));
        assert_eq!((回读.width, 回读.height), (6, 2), "{format:?}：读回的尺寸");
        assert_eq!(回读.cells.len(), 12, "{format:?}：格数 = 6×2");
    }
}

// ───────────────────────────── 搬格 / 空白边框 ─────────────────────────────

/// **裁剪**：只拷区内格、按 `(left, top)` 平移；区外的格不进来。
#[test]
fn 裁剪只搬区内格() {
    let mut doc = 文档(造raw(FormatId::Old12, 6, 4));
    // 原图 bk_img = 格序+1（列优先 x*4+y）
    doc.replace_resized(4, 2, 2, 1).expect("应能裁剪");
    let raw = doc.raw();
    assert_eq!((raw.width, raw.height), (4, 2));
    for x in 0..4u16 {
        for y in 0..2u16 {
            assert_eq!(
                格(raw, x, y),
                u32::from((2 + x) * 4 + (1 + y) + 1),
                "新格 ({x},{y}) 应来自旧格 ({}, {})",
                2 + x,
                1 + y
            );
        }
    }
}

/// **扩边**：旧格整体平移、四周是新空白格（不是旧图循环填充）。
#[test]
fn 扩边四周是空白格() {
    let mut doc = 文档(造raw(FormatId::Old12, 4, 2));
    doc.replace_resized(8, 6, -2, -2).expect("应能扩边");
    let raw = doc.raw();
    assert_eq!((raw.width, raw.height), (8, 6));
    // 旧 (0,0) 现在在 (2,2)
    assert_eq!(格(raw, 2, 2), 1, "旧 (0,0) 的图号要跟着走");
    assert_eq!(格(raw, 5, 3), 8, "旧 (3,1)（bk_img = 3*2+1+1）现在在 (5,3)");
    // 边框全空（图号 0）
    for x in 0..8u16 {
        assert_eq!(格(raw, x, 0), 0, "上边框第一行必须空");
        assert_eq!(格(raw, x, 5), 0, "下边框最后一行必须空");
    }
    for y in 0..6u16 {
        assert_eq!(格(raw, 0, y), 0, "左边框第一列必须空");
        assert_eq!(格(raw, 7, y), 0, "右边框最后一列必须空");
    }
}

/// **整图同尺寸裁剪 = no-op**：不入撤销栈、不置脏（上游 `CropSelection` 的同一条早退）。
#[test]
fn 整图裁剪是空操作() {
    let mut ed = crate::session::EditSession::new(文档(造raw(FormatId::Old12, 4, 4)));
    let 前 = ed.document().try_to_bytes().expect("出字节");
    let (w, h) = ed
        .crop_to(Selection::new(0, 0, 3, 3))
        .expect("整图裁剪不该报错");
    assert_eq!((w, h), (4, 4), "尺寸不变");
    assert_eq!(ed.undo_len(), 0, "整图裁剪不许入撤销栈");
    assert!(!ed.dirty(), "整图裁剪不许置脏");
    assert_eq!(ed.document().try_to_bytes().expect("出字节"), 前, "字节不变");
}

// ───────────────────────────── 拒绝路径 ─────────────────────────────

/// `trailer` 非空 ⇒ 拒绝（说不清那段字节是什么，不许赌）。
#[test]
fn 有尾块拒绝改尺寸() {
    let mut raw = 造raw(FormatId::Old12, 4, 4);
    raw.trailer = vec![0xAB; 8];
    let mut ed = crate::session::EditSession::new(文档(raw));
    let e = ed
        .crop_to(Selection::new(0, 0, 1, 1))
        .expect_err("有尾块必须拒绝");
    assert!(matches!(e, ResizeError::Trailer { bytes: 8 }), "{e:?}");
    assert_eq!(ed.undo_len(), 0, "失败不许压步");
    assert!(!ed.dirty(), "失败不许置脏");
    assert_eq!(ed.document().size(), (4, 4), "失败不许写");
}

/// `Mir3Wemade`（块级背景在头部）与 `CrystalCs`（变长格 + 印章元数据）⇒ 显式拒绝。
#[test]
fn 两档不支持改尺寸() {
    for format in [FormatId::Mir3Wemade, FormatId::CrystalCs] {
        let mut doc = 文档(造raw(format, 4, 4));
        let e = doc
            .replace_resized(6, 6, 0, 0)
            .expect_err("{format:?} 必须拒绝");
        match e {
            ResizeError::UnsupportedFormat { format: f, why } => {
                assert_eq!(f, format);
                assert!(!why.is_empty(), "原因要说清");
            }
            other => panic!("{format:?}：错的变体 {other:?}"),
        }
        assert_eq!(doc.size(), (4, 4), "拒绝后尺寸不变");
    }
}

/// 尺寸上限：单边 > 32767 或格数 > 16,000,000 ⇒ 拒绝。
#[test]
fn 尺寸越界拒绝() {
    let mut doc = 文档(造raw(FormatId::Old12, 2, 2));
    // 5000×5000 = 25,000,000 > 16,000,000
    let e = doc.replace_resized(5000, 5000, 0, 0).expect_err("格数越界");
    assert!(matches!(e, ResizeError::Dimensions { .. }), "{e:?}");
    assert_eq!(doc.size(), (2, 2));
}

/// 扩边判据（上游 `CanExpand`）：负数 / 奇数 / 全 0 各自的原因。
#[test]
fn 扩边判据() {
    assert_eq!(
        expand_target(10, 10, -2, 0, 0, 0),
        Err(ResizeError::NegativeAmount)
    );
    assert_eq!(
        expand_target(10, 10, 1, 0, 0, 0),
        Err(ResizeError::OddAmount)
    );
    assert_eq!(
        expand_target(10, 10, 0, 0, 0, 0),
        Err(ResizeError::NothingToDo)
    );
    assert_eq!(expand_target(10, 10, 2, 0, 0, 4), Ok((12, 14)));
    assert!(matches!(
        expand_target(32767, 2, 2, 0, 0, 0),
        Err(ResizeError::Dimensions { .. })
    ));
}

// ───────────────────────────── 撤销 / 重做 ─────────────────────────────

/// **一步撤销 / 一步重做**：尺寸、格、字节全回原；脏标记跟着步号走。
#[test]
fn 裁剪可撤销可重做() {
    let mut ed = crate::session::EditSession::new(文档(造raw(FormatId::Old12, 6, 4)));
    let 前 = ed.document().try_to_bytes().expect("出字节");
    ed.crop_to(Selection::new(2, 0, 5, 3)).expect("应能裁剪");
    assert_eq!(ed.document().size(), (4, 4), "裁剪后 4×4");
    assert_eq!(ed.undo_len(), 1, "整次裁剪 = 一步");
    assert!(ed.dirty(), "裁剪后必须是脏的");

    let n = ed.undo();
    assert!(n > 0, "撤销要报还原的格数");
    assert_eq!(ed.document().size(), (6, 4), "撤销回原尺寸");
    assert_eq!(ed.document().try_to_bytes().expect("出字节"), 前, "撤销后字节逐字节回原");
    assert!(!ed.dirty(), "撤销回保存点 ⇒ 不脏");
    assert_eq!(ed.undo_len(), 0);
    assert_eq!(ed.redo_len(), 1, "尺寸变化可重做");

    let n = ed.redo();
    assert!(n > 0);
    assert_eq!(ed.document().size(), (4, 4), "重做回 4×4");
    assert!(ed.dirty(), "重做后又是脏的");
}

/// 裁剪后**选区被清掉**（旧坐标在新图里含义变了）。
#[test]
fn 裁剪后清选区() {
    let mut ed = crate::session::EditSession::new(文档(造raw(FormatId::Old12, 8, 8)));
    ed.set_selection(Selection::new(2, 2, 5, 5));
    assert!(ed.selection().is_some());
    ed.crop_to(Selection::new(2, 2, 5, 5)).expect("应能裁剪");
    assert!(ed.selection().is_none(), "裁剪后必须清选区（上游 SetDocument 同义）");
}

/// 扩边 = 一步撤销，撤销后字节逐字节回原。
#[test]
fn 扩边可撤销() {
    let mut ed = crate::session::EditSession::new(文档(造raw(FormatId::Old12, 4, 4)));
    let 前 = ed.document().try_to_bytes().expect("出字节");
    ed.expand_by(2, 4, 6, 8).expect("应能扩边");
    assert_eq!(ed.document().size(), (4 + 2 + 6, 4 + 4 + 8));
    assert!(ed.dirty());
    ed.undo();
    assert_eq!(ed.document().size(), (4, 4));
    assert_eq!(ed.document().try_to_bytes().expect("出字节"), 前);
}

/// **五层计数与视图跟着重建**：改完尺寸后计数 == 全量重算。
#[test]
fn 改尺寸后计数与视图一致() {
    let mut doc = 文档(造raw(FormatId::Old12, 6, 4));
    let 前计数 = doc.layer_counts();
    doc.replace_resized(4, 4, 2, 0).expect("应能裁剪");
    assert_eq!(doc.size(), (4, 4));
    assert_eq!(doc.view().width, 4, "视图宽要跟着改（就地换字段）");
    assert_eq!(doc.view().height, 4);
    assert_eq!(doc.view().cells.len(), 16);
    assert_eq!(doc.layer_counts(), doc.rebuild_layer_counts(), "计数要等于全量重算");
    assert_ne!(doc.layer_counts(), 前计数, "裁掉两列 ⇒ 计数必然变");
}

/// 裁剪后**存盘再打开**（`Document::from_bytes`）尺寸与格内容都对得上。
#[test]
fn 裁剪后可重开() {
    let mut doc = 文档(造raw(FormatId::Old12, 6, 4));
    doc.replace_resized(4, 2, 2, 1).expect("应能裁剪");
    let bytes = doc.try_to_bytes().expect("出字节");
    let 重开 = Document::from_bytes(std::path::Path::new(r"I:\__resize_test__\again.map"), bytes)
        .expect("重开应成功");
    assert_eq!(重开.size(), (4, 2));
    assert_eq!(重开.raw().format, FormatId::Old12);
    // 视图档位仍是 Old12 档（12 B/格）
    assert_eq!(重开.view().format, CellFormat::Old12);
    for x in 0..4u16 {
        for y in 0..2u16 {
            assert_eq!(
                格(重开.raw(), x, y),
                u32::from((2 + x) * 4 + (1 + y) + 1),
                "重开后 ({x},{y}) 的图号"
            );
        }
    }
}
