//! `map_check` 的单元测试（实现见 `map_check.rs`；本文件不参与产物）。
//!
//! 夹具一律**直接构造 [`RawMap`]**（字段都是 `pub`）：检查器只读字节层，
//! 用合成格表就能把「公式 / 掩码 / 哨兵 / 可走」逐条钉死，不必先造九种合法字节流。
//! 唯一走真字节流的用例是 `原始raw与视图area一致`（Old12 / Shanda14，因为这两种
//! `mir2-format` 的 writer 无尾区、往返最干净）。

use std::cell::RefCell;
use std::collections::HashMap;

use mir2_format::{format_by_id, Cell as RawCell, FormatId, RawMap};

use super::*;

// ─────────────────────────────── 夹具 ───────────────────────────────

/// 一张 `w×h` 的空 raw（全 `Cell::default()`；头部长度取该格式的**真值**，尾区空）。
fn 空raw(format: FormatId, w: u16, h: u16) -> RawMap {
    RawMap {
        format,
        width: w,
        height: h,
        header_raw: vec![0u8; format_by_id(format).header_len()],
        cells: vec![RawCell::default(); w as usize * h as usize],
        trailer: Vec::new(),
    }
}

/// 取某格的可变引用（列优先 `x * h + y`，与 `RawMap` 的存储口径一致）。
fn 格(raw: &mut RawMap, x: u16, y: u16) -> &mut RawCell {
    let h = raw.height as usize;
    &mut raw.cells[x as usize * h + y as usize]
}

/// **无头部常数**（七档用：没有 XOR 键、也不是 CrystalCs）。
fn 无键() -> 头部 {
    头部 { xor: 0, crystal_ver: 0 }
}

/// 带 **XOR 键**的头部常数（`AntiHack` / `Wemade2010`）。
fn 键(xor: i16) -> 头部 {
    头部 { xor, crystal_ver: 0 }
}

/// `CrystalCs` 的头部常数（版本号决定装载期折不折前景 `bit15`）。
fn 晶(ver: i16) -> 头部 {
    头部 { xor: 0, crystal_ver: ver }
}

/// 把 XOR 键写到该格式头部的位置（`AntiHack` 偏移 33 / `Wemade2010` 偏移 23）。
fn 设xor(raw: &mut RawMap, key: i16) {
    let off = match raw.format {
        FormatId::AntiHack => mir2_format::formats::antihack::XOR_OFFSET,
        FormatId::Wemade2010 => mir2_format::formats::wemade2010::XOR_OFFSET,
        _ => panic!("该格式没有 XOR 键"),
    };
    raw.header_raw[off..off + 2].copy_from_slice(&key.to_le_bytes());
}

/// 假探针：`帧数` 给每个库的帧数；`坏` 里的 `(族,族内页号,图号)` 直接给指定状态。
#[derive(Default)]
struct 假探针 {
    帧数: HashMap<(LibFamily, u16), u32>,
    坏: HashMap<(LibFamily, u16, u32), ImageState>,
    查过: RefCell<Vec<(MapLibRef, u32)>>,
}

impl 假探针 {
    fn 一次没问(&self) -> bool {
        self.查过.borrow().is_empty()
    }
}

impl LibProbe for 假探针 {
    fn frame_state(&self, lib: MapLibRef, image_1based: u32) -> ImageState {
        self.查过.borrow_mut().push((lib, image_1based));
        if let Some(s) = self.坏.get(&(lib.family, lib.index, image_1based)) {
            return *s;
        }
        match self.帧数.get(&(lib.family, lib.index)) {
            None => ImageState::MissingLibrary,
            Some(n) if image_1based > *n => ImageState::OutOfRange,
            Some(_) => ImageState::Ok,
        }
    }
}

// ─────────────────────────── 公式层（9 档） ───────────────────────────

/// 表 A：前景库的 C# 全局号（含 Mir3 哨兵与 CrystalCs 负数原样）。
#[test]
fn 九档前景库公式() {
    let 改 = |format: FormatId, area: u8, fr_img: u16, fr_idx: i16| {
        let mut raw = 空raw(format, 1, 1);
        let c = 格(&mut raw, 0, 0);
        c.area = area;
        c.fr_img = fr_img;
        c.fr_idx = fr_idx;
        csharp_front_lib(format, c)
    };
    // Wemade 段：area + 2
    for f in [FormatId::Old12, FormatId::AntiHack, FormatId::Wemade2010, FormatId::Heroes34] {
        assert_eq!(改(f, 0, 0, 0), 2, "{f:?}：area=0 ⇒ 全局 2（Objects 本尊）");
        assert_eq!(改(f, 3, 0, 0), 5, "{f:?}：area=3 ⇒ 全局 5");
    }
    // Shanda 段：area + 120
    for f in [FormatId::Shanda14, FormatId::Shanda2012] {
        assert_eq!(改(f, 2, 0, 0), 122, "{f:?}：area=2 ⇒ 全局 122");
        assert_eq!(改(f, 255, 0, 0), 375, "{f:?}：area 是字节，不设哨兵");
    }
    // Mir3Wemade：255 ⇒ -1；否则 +200；再哨兵修正（图号 == 1 且 lib == 200 ⇒ -1）
    assert_eq!(改(FormatId::Mir3Wemade, 255, 7, 0), -1, "磁盘 255 ⇒ -1");
    assert_eq!(改(FormatId::Mir3Wemade, 5, 7, 0), 205);
    assert_eq!(改(FormatId::Mir3Wemade, 0, 5, 0), 200, "图号 6 ⇒ 不触发哨兵修正");
    assert_eq!(
        改(FormatId::Mir3Wemade, 0, 0, 0),
        -1,
        "盘上 fr_img=0 ⇒ csharp_image=1 且 lib==200 ⇒ C# 把它压成 -1（幻影前景）"
    );
    // Mir3Shanda：255 ⇒ -1；否则 +300（上游那条 `frimage==1 && lib==200` 对本支是死代码）
    assert_eq!(改(FormatId::Mir3Shanda, 255, 7, 0), -1);
    assert_eq!(改(FormatId::Mir3Shanda, 4, 0, 0), 304, "死代码不影响：只可能 -1 或 ≥300");
    // CrystalCs：i16 原样（负数照样原样，不 max(0)）
    assert_eq!(改(FormatId::CrystalCs, 0, 7, -7), -7);
    assert_eq!(改(FormatId::CrystalCs, 9, 7, 1234), 1234, "area 不参与：真值在 fr_idx");
}

/// 表 B 地面库：六档常量 / Shanda `tiles+100` / Mir3 `bk_idx±` / CrystalCs 原样。
#[test]
fn 九档地面库公式() {
    let 改 = |format: FormatId, bk_idx: i16, tiles: u16| {
        let mut raw = 空raw(format, 1, 1);
        let c = 格(&mut raw, 0, 0);
        c.bk_idx = bk_idx;
        c.tiles = tiles;
        csharp_back_lib(format, c)
    };
    for f in [FormatId::Old12, FormatId::AntiHack, FormatId::Wemade2010, FormatId::Heroes34] {
        assert_eq!(改(f, 9, 9), 0, "{f:?}：C# 写死 BackIndex = 0");
    }
    for f in [FormatId::Shanda14, FormatId::Shanda2012] {
        assert_eq!(改(f, 0, 7), 107, "{f:?}：btTiles + 100");
        assert_eq!(改(f, 0, 0), 100);
    }
    assert_eq!(改(FormatId::Mir3Wemade, 5, 0), 205, "bk_idx + 200");
    assert_eq!(改(FormatId::Mir3Wemade, 255, 0), -1, "255 哨兵");
    assert_eq!(改(FormatId::Mir3Shanda, 5, 0), 305, "bk_idx + 300");
    assert_eq!(改(FormatId::Mir3Shanda, 255, 0), -1);
    assert_eq!(改(FormatId::CrystalCs, -3, 0), -3, "CrystalCs 原样（负数不夹）");
}

/// 表 B 中层库：六档常量 / Shanda `sm_tiles+110` / Mir3 `mid_idx±` / CrystalCs 原样。
#[test]
fn 九档中层库公式() {
    let 改 = |format: FormatId, mid_idx: i16, sm_tiles: u16| {
        let mut raw = 空raw(format, 1, 1);
        let c = 格(&mut raw, 0, 0);
        c.mid_idx = mid_idx;
        c.sm_tiles = sm_tiles;
        csharp_middle_lib(format, c)
    };
    for f in [FormatId::Old12, FormatId::AntiHack, FormatId::Wemade2010, FormatId::Heroes34] {
        assert_eq!(改(f, 9, 9), 1, "{f:?}：C# 写死 MiddleIndex = 1");
    }
    for f in [FormatId::Shanda14, FormatId::Shanda2012] {
        assert_eq!(改(f, 0, 4), 114, "{f:?}：btsmTiles + 110");
    }
    assert_eq!(改(FormatId::Mir3Wemade, 6, 0), 206);
    assert_eq!(改(FormatId::Mir3Wemade, 255, 0), -1);
    assert_eq!(改(FormatId::Mir3Shanda, 6, 0), 306);
    assert_eq!(改(FormatId::CrystalCs, -9, 0), -9);
}

// ───────────────────────── 层 → 库结论（可查/未检查） ─────────────────────────

/// **六档可查、三档整层未检查**，且 [`layer_source`] 与 [`unchecked_reason`] **同源**。
///
/// 这条是防漂移的钉子：谁只改一边（例如把某个格式从「未检查」放出来），这里当场红。
#[test]
fn 未检查与逐格结论同源() {
    let 全档 = [
        FormatId::Old12, FormatId::Shanda14, FormatId::Shanda2012, FormatId::Wemade2010,
        FormatId::AntiHack, FormatId::Mir3Wemade, FormatId::Mir3Shanda, FormatId::Heroes34,
        FormatId::CrystalCs,
    ];
    let 可查 = [
        FormatId::Old12, FormatId::Shanda14, FormatId::Shanda2012, FormatId::Wemade2010,
        FormatId::AntiHack, FormatId::Heroes34,
    ];
    for format in 全档 {
        let mut raw = 空raw(format, 1, 1);
        let c = 格(&mut raw, 0, 0);
        // 哨兵法外的一格：三个库号都给正号，Mir3 才不会判「必然缺库」
        c.bk_idx = 5;
        c.mid_idx = 5;
        c.area = 5;
        c.fr_img = 5;
        for family in LibFamily::ALL {
            let 族级 = unchecked_reason(format, family);
            match (layer_source(format, family, c), 族级) {
                (LibSource::Page(_), None) => {
                    assert!(可查.contains(&format), "{format:?}/{family:?} 不该是可查");
                }
                (LibSource::Unchecked(逐格), Some(族级原因)) => {
                    assert_eq!(逐格, 族级原因, "{format:?}/{family:?} 两处原因不一致");
                    assert!(!可查.contains(&format), "{format:?}/{family:?} 不该是未检查");
                }
                (LibSource::Unchecked(_), None) => {
                    panic!("{format:?}/{family:?}：逐格未检查但族级说可查（漂移）")
                }
                (LibSource::Page(_), Some(_)) => {
                    panic!("{format:?}/{family:?}：逐格可查但族级说未检查（漂移）")
                }
                (LibSource::Missing, _) => {
                    panic!("{format:?}/{family:?}：非哨兵格不该判缺库")
                }
            }
        }
    }
    // 三档必须整层未检查（R33：Mir3 两支 + CrystalCs）
    for format in [FormatId::Mir3Wemade, FormatId::Mir3Shanda, FormatId::CrystalCs] {
        for family in LibFamily::ALL {
            assert!(
                unchecked_reason(format, family).is_some(),
                "{format:?}/{family:?} 必须显式未检查（不许猜页号）"
            );
        }
    }
}

/// **族内页号 = 地图字段原值**（不是 C# 全局号 +2/+120/+200/+300）。
#[test]
fn 页号是字段原值不是全局号() {
    // Wemade 段：地面/中层是常量 0，物件 = area
    let mut raw = 空raw(FormatId::Wemade2010, 1, 1);
    let c = 格(&mut raw, 0, 0);
    c.area = 3;
    c.tiles = 7;
    c.sm_tiles = 8;
    assert_eq!(
        layer_source(FormatId::Wemade2010, LibFamily::Ground, c),
        LibSource::Page(MapLibRef::ground(0))
    );
    assert_eq!(
        layer_source(FormatId::Wemade2010, LibFamily::Middle, c),
        LibSource::Page(MapLibRef::middle(0))
    );
    assert_eq!(
        layer_source(FormatId::Wemade2010, LibFamily::Objects, c),
        LibSource::Page(MapLibRef::surface(3))
    );
    // Shanda 段：地面 = btTiles、中层 = btsmTiles、物件 = area（**不加 100/110/120**）
    let mut raw = 空raw(FormatId::Shanda14, 1, 1);
    let c = 格(&mut raw, 0, 0);
    c.area = 3;
    c.tiles = 7;
    c.sm_tiles = 8;
    assert_eq!(
        layer_source(FormatId::Shanda14, LibFamily::Ground, c),
        LibSource::Page(MapLibRef::ground(7))
    );
    assert_eq!(
        layer_source(FormatId::Shanda14, LibFamily::Middle, c),
        LibSource::Page(MapLibRef::middle(8))
    );
    assert_eq!(
        layer_source(FormatId::Shanda14, LibFamily::Objects, c),
        LibSource::Page(MapLibRef::surface(3))
    );
}

/// CrystalCs：三层 `未检查`，**一次探针都不调**，也不把 `fr_idx` 原样当页号。
#[test]
fn 晶体格式三层未检查且不探测() {
    let mut raw = 空raw(FormatId::CrystalCs, 2, 2);
    for (i, cell) in raw.cells.iter_mut().enumerate() {
        cell.bk_idx = 0;
        cell.bk_img = 11;
        cell.mid_idx = 1;
        cell.mid_img = 12;
        cell.fr_idx = -7; // 负数也原样：不 max(0)、不猜 0
        cell.fr_img = 13;
        cell.door = 0x80; // 掩码后 = 0 ⇒ 不算门格
        cell.area = i as u8;
    }
    let probe = 假探针::default();
    let r = check_map(&raw, &probe);
    assert_eq!(r.total, 0, "未检查的层不许报问题：{:?}", r.issues);
    assert_eq!(r.未检查.len(), 3, "三层都要如实标记：{:?}", r.未检查);
    assert_eq!(r.unchecked_reason(LibFamily::Objects), Some(UNCHECKED_CRYSTALCS));
    assert!(probe.一次没问(), "未检查的层绝不许探测：{:?}", probe.查过.borrow());
}

/// Mir3：`255` 哨兵那格 = **可证的**缺库（C# 读成 −1），其余槽位 = 未检查；
/// 两种情形都**不调探针**。
#[test]
fn mir3_哨兵算缺库其余未检查() {
    // Mir3Wemade 物件层：area=255 ⇒ lib=-1 ⇒ Missing library
    let mut raw = 空raw(FormatId::Mir3Wemade, 2, 1);
    格(&mut raw, 0, 0).area = 255; // lib = -1 ⇒ 必然缺库
    格(&mut raw, 0, 0).fr_img = 7;
    // 第二格给一个**非哨兵**槽位（area=3 ⇒ lib=203；fr_img=7 ⇒ 图号 8）：
    // 否则 `area=0 && fr_img=0` 会命中 C# 那条活哨兵修正（lib 200 且图号 1 ⇒ −1），
    // 于是两格都"必然缺库"，这条例子的对照面就没了。
    格(&mut raw, 1, 0).area = 3;
    格(&mut raw, 1, 0).fr_img = 7;
    格(&mut raw, 1, 0).area = 3;
    格(&mut raw, 1, 0).fr_img = 7;
    let probe = 假探针::default();
    let r = check_map(&raw, &probe);
    assert_eq!(r.total, 1, "只有哨兵那格报一条：{:?}", r.issues);
    let line = r.issues[0].line();
    assert!(line.starts_with("(0, 0)"), "{line}");
    assert!(line.contains("Objects: Missing library"), "{line}");
    assert!(line.contains("Library -1"), "要如实写 −1：{line}");
    assert!(line.contains("Image 8"), "Mir3 图号是盘值 +1：{line}");
    assert!(probe.一次没问(), "哨兵格也不许探测：{:?}", probe.查过.borrow());
    // 地面：Type5 背图在块级 ⇒ 一律未检查，即使 bk_idx=255 也不从格内臆测哨兵
    assert_eq!(r.unchecked_reason(LibFamily::Ground), Some(UNCHECKED_MIR3_BLOCK_GROUND));
    assert_eq!(r.unchecked_reason(LibFamily::Middle), Some(UNCHECKED_MIR3_NAMED_SLOTS));
    assert_eq!(r.unchecked_reason(LibFamily::Objects), Some(UNCHECKED_MIR3_NAMED_SLOTS));
    // Mir3Shanda 是逐格三族都读：bk_idx=255 ⇒ 地面也有一条缺库（图号 = 盘值 +1）
    let mut raw = 空raw(FormatId::Mir3Shanda, 1, 1);
    格(&mut raw, 0, 0).bk_idx = 255;
    格(&mut raw, 0, 0).bk_img = 9;
    let probe = 假探针::default();
    let r = check_map(&raw, &probe);
    assert_eq!(r.total, 1, "{:?}", r.issues);
    let m = &r.issues[0].message;
    assert!(m.starts_with("Ground: Missing library"), "{m}");
    assert!(m.contains("Image 10"), "{m}");
    assert!(probe.一次没问());
}

// ───────────────────────────── 图号 / 掩码 ─────────────────────────────

/// 地面图号要先过该格式的「C# 内存表示」，再按是否折叠 `bit15` 掩码。
#[test]
fn 地面图号按格式归一化() {
    // 12 字节格：bit15 是阻挡位 ⇒ 折成 0x20000000，掩码后只剩低 15 位
    let mut raw = 空raw(FormatId::Old12, 1, 1);
    assert_eq!(ground_image(FormatId::Old12, 格(&mut raw, 0, 0), 无键()), 0);
    格(&mut raw, 0, 0).bk_img = 0x8005;
    assert_eq!(ground_image(FormatId::Old12, 格(&mut raw, 0, 0), 无键()), 5);
    // Wemade2010：磁盘值整字 XOR 0xAA38AA38（常量，与头部键无关）
    let mut raw = 空raw(FormatId::Wemade2010, 1, 1);
    格(&mut raw, 0, 0).bk_img = 5u32 ^ 0xAA38_AA38;
    assert_eq!(ground_image(FormatId::Wemade2010, 格(&mut raw, 0, 0), 无键()), 5);
    // 上游 Type1 不折 bk 的 bit15（`LegacyReader.cs:190-211` 漏搬）⇒ 它留在图号里
    let mut raw = 空raw(FormatId::Wemade2010, 1, 1);
    格(&mut raw, 0, 0).bk_img = 0x8003u32 ^ 0xAA38_AA38;
    assert_eq!(
        ground_image(FormatId::Wemade2010, 格(&mut raw, 0, 0), 无键()),
        0x8003,
        "XOR 还原后就是 0x8003；不折叠 ⇒ 掩码只到 0x1FFFFFFF"
    );
    // Heroes34：磁盘原样，bit15 折成 0x20000000；高 16 位保留（掩到 0x1FFFFFFF）
    let mut raw = 空raw(FormatId::Heroes34, 1, 1);
    格(&mut raw, 0, 0).bk_img = 0x8005;
    assert_eq!(ground_image(FormatId::Heroes34, 格(&mut raw, 0, 0), 无键()), 5);
    格(&mut raw, 0, 0).bk_img = 0x0001_0005;
    assert_eq!(ground_image(FormatId::Heroes34, 格(&mut raw, 0, 0), 无键()), 0x1_0005);
    // Shanda2012：36 字节格，bit15 同样先折
    let mut raw = 空raw(FormatId::Shanda2012, 1, 1);
    格(&mut raw, 0, 0).bk_img = 0x8005;
    assert_eq!(ground_image(FormatId::Shanda2012, 格(&mut raw, 0, 0), 无键()), 5);
    // AntiHack：先 ^ xor 再折
    let mut raw = 空raw(FormatId::AntiHack, 1, 1);
    设xor(&mut raw, 0x0010);
    格(&mut raw, 0, 0).bk_img = 0x0007 ^ 0x0010;
    assert_eq!(ground_image(FormatId::AntiHack, 格(&mut raw, 0, 0), 键(0x0010)), 7);
    // Mir3Wemade：格内 bk_img 恒 0（背图在块级）⇒ 图号 0，不报
    let mut raw = 空raw(FormatId::Mir3Wemade, 1, 1);
    格(&mut raw, 0, 0).bk_img = 77;
    assert_eq!(ground_image(FormatId::Mir3Wemade, 格(&mut raw, 0, 0), 无键()), 0);
    // Mir3Shanda：逐格 `(盘值 + 1)` ⇒ 盘 9 = 图号 10
    let mut raw = 空raw(FormatId::Mir3Shanda, 1, 1);
    格(&mut raw, 0, 0).bk_img = 9;
    assert_eq!(ground_image(FormatId::Mir3Shanda, 格(&mut raw, 0, 0), 无键()), 10);
}

/// 中层 / 物件图号：XOR 两档先还原、Mir3 先 `+1`、其余按 `& 0x7FFF`。
#[test]
fn 中层与物件图号() {
    // Old12 中层不掩码（上游 `MiddleImage` 原值）
    let mut raw = 空raw(FormatId::Old12, 1, 1);
    格(&mut raw, 0, 0).mid_img = 0x8301;
    assert_eq!(middle_image(FormatId::Old12, 格(&mut raw, 0, 0), 无键()), 0x8301);
    // Wemade2010：`disk = 图号 ^ xor`（键在头部 23..25）
    let mut raw = 空raw(FormatId::Wemade2010, 1, 1);
    设xor(&mut raw, 0x1234);
    格(&mut raw, 0, 0).mid_img = 5 ^ 0x1234;
    格(&mut raw, 0, 0).fr_img = 7 ^ 0x1234;
    let key = 取头部(FormatId::Wemade2010, &raw.header_raw).map(|k| k.xor);
    assert_eq!(key, Some(0x1234), "头部 23..25 就是 XOR 键");
    assert_eq!(middle_image(FormatId::Wemade2010, 格(&mut raw, 0, 0), 键(0x1234)), 5);
    assert_eq!(objects_image(FormatId::Wemade2010, 格(&mut raw, 0, 0), 键(0x1234)), 7);
    // AntiHack：同样 `^ xor`（键在头部 33..35）
    let mut raw = 空raw(FormatId::AntiHack, 1, 1);
    设xor(&mut raw, 0x0100);
    assert_eq!(取头部(FormatId::AntiHack, &raw.header_raw).map(|k| k.xor), Some(0x0100));
    格(&mut raw, 0, 0).mid_img = 6 ^ 0x0100;
    格(&mut raw, 0, 0).fr_img = 0x8008 ^ 0x0100;
    assert_eq!(middle_image(FormatId::AntiHack, 格(&mut raw, 0, 0), 键(0x0100)), 6);
    assert_eq!(
        objects_image(FormatId::AntiHack, 格(&mut raw, 0, 0), 键(0x0100)),
        8,
        "还原后 0x8008 ⇒ 前景图号 8（bit15 是阻挡位）"
    );
    // 键取不到 ⇒ `None`（调用方把三层都判未检查，不猜 0）
    let mut 短 = 空raw(FormatId::Wemade2010, 1, 1);
    短.header_raw = vec![0u8; 8];
    assert_eq!(取头部(FormatId::Wemade2010, &短.header_raw).map(|k| k.xor), None);
    // Mir3 两支：中层与物件都 +1（`LegacyMapReader.cs:427/428`、`:480/483`）
    let mut raw = 空raw(FormatId::Mir3Wemade, 1, 1);
    格(&mut raw, 0, 0).mid_img = 3;
    格(&mut raw, 0, 0).fr_img = 4;
    assert_eq!(middle_image(FormatId::Mir3Wemade, 格(&mut raw, 0, 0), 无键()), 4);
    assert_eq!(objects_image(FormatId::Mir3Wemade, 格(&mut raw, 0, 0), 无键()), 5);
    let mut raw = 空raw(FormatId::Mir3Shanda, 1, 1);
    格(&mut raw, 0, 0).fr_img = 0x7FFF; // +1 溢出 ⇒ 图号 0（空）
    assert_eq!(objects_image(FormatId::Mir3Shanda, 格(&mut raw, 0, 0), 无键()), 0);
    // 其余：`fr_img & 0x7FFF`（bit15 是阻挡位）
    let mut raw = 空raw(FormatId::Old12, 1, 1);
    格(&mut raw, 0, 0).fr_img = 0x8007;
    assert_eq!(objects_image(FormatId::Old12, 格(&mut raw, 0, 0), 无键()), 7);
}

/// 开门帧：`fr != 0 && (door & 0x7F) != 0` ⇒ `fr + door_offset`（1 基）。
#[test]
fn 开门帧只在门低七位非零时加偏移() {
    let mut raw = 空raw(FormatId::Old12, 1, 1);
    let c = 格(&mut raw, 0, 0);
    c.fr_img = 7;
    c.door = 1;
    c.door_offset = 5;
    assert_eq!(open_door_image(FormatId::Old12, c, 无键()), Some(12));
    // door 只有 bit7 ⇒ `& 0x7F == 0` ⇒ 不算开门帧（上游 `:52`）
    c.door = 0x80;
    assert_eq!(open_door_image(FormatId::Old12, c, 无键()), None);
    // 没有前景图 ⇒ 不算
    c.door = 1;
    c.fr_img = 0;
    assert_eq!(open_door_image(FormatId::Old12, c, 无键()), None);
    // door_offset = 0 也照查（`fr + 0` = fr，图号不变但上游会再查一次）
    c.fr_img = 7;
    c.door_offset = 0;
    assert_eq!(open_door_image(FormatId::Old12, c, 无键()), Some(7));
}

/// 门格判据：CrystalCs 的 `Door` 上游已掩过 `0x7F`，其余八档是原字节。
#[test]
fn 门格判据按格式掩码() {
    let mut raw = 空raw(FormatId::Old12, 1, 1);
    格(&mut raw, 0, 0).door = 0x80;
    assert!(has_door(FormatId::Old12, 格(&mut raw, 0, 0)), "其余八档 0x80 算门");
    let mut raw = 空raw(FormatId::CrystalCs, 1, 1);
    格(&mut raw, 0, 0).door = 0x80;
    assert!(!has_door(FormatId::CrystalCs, 格(&mut raw, 0, 0)), "CrystalCs 掩后为 0");
    格(&mut raw, 0, 0).door_offset = 2;
    assert!(has_door(FormatId::CrystalCs, 格(&mut raw, 0, 0)), "门偏移非 0 也算门");
}

// ───────────────────────────── 可走性 ─────────────────────────────

/// 非 Mir3：还原后的 `bk/fr` `bit15` 都为 0（折叠位那五档与上游逐格等价）。
///
/// 顺带钉住**有意差异**：Wemade2010 的 bk `bit15` 上游不折进 `0x20000000`
/// （`LegacyReader.cs:190-211`），本仓按 R33 统一按位判 ⇒ 这里是 `false`（更严格）。
#[test]
fn 非mir3格式可走按bit15() {
    let mut raw = 空raw(FormatId::Old12, 1, 1);
    let c = 格(&mut raw, 0, 0);
    assert!(walkable(FormatId::Old12, c, 无键()));
    c.bk_img = 0x8000;
    assert!(!walkable(FormatId::Old12, c, 无键()), "地面 bit15 = 阻挡");
    c.bk_img = 0;
    c.fr_img = 0x8000;
    assert!(!walkable(FormatId::Old12, c, 无键()), "前景 bit15 = 阻挡");
    c.fr_img = 0x0007;
    assert!(walkable(FormatId::Old12, c, 无键()));

    let mut raw = 空raw(FormatId::Wemade2010, 1, 1);
    let c = 格(&mut raw, 0, 0);
    c.bk_img = 0x8000 ^ 0xAA38_AA38; // 还原后 bit15 置位
    assert!(
        !walkable(FormatId::Wemade2010, c, 无键()),
        "有意差异：上游 Type1 漏搬 bk bit15，本仓照样按位判（更严格，与画布一致）"
    );

    // AntiHack：判据必须用**还原后**的值（C# `:348` → `:360` 两步）。
    // 取 xor = 0x8000：盘值 `0x0010` 还原后是 `0x8010`（bit15 置位 ⇒ 阻挡），
    // 而盘值自己的 bit15 是 0 —— 漏掉 XOR 就会把它判成「可走」。
    let mut raw = 空raw(FormatId::AntiHack, 1, 1);
    let c = 格(&mut raw, 0, 0);
    c.bk_img = 0x0010;
    let key: i16 = -32768; // 0x8000
    assert!(!walkable(FormatId::AntiHack, c, 键(key)), "还原后背景 bit15 = 阻挡");
    assert!(
        walkable(FormatId::AntiHack, c, 无键()),
        "若漏掉 XOR，就会把这一格误判成可走（盘值 bit15 是 0）"
    );
}

/// Mir3：`flag & 1 == 1 && flag & 2 == 2` 才可走（四态）。
#[test]
fn mir3_可走按flag四态() {
    for format in [FormatId::Mir3Wemade, FormatId::Mir3Shanda] {
        let mut raw = 空raw(format, 1, 1);
        let c = 格(&mut raw, 0, 0);
        c.flag = 0;
        assert!(!walkable(format, c, 无键()), "{format:?} flag=0 ⇒ 两层都不可走");
        c.flag = 1;
        assert!(!walkable(format, c, 无键()), "{format:?} flag=1 ⇒ 只有背景可走");
        c.flag = 2;
        assert!(!walkable(format, c, 无键()), "{format:?} flag=2 ⇒ 只有前景可走");
        c.flag = 3;
        assert!(walkable(format, c, 无键()), "{format:?} flag=3 ⇒ 可走");
        // Mir3 的 fr_img 里 bit15 不是阻挡位 ⇒ 不能拿它判
        c.fr_img = 0x8000;
        assert!(walkable(format, c, 无键()), "{format:?} 可走性只看 flag，不看 fr_img 的 bit15");
    }
}

// ─────────────── R34：CrystalCs 的可走轴（原生 bit29/bit30） ───────────────

/// **CrystalCs 的阻挡位是 `bit29/bit30`，不是 `bit15`**（R34 复核项 1）。
///
/// 依据（上游三处互相印证）：
/// - 写盘：`CellCodec.Write` 把 `BackImage` **整字**写进文件（`_upstream/CrystalStudio/Document.cs:116`）⇒ 29/30 位持久化；
/// - 涂地板：`c.BackImage = (c.BackImage & 0x60000000) | image`（`MapViewport.cs:505/546`）
///   ⇒ 碰撞位与图号位**各占一段**，涂图号不动碰撞位；
/// - 判可走：`(BackImage & 0x60000000) == 0`（`MapCheck.cs:100`）。
///
/// ⇒ `bit15` 只是图号高位（`0x8007` 是图号 32775 一类），拿它判阻挡会把**高图号格误挡**。
#[test]
fn 晶体格式可走按原生bit29与bit30() {
    let mut raw = 空raw(FormatId::CrystalCs, 1, 1);
    let c = 格(&mut raw, 0, 0);
    c.fr_img = 0;
    c.bk_img = 0x0000_0007;
    assert!(walkable(FormatId::CrystalCs, c, 晶(3)), "无碰撞位 ⇒ 可走");
    c.bk_img = 0x2000_0007; // 低位阻挡（LowCollision）
    assert!(
        !walkable(FormatId::CrystalCs, c, 晶(3)),
        "原生 bit29 = 低位阻挡 ⇒ 不可走"
    );
    c.bk_img = 0x4000_0007; // 高位阻挡（HighCollision）
    assert!(
        !walkable(FormatId::CrystalCs, c, 晶(3)),
        "原生 bit30 = 高位阻挡 ⇒ 不可走"
    );
    // ★ 关键反例：bit15 置位只是「图号 ≥ 32768」，**不许**当成阻挡
    c.bk_img = 0x0000_8007;
    assert!(
        walkable(FormatId::CrystalCs, c, 晶(3)),
        "bit15 是高图号，不是碰撞位 ⇒ 必须仍可走（旧实现按 bit15 会误挡）"
    );
    // 图号本身照旧：`& 0x1FFFFFFF` 保留 bit15、剥掉 29/30
    c.bk_img = 0x2000_8007;
    assert_eq!(
        ground_image(FormatId::CrystalCs, c, 晶(3)),
        0x8007,
        "图号 = BackImage & 0x1FFFFFFF（bit29 被剥、bit15 保留）"
    );
    // 装载期还会把**前景** bit15 折成 0x40000000（`:598` 仅 ver > 1）
    c.bk_img = 0x0000_0007;
    c.fr_img = 0x8005;
    assert!(
        !walkable(FormatId::CrystalCs, c, 晶(3)),
        "ver>1 时前景 bit15 会被折成 bit30 ⇒ 不可走（C# `:598`）"
    );
    assert!(
        walkable(FormatId::CrystalCs, c, 晶(1)),
        "ver==1 不折前景 bit15（C# `:625` 的 `ver > 1` 守卫）⇒ 仍可走"
    );
}

/// **没有图库时一条图号都不报**（R34 复核项 2；上游 `MapCheck.cs:26` 的 `libraries == null`）。
///
/// 三件事一起钉住：
/// ① 没有图库 ⇒ 连「必然缺库」的 Mir3 哨兵格也不报（旧实现会多报）；
/// ② 门两警告与不连通区**照常**（上游那两段不看库表）；
/// ③ 有图库时同一张图确实会报出来（否则 ① 可能只是"整个检查坏了"）。
#[test]
fn 无库时不报任何图号问题() {
    // Mir3Wemade：`area = 255` 是**可证的**缺库哨兵（lib = -1），但有库时才报
    let mut raw = 空raw(FormatId::Mir3Wemade, 2, 1);
    格(&mut raw, 0, 0).area = 255; // lib = -1 ⇒ 必然缺库
    格(&mut raw, 0, 0).fr_img = 7;
    // 第二格给一个**非哨兵**槽位（area=3 ⇒ lib=203；fr_img=7 ⇒ 图号 8）：
    // 否则 `area=0 && fr_img=0` 会命中 C# 那条活哨兵修正（lib 200 且图号 1 ⇒ −1），
    // 于是两格都"必然缺库"，这条例子的对照面就没了。
    格(&mut raw, 1, 0).area = 3;
    格(&mut raw, 1, 0).fr_img = 7;
    let 无库 = check_map(&raw, &NoLibProbe);
    assert_eq!(无库.total, 0, "没有图库 ⇒ 一条图号问题都不许报：{:?}", 无库.issues);
    assert!(无库.无图库, "报告要如实标出「图号引用整项没查」");
    assert_eq!(无库.未检查.len(), 3, "未检查的层仍要如实标出");
    let 有库 = check_map(&raw, &假探针::default()); // 默认 has_libraries() == true
    assert_eq!(有库.total, 1, "有图库时哨兵格要报：{:?}", 有库.issues);
    assert!(!有库.无图库, "有库表时这个标志必须为 false");

    // Old12：门警告与不连通区**不看库表** ⇒ 没有图库时照报
    let mut raw = 空raw(FormatId::Old12, 8, 8);
    for c in raw.cells.iter_mut() {
        c.bk_img = 0x8005; // 全不可走 + 图号 5（有库时会报越界）
        c.fr_img = 7; // 让门格有前景，只留「没有可走邻格」那条
    }
    raw.cells[3 * 8 + 3].door = 1;
    let 无库 = check_map(&raw, &NoLibProbe);
    assert!(
        无库
            .issues
            .iter()
            .all(|i| !i.message.contains(MSG_LIB_MISSING)
                && !i.message.contains(MSG_IMAGE_RANGE)
                && !i.message.contains(MSG_IMAGE_EMPTY)),
        "没有图库时不许出现任何图号类文案：{:?}",
        无库.issues
    );
    assert_eq!(
        无库.issues.first().map(|i| i.message.as_str()),
        Some(MSG_DOOR_NO_WALKABLE),
        "门警告照常：{:?}",
        无库.issues
    );
    let 有库 = check_map(&raw, &假探针::default());
    assert!(
        有库.total > 无库.total,
        "有图库时该多出图号问题：无库 {} 条 / 有库 {} 条",
        无库.total,
        有库.total
    );
}

/// 文案里的 `Library` 是 **C# 扁平全局号**（上游 `MapCheck.cs:38` 同形），不是族内页号。
///
/// 六档可查格式上「全局号 − 基址 = 族内页号」，两条口径都要能对上：
/// 消息给出 C# 号（可与 C# 报告对号），`MapLibRef.index` 给出族内页号（查文件用）。
#[test]
fn 图号文案用csharp全局库号() {
    // Shanda14：btTiles = 7 ⇒ C# 107；物件 area = 3 ⇒ C# 123
    let mut raw = 空raw(FormatId::Shanda14, 1, 1);
    格(&mut raw, 0, 0).tiles = 7;
    格(&mut raw, 0, 0).bk_img = 5;
    格(&mut raw, 0, 0).area = 3;
    格(&mut raw, 0, 0).fr_img = 9;
    let r = check_map(&raw, &假探针::default());
    let 文案: Vec<&str> = r.issues.iter().map(|i| i.message.as_str()).collect();
    assert!(文案.iter().any(|m| m.starts_with("Ground:") && m.contains("Library 107")), "{文案:?}");
    assert!(文案.iter().any(|m| m.starts_with("Objects:") && m.contains("Library 123")), "{文案:?}");
    // 族内页号仍是字段原值（两者只差基址）
    assert_eq!(
        layer_source(FormatId::Shanda14, LibFamily::Ground, 格(&mut raw, 0, 0)),
        LibSource::Page(MapLibRef::ground(7))
    );
    // Old12：地面全局号 0（= 常量）、物件 area = 3 ⇒ 全局 5
    let mut raw = 空raw(FormatId::Old12, 1, 1);
    格(&mut raw, 0, 0).bk_img = 5;
    格(&mut raw, 0, 0).area = 3;
    格(&mut raw, 0, 0).fr_img = 9;
    let r = check_map(&raw, &假探针::default());
    let 文案: Vec<&str> = r.issues.iter().map(|i| i.message.as_str()).collect();
    assert!(文案.iter().any(|m| m.contains("Library 0,")), "{文案:?}");
    assert!(文案.iter().any(|m| m.contains("Library 5,")), "{文案:?}");
}

/// **全局号 = 基址 + 族内页号**（六档可查格式 × 三族，逐格核）——防两条口径漂移。
#[test]
fn 全局号等于基址加族内页() {
    let 改 = |format: FormatId, area: u8, tiles: u16, sm: u16| {
        let mut raw = 空raw(format, 1, 1);
        let c = 格(&mut raw, 0, 0);
        c.area = area;
        c.tiles = tiles;
        c.sm_tiles = sm;
        (c.area, c.tiles, c.sm_tiles)
    };
    let 基址 = |format: FormatId, family: LibFamily| -> i16 {
        use FormatId as F;
        match (format, family) {
            (F::Old12 | F::AntiHack | F::Wemade2010 | F::Heroes34, LibFamily::Ground) => 0,
            (F::Old12 | F::AntiHack | F::Wemade2010 | F::Heroes34, LibFamily::Middle) => 1,
            (F::Old12 | F::AntiHack | F::Wemade2010 | F::Heroes34, LibFamily::Objects) => 2,
            (F::Shanda14 | F::Shanda2012, LibFamily::Ground) => 100,
            (F::Shanda14 | F::Shanda2012, LibFamily::Middle) => 110,
            (F::Shanda14 | F::Shanda2012, LibFamily::Objects) => 120,
            _ => panic!("只有六档可查格式"),
        }
    };
    for format in [
        FormatId::Old12,
        FormatId::AntiHack,
        FormatId::Wemade2010,
        FormatId::Heroes34,
        FormatId::Shanda14,
        FormatId::Shanda2012,
    ] {
        let mut raw = 空raw(format, 1, 1);
        let (a, t, s) = 改(format, 3, 7, 8);
        let c = 格(&mut raw, 0, 0);
        c.area = a;
        c.tiles = t;
        c.sm_tiles = s;
        for family in LibFamily::ALL {
            let page = match layer_source(format, family, c) {
                LibSource::Page(r) => r.index,
                other => panic!("{format:?}/{family:?} 该可查：{other:?}"),
            };
            assert_eq!(
                u16::try_from(全局库号(format, family, c) - 基址(format, family)).unwrap_or(u16::MAX),
                page,
                "{format:?}/{family:?}：全局号 − 基址 必须等于族内页号"
            );
        }
    }
}

// ───────────────────────────── 门两警告 ─────────────────────────────

/// 门格两条：没有前景图 / 3×3 没有可走格；补齐后都不报。
#[test]
fn 门格两警告() {
    let mut raw = 空raw(FormatId::Old12, 8, 8);
    for c in raw.cells.iter_mut() {
        c.bk_img = 0x8000; // 全图不可走
    }
    let i = 3 * 8 + 3;
    raw.cells[i].door = 1;
    let probe = 假探针::default();
    let r = check_map(&raw, &probe);
    let 文案: Vec<&str> = r.issues.iter().map(|i| i.message.as_str()).collect();
    assert!(文案.contains(&MSG_DOOR_NO_FOREGROUND), "{文案:?}");
    assert!(文案.contains(&MSG_DOOR_NO_WALKABLE), "{文案:?}");
    assert_eq!(r.total, 2, "整图不可走 ⇒ BFS 无块可报：{文案:?}");
    // 补前景 + 让一个邻居可走 ⇒ 两条都不报
    raw.cells[i].fr_img = 7;
    let j = 2 * 8 + 2;
    raw.cells[j].bk_img = 0;
    let r = check_map(&raw, &假探针::default());
    assert!(
        !r.issues.iter().any(|i| i.message == MSG_DOOR_NO_FOREGROUND
            || i.message == MSG_DOOR_NO_WALKABLE),
        "{:?}",
        r.issues
    );
}

/// Mir3 哨兵格：图号非 0 但 C# 没有 `Foreground` ⇒ 必须报「门没有前景图」
/// （否则会**漏报**；判据是 `frindex > 0` 而不是只看图号）。
#[test]
fn mir3_哨兵格没有前景() {
    let mut raw = 空raw(FormatId::Mir3Wemade, 4, 4);
    for c in raw.cells.iter_mut() {
        c.flag = 3; // 全图可走（避免「没有邻居」那条）
    }
    let i = 4 + 1; // 格 (1, 1)
    raw.cells[i].area = 255; // frindex = -1 ⇒ 没有前景
    raw.cells[i].fr_img = 7;
    raw.cells[i].door = 1; // 门格判据在 Mir3 上靠 door（真实文件里恒 0，这里人为置位）
    let r = check_map(&raw, &假探针::default());
    assert!(
        r.issues.iter().any(|i| i.message == MSG_DOOR_NO_FOREGROUND),
        "{:?}",
        r.issues
    );
    assert!(
        !r.issues.iter().any(|i| i.message == MSG_DOOR_NO_WALKABLE),
        "邻居都可走，不该报「没有可走邻格」：{:?}",
        r.issues
    );
    // 反过来：非哨兵（area=3 ⇒ frindex=203 > 0）就不该报
    raw.cells[i].area = 3;
    let r = check_map(&raw, &假探针::default());
    assert!(!r.issues.iter().any(|i| i.message == MSG_DOOR_NO_FOREGROUND));
}

// ─────────────────────────── 连通块 / 上限 / 缓存 ───────────────────────────

/// 8 邻 BFS：最大块不报，其余逐块报格数（上游 `:61-95`）。
#[test]
fn 最连通块不报其余报() {
    let mut raw = 空raw(FormatId::Old12, 8, 8);
    for c in raw.cells.iter_mut() {
        c.bk_img = 0x8000; // 全不可走
    }
    // 3×3 的块（9 格）在左上
    for x in 0..3u16 {
        for y in 0..3u16 {
            格(&mut raw, x, y).bk_img = 0;
        }
    }
    // 2 格的块在 (6,0)/(6,1)
    for y in 0..2u16 {
        格(&mut raw, 6, y).bk_img = 0;
    }
    let r = check_map(&raw, &假探针::default());
    assert_eq!(r.total, 1, "只该报断开的那块：{:?}", r.issues);
    let line = r.issues[0].line();
    assert!(line.starts_with("(6, 0)"), "{line}");
    assert!(line.ends_with("2 cells"), "{line}");
}

/// 两块**同样大**时：上游 `size > largest.Size` 是严格大于 ⇒ 报**后**出现的那块。
#[test]
fn 并列时最大块取先出现() {
    let mut raw = 空raw(FormatId::Old12, 8, 8);
    for c in raw.cells.iter_mut() {
        c.bk_img = 0x8000;
    }
    // 两块各 2 格：C# 键序（行优先）里先遇到 (1,0)，后遇到 (1,5)
    格(&mut raw, 1, 0).bk_img = 0;
    格(&mut raw, 2, 0).bk_img = 0;
    格(&mut raw, 1, 5).bk_img = 0;
    格(&mut raw, 2, 5).bk_img = 0;
    let r = check_map(&raw, &假探针::default());
    assert_eq!(r.total, 1, "{:?}", r.issues);
    let line = r.issues[0].line();
    assert!(line.starts_with("(1, 5)"), "并列时该报后出现的那块：{line}");
}

/// 清单截到 1000 条，`total` 照实（上游 `:18-22`）。
#[test]
fn 清单截断总数照实() {
    let mut raw = 空raw(FormatId::Old12, 40, 40); // 1600 格
    for c in raw.cells.iter_mut() {
        c.bk_img = 5;
    }
    let probe = 假探针 {
        帧数: HashMap::from([((LibFamily::Ground, 0), 3)]), // 图号 5 > 3 ⇒ 越界
        ..Default::default()
    };
    let r = check_map(&raw, &probe);
    assert_eq!(r.issues.len(), MAP_CHECK_ISSUE_CAP);
    assert_eq!(r.total, 1600);
    assert_eq!(r.truncated(), 600);
}

/// 探针结论按 `(库, 图号)` 缓存：同一对只问一次（上游 `checkedImages`）。
#[test]
fn 探针按库图号缓存() {
    let mut raw = 空raw(FormatId::Old12, 4, 4); // 16 格，全是 (地面 0, 图号 5)
    for c in raw.cells.iter_mut() {
        c.bk_img = 5;
    }
    let probe = 假探针 {
        帧数: HashMap::from([((LibFamily::Ground, 0), 9)]),
        ..Default::default()
    };
    let r = check_map(&raw, &probe);
    assert_eq!(r.total, 0);
    assert_eq!(probe.查过.borrow().len(), 1, "同一 (库,图号) 只该问一次");
}

/// 四类结论（缺库 / 越界 / 空帧 / 读不出）各一条，且**都带上库号与图号**。
#[test]
fn 四类结论各一条() {
    let mut raw = 空raw(FormatId::Old12, 1, 1);
    格(&mut raw, 0, 0).bk_img = 5;
    for (态, 期望) in [
        (ImageState::MissingLibrary, MSG_LIB_MISSING),
        (ImageState::OutOfRange, MSG_IMAGE_RANGE),
        (ImageState::Empty, MSG_IMAGE_EMPTY),
        (ImageState::Unreadable, MSG_IMAGE_UNREADABLE),
    ] {
        let probe = 假探针 {
            帧数: HashMap::from([((LibFamily::Ground, 0), 9)]),
            坏: HashMap::from([((LibFamily::Ground, 0, 5), 态)]),
            ..Default::default()
        };
        let r = check_map(&raw, &probe);
        assert_eq!(r.total, 1, "{态:?}：{:?}", r.issues);
        let m = &r.issues[0].message;
        assert!(m.contains(期望), "{态:?}：{m}");
        assert!(m.contains("Library 0"), "{m}");
        assert!(m.contains("Image 5"), "{m}");
        assert!(m.starts_with("Ground: "), "{m}");
    }
}

/// 形状非法（`cells.len() != w*h`）⇒ 三族都记「未检查」，不逐格瞎读。
#[test]
fn 形状非法时三族未检查() {
    let mut raw = 空raw(FormatId::Old12, 4, 4);
    raw.cells.truncate(5);
    let r = check_map(&raw, &假探针::default());
    assert_eq!(r.total, 0);
    assert_eq!(r.未检查.len(), 3);
    for (_, reason) in &r.未检查 {
        assert_eq!(*reason, UNCHECKED_SIZE_MISMATCH);
    }
}

/// 头部不够长（拿不到 XOR 键）⇒ 三族都记「未检查」，不猜 0。
#[test]
fn 缺xor键时三族未检查() {
    let mut raw = 空raw(FormatId::Wemade2010, 2, 2);
    raw.header_raw.truncate(8); // 不足 25
    let r = check_map(&raw, &假探针::default());
    assert_eq!(r.total, 0);
    assert_eq!(r.未检查.len(), 3);
    for (_, reason) in &r.未检查 {
        assert_eq!(*reason, UNCHECKED_NO_XOR_KEY);
    }
}

// ───────────────────────────── 只读／一致性 ─────────────────────────────

/// **只读四项**：字节、撤销栈、脏标记、提示都不变。
#[test]
fn 检查只读不改文档状态() {
    use mir2_map::CellFormat;

    let ed = crate::session::EditSession::new(crate::testutil::造文档(16, 16, CellFormat::New14));
    let 前 = ed.document().to_bytes();
    let probe = 假探针 {
        帧数: HashMap::from([
            ((LibFamily::Ground, 0), 9),
            ((LibFamily::Middle, 0), 9),
            ((LibFamily::Objects, 0), 9),
        ]),
        ..Default::default()
    };
    let r = check_map(ed.document().raw(), &probe);
    assert_eq!(ed.document().to_bytes(), 前, "不许改字节");
    assert_eq!(ed.undo_len(), 0, "不许进撤销栈");
    assert!(!ed.dirty(), "不许标脏");
    assert_eq!(ed.note(), None, "不许写提示");
    let _ = r;
}

/// **raw 与视图的 `area` 一致**（Old12 / Shanda14 走真字节往返；其余走 `view_from_raw`）。
///
/// 为什么这条要测：检查器的物件库号取的是 `RawCell.area`，而 GUI 的 `MapDoc::raw()`
/// 是「视图同步回 raw」的快照 ⇒ 两边 `area` 必须同口径，否则检查会查错库。
#[test]
fn 原始raw与视图area一致() {
    // ① 走真字节：造 raw → writer → read_any → view_from_raw，逐格比 area
    for format in [FormatId::Old12, FormatId::Shanda14] {
        let mut raw = 空raw(format, 3, 2);
        for (i, c) in raw.cells.iter_mut().enumerate() {
            c.area = (i as u8) + 1;
        }
        // 头部宽高（这两种格式都在头部 0..4 放 `w, h`，与 `testutil::造图` 同口径）
        let (w, h) = (raw.width.to_le_bytes(), raw.height.to_le_bytes());
        raw.header_raw[0..2].copy_from_slice(&w);
        raw.header_raw[2..4].copy_from_slice(&h);
        let bytes = format_by_id(format).write(&raw).expect("writer 应能写");
        // 用该格式自己的 `read`（不是 `read_any`）：合成头没有判别魔数，`detect` 认不出来
        let 回读 = format_by_id(format).read(&bytes).expect("该格式的 read 应能读回");
        let view = crate::convert::view_from_raw(&回读);
        for (i, cell) in view.cells.iter().enumerate() {
            assert_eq!(
                cell.area,
                回读.cells[i].area,
                "{format:?} 第 {i} 格：视图与 raw 的 area 不同口径"
            );
        }
    }
    // ② 其余五档（Wemade2010/Heroes34/Mir3 两支/CrystalCs）只验映射表本身是恒等
    for format in [
        FormatId::Wemade2010,
        FormatId::Heroes34,
        FormatId::Mir3Wemade,
        FormatId::Mir3Shanda,
        FormatId::CrystalCs,
    ] {
        let mut raw = 空raw(format, 2, 2);
        for (i, c) in raw.cells.iter_mut().enumerate() {
            c.area = (i as u8) + 1;
        }
        let view = crate::convert::view_from_raw(&raw);
        for (i, cell) in view.cells.iter().enumerate() {
            assert_eq!(cell.area, raw.cells[i].area, "{format:?} 第 {i} 格");
        }
    }
}

/// **没有图库 ⇒ 图号引用一条不报**（= 上游 `libraries == null` 的 `return`），
/// 但门两警告与不连通区照报。
#[test]
fn 无库探针不报图号问题() {
    let mut raw = 空raw(FormatId::Old12, 8, 8);
    // 每格都引用一个不存在的图号；另外开一个门格、并把图挖出两块
    for c in raw.cells.iter_mut() {
        c.bk_img = 5;
        c.mid_img = 6;
        c.fr_img = 7;
    }
    let i = 3 * 8 + 3;
    raw.cells[i].door = 1; // 门格：前景 7 非 0 ⇒ 只报「没有可走邻格」那一类
    for c in raw.cells.iter_mut() {
        c.bk_img = 0x8000; // 全不可走 ⇒ 门格没有可走邻格
    }
    let r = check_map(&raw, &NoLibProbe);
    assert_eq!(r.total, 1, "只该有门那一条：{:?}", r.issues);
    assert_eq!(r.issues[0].message, MSG_DOOR_NO_WALKABLE);
    // 对照：同一张图换个「全部缺库」的探针 ⇒ 图号问题会刷满
    let probe = 假探针::default(); // 任何库都没登记 ⇒ MissingLibrary
    let r2 = check_map(&raw, &probe);
    assert!(r2.total > 1, "有探针时图号问题必须报出来");
}

/// 干净图（无图号 / 无门 / 全可走）不该有任何问题，且六档可查 ⇒ 未检查列表为空。
#[test]
fn 干净图零问题() {
    for format in [
        FormatId::Old12,
        FormatId::AntiHack,
        FormatId::Wemade2010,
        FormatId::Heroes34,
        FormatId::Shanda14,
        FormatId::Shanda2012,
    ] {
        let mut raw = 空raw(format, 8, 8);
        if format == FormatId::Wemade2010 {
            // 该格式的背景图号是磁盘值整字 XOR 过的 ⇒ 「空图号」的磁盘值是 0 ^ 0xAA38AA38
            for c in raw.cells.iter_mut() {
                c.bk_img = 0xAA38_AA38;
            }
        }
        let probe = 假探针 {
            帧数: HashMap::from([
                ((LibFamily::Ground, 0), 9),
                ((LibFamily::Middle, 0), 9),
                ((LibFamily::Objects, 0), 9),
            ]),
            ..Default::default()
        };
        let r = check_map(&raw, &probe);
        assert_eq!(r.total, 0, "{format:?}：{:?}", r.issues);
        assert!(r.未检查.is_empty(), "{format:?}：{:?}", r.未检查);
    }
}
