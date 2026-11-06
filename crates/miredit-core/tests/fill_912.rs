//! 验收门 3 + 4：**填充 912 格**（`0.map` 点 `(358,0)`）与**撤销逐字节相等**。
//!
//! 跑法：`cargo test -p miredit-core --test fill_912 -- --nocapture`
//!
//! 真值地图：`E:\GodM2\client\热血传奇23周年客户端\Map\0.map`
//! （700×700 / `Old12` / 5,880,052 字节；`(358,0)` 的 `bk_img = 0x815F`）。
//!
//! ⚠ **只读**：本文件把字节读进内存后用 [`miredit_core::Document::from_bytes`] 构造文档，
//! 全程**不调 `Save`**；末尾再读一次磁盘比对，证明原文件一个字节都没动。
//!
//! 判据（任务书 §六.3 / §六.4）：
//! 1. 填充 912 格；
//! 2. 块内改后取值集合 = `{0x8001}`（不是 `{0x0001}` —— 那是修复前的旧值）；
//! 3. `bit15` 变化格数 = 0；
//! 4. 全图 `walkable` 变化格数 = 0；
//! 5. 撤销一步 ⇒ `to_bytes()` 与磁盘原文件**逐字节相等**。

use std::collections::BTreeSet;

use miredit_core::{
    apply, Command, EditLayer, EditSession, FillOutcome, LibRef, COLLISION_BIT, SPRITE_MAX,
};

/// 真值地图的绝对路径（写死真实位置，不是猜的）。
const 真图: &str = r"E:\GodM2\client\热血传奇23周年客户端\Map\0.map";

/// 只读载入真图；读不到就 panic（**不许静默跳过**，禁假数据）。
fn 读真图() -> Vec<u8> {
    std::fs::read(真图)
        .unwrap_or_else(|e| panic!("真值地图读不到，不许静默跳过：{真图} ⇒ {e}"))
}

fn 会话(bytes: Vec<u8>) -> EditSession {
    let doc = miredit_core::Document::from_bytes(std::path::Path::new(真图), bytes)
        .expect("真值地图应能解析（Old12 / 700×700）");
    assert_eq!(doc.format_name(), "Old12", "0.map 必须是 12 字节/格档");
    assert_eq!(doc.size(), (700, 700));
    EditSession::new(doc)
}

/// 造"图号 1"的地表笔刷（帧索引 0 + 1），并切到地表层 —— 与主线程复现时用的输入一致。
fn 备好地表笔刷(ed: &mut EditSession) {
    apply(
        ed,
        Command::SetLayer {
            layer: "ground".to_string(),
        },
    )
    .expect("层名合法");
    let out = miredit_core::set_brush_from_palette(ed, LibRef::ground(0), 0);
    assert!(
        out.note.as_deref().unwrap_or("").contains("图号 1"),
        "笔刷应是图号 1：{:?}",
        out.note
    );
}

/// **门 3**：`0.map` 副本 `(358,0)` 填充 ⇒ 912 格、取值集合 `{0x8001}`、`bit15` 变化 0、`walkable` 变化 0。
#[test]
fn 门3_填充912格全部保留bit15() {
    let bytes = 读真图();
    let mut ed = 会话(bytes.clone());
    备好地表笔刷(&mut ed);

    // 点击格的真值先钉住（不是猜的）
    let 点击前 = *ed.map().cell(358, 0).expect("(358,0) 在图内");
    assert_eq!(点击前.bk_img, 0x815F, "真值：(358,0) 带 bit15、低 15 位 0x015F");
    assert!(!点击前.is_walkable(), "带 bit15 ⇒ 不可走");

    let 改前 = bytes;
    let out = apply(&mut ed, Command::Fill { x: 358, y: 0 }).expect("填充不该报错");
    assert_eq!(out.changed, 912, "主线程复现：912 格");
    assert_eq!(
        ed.last_fill().map(|f| (f.cells, f.layer)),
        Some((912, EditLayer::Ground)),
        "回执必须记 912 格 / 地表层"
    );

    // 逐格核对：bit15 / walkable / 取值集合
    let 原 = mir2_map::Map::parse(&改前).expect("原图应能解析");
    let mut bit15变 = 0usize;
    let mut 可走变 = 0usize;
    let mut 改了 = 0usize;
    let mut 改前集合 = BTreeSet::new();
    let mut 改后集合 = BTreeSet::new();
    for i in 0..ed.map().cells.len() {
        let b = 原.cells[i];
        let a = ed.map().cells[i];
        if (a.bk_img ^ b.bk_img) & COLLISION_BIT != 0 {
            bit15变 += 1;
        }
        if a.is_walkable() != b.is_walkable() {
            可走变 += 1;
        }
        if a != b {
            改了 += 1;
            改前集合.insert(b.bk_img);
            改后集合.insert(a.bk_img);
        }
    }
    let 改前hex: Vec<String> = 改前集合.iter().map(|v| format!("0x{v:04X}")).collect();
    let 改后hex: Vec<String> = 改后集合.iter().map(|v| format!("0x{v:04X}")).collect();
    assert_eq!(改了, 912, "真改的格数 = 912");
    assert_eq!(bit15变, 0, "bit15 变化格数必须为 0");
    assert_eq!(可走变, 0, "改图号不得改变任何格的可走性");
    assert_eq!(改前hex, vec!["0x815F"], "块内改前取值集合");
    assert_eq!(
        改后hex,
        vec!["0x8001"],
        "块内改后取值集合（不再是 0x0001 —— 那是修复前的旧值）"
    );
    assert_eq!(
        点击前.bk_img & SPRITE_MAX,
        0x015F,
        "低 15 位原本是 0x015F"
    );
    assert_eq!(
        ed.map().cell(358, 0).unwrap().bk_img & SPRITE_MAX,
        1,
        "填完低 15 位是 1"
    );

    eprintln!(
        "BIT15_FILL path={真图} clicked=(358,0) cells=912 before_set=[{}] after_set=[{}] bit15_changed={bit15变} walk_changed={可走变} 旧逻辑会得=0x0001",
        改前hex.join(","),
        改后hex.join(","),
    );

    // 只读自证：磁盘原文件一个字节都没动
    assert_eq!(读真图(), 改前, "本用例绝不许写磁盘");
}

/// **门 4**：填充后撤销 ⇒ `to_bytes()` 与磁盘原文件**逐字节相等**。
#[test]
fn 门4_填充撤销后逐字节相等() {
    let bytes = 读真图();
    let mut ed = 会话(bytes.clone());
    备好地表笔刷(&mut ed);

    let 填充结果 = ed.fill(358, 0);
    assert_eq!(填充结果, FillOutcome::Filled(912));
    assert!(ed.dirty(), "填完必须脏");
    let 填充后 = ed.map().to_bytes();
    assert_ne!(填充后, bytes, "填完必须与原文件不同（否则下面的相等是假的）");

    assert_eq!(ed.undo_len(), 1, "整次填充 = 一步");
    let 撤销格数 = ed.undo();
    assert_eq!(撤销格数, 912, "撤销还原 912 格");

    let 撤销后 = ed.map().to_bytes();
    assert_eq!(撤销后.len(), bytes.len(), "长度必须一致");
    assert_eq!(撤销后, bytes, "撤销后必须与磁盘原文件逐字节相等");
    assert!(ed.document().diff().same(), "diff 必须报 0 差异");
    assert!(!ed.dirty(), "撤销回保存点后不该脏");
    assert_eq!(ed.undo_len(), 0);
    assert_eq!(ed.redo_len(), 1);

    // 重做 ⇒ 回到填充后的字节（证明不是"撤销把整张图重置了"）
    assert_eq!(ed.redo(), 912);
    assert_eq!(ed.map().to_bytes(), 填充后, "重做应回到填充后的字节");
    assert!(ed.dirty());

    eprintln!(
        "FILL_UNDO_BYTES path={真图} 原={} 字节 填充后={} 字节 撤销后={} 字节 逐字节相等={} 重做后相等={}",
        bytes.len(),
        填充后.len(),
        撤销后.len(),
        撤销后 == bytes,
        ed.map().to_bytes() == 填充后,
    );

    assert_eq!(读真图(), bytes, "本用例绝不许写磁盘");
}
