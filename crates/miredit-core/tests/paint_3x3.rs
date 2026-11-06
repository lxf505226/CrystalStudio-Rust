//! 验收门 6：**单格落笔字节核对** —— `0.map` 的 `(0,0)` 用 3×3 笔刷落一笔。
//!
//! 跑法：`cargo test -p miredit-core --test paint_3x3 -- --nocapture`
//!
//! 判据（任务书 §六.6）：
//! 1. 改 **4 格**（3×3 笔刷在左上角被裁成 2×2）；
//! 2. **全文件差异 5 字节**（`(0,0)` 的 `bk_img` 两个字节都变，另外三格各变 1 字节）；
//! 3. `(0,0)` 的 `bk_img` 从 `0x873C` 变成 `0x800A`；
//! 4. `bit15` 保持 1（改图号不许把墙拆了）。
//!
//! ⚠ 只读真图：本用例绝不写磁盘（末尾再读一次比对）。

use miredit_core::{apply, Command, EditSession, LibRef, COLLISION_BIT};

const 真图: &str = r"E:\GodM2\client\热血传奇23周年客户端\Map\0.map";

fn 读真图() -> Vec<u8> {
    std::fs::read(真图).unwrap_or_else(|e| panic!("真值地图读不到，不许静默跳过：{真图} ⇒ {e}"))
}

#[test]
fn 门6_单格落笔改4格差异5字节() {
    let bytes = 读真图();
    let doc = miredit_core::Document::from_bytes(std::path::Path::new(真图), bytes.clone())
        .expect("真值地图应能解析");
    let h = doc.view().height;
    let size = doc.view().format.cell_size();
    let mut ed = EditSession::new(doc);

    // 真值先钉住：0.map 的 (0,0) = 0x873C（带 bit15），邻居 (1,0)/(0,1)/(1,1) = 0x8000
    let 前00 = *ed.map().cell(0, 0).unwrap();
    assert_eq!(前00.bk_img, 0x873C, "真值：(0,0) 的 bk_img");
    for (x, y) in [(1u16, 0u16), (0, 1), (1, 1)] {
        assert_eq!(
            ed.map().cell(x, y).unwrap().bk_img,
            0x8000,
            "真值：({x},{y}) 只有阻挡位、图号 0"
        );
    }

    apply(
        &mut ed,
        Command::SetLayer {
            layer: "ground".to_string(),
        },
    )
    .expect("层名合法");
    // 帧索引 9 ⇒ 图号 10（0x000A）
    miredit_core::set_brush_from_palette(&mut ed, LibRef::ground(0), 9);
    assert_eq!(ed.brush().unwrap().sprite(), 10, "笔刷图号应是 10");

    let out = apply(&mut ed, Command::Paint { x: 0, y: 0 }).expect("落笔不该报错");
    assert_eq!(out.changed, 4, "3×3 笔刷在 (0,0) 被裁成 2×2 = 4 格");

    // ① 目标格：0x873C → 0x800A，bit15 保持 1
    let 后00 = *ed.map().cell(0, 0).unwrap();
    assert_eq!(后00.bk_img, 0x800A, "(0,0)：0x873C 写图号 10 应得 0x800A");
    assert_eq!(
        后00.bk_img & COLLISION_BIT,
        前00.bk_img & COLLISION_BIT,
        "bit15 必须保持 1"
    );
    assert_eq!(后00.bk_img & COLLISION_BIT, COLLISION_BIT);
    assert!(!后00.is_walkable(), "原本不可走，改完还得不可走");

    // ② 三格邻居：0x8000 → 0x800A
    for (x, y) in [(1u16, 0u16), (0, 1), (1, 1)] {
        assert_eq!(
            ed.map().cell(x, y).unwrap().bk_img,
            0x800A,
            "({x},{y})：0x8000 写图号 10 应得 0x800A"
        );
    }

    // ③ 全文件差异 5 字节 / 4 格 / 头部 0
    let d = ed.document().diff();
    assert_eq!(d.bytes, 5, "全文件差异必须是 5 字节：{d:?}");
    assert_eq!(d.cells, 4, "差异只涉及 4 个格：{d:?}");
    assert_eq!(d.header, 0, "52 字节头不许动");

    // ④ 逐个差异偏移都要落在 4 个格子的 bk_img 两个字节内（不靠"总数对上了"蒙对）
    let now = ed.map().to_bytes();
    let mut 偏移 = Vec::new();
    for (i, (a, b)) in now.iter().zip(bytes.iter()).enumerate() {
        if a != b {
            偏移.push(i);
        }
    }
    assert_eq!(偏移.len(), 5, "逐字节扫出来的差异数也必须是 5");
    let 合法偏移: Vec<usize> = [(0u16, 0u16), (1, 0), (0, 1), (1, 1)]
        .iter()
        .flat_map(|(x, y)| {
            let base = mir2_map::HEADER_LEN + (*x as usize * h as usize + *y as usize) * size;
            [base, base + 1]
        })
        .collect();
    for o in &偏移 {
        assert!(
            合法偏移.contains(o),
            "差异偏移 {o} 不在 4 个目标格的 bk_img 字节内：{偏移:?}"
        );
    }
    // 长度不变（不凭空增删字节）
    assert_eq!(now.len(), bytes.len());

    eprintln!(
        "PAINT_3X3 path={真图} 落笔=(0,0) 尺寸=3 改格={} 差异字节={} 差异格={} 差异偏移={:?} (0,0): 0x{:04X}→0x{:04X} bit15=1→{} walkable={}→{}",
        out.changed,
        d.bytes,
        d.cells,
        偏移,
        前00.bk_img,
        后00.bk_img,
        u16::from(后00.bk_img & COLLISION_BIT != 0),
        u16::from(前00.is_walkable()),
        u16::from(后00.is_walkable()),
    );

    // 只读自证
    assert_eq!(读真图(), bytes, "本用例绝不许写磁盘");
}

/// 同一笔的撤销：逐字节回到原文件（顺带证明"整格快照"能还原 `Old12` 档）。
#[test]
fn 门6补充_单笔撤销逐字节还原() {
    let bytes = 读真图();
    let doc = miredit_core::Document::from_bytes(std::path::Path::new(真图), bytes.clone())
        .expect("真值地图应能解析");
    let mut ed = EditSession::new(doc);
    miredit_core::set_brush_from_palette(&mut ed, LibRef::ground(0), 9);
    assert_eq!(apply(&mut ed, Command::Paint { x: 0, y: 0 }).unwrap().changed, 4);
    assert_eq!(ed.undo(), 4);
    assert_eq!(ed.map().to_bytes(), bytes, "撤销后必须与磁盘原文件逐字节相等");
    assert!(ed.document().diff().same());
    assert!(!ed.dirty());
}

/// 三档真图各找一格带 `bit15` 的写图号 10 —— 逐档核对"只动 `bk_img` 的 2 字节、`bit15` 前后相同"。
///
/// 与 `miredit-gui/src/edit.rs:3897` 的同名用例同口径（GUI 侧用 `mir2_assets::LibRef`，
/// 本 crate 用自己的 [`LibRef`]，判据一致）。三档：
/// `0.map` = `Old12`、`41.map` = `New14`、`n0104.map` = `Ext36`。
#[test]
fn 三档真图写图号都保留bit15() {
    for (tag, path, want_fmt) in [
        ("Old12", 真图, "Old12"),
        ("New14", r"E:\GodM2\GodotClient\maps\41.map", "New14"),
        (
            "Ext36",
            r"E:\GodM2\client\热血传奇23周年客户端\Map\n0104.map",
            "Ext36",
        ),
    ] {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|e| panic!("真值地图读不到，不许静默跳过：{tag} {path} ⇒ {e}"));
        let doc = miredit_core::Document::from_bytes(std::path::Path::new(path), bytes.clone())
            .expect("真值地图应能解析");
        assert_eq!(doc.format_name(), want_fmt, "{tag} 档位与口径不符");
        let h = doc.view().height as usize;

        // 找第一格 bk_img 带 bit15 且低 15 位不是 10 的格（保证落笔一定会变）
        let (x, y, idx) = {
            let mut found = None;
            for i in 0..doc.view().cells.len() {
                let c = &doc.view().cells[i];
                if c.bk_img & COLLISION_BIT != 0 && c.bk_img & 0x7FFF != 10 {
                    found = Some(((i / h) as u16, (i % h) as u16, i));
                    break;
                }
            }
            found.unwrap_or_else(|| panic!("{tag} 里找不到带 bit15 的格"))
        };
        let mut ed = EditSession::new(doc);
        // 库号对齐目标格 ⇒ 库号字节写回同值，差异只可能落在图号 2 字节
        let lib_byte = ed.map().cells[idx].tiles;
        let _ = ed.set_brush_from_palette(LibRef::ground(u16::from(lib_byte)), 9);
        let 前 = ed.map().cells[idx];
        assert_eq!(ed.apply_once(x, y), 1, "{tag} 应真的改一格");
        let 后 = ed.map().cells[idx];

        let d = ed.document().diff();
        assert_eq!(d.header, 0, "{tag} 头部不许动");
        assert_eq!(d.cells, 1, "{tag} 只许涉及目标格");
        assert_eq!(
            d.bytes,
            if (前.bk_img ^ 后.bk_img) & 0xFF != 0
                && (前.bk_img ^ 后.bk_img) & 0xFF00 != 0
            {
                2
            } else {
                1
            },
            "{tag} 差异只许落在 bk_img 的字节内：{d:?}"
        );
        assert!(!ed.document().diff().same(), "{tag} 图号确实要变");
        assert_eq!(
            后.bk_img & COLLISION_BIT,
            前.bk_img & COLLISION_BIT,
            "{tag} bit15 前后必须相同"
        );
        assert_eq!(后.bk_img & 0x7FFF, 10, "{tag} 低 15 位应换成 10");
        assert_eq!(
            后.is_walkable(),
            前.is_walkable(),
            "{tag} 改图号不许改变可走性"
        );
        // 其余格 0 差异
        let 原 = mir2_map::Map::parse(&bytes).unwrap();
        assert_eq!(ed.map().cells[..idx], 原.cells[..idx], "{tag} 目标格之前 0 差异");
        assert_eq!(
            ed.map().cells[idx + 1..],
            原.cells[idx + 1..],
            "{tag} 目标格之后 0 差异"
        );
        eprintln!(
            "BIT15_REAL_MAP tag={tag} at=({x},{y}) before=0x{:04X} after=0x{:04X} bit15={} diff_bytes={} diff_cells={} walkable={}→{}",
            前.bk_img,
            后.bk_img,
            u16::from(后.bk_img & COLLISION_BIT != 0),
            d.bytes,
            d.cells,
            u16::from(前.is_walkable()),
            u16::from(后.is_walkable()),
        );
    }
}
