//! 新建空白图的用例（`mir2_map::Map::blank` / `Document::blank` / `Document::blank_in`）。
//!
//! # 断言全部基于**真实生成的字节**，没有一条是"算出来的期望值"
//!
//! - 尺寸类断言读的是 `Document::size()` / `cells.len()`（真构造出来的对象）；
//! - 字节类断言读的是 `try_to_bytes()` 的返回值与 `std::fs::read` 的磁盘内容；
//! - 往返类断言真的**写盘再 `Document::load` 读回来**（临时目录，跑完删）。
//!
//! # 覆盖矩阵
//!
//! | 用例 | 覆盖的判据 |
//! |---|---|
//! | [`空白文档尺寸正确且格全零`] | `MapDocument(int,int)` 语义（宽高 + 全 0 格） |
//! | [`空白文档真实字节`] | 头 4 字节 / 总长 786,484 / 无尾区 |
//! | [`空白文档未关联文件`] | `path` 占位 + `file_name` + 不脏 + 0 差异 |
//! | [`空白文档写盘后能被自己load回来`] | 真落盘 → 真 `load` → 逐字节相等 |
//! | [`空白文档视图与空白图同源`] | `Map::blank` 与 `raw→view` 投影不漂 |
//! | [`创建判据逐条对齐上游`] | `MapDimensions.cs:31-33` 的两条边界 |
//! | [`新建尺寸边界一律报错不panic`] | 0 / <16 / >4096 / 超格数 |
//! | [`不支持格式一律显式拒绝并给原因`] | 7 种格式的中文拒绝理由 |
//! | [`支持的两种格式都能往返`] | `Old12` + `Shanda14` |

use super::*;
use crate::save::save_atomic;
// `RawCell` 是 `mir2_format::Cell`（字节层格）在 `lib.rs` 的再导出名（`document.rs` 里只
// 导了 `RawMap`），本文件要逐格断言「字节层的格也是全 0」。
use crate::RawCell;

/// 本进程专属的临时目录（不写死盘符，公开仓/PR 的机器上没有 `I:`）。
fn 临时目录(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "miredit_core_blank_{tag}_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&d).expect("临时目录应能建出来");
    d
}

// ───────────────────────── 构造语义（对齐上游 `MapDocument(int,int)`） ─────────────────────────

/// 上游 `MapDocument(int width, int height)` 的语义等价物：
/// 宽高原样存下、格**全 0**（C# 是稀疏字典 + 全 0 的 `empty`，见 `Document.cs:43-50`）。
#[test]
fn 空白文档尺寸正确且格全零() {
    let doc = Document::blank(256, 256).expect("256×256 是上游 CreationError 允许的尺寸");

    assert_eq!(doc.size(), (256, 256), "宽高必须原样存下");
    assert_eq!(doc.format_id(), FormatId::Old12, "默认档位是 Old12");
    assert_eq!(doc.disk_cell_len(), Some(12), "Old12 的磁盘格长是 12");
    // 视图档位是**语义投影**，不是磁盘格长（见 `convert::view_cell_format`）
    assert_eq!(doc.format_name(), "Old12");

    // 视图侧：65536 个格，逐字段等于 `Cell::default()`
    assert_eq!(doc.view().cells.len(), 65_536, "w*h 个格一个都不能少");
    assert!(
        doc.view().cells.iter().all(|c| *c == Cell::default()),
        "视图里的格必须全是 `Cell::default()`（全 0）"
    );

    // 字节层侧：同样 65536 个格、同样全 0
    assert_eq!(doc.raw().cells.len(), 65_536);
    assert!(
        doc.raw().cells.iter().all(|c| *c == RawCell::default()),
        "字节层的格也必须全是全 0"
    );

    // 无尾区、不脏、五层计数全 0
    assert_eq!(doc.trailer().len(), 0, "空白图不许带尾区");
    assert_eq!(doc.tolerant_trailer(), 0, "空白图没有未解释字节");
    assert!(!doc.dirty(), "刚建出来不脏");
    assert_eq!(
        doc.layer_counts(),
        LayerCounts::default(),
        "全 0 图的五层计数必须全 0"
    );

    // 新建的图不该一打开就挂常驻黄条（Old12 是 editable 且无尾区，见 `model.rs:471`）
    assert_eq!(
        doc.edit_warning(),
        None,
        "新建的空白 Old12 图不该有任何常驻告警：{:?}",
        doc.edit_warning()
    );

    // 与自己的 orig_bytes 零差异（orig_bytes 就是构造那一刻出的字节）
    assert!(doc.diff().same(), "刚建出来应与自己的 orig_bytes 逐字节相同");
}

/// 256×256 `Old12` 空白文档的**真实字节**：`52 + 12 × 65536 = 786,484` 字节，
/// 头 4 字节 = `00 01 00 01`（w=256 小端、h=256 小端），其余全 0。
///
/// ⚠ 断言点明一个坑：**首字节 `0x00` 不代表这是无标题的 Mir3 图** ——
/// 256 的小端低字节就是 `0x00`。认格式的是长度等式
/// （`mir2-format/src/registry.rs:139-165` 长度优先），不是首字节。
#[test]
fn 空白文档真实字节() {
    let doc = Document::blank(256, 256).expect("256×256 合法");
    let bytes = doc.try_to_bytes().expect("空白图出字节必须成功");

    assert_eq!(bytes.len(), 786_484);
    assert_eq!(bytes.len(), mir2_map::HEADER_LEN + 12 * 65_536);
    assert_eq!(bytes.len(), doc.orig_len(), "orig_bytes 就是这一份");

    // 宽高：小端 `00 01` / `00 01`
    assert_eq!(bytes[0], 0x00);
    assert_eq!(bytes[1], 0x01);
    assert_eq!(bytes[2], 0x00);
    assert_eq!(bytes[3], 0x01);
    assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 256);
    assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 256);

    // 头 4..52（sTitle / UpdateDate / Reserved）与全部格必须全 0
    assert!(
        bytes[4..].iter().all(|b| *b == 0),
        "头 4..52 与 786,432 字节格区必须全 0"
    );

    // `orig_bytes` 与现算字节一致（构造时没有别的来源）
    assert_eq!(doc.orig_bytes(), &bytes[..]);

    // 把真实字节的关键指纹打出来（`cargo test -- --nocapture` 可见），便于事后取证复核
    eprintln!(
        "[空白图真实字节] 总长={} 头4={:02X?} 头4..52全0={} 格区52..全0={}",
        bytes.len(),
        &bytes[..4],
        bytes[4..mir2_map::HEADER_LEN].iter().all(|b| *b == 0),
        bytes[mir2_map::HEADER_LEN..].iter().all(|b| *b == 0)
    );
}

/// 空白文档**未关联磁盘文件**：`path` 是占位、`file_name` 是 `Untitled`。
///
/// 上游真值：`MainWindow.MapTabs.cs:5` 的 `string? path`（新建时 `null`）、
/// `:125-131` 的 `MapDisplayName`（`Path == null` ⇒ `"Untitled {N}"`）、
/// `MainWindow.axaml.cs:1035` 的窗口标题（`mapPath == null` ⇒ `"Untitled"`）。
#[test]
fn 空白文档未关联文件() {
    let doc = Document::blank(64, 64).expect("64×64 合法");

    assert!(!doc.has_path(), "空白文档尚未关联文件");
    assert_eq!(doc.path, Path::new(BLANK_PATH), "占位必须是 BLANK_PATH");
    assert!(doc.path.as_os_str().is_empty(), "BLANK_PATH 必须是空路径");
    assert_eq!(doc.file_name, UNTITLED_FILE_NAME);
    assert_eq!(doc.file_name, "Untitled");

    // 反向对照：从真文件打开的文档 `has_path()` 必须为 true
    let dir = 临时目录("has_path");
    let p = dir.join("real.map");
    save_atomic(&p, &doc.try_to_bytes().expect("出字节")).expect("落盘");
    let loaded = Document::load(&p).expect("能读回");
    assert!(loaded.has_path(), "从磁盘打开 ⇒ 一定已关联文件");
    assert_eq!(loaded.file_name, "real.map");
    std::fs::remove_dir_all(&dir).ok();
}

// ───────────────────────── 落盘往返（真文件） ─────────────────────────

/// **空白图写盘 → 能被自己 `Document::load` 读回来 → 再出字节逐字节相等。**
///
/// 这是本单的核心验收：不允许"造得出来但自己读不回来"。
#[test]
fn 空白文档写盘后能被自己load回来() {
    let doc = Document::blank(256, 256).expect("256×256 合法");
    let bytes = doc.try_to_bytes().expect("空白图出字节必须成功");

    let dir = 临时目录("roundtrip");
    let p = dir.join("blank-256x256.map");
    save_atomic(&p, &bytes).expect("原子写应成功");

    // 磁盘上真的是这些字节
    assert_eq!(std::fs::read(&p).expect("读回磁盘字节"), bytes);

    let back = Document::load(&p).expect("空白图必须能被自己的 Document::load 读回来");
    assert_eq!(back.size(), (256, 256), "宽高读回必须正确");
    assert_eq!(back.format_id(), FormatId::Old12, "格式必须判回 Old12");
    assert_eq!(back.view().cells.len(), 65_536);
    assert!(
        back.view().cells.iter().all(|c| *c == Cell::default()),
        "读回的格必须还是全 0"
    );
    assert_eq!(back.trailer().len(), 0, "读回也不许冒出尾区");
    assert_eq!(
        back.try_to_bytes().expect("再出字节"),
        bytes,
        "写盘 → 读回 → 再写，必须逐字节相等"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// `Map::blank` 与 `raw → view` 投影**必须同源**（两个 crate 的「全 0 格」口径不许漂）。
///
/// 真漂了会怎样：`Document::blank` 的视图来自 `view_from_raw`，而调用方若按
/// `Map::blank` 的约定去理解视图（比如将来 GUI 直接拿 `Map::blank` 当预览），
/// 两边差一格就会"预览是空的、真图不是"。
#[test]
fn 空白文档视图与空白图同源() {
    for (w, h, f) in [
        (16u16, 16u16, FormatId::Old12),
        (256, 256, FormatId::Old12),
        (17, 33, FormatId::Shanda14),
    ] {
        let doc = Document::blank_in(w, h, f).expect("尺寸合法");
        let view = Map::blank(w, h, view_cell_format(f)).expect("Map::blank 同样合法");
        assert_eq!(doc.view(), &view, "{w}×{h} {f:?}：视图必须与 Map::blank 逐格相同");
        // 头部也要一致（w/h 写在小端前 4 字节）
        assert_eq!(doc.view().header_raw, view.header_raw, "{w}×{h} {f:?}：视图头必须一致");
    }
}

// ───────────────────────── 尺寸边界（对齐上游 `CreationError`） ─────────────────────────

/// `creation_error` 逐条对齐上游 `MapDimensions.cs:28-41`（`MapCreationKind.Blank`）。
///
/// 只做**纯算术**断言，不构造文档 —— 所以 `4000×4000`（16M 格）这种能过判据的
/// 大尺寸在这里是安全的（不会去铺 576 MB 的格）。
#[test]
fn 创建判据逐条对齐上游() {
    // ① `:31-32` 窗口 `16..=4096`（含边界）
    assert_eq!(creation_error(16, 16), None, "16×16 是窗口下界，必须放行");
    assert_eq!(creation_error(4096, 16), None, "单边 4096 在窗口内");
    assert_eq!(creation_error(16, 4096), None);

    // ② `:33` 格数上限 16,000,000（`4096×4096 = 16,777,216` 超限，单边却合格）
    assert_eq!(
        creation_error(4000, 4000),
        None,
        "4000×4000 = 恰好 16,000,000 格，必须放行"
    );
    let 超格 = creation_error(4096, 4096).expect("4096×4096 必须被格数上限拦下");
    assert!(
        超格.contains("16,777,216") || 超格.contains("16777216"),
        "原因里要带实际格数：{超格}"
    );
    assert!(超格.contains("MapDimensions.cs:33"), "原因里要点名上游行号：{超格}");
    assert!(
        creation_error(4001, 4000).is_some(),
        "4001×4000 = 16,004,000 格，必须拒绝"
    );

    // ③ 窗口外的各种形态（下界 / 上界 / 0）
    for (w, h) in [(15u16, 16u16), (16, 15), (0, 0), (0, 16), (16, 0), (4097, 16), (16, 4097)] {
        let 原因 = creation_error(w, h)
            .unwrap_or_else(|| panic!("{w}×{h} 不在 16..=4096 窗口内，必须拒绝"));
        assert!(
            原因.contains("MapDimensions.cs:31-32"),
            "{w}×{h} 的原因要点名上游行号：{原因}"
        );
    }
}

/// 尺寸边界：0 / 小于 16 / 大于 4096 / 超格数 ⇒ **一律 `Err`，不 panic**。
///
/// 每一条都必须「在分配之前」返回 —— 所以 `4096×4096`（16.7M 格）这类输入
/// 绝不能先铺格再报错（否则测试机直接吃掉 576 MB）。
#[test]
fn 新建尺寸边界一律报错不panic() {
    for (w, h) in [
        (0u16, 0u16),
        (0, 16),
        (16, 0),
        (15, 16),
        (16, 15),
        (1, 1),
        (4097, 16),
        (16, 4097),
        (4096, 4096), // 单边合格但格数超 16M
        (5000, 5000),
        (65535, 1),
        (1, 65535),
    ] {
        let err = Document::blank(w, h)
            .err()
            .unwrap_or_else(|| panic!("{w}×{h} 必须 Err（上游 CreationError 不允许）"));
        // 必须是 `CoreError::Map`（不是 Io / Save / BadLayer），且带占位路径
        match err {
            CoreError::Map { path, err } => {
                assert_eq!(path, Path::new(BLANK_PATH), "{w}×{h}：错误里应是占位路径");
                let msg = err.to_string();
                assert!(!msg.is_empty(), "{w}×{h}：原因不许是空串");
            }
            other => panic!("{w}×{h} 应报 Map，实际 {other:?}"),
        }
    }

    // 边界放行侧：16×16（窗口下界）与 4096×16（窗口上界 + 格数合格）
    assert!(Document::blank(16, 16).is_ok(), "16×16 是窗口下界，必须能建");
    assert!(Document::blank(4096, 16).is_ok(), "4096×16 在窗口内，必须能建");
    assert!(Document::blank(16, 4096).is_ok(), "16×4096 在窗口内，必须能建");
}

// ───────────────────────── 格式闸门（9 种里只有 2 种能新建） ─────────────────────────

/// 不支持新建的 7 种格式：**必须显式拒绝**，原因里点名格式与具体依据。
///
/// 判据见 [`Document::blank_in`] 文档第一节（`editable == true` ∩ `TAIL_LEN == 0` ∩
/// `detect_format` 长度优先可认）。这一条防的是「静默产出坏图」：
/// 若放行 `CrystalCs`/`Mir3Wemade` 这类变长格，写出来的字节连本仓自己都读不回来。
#[test]
fn 不支持格式一律显式拒绝并给原因() {
    let 不支持 = [
        (FormatId::Shanda2012, "22 字节尾区"),
        (FormatId::AntiHack, "异或键"),
        (FormatId::Wemade2010, "异或键"),
        (FormatId::Heroes34, "尾区"),
        (FormatId::Mir3Wemade, "背景块"),
        (FormatId::Mir3Shanda, "标题"),
        (FormatId::CrystalCs, "变长格"),
    ];
    for (f, 关键词) in 不支持 {
        let err = Document::blank_in(16, 16, f)
            .err()
            .unwrap_or_else(|| panic!("{f:?} 不支持新建，必须 Err"));
        match err {
            CoreError::Map { path, err } => {
                assert_eq!(path, Path::new(BLANK_PATH));
                let msg = err.to_string();
                // 点名格式（人读名里带格式名，如 "Shanda2012（36B/格…）"）
                assert!(
                    msg.contains(&format!("{f:?}")),
                    "{f:?}：拒绝原因里必须点名格式：{msg}"
                );
                // 点名具体原因（不是一句"不支持"了事）
                assert!(
                    msg.contains(关键词),
                    "{f:?}：拒绝原因里必须写出「{关键词}」：{msg}"
                );
                assert!(
                    msg.contains("本仓还不能新建"),
                    "{f:?}：必须是显式拒绝措辞：{msg}"
                );
            }
            other => panic!("{f:?} 应报 Map，实际 {other:?}"),
        }
    }

    // 反向对照：支持的两种必须放行（否则上面那些断言可能只是"什么都拒"）
    assert!(Document::blank_in(16, 16, FormatId::Old12).is_ok());
    assert!(Document::blank_in(16, 16, FormatId::Shanda14).is_ok());
}

/// 支持的两种格式（`Old12` / `Shanda14`）都能造出来并往返。
#[test]
fn 支持的两种格式都能往返() {
    for (f, 格长, 视图档位) in [
        (FormatId::Old12, 12usize, "Old12"),
        (FormatId::Shanda14, 14, "New14"),
    ] {
        let doc = Document::blank_in(64, 64, f).expect("64×64 合法");
        assert_eq!(doc.format_id(), f, "{f:?}：字节层格式必须就是它");
        assert_eq!(doc.disk_cell_len(), Some(格长), "{f:?}：磁盘格长");
        assert_eq!(doc.format_name(), 视图档位, "{f:?}：视图档位是语义投影");
        assert_eq!(doc.view().cells.len(), 4096);
        assert!(doc.view().cells.iter().all(|c| *c == Cell::default()));
        assert_eq!(doc.trailer().len(), 0, "{f:?}：空白图无尾区");

        let bytes = doc.try_to_bytes().expect("出字节");
        assert_eq!(bytes.len(), mir2_map::HEADER_LEN + 格长 * 4096, "{f:?}：字节数");

        // 真落盘 → 真读回
        let dir = 临时目录(&format!("fmt_{f:?}"));
        let p = dir.join("blank.map");
        save_atomic(&p, &bytes).expect("落盘");
        let back = Document::load(&p).expect("读回");
        assert_eq!(back.format_id(), f, "{f:?}：读回格式必须一致");
        assert_eq!(back.size(), (64, 64));
        assert_eq!(back.try_to_bytes().expect("再出字节"), bytes, "{f:?}：往返逐字节相等");
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// `blank_gate` 的支持集就是那两种，且**判据本身**可单独复核（防"改一处忘了另一处"）。
#[test]
fn 格式闸门支持集就是两种() {
    let 支持: Vec<FormatId> = [
        FormatId::CrystalCs,
        FormatId::Mir3Wemade,
        FormatId::Mir3Shanda,
        FormatId::AntiHack,
        FormatId::Wemade2010,
        FormatId::Shanda2012,
        FormatId::Shanda14,
        FormatId::Heroes34,
        FormatId::Old12,
    ]
    .into_iter()
    .filter(|f| blank_gate(*f).is_ok())
    .collect();
    assert_eq!(支持, vec![FormatId::Shanda14, FormatId::Old12]);

    // 支持集必须与「`editable == true` 的格式」重合（判据 1 的自证）
    let 可编辑: Vec<FormatId> = mir2_format::registry()
        .iter()
        .filter(|f| f.caps().editable)
        .map(|f| f.id())
        .collect();
    assert_eq!(
        支持, 可编辑,
        "空白图的支持集必须与 caps().editable == true 的集合重合（`model.rs:471-472`）"
    );
}
