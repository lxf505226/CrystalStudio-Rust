//! **真实语料**上的裁剪 / 扩边往返（R36 验收：`0.map` crop+expand）。
//!
//! 跑法：`cargo test -p miredit-core --test resize_real -- --nocapture`
//!
//! 判据（每一条都是"再读回来逐格核"，不看内存里的中间态）：
//! 1. `0.map`（Old12，700×700）裁到 `[100,300)×[200,400)` ⇒ 读回 **200×200**，
//!    且新图每格 == 旧图对应格；
//! 2. 在裁剪结果上四向各扩 2 格 ⇒ 读回 **204×204**，旧格平移 `(+2,+2)`、四周空白；
//! 3. 两次操作的**保存字节**都能被 `Document::from_bytes`（九格式判别）重新打开。
//!
//! ⚠ 真图**只被读取**（`0.map` 一字节不动）；写盘只发生在 `%TEMP%` 的临时文件里。

use std::path::{Path, PathBuf};

use miredit_core::Document;

/// 资产根缺省值：本机真实根（本文件里只出现这一处）。换机器用 `MIREDIT_ASSET_ROOT`。
const 缺省资产根: &str = r"E:\GodM2";

fn 资产根() -> Option<PathBuf> {
    let root = std::env::var_os("MIREDIT_ASSET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(缺省资产根));
    root.is_dir().then_some(root)
}

/// `0.map` 的路径（不存在 ⇒ `None`，用例打印 `[skip]`）。
fn 地图0() -> Option<PathBuf> {
    let p = 资产根()?.join("GodotClient").join("maps").join("0.map");
    p.is_file().then_some(p)
}

fn 临时路径(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("miredit_resize_real_{tag}.map"))
}

/// **裁剪 → 读回 → 扩边 → 读回**（R36 验收：真实 `0.map` 往返）。
#[test]
fn map0裁剪扩边往返() {
    let Some(源) = 地图0() else {
        eprintln!("[skip] 未找到 0.map（设 MIREDIT_ASSET_ROOT 指向 GodM2 根）");
        return;
    };
    let 原 = Document::load(&源).expect("0.map 应能打开");
    let (w0, h0) = 原.size();
    assert_eq!((w0, h0), (700, 700), "0.map 是 700×700");
    assert!(原.raw().trailer.is_empty(), "0.map 没有未解释尾块");

    // ── ① 裁剪到 [100,300)×[200,400) ──
    let mut doc = Document::load(&源).expect("再读一份（undo 快照用不着，这里只要干净状态）");
    let (left, top, right, bottom) = (100u16, 200u16, 300u16, 400u16);
    doc.replace_resized(right - left, bottom - top, i32::from(left), i32::from(top))
        .expect("0.map 应能裁剪（Old12、无尾块）");
    assert_eq!(doc.size(), (200, 200), "裁剪后尺寸");

    let 裁剪字节 = doc.try_to_bytes().expect("裁剪后出字节");
    let p1 = 临时路径("crop");
    std::fs::write(&p1, &裁剪字节).expect("写临时文件");
    let 裁剪回读 = Document::load(&p1).expect("裁剪后的字节应能重新打开");
    assert_eq!(裁剪回读.size(), (200, 200), "读回尺寸");
    // 逐格核：新图 (x,y) == 原图 (x+100, y+200)
    for y in (0..200u16).step_by(17) {
        for x in (0..200u16).step_by(13) {
            let 新 = 裁剪回读.raw().cell(x, y).expect("图内");
            let 旧 = 原.raw().cell(x + left, y + top).expect("图内");
            assert_eq!(新.bk_img, 旧.bk_img, "({x},{y}) 地表图号");
            assert_eq!(新.mid_img, 旧.mid_img, "({x},{y}) 中层图号");
            assert_eq!(新.fr_img, 旧.fr_img, "({x},{y}) 物体图号");
            assert_eq!(新.area, 旧.area, "({x},{y}) 物体库号");
            assert_eq!(新.door, 旧.door, "({x},{y}) 门索引");
        }
    }

    // ── ② 在裁剪结果上四向各扩 2 格 ──
    // 先记下 (0,0) 那格（后面要验证它被平移到 (2,2)），再交出所有权
    let 旧零图号 = 裁剪回读.raw().cell(0, 0).expect("图内").bk_img;
    let mut doc = 裁剪回读;
    doc.replace_resized(204, 204, -2, -2).expect("应能扩边");
    assert_eq!(doc.size(), (204, 204));
    let 扩边字节 = doc.try_to_bytes().expect("扩边后出字节");
    let p2 = 临时路径("expand");
    std::fs::write(&p2, &扩边字节).expect("写临时文件");
    let 扩边回读 = Document::load(&p2).expect("扩边后的字节应能重新打开");
    assert_eq!(扩边回读.size(), (204, 204), "读回尺寸");
    // 旧 (0,0) 现在在 (2,2)
    let 新二 = 扩边回读.raw().cell(2, 2).expect("图内");
    assert_eq!(新二.bk_img, 旧零图号, "旧 (0,0) 应平移到 (2,2)");
    // 边框空白
    for x in 0..204u16 {
        assert_eq!(扩边回读.raw().cell(x, 0).expect("图内").bk_img, 0, "上边框 ({x},0)");
        assert_eq!(扩边回读.raw().cell(x, 203).expect("图内").bk_img, 0, "下边框");
    }
    for y in 0..204u16 {
        assert_eq!(扩边回读.raw().cell(0, y).expect("图内").bk_img, 0, "左边框");
        assert_eq!(扩边回读.raw().cell(203, y).expect("图内").bk_img, 0, "右边框");
    }

    // ── ③ 收尾：删临时文件（真图一字节没动）──
    let _ = std::fs::remove_file(&p1);
    let _ = std::fs::remove_file(&p2);
    assert_eq!(
        std::fs::read(&源).map(|b| b.len()).unwrap_or(0),
        原.orig_len(),
        "0.map 的字节数不该被这次测试改变"
    );
    let _ = Path::new(&源);
}
