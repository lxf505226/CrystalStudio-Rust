//! `mir2-format` —— 传奇 M2 **九种地图格式**的模块化注册表（CrystalStudio 全量移植）。
//!
//! 本 crate 只做一件事：把任意一种 `.map` 解析成 [`RawMap`]，再**逐字节**写回。
//! 不猜字段语义、不做「顺手归一化」、不静默补零；解析失败一律 [`FormatError`]。
//!
//! # 一、九种格式一览（顺序 = C# `LegacyMapReader.cs:55-117` 判别链）
//!
//! | # | [`FormatId`] | C# 分支 | 头部 | 单格 | 真实样本 | 模块 |
//! |---|--------------|---------|------|------|----------|------|
//! | 1 | [`FormatId::CrystalCs`] | `LoadMapType100` | 8 | **变长** 26/38/35/33+ | ❌ 无 | [`formats::crystal_cs`] |
//! | 2 | [`FormatId::Mir3Wemade`] | `LoadMapType5` | 28 + 背景块 | 14 | ❌ 无 | [`formats::mir3_wemade`] |
//! | 3 | [`FormatId::Mir3Shanda`] | `LoadMapType6` | 40 | 20 | ❌ 无 | [`formats::mir3_shanda`] |
//! | 4 | [`FormatId::AntiHack`] | `LoadMapType4` | 64 | 12 | ❌ 无 | [`formats::antihack`] |
//! | 5 | [`FormatId::Wemade2010`] | `LoadMapType1` | 54 | 15 | ❌ 无 | [`formats::wemade2010`] |
//! | 6 | [`FormatId::Shanda2012`] | `LoadMapType3` | 52 | **36** | ✅ 195 个 | [`formats::shanda2012`] |
//! | 7 | [`FormatId::Shanda14`] | `LoadMapType2` | 52 | 14 | ✅ 107 个 | [`formats::shanda14`] |
//! | 8 | [`FormatId::Heroes34`] | `LoadMapType7` | 54 | 15 | ❌ 无 | [`formats::heroes34`] |
//! | 9 | [`FormatId::Old12`] | `LoadMapType0` | 52 | 12 | ✅ 856 个 | [`formats::old12`] |
//!
//! ## 「真实样本」一列的**两套口径**（2026-11-05 订正，依据审查报告 §三⑥）
//!
//! 样本总量 **1,158**：`E:\GodM2\GodotClient\maps\*.map`（61 个）+
//! `E:\GodM2\client\热血传奇23周年客户端\Map\*.map`（1,097 个），2026-11-04 实测。
//! 同一批文件，两套判别给出的分布**不同**，**必须分列、不可混用**：
//!
//! | 口径 | Old12 | Shanda14 | Shanda2012 | 无法严格判别 | 合计 |
//! |------|-------|----------|------------|--------------|------|
//! | **本 crate 判别**（上表这一列用的就是它） | **856**<br>（789 严格往返成功 + 67 只能走 `read_any_tolerant`） | **107** | **195** | 67（含在上面 856 里） | 1,158 |
//! | **C# 口径**（只看标题字节） | **869** | **94** | **195** | 0（C# 无此概念） | 1,158 |
//!
//! 两套口径在 **21 个文件**上分歧，**全部是 crate 更对**（审查报告 §三⑥）：
//!
//! - **17 个**：14 B/格，但标题首字节不是 `0x0F`（`10.map` 是 `0x07`、`33.map`/`4.map` 是 `0x03`、
//!   `42/58/64.map` 是 `0x0B`、`14.map` 是 `0x0D`、`20.map` 是 `0xFF` 等）⇒ C# 落兜底按 12 B 读成花图；
//! - **4 个**：12 B/格，但标题恰好命中 14 B 签名（`hell107/108/109.map`、`DX002_01.map`）⇒ C# 按 14 B 读穿。
//!
//! 净差额 = 17 − 4 = 13，正好等于 `869 − 856 = 94 − 107 = 13`（两侧对得上）。
//! ⚠ 那 67 个「长度不自洽」的文件在**两套口径里都被算进 Old12 桶**，它们**严格入口全部失败**
//! （`52 + 12×1050 = 12,652`，而文件是 20,692 字节），只能走 [`read_any_tolerant`]，
//! 且读出的 `trailer` 非空 ⇒ 上层**必须告警**（见 [`RawMap::trailer_len`]）。
//!
//! # 二、字节级往返的三条保证
//!
//! 1. **头部原样**：[`RawMap::header_raw`] 是「格数组之前的全部字节」，写回时逐字节照抄；
//! 2. **未知尾区原样**：[`Cell::tail`] 装格内未建模字节，[`RawMap::trailer`] 装格数组之后
//!    剩余的字节，两者都原样写回；
//! 3. **存原样值，不存渲染值**：C# 在读的时候就做的变换（XOR、`+100/+110/+120/+200/+300`、
//!    `& 0x7FFF | 0x20000000`、`light & 0x0F * 4`、`door & 0x7F`）**一律不改原值**，
//!    而是实现成各模块里带 `csharp_` 前缀的独立函数，逐条标注源码行号。
//!
//! # 三、存储顺序：**列优先**
//!
//! 九种格式**全部**是列优先（C# 九支读法都是 `for (x) for (y)` 且偏移在内层单调推进；
//! 与 `client/reference_full/Classes/map/TMap.cpp:163-209` 的
//! `offset = sizeof(TMapHeader) + nAline*i + sizeof(TMapInfo)*nTy` 同构）：
//!
//! ```text
//! offset = 头部长度 + (x * h + y) * 格长
//! idx    = x * h + y            // RawMap::cells 的下标
//! ```
//!
//! # 四、只依赖 std
//!
//! `Cargo.toml` 的 `[dependencies]` 为空 —— 不引 serde、不引 thiserror、不引任何第三方。
//!
//! # 五、快速上手
//!
//! ```
//! use mir2_format::{read_any, detect_format, registry};
//!
//! // 造一张最小的 Old12 图（2×2，头部 52 字节 + 4 格 × 12 字节）
//! let mut raw = vec![0u8; 52 + 4 * 12];
//! raw[0..2].copy_from_slice(&2u16.to_le_bytes());
//! raw[2..4].copy_from_slice(&2u16.to_le_bytes());
//!
//! let map = read_any(&raw).expect("应解析成功");
//! assert_eq!(map.format, mir2_format::FormatId::Old12);
//! assert_eq!(detect_format(&raw).map(|f| f.id()), Some(map.format));
//! assert_eq!(registry().len(), 9);
//!
//! // 写回必须逐字节相等
//! let out = mir2_format::format_by_id(map.format).write(&map).expect("应写回");
//! assert_eq!(out, raw);
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod error;
pub mod formats;
pub mod model;
pub mod registry;

pub use error::{FormatError, MAX_CELLS};
pub use model::{Cell, CollisionPolarity, FormatCaps, FormatId, MapFormat, RawMap};
pub use registry::{
    csharp_40b_variant, detect_format, detect_format_csharp_order, detect_format_tolerant,
    format_by_id, read_any, read_any_tolerant, registry,
};
