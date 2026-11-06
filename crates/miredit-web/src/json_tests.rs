//! `json.rs` 的单元测试段（用 `#[cfg(test)] #[path]` 挂回去，两边仍属同一个模块）。
//!
//! 为什么单独一个文件：`json.rs` 本体加测试会到 700 行，超过「单文件 ≤600 行」的硬上限
//! （与 `miredit-core/src/session_tests.rs` 同一个做法）。
//! 拆的时候**一行断言都没改**（只去掉整体缩进 4 空格）。
use super::*;

fn 解析(s: &str) -> ApiResult<EditCmd> {
    parse_edit_cmd(&serde_json::from_str(s).expect("测试 JSON 应合法"))
}

/// 十一条内核命令 + 一条网页侧命令都必须能解析出来（少一条就是断链）。
#[test]
fn 命令集逐条解析() {
    assert!(matches!(
        解析(r#"{"SetLayer":{"layer":"ground"}}"#).expect("SetLayer"),
        EditCmd::Core(Command::SetLayer { layer }) if layer == "ground"
    ));
    assert!(matches!(
        解析(r#"{"SetBrushSize":{"size":7}}"#).expect("SetBrushSize"),
        EditCmd::Core(Command::SetBrushSize { size: 7 })
    ));
    assert!(matches!(
        解析(r#"{"Paint":{"x":100,"y":200}}"#).expect("Paint"),
        EditCmd::Core(Command::Paint { x: 100, y: 200 })
    ));
    assert!(matches!(
        解析(r#"{"Stroke":{"from":[10,10],"to":[20,20]}}"#).expect("Stroke"),
        EditCmd::Core(Command::Stroke { from: (10, 10), to: (20, 20) })
    ));
    assert!(matches!(
        解析(r#"{"Erase":{"x":10,"y":10}}"#).expect("Erase"),
        EditCmd::Core(Command::Erase { x: 10, y: 10 })
    ));
    assert!(matches!(
        解析(r#"{"Fill":{"x":358,"y":0}}"#).expect("Fill"),
        EditCmd::Core(Command::Fill { x: 358, y: 0 })
    ));
    assert!(matches!(
        解析(r#"{"Pick":{"x":10,"y":10}}"#).expect("Pick"),
        EditCmd::Core(Command::Pick { x: 10, y: 10 })
    ));
    assert!(matches!(
        解析(r#""Undo""#).expect("Undo"),
        EditCmd::Core(Command::Undo)
    ));
    assert!(matches!(
        解析(r#""Redo""#).expect("Redo"),
        EditCmd::Core(Command::Redo)
    ));
    assert!(matches!(
        解析(r#""Save""#).expect("Save"),
        EditCmd::Core(Command::Save)
    ));
    // 无参变体的对象形式也收
    assert!(matches!(
        解析(r#"{"Undo":null}"#).expect("Undo 对象形式"),
        EditCmd::Core(Command::Undo)
    ));
    // 网页侧命令：选帧
    assert!(matches!(
        解析(r#"{"SetBrushFromPalette":{"family":"object","lib":0,"frame":3}}"#).expect("选帧"),
        EditCmd::SetBrushFromPalette { family: Family::Surface, lib: 0, frame: 3 }
    ));
    // 换工具：**内核变体**（键原样透传给 `cmd::set_tool_by_key`，本层不翻译别名）
    assert!(matches!(
        解析(r#"{"SetTool":{"tool":"eraser"}}"#).expect("换工具"),
        EditCmd::Core(Command::SetTool { tool }) if tool == "eraser"
    ));
    assert_eq!(解析(r#"{"SetTool":{"tool":"eraser"}}"#).expect("换工具").name(), "SetTool");
    // 放印章：**内核变体**（格表注册在 routes::edit，本层只翻 JSON）
    assert!(matches!(
        解析(r#"{"PlaceStamp":{"x":11,"y":12,"stamp":"mir/mir_00000"}}"#).expect("放印章"),
        EditCmd::Core(Command::PlaceStamp { x: 11, y: 12, stamp }) if stamp == "mir/mir_00000"
    ));
    assert_eq!(
        解析(r#"{"PlaceStamp":{"x":0,"y":0,"stamp":"a"}}"#).expect("放印章").name(),
        "PlaceStamp"
    );
    // ── 选区 / 剪贴板七条（2026-11-05 接入）──
    assert!(matches!(
        解析(r#"{"SetSelection":{"x0":1,"y0":2,"x1":5,"y1":6}}"#).expect("设选区"),
        EditCmd::Core(Command::SetSelection { x0: 1, y0: 2, x1: 5, y1: 6 })
    ));
    assert!(matches!(
        解析(r#"{"PasteSelection":{"x":3,"y":4}}"#).expect("粘贴"),
        EditCmd::Core(Command::PasteSelection { x: 3, y: 4 })
    ));
    // 五条无参：字符串形式与对象形式都收
    for (name, want) in [
        ("ClearSelection", "ClearSelection"),
        ("CopySelection", "CopySelection"),
        ("CutSelection", "CutSelection"),
        ("DeleteSelection", "DeleteSelection"),
        ("FillSelection", "FillSelection"),
    ] {
        let as_str = 解析(&format!("\"{name}\"")).unwrap_or_else(|e| panic!("{name} 字符串形式：{e:?}"));
        assert_eq!(as_str.name(), want, "{name} 字符串形式");
        let as_obj = 解析(&format!("{{\"{name}\":null}}")).unwrap_or_else(|e| panic!("{name} 对象形式：{e:?}"));
        assert_eq!(as_obj.name(), want, "{name} 对象形式");
    }
}

/// 结构不对必须是 400（不是 200 + error，也不是 500）。
///
/// ⚠ 非法**工具键**不在这里：那是内核的语义拒绝（`Ok` + `changed=0` + 中文原因），
/// 不是"这次调用没完成"。本层只挡结构错误。
#[test]
fn 非法命令一律400() {
    for bad in [
        r#"{"Paint":{"x":1}}"#,
        r#"{"Paint":{"x":-1,"y":2}}"#,
        r#"{"Paint":{"x":70000,"y":2}}"#,
        r#"{"Stroke":{"from":[1],"to":[2,3]}}"#,
        r#"{"SetLayer":{"layer":1}}"#,
        r#"{"Nope":{}}"#,
        r#""nope""#,
        r#"{"Paint":{},"Fill":{}}"#,
        r#"{}"#,
        r#"123"#,
        r#"{"SetBrushFromPalette":{"family":"nope","lib":0,"frame":0}}"#,
        // Erase 与 Paint 同形状：坐标缺一不可、不许越界
        r#"{"Erase":{"x":1}}"#,
        r#"{"Erase":{"x":70000,"y":2}}"#,
        // SetTool 只挡**结构**：缺字段 / tool 不是字符串
        r#"{"SetTool":{}}"#,
        r#"{"SetTool":{"tool":1}}"#,
        // PlaceStamp 也只挡结构：坐标越界 / 缺 stamp
        r#"{"PlaceStamp":{"x":1,"y":2}}"#,
        r#"{"PlaceStamp":{"x":70000,"y":2,"stamp":"a"}}"#,
        r#"{"PlaceStamp":{"x":1,"y":2,"stamp":3}}"#,
        // 选区：四个端点缺一不可、不许越界
        r#"{"SetSelection":{"x0":1,"y0":2,"x1":5}}"#,
        r#"{"SetSelection":{"x0":70000,"y0":2,"x1":5,"y1":6}}"#,
        r#"{"PasteSelection":{"x":1}}"#,
    ] {
        let err = 解析(bad).expect_err(bad);
        assert_eq!(err.status(), axum::http::StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(err.code(), "bad_request", "{bad}");
    }
}

/// 族名别名：`object` / `front` 都指向表层（与前端词汇对齐）。
#[test]
fn 族名别名() {
    assert_eq!(parse_family("ground").expect("ground"), Family::Ground);
    assert_eq!(parse_family("MIDDLE").expect("middle"), Family::Middle);
    assert_eq!(parse_family("object").expect("object"), Family::Surface);
    assert_eq!(parse_family("front").expect("front"), Family::Surface);
    assert!(parse_family("tiles").is_err());
}

/// 工具键**本层不校验**：`fill` / `unwired` 也照样解析成 `Command::SetTool`，
/// 由内核的 `set_tool_by_key` 判（它只收四个小写键，非法键给 changed=0 + 中文原因）。
///
/// 这条钉住的是"词汇表只有一个真源（core）"，web 侧不许长出第二套。
#[test]
fn 工具键原样透传() {
    for key in ["brush", "bucket", "eraser", "pickup", "fill", "unwired", "激光"] {
        let json = format!(r#"{{"SetTool":{{"tool":"{key}"}}}}"#);
        let cmd = 解析(&json).expect(&json);
        assert!(
            matches!(&cmd, EditCmd::Core(Command::SetTool { tool }) if tool == key),
            "{json} 必须原样透传（键校验是内核的事）"
        );
    }
}

/// `touched_cell()`：**真的改格**的命令都要报出坐标（`rerender` 区域靠它）。
///
/// 2026-11-05 补 `Erase`：上次加 `Erase` 变体时漏了这里，落进 `_ => None`
/// ⇒ 前端只能收到「整屏」提示（`w=h=0`）。现在**每条都显式写**，不再靠兜底。
#[test]
fn 改格命令都报坐标() {
    for (json, want) in [
        (r#"{"Paint":{"x":1,"y":2}}"#, Some((1, 2))),
        (r#"{"Erase":{"x":3,"y":4}}"#, Some((3, 4))),
        (r#"{"Fill":{"x":5,"y":6}}"#, Some((5, 6))),
        (r#"{"Pick":{"x":7,"y":8}}"#, Some((7, 8))),
        (r#"{"Stroke":{"from":[0,0],"to":[9,10]}}"#, Some((9, 10))),
        // 粘贴以 (x,y) 为左上角 ⇒ 报它
        (r#"{"PasteSelection":{"x":13,"y":14}}"#, Some((13, 14))),
        // 选区是矩形，一个坐标装不下 ⇒ None（整屏）
        (r#"{"SetSelection":{"x0":1,"y0":2,"x1":5,"y1":6}}"#, None),
        // 五条作用在"当前选区"上、命令里没有坐标 ⇒ None（整屏）
        (r#""ClearSelection""#, None),
        (r#""CopySelection""#, None),
        (r#""CutSelection""#, None),
        (r#""DeleteSelection""#, None),
        (r#""FillSelection""#, None),
        // 与坐标无关的命令必须 None
        (r#"{"SetLayer":{"layer":"middle"}}"#, None),
        (r#"{"SetBrushSize":{"size":5}}"#, None),
        (r#"{"SetTool":{"tool":"eraser"}}"#, None),
        (r#""Undo""#, None),
        (r#""Save""#, None),
    ] {
        let cmd = 解析(json).expect(json);
        assert_eq!(cmd.touched_cell(), want, "{json}");
    }
    // `PlaceStamp` 也真的改格 —— 报的是**锚点**
    let stamp = EditCmd::Core(Command::PlaceStamp {
        x: 11,
        y: 12,
        stamp: "mir/x".to_string(),
    });
    assert_eq!(stamp.touched_cell(), Some((11, 12)));
    assert_eq!(stamp.name(), "PlaceStamp");
}

// ───────────────────────── R12-F1：单元格元数据（SetMetadata） ─────────────────────────

/// **`0` ≠ `null`**：`light: 0` 必须解析成 `Some(0)`（"设成 0"），
/// `light: null` / 缺省才是 `None`（"不动"）。这是 Mixed 选区不被抹平的前提。
///
/// 顺带钉住 `area` 的四元组形状与"缺字段报 400"。
#[test]
fn 元数据命令零与空不是一回事() {
    let cmd = 解析(
        r#"{"SetMetadata":{"light":0,"door":255,"offset":null,"area":[1,2,3,4]}}"#,
    )
    .expect("应能解析");
    let EditCmd::Core(Command::SetMetadata {
        light,
        door,
        offset,
        area,
    }) = cmd
    else {
        panic!("必须解析成 SetMetadata");
    };
    assert_eq!(light, Some(0), "light: 0 ⇒ Some(0)（不是 None）");
    assert_eq!(door, Some(255), "door: 255 ⇒ Some(255)");
    assert_eq!(offset, None, "offset: null ⇒ None（这一字段不动）");
    assert_eq!(area, (1, 2, 3, 4), "area 是闭区间四元组");

    // 缺字段 ⇒ 全部 None（"什么都不改"），不是 400：这条让前端可以只发一个字段
    let cmd = 解析(r#"{"SetMetadata":{"light":7,"area":[0,0,0,0]}}"#).expect("缺省字段合法");
    let EditCmd::Core(Command::SetMetadata {
        light,
        door,
        offset,
        ..
    }) = cmd
    else {
        panic!("必须解析成 SetMetadata");
    };
    assert_eq!((light, door, offset), (Some(7), None, None));

    // 形状不对 ⇒ 400（并且不会静默当成 0）
    for bad in [
        r#"{"SetMetadata":{"light":256,"area":[0,0,0,0]}}"#,
        r#"{"SetMetadata":{"light":-1,"area":[0,0,0,0]}}"#,
        r#"{"SetMetadata":{"light":"7","area":[0,0,0,0]}}"#,
        r#"{"SetMetadata":{"light":7,"area":[0,0,0]}}"#,
        r#"{"SetMetadata":{"light":7}}"#,
    ] {
        assert!(解析(bad).is_err(), "{bad} 必须 400");
    }
}

/// **字节断言**：`SetMetadata` 落到 `Old12` 上只许动 `off6`（门索引）/ `off7`（门偏移）/
/// `off11`（光照）三个字节；Mixed（没给的字段）保持原值；整批 = 一步撤销且撤销回原字节。
///
/// 这里走的是 `parse_edit_cmd`（web 的 JSON 层）→ `miredit_core::apply` 的**同一条链路**，
/// 只把 HTTP 那一层摘掉；字节口径由 core 的 writer 决定（`old12.rs`）。
#[test]
fn 元数据命令只动三个字节且一步撤销() {
    use miredit_core::{Command as C, Document, EditSession};

    /// 12 B/格合成字节：52 字节头；每格 `door=7`（off6）、`light=9`（off11）、偏移 0（off7）。
    fn 造旧档(w: u16, h: u16) -> Vec<u8> {
        let mut b = vec![0u8; 52 + 12 * (w as usize * h as usize)];
        b[0..2].copy_from_slice(&w.to_le_bytes());
        b[2..4].copy_from_slice(&h.to_le_bytes());
        for i in 0..(w as usize * h as usize) {
            let base = 52 + i * 12;
            b[base..base + 2].copy_from_slice(&((i as u16) + 1).to_le_bytes());
            b[base + 6] = 7;
            b[base + 11] = 9;
        }
        b
    }

    let 原 = 造旧档(2, 2);
    let doc = Document::from_bytes(std::path::Path::new(r"I:\__web_meta__\m.map"), 原.clone())
        .expect("合成 Old12 图应能解析");
    let mut ed = EditSession::new(doc);

    // ① 只给 light ⇒ 只动 off11（门/偏移保持原值，Mixed 不被抹平）
    let EditCmd::Core(cmd) = 解析(r#"{"SetMetadata":{"light":200,"area":[0,0,0,0]}}"#)
        .expect("解析")
    else {
        panic!("必须是 SetMetadata");
    };
    let out = miredit_core::apply(&mut ed, cmd).expect("应用");
    assert_eq!(out.changed, 1, "1 格真的改了");
    assert!(
        out.note.as_deref().unwrap_or("").contains("光照"),
        "回执要说清改的是哪个字段：{:?}",
        out.note
    );
    let 新 = ed.document().to_bytes();
    let 基 = 52;
    let mut 变: Vec<usize> = (0..原.len()).filter(|i| 原[*i] != 新[*i]).collect();
    变.sort_unstable();
    assert_eq!(变, vec![基 + 11], "只给 light ⇒ 只许动 off11");
    assert_eq!((新[基 + 6], 新[基 + 7], 新[基 + 11]), (7, 0, 200));

    // ② 三字段全给 ⇒ off6 / off7 / off11 各变一个字节
    let EditCmd::Core(cmd) =
        解析(r#"{"SetMetadata":{"light":5,"door":6,"offset":8,"area":[0,0,0,0]}}"#).expect("解析")
    else {
        panic!("必须是 SetMetadata");
    };
    let out = miredit_core::apply(&mut ed, cmd).expect("应用");
    assert_eq!(out.changed, 1);
    let 新2 = ed.document().to_bytes();
    let mut 变2: Vec<usize> = (0..新.len()).filter(|i| 新[*i] != 新2[*i]).collect();
    变2.sort_unstable();
    assert_eq!(
        变2,
        vec![基 + 6, 基 + 7, 基 + 11],
        "只许动 door(off6)/offset(off7)/light(off11)"
    );
    assert_eq!((新2[基 + 6], 新2[基 + 7], 新2[基 + 11]), (6, 8, 5));

    // ③ 整批 = 一步撤销；撤两步回到最初字节
    assert_eq!(ed.undo_len(), 2, "两次改属性 = 两步（每批一步）");
    miredit_core::apply(&mut ed, C::Undo).expect("撤销第二步");
    assert_eq!(ed.document().to_bytes(), 新);
    miredit_core::apply(&mut ed, C::Undo).expect("撤销第一步");
    assert_eq!(ed.document().to_bytes(), 原, "撤销到底必须逐字节回原样");

    // ④ 三个都 null ⇒ 一格不动、不开撤销步
    let EditCmd::Core(cmd) = 解析(r#"{"SetMetadata":{"light":null,"door":null,"offset":null,"area":[0,0,1,1]}}"#)
        .expect("全空合法")
    else {
        panic!("必须是 SetMetadata");
    };
    let out = miredit_core::apply(&mut ed, cmd).expect("全空不报错");
    assert_eq!(out.changed, 0);
    assert_eq!(ed.undo_len(), 0, "全空不许开撤销步");
    assert_eq!(ed.document().to_bytes(), 原);
}
