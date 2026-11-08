//! 单元格检查器取数：把 `.map` 里的一格 + 资源库里的帧，翻成界面上那六行真实文字。
//!
//! ## 为什么单独一个模块
//! 「面板上显示什么」必须能在**没有窗口**的情况下被测到（`cargo test --lib`），
//! 否则只能靠肉眼看截图判断"数字对不对"。所以取数逻辑与绘制彻底分开：
//! 本模块是**纯函数**（输入 `&Cell` + 帧源，输出字符串），绘制端只管排版。
//!
//! ## 字段与出处（逐条对得上，不猜）
//!
//! | 面板条目 | 取值 | 出处 |
//! |---|---|---|
//! | 阻挡 低 | `bk_img & 0x8000 != 0` | `mir2_map::Cell::is_walkable`（`mir2-map/src/cell.rs:181-188`）|
//! | 阻挡 高 | `fr_img & 0x8000 != 0` | 同上 |
//! | 地表 图号/库号 | `bk_img & 0x7FFF` / `btTiles`（`Cell::tiles`） | `miredit-render/src/compose.rs:13`（1-based 帧号，取图 `(img&0x7FFF)-1`）|
//! | 中层 图号/库号 | `mid_img & 0x7FFF` / `btsmTiles`（`Cell::sm_tiles`） | 同上 `:13`（帧号表：1-based、取图 `(img & 0x7FFF) - 1`）|
//! | 物体 图号/库号 | `fr_img & 0x7FFF` / `btArea`（`Cell::area`） | 同上 `:127`（`索引 = 帧号(cell.fr_img) // = (fr_img & 0x7FFF) - 1`）、`:229`（上游 `MapPreview.FrontImage` = `(front.Image & 0x7FFF) - 1`）|
//! | 帧尺寸 | 向帧源要该帧的 `width×height` | `mir2_assets::SpriteSource::frame` |
//! | 元数据 | `light` / `door_index` / `door_offset` | `mir2-map/src/cell.rs` 字段表 off 6/7/11 |
//! | 图块动画 | `ani_frame` / `ani_tick` | 同上 off 8/9 |
//!
//! ⚠ **行号订正（2026-11-08，三处同一病「兄弟行未同步」）**：上一版三行分别引
//! `compose.rs:98-102`（地表）、`:123-127`（中层）、`:148-152`（物体），三处**都不是它们声称的内容**——
//! `:98-102` 是 §五「已知差异」（96×64 / C++ 只画 48×32 表层），`:123-127` 是 §七 里**物体**（`fr_img`）
//! 帧号的「本库等价写法」（不是中层），`:148-152` 是 §八「地图光照」段。现按各条**实际声称的内容**改指：
//! 帧号通用规则 → `:13`（帧号表第 7 条）；物体帧号的本库写法 → `:127`；上游同口径 → `:229`。
//!
//! ⚠ **中层（`mid_img`）没有专门一行**：`compose.rs` 只对 `fr_img` 写了逐字等价式，
//! 中层与地表共用 `:13` 的通用规则（`(img & 0x7FFF) - 1`）；库号路由（`btsmTiles`→中层 /
//! `btArea`→表层）记在 `:14`，本表未逐行引，需要时按 `:14` 查。
//!
//! ⚠ **`btArea`（off10）只叫「表层库号」，且只在「物体」行显示一次**（审计 E2，2026-11-04）。
//! 上一版「图块动画」行还有第三列叫「区域」、取的是同一个 `Cell::area` —— 同一个字节在同一面板
//! 里有两个名字，其中一个还是错的。它是什么有四方证据：
//! 客户端 `GameScene.cpp:3468,3510`（`wunit = btArea` → `GetObjs(wunit, …)`）、
//! `map_loader.gd:203-208`（`get_front_lib_index` = off10）、
//! `mir2-assets/src/lib_paths.rs:15`（`Surface ← btArea`）、
//! C# `_upstream/CrystalStudio/LegacyMapReader.cs:298` + `_upstream/CrystalStudio/Assets.cs:363,365`（`frindex = off + 120` = Objects 家族）。
//! 命名源头在 `mir2-map/src/cell.rs:168-169`（那个 crate 本单不许改），所以**只在 gui 侧改显示名**。
//! `.map` 里没有第三个动画字段（off8/9/10 已被上面的表列全），故「图块动画」如实只留两行。
//!
//! ## 取不到的字段怎么办
//! **一律 `—`**（[`crate::ui_text::VAL_UNKNOWN`]），并且**不解释、不猜**：
//! - `Old12` 档没有 `btTiles`/`btsmTiles` 两个字节 ⇒ 地表/中层的「库号」是 `—`
//!   （不是 `0`：`0` 在这里是"确实用了 0 号库"的意思，两者不能混）；
//! - 帧取不到（库没登记 / 帧号越界 / 空帧）⇒ 「帧」是 `—`；
//! - 该层图号为 0（本来就没图）⇒ 图号照实写 `0`，帧写 `—`。
//!
//! ⚠ 任务书 §三.6 提到的「帧数」**不在 `.map` 里**（`mir2-map` 的格子只有 `ani_frame`
//! 一个字节 = 当前帧序号，没有"总帧数"），`mir2_assets` 也没有暴露每库帧数 ⇒ 本条如实不给，
//! 改用三个真取得到的字段（见上表末行）。

use mir2_assets::{LibRef, SpriteSource};
use mir2_map::{Cell, CellFormat};

use crate::ui_text as tx;

/// 六行值 + 标题行（顺序与 [`tx::INSPECTOR_KEYS`] 一一对应）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellLines {
    /// 标题行：`单元格 x,y · N 个物体`。
    pub head: String,
    /// 六个值（第 0 条 = 阻挡，第 5 条 = 图块动画）。
    pub values: [String; 6],
}

impl CellLines {
    /// 什么都没悬停时的一屏：标题 `—`，六行模板里所有占位符都填 `—`。
    ///
    /// 注意**仍然走模板**（而不是另写一句"无数据"），这样占位符少填一个
    /// 会在界面上原样露出 `{size}`，一眼就能发现。
    pub fn unknown() -> Self {
        let u = tx::VAL_UNKNOWN;
        // 注意：`area` 不再出现在任何模板里（审计 E2 —— 它是「表层库号」，
        // 已经由 `VAL_OBJECTS_TMPL` 的 `{lib}` 显示一次，不再重复显示）。
        let kv: [(&str, &str); 10] = [
            ("low", u),
            ("high", u),
            ("img", u),
            ("lib", u),
            ("size", u),
            ("light", u),
            ("door", u),
            ("offset", u),
            ("frame", u),
            ("tick", u),
        ];
        Self {
            head: u.to_owned(),
            values: [
                tx::fill(tx::VAL_COLLISION_TMPL, &kv),
                tx::fill(tx::VAL_GROUND_TMPL, &kv),
                tx::fill(tx::VAL_MIDDLE_TMPL, &kv),
                tx::fill(tx::VAL_OBJECTS_TMPL, &kv),
                tx::fill(tx::VAL_METADATA_TMPL, &kv),
                tx::fill(tx::VAL_TILE_ANIMATION_TMPL, &kv),
            ],
        }
    }
}

/// 取一格的六行信息。
///
/// `cell == None`（鼠标不在图上 / 越界）⇒ [`CellLines::unknown`]。
pub fn cell_lines(
    x: u16,
    y: u16,
    cell: Option<&Cell>,
    fmt: CellFormat,
    src: Option<&dyn SpriteSource>,
) -> CellLines {
    let Some(c) = cell else {
        return CellLines::unknown();
    };

    let img = |raw: u16| raw & 0x7FFF; // 最高位是"不可通过"，不参与取图
    let g_img = img(c.bk_img);
    let m_img = img(c.mid_img);
    let f_img = img(c.fr_img);

    // 「N 个物体」= 该格有几层真的带图（图号非 0）——不是猜"物体数"，是数出来的。
    let objects = [g_img, m_img, f_img].iter().filter(|v| **v != 0).count();

    let head = tx::fill(
        tx::INSPECTOR_HEAD_TMPL,
        &[
            ("x", &x.to_string()),
            ("y", &y.to_string()),
            ("n", &objects.to_string()),
        ],
    );

    // 库号：Old12 档根本没有这两个字节 ⇒ 如实 `—`
    let lib_of = |v: u8| -> String {
        if fmt.has_lib_bytes() {
            v.to_string()
        } else {
            tx::VAL_UNKNOWN.to_owned()
        }
    };

    let collision = tx::fill(
        tx::VAL_COLLISION_TMPL,
        &[
            (
                "low",
                if c.bk_img & 0x8000 != 0 {
                    tx::VAL_YES
                } else {
                    tx::VAL_NO
                },
            ),
            (
                "high",
                if c.fr_img & 0x8000 != 0 {
                    tx::VAL_YES
                } else {
                    tx::VAL_NO
                },
            ),
        ],
    );

    let ground = layer_value(
        g_img,
        &lib_of(c.tiles),
        size_of(src, LibRef::ground(u16::from(c.tiles)), g_img),
        tx::VAL_GROUND_TMPL,
    );
    let middle = layer_value(
        m_img,
        &lib_of(c.sm_tiles),
        size_of(src, LibRef::middle(u16::from(c.sm_tiles)), m_img),
        tx::VAL_MIDDLE_TMPL,
    );
    let objects_v = layer_value(
        f_img,
        &c.area.to_string(),
        size_of(src, LibRef::surface(u16::from(c.area)), f_img),
        tx::VAL_OBJECTS_TMPL,
    );

    let meta = tx::fill(
        tx::VAL_METADATA_TMPL,
        &[
            ("light", &c.light.to_string()),
            ("door", &c.door_index.to_string()),
            ("offset", &c.door_offset.to_string()),
        ],
    );
    // 「图块动画」只填帧数/间隔（官方译名；审计 E2）：`area` 是**表层库号**，已经在「物体」行显示过，
    // 这里再显示一遍就是同一字节两个名字（上一版第二个名字还错叫「区域」）。
    let anim = tx::fill(
        tx::VAL_TILE_ANIMATION_TMPL,
        &[
            ("frame", &c.ani_frame.to_string()),
            ("tick", &c.ani_tick.to_string()),
        ],
    );

    CellLines {
        head,
        values: [collision, ground, middle, objects_v, meta, anim],
    }
}

/// 填「图号 / 库号 / 帧尺寸」这一行。
fn layer_value(img: u16, lib: &str, size: Option<(u32, u32)>, tmpl: &str) -> String {
    let size = match size {
        Some((w, h)) => tx::fill(tx::SIZE_TMPL, &[("w", &w.to_string()), ("h", &h.to_string())]),
        None => tx::VAL_UNKNOWN.to_owned(),
    };
    tx::fill(
        tmpl,
        &[("img", &img.to_string()), ("lib", lib), ("size", &size)],
    )
}

/// 向帧源要该帧的像素尺寸；图号为 0 或取不到帧 ⇒ `None`（界面写 `—`）。
fn size_of(
    src: Option<&dyn SpriteSource>,
    lib: LibRef,
    img: u16,
) -> Option<(u32, u32)> {
    if img == 0 {
        return None; // 该层本来没图，不是"缺帧"
    }
    let f = src?.frame(lib, u32::from(img - 1))?; // 1-based 帧号 ⇒ 0-based 索引
    Some((f.width, f.height))
}

// ---------------------------------------------------------------------------
// 悬停地砖信息浮层的取数（视图 → 地砖信息悬浮）
// ---------------------------------------------------------------------------

/// 悬停浮层的**三块内容**（`_upstream/CrystalStudio/MainWindow.CellHover.cs:56-63` 的三行）。
///
/// 只给文字，不给几何：排版（宽 350 / 越界翻边 / 四边各夹 8）在
/// [`crate::cell_hover`] 里算 —— 与本模块「取数 / 绘制分开」的既有分工一致。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverText {
    /// ① 坐标行（[`tx::HOVER_COORD_TMPL`] 已填好，坐标带千分位，如 `单元格 1,234,261`）。
    pub coords: String,
    /// ② 该格是否可行走（[`Cell::is_walkable`] 的真值）。
    pub walkable: bool,
    /// ② 的状态词（[`tx::HOVER_WALKABLE`] / [`tx::HOVER_NOT_WALKABLE`]）。
    pub walkable_text: &'static str,
    /// ③ 六组明细，每组一条「标题: 值」（顺序与 [`tx::INSPECTOR_KEYS`] 一致）。
    pub details: Vec<String>,
}

/// 取一格的悬停浮层内容。
///
/// # 「可行走」的真值 = [`Cell::is_walkable`]，与上游的红色判定**等价**
///
/// 上游 `_upstream/CrystalStudio/MainWindow.CellHover.cs:59` 判 `(cell.BackImage & 0x60000000) != 0`。
/// 那两个位不是磁盘字节，而是 C# 读 12/14 B 档时**归一出来的**：
///
/// | 磁盘位 | C# 归一 | 出处 |
/// |---|---|---|
/// | 背景层 `bk_img & 0x8000` | `BackImage \|= 0x20000000`（并清掉 `0x8000`） | `_upstream/CrystalStudio/LegacyMapReader.cs:149-150` |
/// | 前景层 `fr_img & 0x8000` | `BackImage \|= 0x40000000` | `_upstream/CrystalStudio/LegacyMapReader.cs:160-161` |
///
/// ⇒ `(BackImage & 0x60000000) != 0` ⇔ 「背景层带 `0x8000` **或** 前景层带 `0x8000`」
/// ⇔ `!Cell::is_walkable()`（`mir2-map/src/cell.rs`：两层都不带 `0x8000` 才叫可走）。
/// 所以本函数直接用 `is_walkable()`，**不另造一套阻挡判据**。
pub fn hover_text(x: u16, y: u16, cell: &Cell, lines: &CellLines) -> HoverText {
    let walkable = cell.is_walkable();
    let details = tx::INSPECTOR_KEYS
        .iter()
        .zip(lines.values.iter())
        .map(|(key, value)| {
            tx::fill(
                tx::HOVER_DETAIL_TMPL,
                &[("title", key), ("value", &hover_detail_text(value))],
            )
        })
        .collect();
    // 坐标行按上游 `_upstream/CrystalStudio/MainWindow.CellHover.cs:58` 的 `{0:N0}` 走千分位。
    // `tx::fill` 只做 `{name}` 字面替换、**不认格式说明符**（见 `ui_text::fill` 的文档），
    // 所以先格成字符串再填 —— 格式化的决定点仍然只有一个（[`千分位`]）。
    let xs = 千分位(i64::from(x));
    let ys = 千分位(i64::from(y));
    HoverText {
        coords: tx::fill(tx::HOVER_COORD_TMPL, &[("x", &xs), ("y", &ys)]),
        walkable,
        walkable_text: if walkable {
            tx::HOVER_WALKABLE
        } else {
            tx::HOVER_NOT_WALKABLE
        },
        details,
    }
}

/// 按 .NET `N0` 的规则把整数格成**带千分位分隔符**的字符串（上游 `_upstream/CrystalStudio/MainWindow.CellHover.cs:58`）。
///
/// 上游原文：`LocalText.Format("Cell {0:N0}, {1:N0}", x, y)`；`_upstream/CrystalStudio/LocalText.cs:56` 把它落到
/// `string.Format(CultureInfo.GetCultureInfo(Language), …)` ⇒ 标准数字格式 `N` + 精度 `0`：
///
/// | 输入 | 输出 | 规则 |
/// |---|---|---|
/// | `0` / `1` / `999` | `0` / `1` / `999` | 不足三位不分组 |
/// | `1000` | `1,000` | 每三位一个 `,` |
/// | `1234` | `1,234` | |
/// | `999999` | `999,999` | |
/// | `-5` / `-1234` | `-5` / `-1,234` | 负号在分组**之外**、紧贴首位数字 |
/// | `i64::MIN` | `-9,223,372,036,854,775,808` | 不取绝对值 ⇒ 不溢出、不 panic |
///
/// 分组符固定 `,`、组距固定 3：本仓界面语言只有 `zh-CN` / `en-US` 两套
/// （`menu::model::Lang`），两者的 `N0` 都是 `,` + 3 位 ⇒ 与上游等价。
/// 将来若加 `de-DE` 这类用 `.` 分组的语言，这里得跟 culture 走（上游是 culture-aware）。
///
/// ⚠ 不走 `tx::fill` 的格式说明符：`fill` 只做 `{name}` 字面替换，不认 `N0`。
pub fn 千分位(v: i64) -> String {
    let s = v.to_string();
    // 负号单独摘出来：**绝不** `v.abs()`（`i64::MIN.abs()` 会溢出 panic）
    let (sign, digits) = match s.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", s.as_str()),
    };
    let n = digits.len();
    let mut out = String::with_capacity(s.len() + n / 3 + 1);
    out.push_str(sign);
    for (i, ch) in digits.char_indices() {
        // 从右往左每满三位插一个分组符（首位之前不插）
        if i > 0 && (n - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// 把常驻检查器的**多行值**压成一行（供悬停浮层用；理由见 [`tx::HOVER_DETAIL_JOIN`]）。
///
/// 规则：按 `\n` 切段 → 每段去尾部空白 → 去段尾的 ` ·`（模板里 `低位阻挡{low} ·\n…`
/// 就是这么收尾的）→ 去尾部空白 → 丢掉空段 → 用 `" · "` 连起来。
///
/// 为什么"去段尾 ` ·`"这一步不能省：省了会拼出 `低位阻挡否 · · 高位阻挡否`（双分隔符），
/// 单测 `悬停明细不出现双分隔符` 钉住这一条。
pub fn hover_detail_text(value: &str) -> String {
    value
        .split('\n')
        .map(|seg| seg.trim().trim_end_matches('·').trim())
        .filter(|seg| !seg.is_empty())
        .collect::<Vec<_>>()
        .join(tx::HOVER_DETAIL_JOIN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_assets::{Family, Frame};

    /// 假帧源：按族给出固定尺寸，便于断言"帧尺寸是问来的，不是编的"。
    struct 假源;

    impl SpriteSource for 假源 {
        fn frame(&self, lib: LibRef, index: u32) -> Option<Frame> {
            // 地表 96×64（覆盖 2×2 格）；中层 48×32；表层按族给 48×64（高于一格）
            let (w, h) = match lib.family {
                Family::Ground => (96, 64),
                Family::Middle => (48, 32),
                Family::Surface => (48, 64),
            };
            // 帧号 0 故意返回 None：用来验证"取不到帧 ⇒ —"
            if index == 0 {
                return None;
            }
            Some(Frame {
                width: w,
                height: h,
                rgba: vec![0; (w * h) as usize * 4],
                // 本测试只验帧尺寸是"问来的"，与帧锚点无关 ⇒ 其余字段（含 anchor）走 Default，
                // 将来 Frame 再加字段时这里不必再改（见 mir2-assets/src/frame.rs 的 Default 文档）。
                ..Frame::default()
            })
        }
    }

    /// 空悬停：标题与六个值全是 `—`，且模板占位符全部被替换（不残留 `{`）。
    #[test]
    fn 无悬停时全是破折号() {
        let l = CellLines::unknown();
        assert_eq!(l.head, tx::VAL_UNKNOWN);
        for v in &l.values {
            assert!(!v.contains('{'), "模板占位符没填干净：{v:?}");
            assert!(v.contains(tx::VAL_UNKNOWN), "空值行里应出现 —：{v:?}");
        }
    }

    /// 一个三层齐全、带阻挡位的格：逐字段核对。
    #[test]
    fn 三层齐全的格逐字段正确() {
        let c = Cell {
            bk_img: 0x8000 | 7,  // 低位阻挡 + 图号 7
            mid_img: 3,          // 图号 3
            fr_img: 0x8000 | 9,  // 高位阻挡 + 图号 9
            door_index: 2,
            door_offset: 5,
            ani_frame: 4,
            ani_tick: 6,
            area: 11,   // 表层库号
            light: 13,
            tiles: 1,   // 地表库号
            sm_tiles: 2, // 中层库号
            tail: [0; 22],
        };
        let l = cell_lines(380, 310, Some(&c), CellFormat::New14, Some(&假源));
        assert_eq!(l.head, "单元格 380,310 · 3 个物体");
        assert_eq!(l.values[0], "低位阻挡有 ·\n高位阻挡有");
        assert_eq!(l.values[1], "图号 7\n库号 1\n帧 96×64");
        assert_eq!(l.values[2], "图号 3\n库号 2\n帧 48×32");
        // 表层：帧号 9（1-based）⇒ 索引 8 ≠ 0 ⇒ 取到 48×64；`btArea` 叫「表层库号」
        assert_eq!(l.values[3], "图号 9\n表层库号 11\n帧 48×64");
        assert_eq!(l.values[4], "光照 13 · 门 2\n偏移 5");
        // 官方把 Frames/Tick 译作「帧数/间隔」（`0Frames1Tick2_DFC23F91`），照抄
        assert_eq!(l.values[5], "帧数 4 ·\n间隔 6");
    }

    /// **审计 E2**：`btArea`（off10）不许再叫「区域」，也不许被两个条目各显示一次。
    ///
    /// 这条是"必红"级回归：旧实现的 `values[5]` 是 `帧序 4 ·\n节拍 6 ·\n区域 11`（旧称），
    /// 其中 `11` 与 `values[3]` 的「库号 11」是**同一个字节**（`Cell::area`），
    /// 而且在同一个面板里被叫成了「区域」。任一条恢复旧行为，下面的断言就失败。
    #[test]
    fn 表层库号不许叫区域也不许重复显示() {
        let c = Cell {
            bk_img: 7,
            fr_img: 9,
            ani_frame: 4,
            ani_tick: 6,
            area: 40, // ← 表层资源库号（`btArea`），**唯一**语义
            tiles: 1,
            sm_tiles: 2,
            ..Default::default()
        };
        let l = cell_lines(1, 2, Some(&c), CellFormat::New14, Some(&假源));

        // ① 叫对名字：物体行必须点名「表层库号」
        assert!(
            l.values[3].contains("表层库号 40"),
            "btArea 必须显示为「表层库号」：{:?}",
            l.values[3]
        );
        // ② 不许再出现「区域」这个词（错标签）
        for v in &l.values {
            assert!(!v.contains("区域"), "面板里不许再出现「区域」：{v:?}");
        }
        // ③ 同一个字节只许出现一次：`area=40` 只能在物体行出现
        let hits = l.values.iter().filter(|v| v.contains("40")).count();
        assert_eq!(hits, 1, "0x0A 这一个字节只许显示一次，实际 {} 处", hits);
        // ④ 「图块动画」只剩两个真字段（官方译名：帧数 / 间隔）
        assert_eq!(l.values[5], "帧数 4 ·\n间隔 6");
    }

    /// **文案表里也不许再有「区域」**（防它从别的常量里爬回来）。
    ///
    /// 验收门要求「全 gui crate grep 区域 为 0（注释除外）」；这条把它变成一条能跑的门禁。
    #[test]
    fn 文案表里没有区域二字() {
        for (name, text) in tx::ALL {
            assert!(
                !text.contains("区域"),
                "文案 {name} 里还有「区域」（btArea 是表层库号，不是区域）：{text:?}"
            );
        }
    }

    /// 阻滞位为 0、图号为 0 的空格：图号照实写 0，帧写 `—`（不写尺寸）。
    #[test]
    fn 空格照实写零不写尺寸() {
        let c = Cell::default();
        let l = cell_lines(1, 2, Some(&c), CellFormat::New14, Some(&假源));
        assert_eq!(l.head, "单元格 1,2 · 0 个物体");
        assert_eq!(l.values[0], "低位阻挡无 ·\n高位阻挡无");
        assert_eq!(l.values[1], "图号 0\n库号 0\n帧 —");
    }

    /// Old12 档没有库号字节 ⇒ 库号必须是 `—`（**不能是 0**，那是"0 号库"的意思）。
    #[test]
    fn old12档库号为空() {
        let c = Cell {
            bk_img: 2, // 2（1-based）⇒ 索引 1 ≠ 0 ⇒ 假源给 96×64
            ..Default::default()
        };
        let l = cell_lines(0, 0, Some(&c), CellFormat::Old12, Some(&假源));
        assert_eq!(l.values[1], "图号 2\n库号 —\n帧 96×64");
    }

    /// 帧源缺失（资源库装载失败）⇒ 帧尺寸全是 `—`，其它字段照常。
    #[test]
    fn 没有帧源时尺寸为空() {
        let c = Cell {
            bk_img: 1,
            tiles: 3,
            ..Default::default()
        };
        let l = cell_lines(0, 0, Some(&c), CellFormat::New14, None);
        assert_eq!(l.values[1], "图号 1\n库号 3\n帧 —");
    }

    /// 帧号越界/空帧（帧源返回 None）⇒ 帧也是 `—`。
    #[test]
    fn 取不到帧时尺寸为空() {
        let c = Cell {
            bk_img: 1, // 1-based ⇒ 索引 0 ⇒ 假源故意返回 None
            ..Default::default()
        };
        let l = cell_lines(0, 0, Some(&c), CellFormat::New14, Some(&假源));
        assert_eq!(l.values[1], "图号 1\n库号 0\n帧 —");
    }

    // -----------------------------------------------------------------------
    // 悬停浮层取数（视图 → 地砖信息悬浮）
    // -----------------------------------------------------------------------

    /// **三行内容逐条对照**：坐标行 + 可行走状态词 + 六组「标题: 值」。
    #[test]
    fn 悬停内容三块齐全() {
        let c = Cell {
            bk_img: 2,
            mid_img: 3,
            fr_img: 4,
            area: 5,
            light: 1,
            ..Default::default()
        };
        let l = cell_lines(265, 261, Some(&c), CellFormat::New14, Some(&假源));
        let h = hover_text(265, 261, &c, &l);
        assert_eq!(h.coords, "单元格 265,261", "坐标行照 `HOVER_COORD_TMPL`");
        assert!(h.walkable, "三层都没有 0x8000 ⇒ 可走");
        assert_eq!(h.walkable_text, tx::HOVER_WALKABLE);
        assert_eq!(h.details.len(), 6, "六组各一条（与 INSPECTOR_KEYS 同数）");
        for (i, key) in tx::INSPECTOR_KEYS.iter().enumerate() {
            let prefix = tx::fill(tx::HOVER_DETAIL_TMPL, &[("title", key), ("value", "")]);
            assert!(
                h.details[i].starts_with(&prefix),
                "第 {i} 行必须以「{prefix}」开头，实得 {:?}",
                h.details[i]
            );
            assert!(
                !h.details[i].contains('\n'),
                "浮层每组必须压成一行（否则 350px 宽的框会长到十几行）：{:?}",
                h.details[i]
            );
        }
        // 具体值也要对（不是只看前缀）：地表那组 = 假源的 96×64
        assert_eq!(h.details[1], "地表: 图号 2 · 库号 0 · 帧 96×64");
    }

    /// **「可行走」取 `Cell::is_walkable()` 的真值**，两层的高位都要能触发「不可行走」。
    ///
    /// 等价性依据见 [`hover_text`] 的文档（C# 把 `bk 0x8000` 归一成 `0x20000000`、
    /// 把 `fr 0x8000` 或进 `0x40000000`，`_upstream/CrystalStudio/MainWindow.CellHover.cs:59` 判的正是这两位）。
    #[test]
    fn 悬停可行走取两层高位真值() {
        let l = CellLines::unknown();
        // ① 两层都干净 ⇒ 可走
        let c = Cell::default();
        let h = hover_text(0, 0, &c, &l);
        assert!(h.walkable && h.walkable_text == tx::HOVER_WALKABLE);
        // ② 背景层带 0x8000 ⇒ 不可走
        let c = Cell {
            bk_img: 0x8001,
            ..Default::default()
        };
        let h = hover_text(0, 0, &c, &l);
        assert!(!h.walkable && h.walkable_text == tx::HOVER_NOT_WALKABLE);
        // ③ 前景层带 0x8000 ⇒ 同样不可走（只判背景层就漏这一半）
        let c = Cell {
            fr_img: 0x8002,
            ..Default::default()
        };
        let h = hover_text(0, 0, &c, &l);
        assert!(!h.walkable, "前景层 0x8000 也必须判成不可行走");
        // ④ 图号本身（去掉高位）不影响判定
        let c = Cell {
            bk_img: 0x0002,
            fr_img: 0x0003,
            ..Default::default()
        };
        assert!(hover_text(0, 0, &c, &l).walkable);
    }

    /// 多行值压成一行时**不许出现双分隔符**（`低位阻挡是 · · 高位阻挡否` 那种）。
    #[test]
    fn 悬停明细不出现双分隔符() {
        // 真模板：`低位阻挡{low} ·\n高位阻挡{high}`（第一段以 ` ·` 收尾）
        assert_eq!(hover_detail_text("低位阻挡是 ·\n高位阻挡否"), "低位阻挡是 · 高位阻挡否");
        assert_eq!(hover_detail_text("图号 1\n库号 2\n帧 3"), "图号 1 · 库号 2 · 帧 3");
        // 空段被丢掉、首尾空白被去掉：不产生 `a ·  · b`
        assert_eq!(hover_detail_text(" a \n\n b "), "a · b");
        assert_eq!(hover_detail_text(""), "");
        assert!(!hover_detail_text("低位阻挡是 ·\n高位阻挡否").contains("· ·"));
    }

    // -----------------------------------------------------------------------
    // 千分位（A16：上游 `_upstream/CrystalStudio/MainWindow.CellHover.cs:58` 的 `{0:N0}`）
    // -----------------------------------------------------------------------

    /// **逐值对照 .NET `N0`**：覆盖派单要求的全部边界
    /// （`0` / `1` / `999` / `1000` / `1234` / `999999` / 负数）。
    ///
    /// 右列是 .NET `v.ToString("N0", zh-CN | en-US)` 的输出（两套 culture 的分组符与组距相同）。
    #[test]
    fn 千分位对齐dotnet的n0() {
        for (v, want) in [
            (0i64, "0"),
            (1, "1"),
            (9, "9"),
            (12, "12"),
            (123, "123"),
            (999, "999"),
            (1000, "1,000"),
            (1234, "1,234"),
            (12345, "12,345"),
            (32767, "32,767"),
            (999_999, "999,999"),
            (1_000_000, "1,000,000"),
            (-1, "-1"),
            (-5, "-5"),
            (-999, "-999"),
            (-1000, "-1,000"),
            (-1234, "-1,234"),
            (-999_999, "-999,999"),
        ] {
            assert_eq!(千分位(v), want, "千分位({v}) 与 .NET N0 不符");
        }
    }

    /// `i64` 两端极值：**不 panic、不溢出**。
    ///
    /// `i64::MIN.abs()` 会溢出 panic，所以实现里不取绝对值、只摘负号。
    #[test]
    fn 千分位极值不崩() {
        assert_eq!(千分位(i64::MAX), "9,223,372,036,854,775,807");
        assert_eq!(千分位(i64::MIN), "-9,223,372,036,854,775,808");
        // 19 位数字 ⇒ 6 个分组符（19 = 3 × 6 + 1）
        assert_eq!(千分位(i64::MAX).matches(',').count(), 6);
        assert_eq!(千分位(i64::MIN).matches(',').count(), 6);
    }

    /// 坐标行**端到端**带千分位（`hover_text` → `HOVER_COORD_TMPL`），
    /// 且 `< 1000` 的坐标逐字不变 —— 这正是「A16 不该改动基线截图」的依据。
    #[test]
    fn 悬停坐标行走千分位() {
        let l = CellLines::unknown();
        let c = Cell::default();
        // 265 / 261 都 < 1000 ⇒ 与 A16 之前逐字相同
        assert_eq!(hover_text(265, 261, &c, &l).coords, "单元格 265,261");
        // 1000 以上才出现分组符
        assert_eq!(hover_text(1000, 0, &c, &l).coords, "单元格 1,000,0");
        assert_eq!(hover_text(1234, 5678, &c, &l).coords, "单元格 1,234,5,678");
        // 地图尺寸上限 32767 也必须分组
        assert_eq!(hover_text(32767, 32767, &c, &l).coords, "单元格 32,767,32,767");
    }
}
