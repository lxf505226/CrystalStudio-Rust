//! `GET /api/stamps` —— 印章目录（`mir2-stamp` 的 9,159 条 + 中文分类）。
//!
//! # 一、返回什么
//!
//! ```json
//! {"items":[{"id":"mir/mir_00000","name":"…","kind":"Building","kind_zh":"建筑",
//!            "cells":12,"bounds":[0,0,5,3]}],"total":25}
//! ```
//!
//! | 字段 | 来源 | 说明 |
//! |---|---|---|
//! | `id` | `palette.json` 的 `id` | 主键 |
//! | `name` | 同上 `name` | 显示名 |
//! | `kind` | 同上 `type` | 英文分类（`null` = 未分类） |
//! | `kind_zh` | `mir2_stamp::kind_label_zh` | 中文分类（`null` ⇒ 「未分类」） |
//! | `cells` | `StampCatalog::resolve` 解析 `.X` 得到的格数 | **读盘失败 ⇒ `null`** |
//! | `bounds` | 同上，`[min_x,min_y,max_x,max_y]` 闭区间 | 同上 |
//!
//! # 二、`q` 与 `kind` 的组合口径
//!
//! - `q` 非空 ⇒ 走 `StampCatalog::search`（只匹配 `id` / `name` 的子串，大小写不敏感）；
//! - `kind` 非空 ⇒ 再按分类过滤（**精确、大小写不敏感**）；
//! - 两者都空 ⇒ 全量（9,159 条，靠分页取）。
//! - `total` 是**过滤之后**的总数（前端据此算页数）。
//!
//! # 三、`cells` / `bounds` 为什么要读盘
//!
//! 这两个值只能从 `.X` 文件里解析出来（`mir2-stamp` 的 `resolve`），**没有就填 `null`**——
//! 绝不按"大概 5×4 格"编一个。`resolve` 内部有缓存（同一 id 只读一次盘），
//! 所以翻页不会反复读同一批文件。
//!
//! # 四、`load_stamp_cells`：给 `POST /api/edit {"PlaceStamp":…}` 用的格表加载器
//!
//! 分工（契约裁定）：**web 负责读盘，core 负责换算**。
//!
//! ```text
//! web:  StampCatalog::resolve(id) → 路径 → 读 .X → Stamp::parse
//!       → miredit_core::cells_from_stamp（**全仓唯一**的换算口）
//!       → session.register_stamp(id, cells) → apply(Command::PlaceStamp{ x, y, stamp: id })
//! core: cells_from_stamp / register_stamp / place_stamp_by_id / Command::PlaceStamp（零 IO）
//! ```
//!
//! ## 为什么本文件不再逐字段填 `StampCellInput`
//!
//! 2026-11-05 之前，这里自己写了一遍映射，其中三族库号是
//! `back_library as u8` / `middle_library as u8` / `foreground.library as u8`
//! —— **`i16 → u8` 的 `as` 是静默截断**：库号 `300` 会变成 `44`，
//! 贴出来的印章指向**另一个图库**，外观错、还不报错（正是本项目零容忍的「看起来成功」）。
//! 同一份映射当时在 GUI 与 core 测试里还各有一份，三份口径互不约束。
//!
//! 现在口径只有一处：`miredit-core/src/stamp_convert.rs` 的 `cells_from_stamp`
//! （落点 `stamp_convert.rs:154`）。本文件只剩「读字节 → 调 core → 翻错误」。
//!
//! | 项 | 口径（详见 `miredit_core::stamp_convert` 模块文档） |
//! |---|---|
//! | 三族图号 `bk` / `mid` / `fr` | 一律 `& 0x7FFF`（`bit15` 是阻挡标记，不是图号） |
//! | 三族库号 `bk_lib` / `mid_lib` / `fr_lib` | 必须落在 `0..=255`，否则 **`Err`**（绝不截断） |
//! | `door` / `door_offset` / `light` / 动画五字段 | 逐字段照搬 `.X` 真值（不填缺省 0） |
//!
//! ⚠ 本层**连掩码都不做**：`back_image` 的高位（C# 的 `0x60000000` 标记位）与 `bit15`
//! 都由 core 处理。在这里再掩一次就是**第二份口径**，迟早与 core 漂移。
//!
//! ⚠ **口径变更（显式记录）**：改前 `bk` 保留 `bit15`（`as u16` 不掩），改后按 core 掩掉。
//! 依据 `stamp_convert.rs` 模块文档 §三：`bit15` 在 CrystalStudio 里是阻挡位，
//! 与 `crate::paint` 的写入口径（`v & 0x7FFF`）对齐才不会把标记位当图号写进地图。
//! 测试 `tests::地表图号高位由内核掩掉` 钉住这条。

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::Json;
use mir2_stamp::{kind_label_zh, StampEntry};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::error::{ApiError, ApiResult};
use crate::routes::Params;
use crate::state::Shared;

/// 每页缺省条数（前端 `web/README.md` §3.8 的「每页 100 条」）。
const DEFAULT_PAGE_SIZE: u32 = 100;
/// 每页上限：一条 item 可能要解析一个 `.X` 文件，别让一次请求解析上千个。
const MAX_PAGE_SIZE: u32 = 200;

/// `GET /api/stamps?kind=&q=&page=&size=`。
pub async fn stamps(
    State(st): State<Shared>,
    q: Result<Query<HashMap<String, String>>, QueryRejection>,
) -> ApiResult<Json<Value>> {
    let p = Params::from_query(q)?;
    let kind = p.raw("kind").map(str::to_ascii_lowercase);
    let needle = p.raw("q").map(str::to_string);
    let page = p.u32_or("page", 0)?;
    let size = p.u32_or("size", DEFAULT_PAGE_SIZE)?;
    if size == 0 || size > MAX_PAGE_SIZE {
        return Err(ApiError::bad_request(format!(
            "size 必须在 1..={MAX_PAGE_SIZE} 之间，收到：{size}"
        )));
    }

    // 印章目录是懒加载的（首次请求才读 5 MB JSON）；失败 ⇒ 503，错误文本原样带出
    st.with_stamps(|catalog| {
        // ① 先按关键词（若有）取候选集；② 再按分类过滤；③ 只把当前页的「id + 展示字段」
        //    克隆出来（**不跨借用**：后面 `resolve` 要 &mut catalog）
        let (total, page_rows) = {
            let mut hits: Vec<&StampEntry> = match needle.as_deref() {
                Some(n) => catalog.search(n),
                None => catalog.all().iter().collect(),
            };
            if let Some(k) = &kind {
                hits.retain(|e| {
                    e.kind
                        .as_deref()
                        .is_some_and(|own| own.eq_ignore_ascii_case(k))
                });
            }
            let total = hits.len() as u64;
            let skip = (page as usize).saturating_mul(size as usize);
            let rows: Vec<(String, String, Option<String>)> = hits
                .into_iter()
                .skip(skip)
                .take(size as usize)
                .map(|e| (e.id.clone(), e.name.clone(), e.kind.clone()))
                .collect();
            (total, rows)
        };

        // 只有当前页才解析 `.X`（见模块文档第三节）
        let mut items = Vec::with_capacity(page_rows.len());
        for (id, name, kind_en) in page_rows {
            let resolved = catalog.resolve(&id);
            let kind_zh = match kind_en.as_deref() {
                Some(k) => kind_label_zh(k).map(str::to_string),
                // JSON 里 kind 为 null ⇒ 库里按空串定义为「未分类」
                None => kind_label_zh("").map(str::to_string),
            };
            items.push(json!({
                "id": id,
                "name": name,
                "kind": kind_en,
                "kind_zh": kind_zh,
                "cells": resolved.as_ref().map(|r| r.cell_count),
                "bounds": resolved.as_ref().map(|r| {
                    let (a, b, c, d) = r.bounds;
                    json!([a, b, c, d])
                }),
            }));
        }

        Ok(Json(json!({"items": items, "total": total})))
    })
}

/// 把一枚印章 id 解析成**内核要的格表**（`Vec<miredit_core::StampCellInput>`）。
///
/// 四条失败路径，**各自给不同状态码**（不混成一个 500）：
///
/// | 情况 | 状态码 | 理由 |
/// |---|---|---|
/// | 印章目录不可用（`palette.json` 读不了） | **503** `assets_unavailable` | 资源没就绪，不是调用方的错 |
/// | id 不在目录里 | **404** `not_found` | 调用方给的 id 不存在（附检索提示） |
/// | `.X` 读不了 / 解析失败 | **500** `io_error` / **422** `parse_failed` | 目录说它在、但文件坏了 |
/// | 解析成功但某族库号越界 | **422** `parse_failed` | 文件内容地图侧装不下，见下 |
///
/// 最后一条由 [`ApiError::from_stamp_convert`] 产出：`message` 是 core 的原文
/// （形如「第 1 格 / 地表 / 值 300 超出库号字节范围 0..=255（拒绝截断…）」）。
/// 它与上一条共用 422/`parse_failed` —— 前端码表里 422 只有这一个码（见 `error.rs` 第四节）。
///
/// 已注册过的印章不会走到这里（调用方先查 `session.stamp_cells(id)`），
/// 所以同一枚印章在一次会话里只读一次盘。
pub fn load_stamp_cells(
    st: &Shared,
    id: &str,
) -> ApiResult<Vec<miredit_core::StampCellInput>> {
    // ① 目录里查 id（`resolve` 自带缓存；它只 stat 文件、不读内容）
    let resolved = st.with_stamps(|catalog| Ok(catalog.resolve(id)))?;
    let resolved = resolved.ok_or_else(|| {
        ApiError::not_found(format!(
            "印章 id 不存在：{id}（用 GET /api/stamps?q=… 检索可用 id）"
        ))
    })?;

    // ② 读 `.X`（几十 KB 级，阻塞读盘放到 worker 之外由调用方决定时机）
    let bytes = std::fs::read(&resolved.path).map_err(|e| {
        ApiError::io_error(format!(
            "印章文件读取失败（{id} · {}）：{e}",
            resolved.path.display()
        ))
    })?;

    // ③ 解析（v0/v1/v2 都收）—— 失败是**文件坏了**，不是调用方的输入错
    let stamp = mir2_stamp::Stamp::parse(&bytes).map_err(|e| {
        ApiError::parse_failed(format!(
            "印章 .X 解析失败（{id} · {}）：{e}",
            resolved.path.display()
        ))
    })?;

    // ④ 换算成内核的格表 —— 字段映射与库号号段换算全在 core，这里只翻错误
    to_stamp_inputs(&stamp, st.号段)
}

/// 薄封装：`mir2_stamp::Stamp` → 内核格表，错误翻成 HTTP（**纯函数**，便于单测钉住）。
///
/// **本函数一个字段都不搬** —— 映射口径全在 [`miredit_core::cells_from_stamp`]
/// （`miredit-core/src/stamp_convert.rs:154`）。这里只做「core 错误 → [`ApiError`]」一步，
/// 保留它是为了给调用方一个「拿到就是 HTTP 错误」的入口（`edit.rs` 的 `prepare_stamp` 用 `?` 直出）。
///
/// 库号越界 ⇒ [`ApiError::from_stamp_convert`] 给的 **422**；改前的 `as u8` 会静默截断，见模块文档第四节。
pub fn to_stamp_inputs(
    stamp: &mir2_stamp::Stamp,
    号段: miredit_core::库号号段,
) -> ApiResult<Vec<miredit_core::StampCellInput>> {
    miredit_core::cells_from_stamp(stamp, 号段).map_err(ApiError::from_stamp_convert)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_stamp::{Front, StampCell, StampCellData};

    /// 格表映射**经 core** 走通：字段值仍逐条对照 `miredit-core/src/stamp.rs` 的
    /// `StampCellInput` 文档表（本文件已不搬字段，这条钉的是「web 真的调了 core」）。
    ///
    /// 用 `Stamp::to_bytes()` → `Stamp::parse()` 走一遍真序列化，
    /// 保证测的是**真 `.X` 语义**，不是我手搓的结构体（不造假数据）。
    #[test]
    fn 印章格表映射() {
        let stamp = mir2_stamp::Stamp {
            version: 2,
            cells: vec![
                StampCell {
                    x: -1,
                    y: 2,
                    cell: StampCellData {
                        // A-024：库号是 **C# 全局分配号**，必须在 Wemade 号段内
                        // （地表 0 / 中层 1 / 物体 2..=99），换算后才是族内页号
                        back_library: 0,
                        back_image: 1234,
                        middle_library: 1,
                        middle_image: 567,
                        foreground: Some(Front {
                            library: 5,
                            image: 89,
                            frames: 1,
                            tick: 2,
                        }),
                        door: 7,
                        door_offset: 9,
                        light: 11,
                        middle_frames: 13,
                        middle_tick: 15,
                        tile_animation: 17,
                        tile_offset: 19,
                        tile_frames: 21,
                    },
                },
                // 无前景的格：`fr` / `fr_lib` 必须是 0（= 该族没内容，不是"写 0"）
                StampCell {
                    x: 0,
                    y: 0,
                    cell: StampCellData {
                        back_library: 0,
                        back_image: 0,
                        middle_library: 1,
                        middle_image: 42,
                        foreground: None,
                        ..Default::default()
                    },
                },
            ],
        };
        let bytes = stamp.to_bytes();
        let parsed = mir2_stamp::Stamp::parse(&bytes).expect("自造 .X 应能解析");
        let cells = to_stamp_inputs(&parsed, miredit_core::库号号段::Wemade)
            .expect("样本库号应落在 Wemade 号段内");
        assert_eq!(cells.len(), 2);

        let a = cells[0];
        assert_eq!((a.dx, a.dy), (-1, 2), "dx/dy 取 StampCell.x/y（可负）");
        assert_eq!(a.bk, 1234);
        // 库号：`.X` 全局号 0/1/5 ⇒ 页 0/0/3（A-024：不是原样透传）
        assert_eq!(a.bk_lib, 0, "地表全局号 0 → Tiles 页 0");
        assert_eq!(a.mid, 567);
        assert_eq!(a.mid_lib, 0, "中层全局号 1 → SmTiles 页 0");
        assert_eq!(a.fr, 89, "物体图号取 foreground.image");
        assert_eq!(a.fr_lib, 3, "物体全局号 5 → Objects 页 3");
        // 附加字段：`.X` 里的真数据必须逐字段搬过去，不许落成缺省 0
        assert_eq!((a.door, a.door_offset, a.light), (7, 9, 11), "门/门偏移/光照原样搬");
        assert_eq!((a.ani_frame, a.ani_tick), (1, 2), "前景动画帧/节拍取 Front.frames/tick");
        assert_eq!((a.mid_ani_frame, a.mid_ani_tick), (13, 15), "中层动画帧/节拍取 middle_*");
        assert_eq!(
            (a.tile_animation, a.tile_offset, a.tile_frames),
            (17, 19, 21),
            "地表动画三字段原样搬"
        );

        let b = cells[1];
        assert_eq!((b.bk, b.bk_lib), (0, 0));
        assert_eq!((b.fr, b.fr_lib), (0, 0), "无前景 ⇒ 0（内核据此不写该族）");
        assert_eq!((b.mid, b.mid_lib), (42, 0), "中层全局号 1 → SmTiles 页 0");
        assert_eq!(
            (b.ani_frame, b.ani_tick),
            (0, 0),
            "无前景 ⇒ 前景动画帧/节拍只能是 0（Front 不存在）"
        );
    }

    /// `back_image` 的高位（C# 编辑器标记位 `0x2000_0000`）与 `bit15`（阻挡标记）
    /// **都不进 `bk`** —— 由内核统一掩 `0x7FFF`，本文件不再自己截。
    ///
    /// ⚠ 口径变更（显式记录）：改前本文件写 `as u16`，`bit15` 会保留（旧断言是 `0x8000 | 7`）；
    /// 改后按 `miredit_core::stamp_convert` 模块文档 §三 掩掉。依据：`bit15` 是阻挡位不是图号，
    /// 必须与 `crate::paint` 的写入口径（`v & 0x7FFF`）对齐。
    #[test]
    fn 地表图号高位由内核掩掉() {
        let stamp = mir2_stamp::Stamp {
            version: 2,
            cells: vec![StampCell {
                x: 0,
                y: 0,
                cell: StampCellData {
                    back_image: 0x2000_0000 | 0x8000 | 7,
                    ..Default::default()
                },
            }],
        };
        let cells = to_stamp_inputs(&stamp, miredit_core::库号号段::Wemade).expect("库号全 0，应转换成功");
        assert_eq!(
            cells[0].bk, 7,
            "高位标记位与 bit15 都不进 bk，只剩纯图号 7"
        );
    }

    /// 库号 `300` ⇒ web 侧 **422**，`message` 是 core 原文且含 `300`。
    ///
    /// 这条是「消掉截断」的验收点：改前 `back_library as u8` 会把 `300` 静默变成 `44`，
    /// 于是**拿到一份成功的格表**、贴出来指向另一个图库 —— 本条钉住它不再发生。
    #[test]
    fn 库号三百返回422且消息含三百() {
        let stamp = mir2_stamp::Stamp {
            version: 2,
            cells: vec![StampCell {
                x: 0,
                y: 0,
                cell: StampCellData {
                    // `i16` 装得下 300，地图侧单字节装不下 —— 正是必须报错的那一档
                    back_library: 300,
                    back_image: 7,
                    ..Default::default()
                },
            }],
        };

        let err = to_stamp_inputs(&stamp, miredit_core::库号号段::Wemade)
            .expect_err("库号 300 不在号段内，必须拒绝");
        assert_eq!(
            err.status(),
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "库号越界必须是 422"
        );
        assert_eq!(err.code(), "parse_failed", "422 沿用现有的错误码，不新造码");

        let msg = err.message();
        assert!(msg.contains("300"), "message 必须含原始值 300：{msg}");

        // 「原样透传」自证：与 core 的 `Display` **逐字**相同。
        // 不硬编码整句 —— core 改措辞时这条不该红，但 web 偷偷加前缀时必须红。
        let core_msg = miredit_core::cells_from_stamp(&stamp, miredit_core::库号号段::Wemade)
            .expect_err("core 侧同样应拒绝")
            .to_string();
        assert_eq!(msg, core_msg, "message 必须是 core 原文，web 不许二次包装");
        assert!(core_msg.contains("第 1 格"), "core 原文应含 1 起的格号：{core_msg}");
        assert!(core_msg.contains("地表"), "core 原文应含族名：{core_msg}");
    }

    // -----------------------------------------------------------------------
    // A-024：库号是**全局 id**，落到地图字节前必须换算成族内页号
    // -----------------------------------------------------------------------

    /// 取证根：`MIREDIT_UPSTREAM_ROOT` → 缺省 `I:\miredit\_upstream`；根不存在 ⇒ `None`。
    fn 取证根() -> Option<std::path::PathBuf> {
        let root = std::env::var_os("MIREDIT_UPSTREAM_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(r"I:\miredit\_upstream"));
        root.is_dir().then_some(root)
    }

    /// 真实印章 `mir_00000.X`（25 格）—— 缺根/缺文件 ⇒ `None`（调用方跳过，不 panic）。
    fn 真实印章() -> Option<mir2_stamp::Stamp> {
        let p = 取证根()?.join("Objects").join("mir").join("mir_00000.X");
        if !p.is_file() {
            eprintln!("[跳过] 缺文件 {}", p.display());
            return None;
        }
        let bytes = std::fs::read(&p).expect("读得到 .X");
        Some(mir2_stamp::Stamp::parse(&bytes).expect("解析得了 .X"))
    }

    /// 合成一张 `w×h` 的 `格长` 档图，做成**会话**（`EditSession` 自己持有 `Document`，
    /// `place_stamp_by_id` 改的是会话里的那份视图 ⇒ 断言直接读 `ed.document().to_bytes()`）。
    fn 会话图(w: u16, h: u16, 格长: usize) -> miredit_core::EditSession {
        let mut bytes = vec![0u8; mir2_map::HEADER_LEN + 格长 * (w as usize * h as usize)];
        bytes[0..2].copy_from_slice(&w.to_le_bytes());
        bytes[2..4].copy_from_slice(&h.to_le_bytes());
        let doc = miredit_core::Document::from_bytes(
            std::path::Path::new(r"E:\__stamp_web_test__.map"),
            bytes,
        )
        .expect("合成图应能建文档");
        miredit_core::EditSession::new(doc)
    }

    /// **真实 `.X`（25 格）经 web 换算后三族库号全是「页 0」**。
    ///
    /// 同时钉住 A-024 的病根：`.X` 原文是全局号 `0 / 1 / 2`，
    /// 若原样透传，`mid_lib` 会是 `1`、`fr_lib` 会是 `2`（→ `SmTiles2` / `Objects3`）。
    #[test]
    fn 真实印章库号换算成页0() {
        let Some(stamp) = 真实印章() else {
            return;
        };
        let cells = to_stamp_inputs(&stamp, miredit_core::库号号段::Wemade)
            .expect("真实印章应能在 Wemade 号段内换算");
        assert_eq!(cells.len(), 25, "mir_00000.X 是 25 格");

        // ① `.X` 原文（全局分配号）
        // 地表 / 中层库号每格都有；前景库号只有带前景块的格才有（其余是 `None`）
        assert!(
            stamp.cells.iter().all(|c| c.cell.back_library == 0),
            "本样本地表库号原文全是 0（Tiles）"
        );
        assert!(
            stamp.cells.iter().all(|c| c.cell.middle_library == 1),
            "本样本中层库号原文全是 1（SmTiles）"
        );
        let 前景原文: std::collections::BTreeSet<i16> = stamp
            .cells
            .iter()
            .filter_map(|c| c.cell.foreground.map(|f| f.library))
            .collect();
        assert_eq!(
            前景原文,
            std::collections::BTreeSet::from([2]),
            "带前景的 8 格库号原文全是 2（Objects）"
        );

        // ② 换算后的页号（落进地图字节的就是这三个）
        assert!(
            cells
                .iter()
                .all(|c| (c.bk_lib, c.mid_lib, c.fr_lib) == (0, 0, 0)),
            "三族全局号 0/1/2 ⇒ Tiles/SmTiles/Objects 的**页 0**"
        );
        // ③ 反向确认：不许再出现「原样透传」的 1 / 2
        assert!(
            cells.iter().all(|c| c.mid_lib != 1 && c.fr_lib != 2),
            "不许再把全局号 1/2 原样写进 btsmTiles/btArea"
        );
    }

    /// **落位到 12 字节档：8 个前景格的 `btArea`（off10）必须是 0**（A-024 核心验收）。
    ///
    /// 走的是 web 放置路径在 core 侧的那两个调用（`register_stamp` + `place_stamp_by_id`），
    /// 只看**落盘字节**，不看任何推导值。
    #[test]
    fn 落位12字节档的area字节为0() {
        let Some(stamp) = 真实印章() else {
            return;
        };
        let cells = to_stamp_inputs(&stamp, miredit_core::库号号段::Wemade).expect("换算");
        let mut ed = 会话图(16, 16, 12); // 12 B/格 = Old12
        ed.register_stamp("t".to_string(), cells).expect("注册应成功");
        // 包围盒 (-1,-2,4,3) + 锚点 (3,3) ⇒ 覆盖 x 2..=7 / y 1..=6
        let out = ed.place_stamp_by_id(3, 3, "t");
        assert_eq!(
            out,
            miredit_core::PlaceOutcome::Placed {
                cells: 8,
                skipped: 0
            },
            "纯物件印章改 8 格"
        );

        let raw = ed.document().to_bytes();
        let h = 16usize;
        let mut 前景 = 0;
        for x in 2..=7usize {
            for y in 1..=6usize {
                let off = mir2_map::HEADER_LEN + (x * h + y) * 12;
                let fr = u16::from_le_bytes([raw[off + 4], raw[off + 5]]);
                if fr & 0x7FFF == 0 {
                    continue;
                }
                前景 += 1;
                assert_eq!(
                    raw[off + 10],
                    0,
                    "({x},{y}) 的 `btArea` 必须是 0（Objects 页 0），不是全局号 2"
                );
            }
        }
        assert_eq!(前景, 8, "8 个前景格必须逐格验到");
    }

    /// **落位到 14 字节档：`btsmTiles` = 0 且 `btArea` = 0，无地表格的 `btTiles` 保持原值**。
    #[test]
    fn 落位14字节档的sm_tiles与area为0() {
        // 中层有图（全局 1）+ 前景有图（全局 2）的合成印章
        let stamp = mir2_stamp::Stamp {
            version: 2,
            cells: vec![StampCell {
                x: 0,
                y: 0,
                cell: StampCellData {
                    back_library: 0,
                    back_image: 0,
                    middle_library: 1,
                    middle_image: 5,
                    foreground: Some(Front {
                        library: 2,
                        image: 9,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            }],
        };
        let cells = to_stamp_inputs(&stamp, miredit_core::库号号段::Wemade).expect("换算");
        assert_eq!(
            (cells[0].bk_lib, cells[0].mid_lib, cells[0].fr_lib),
            (0, 0, 0),
            "三族全局号都换算成页 0"
        );

        let mut ed = 会话图(4, 4, 14); // 14 B/格 = New14
        // 先把目标格的 `btTiles` 设成 4，验「没有地表的层不许被写成 0」
        ed.document_mut().view_mut().cells[0].tiles = 4;
        ed.register_stamp("t".to_string(), cells).expect("注册应成功");
        assert_eq!(
            ed.place_stamp_by_id(0, 0, "t"),
            miredit_core::PlaceOutcome::Placed {
                cells: 1,
                skipped: 0
            }
        );

        let raw = ed.document().to_bytes();
        let off = mir2_map::HEADER_LEN; // (0,0) 是第一格
        assert_eq!(raw[off + 13], 0, "`btsmTiles` 必须是 0（SmTiles 页 0）");
        assert_eq!(raw[off + 10], 0, "`btArea` 必须是 0（Objects 页 0）");
        assert_eq!(
            raw[off + 12], 4,
            "本格没有地表图 ⇒ `btTiles` 保持原值 4（印章不写无图层的库号）"
        );
    }
}
