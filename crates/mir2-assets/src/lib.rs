//! `mir2-assets` —— 传奇 M2 `.wil/.wix/.wz*` 图库解码层（地图编辑器渲染地基）。
//!
//! 本 crate 只做一件事：把「地图格里的图库字段 + 帧号」变成**行优先 RGBA8 像素**，
//! 且与客户端离线烘焙的图集**逐字节一致**。不做 GUI、不做 CLI、不做图集打包。
//!
//! # 一、库号 → 文件规则（实测 + C++ 实证，非猜测）
//!
//! 客户端 `Classes/common/MShare.cpp:827-873` 逐个 `sprintf` 图库文件名：`i == 0` 用基名常量，
//! `i > 0` 用 `%d` 模板并传 `i + 1`（宏见 `Classes/common/Share.h:36-41`）。
//!
//! 定位键是 [`LibRef`]（**族 + 族内索引**），`index` 就是地图格字段的**原值**：
//!
//! | 族（[`Family`]） | 地图格字段 | `LibRef::index == 0` | `LibRef::index > 0` |
//! |---|---|---|---|
//! | [`Family::Ground`] 地面 | `btTiles` | `Tiles` | `Tiles{index+1}` |
//! | [`Family::Middle`] 中层 | `btsmTiles` | `SmTiles` | `SmTiles{index+1}` |
//! | [`Family::Surface`] 表层 | `btArea` | `Objects` | `Objects{index+1}` |
//!
//! 细节：
//!
//! - **只认基名，不认扩展名**。宏里写 `.wil`，本机 23 周年客户端目录里实际是 `.wzl`；
//!   两种容器都收（`wzl+wzx`、`wil+wix`），按数据文件扩展名分派解码器。
//! - 后缀数字只允许「无后缀」或 `{index+1}` 且 `index > 0`；`{Stem}1` 在 C++ 中**永不生成**，
//!   故按规则外处理并记录（[`LibPaths::skipped`]），不做猜测映射。
//! - **三族索引空间互不相通**：`LibRef::ground(0)`（`Tiles`）与 `LibRef::surface(0)`（`Objects`）
//!   是两个不同的库。这里刻意**不提供**「全局扁平号」—— 那种设计会让跨族误传的值
//!   静默命中另一族的某一页，比报错危险得多。
//! - 取文件用 [`LibPaths::resolve`] / [`LibPaths::resolve_lib`]，取像素用 [`SpriteSource::frame`]。
//!
//! # 二、与客户端图集的验收口径（「编辑器所见 = 客户端所见」的机器证据）
//!
//! 客户端不直接读 `.wzl`，读的是离线烘焙产物
//! `GodotClient/assets/<库>/<库>_atlas_NN.png` + `<库>_atlas_meta.json`
//! （`frames[i]` 含 `idx / w / h / anchor / atlas / region`）。所以本 crate 的正确性判据是：
//!
//! > 用本 crate 解码出的 `Frame.rgba`，与「图集 PNG 里该帧 `region` 裁出来的同尺寸像素」
//! > **逐字节相等**（不是「看起来差不多」，不做模糊比较）。
//!
//! 集成测试 `tests/acceptance.rs` 按此口径跑：`Tiles / Tiles5 / Objects / Objects21`
//! 四个库，每库确定性伪随机抽 50 个「`w > 0` 且 `atlas` 非空」的帧号，逐像素比对；
//! 不一致时打印「库名 / 帧号 / 首个不一致像素坐标 / 两边 RGBA」。
//!
//! 结构性保证还有一层：解码与调色板**完全复用** `tools/wil2atlas`
//! （`wil_decoder.rs` / `wzl_decoder.rs` / `palette_client.rs`），本 crate 不复制一行解析代码，
//! 见 [`wil_source`] 模块文档。
//!
//! # 三、本机资源实测（2026-11-04，`E:\GodM2\client` 全扫）
//!
//! | 客户端目录 | 库文件 | 说明 |
//! |---|---|---|
//! | `热血传奇23周年客户端\data` | 628 对 `.wzl/.wzx` | 本 crate 默认扫描根；三族齐全 |
//! | `reference_full\Resources\data` | 21 对 `.wil/.wix`（另有 1 个 `StateItem2.wil` 无 `.wix`，不成对） | 旧版 WIL 容器 |
//! | `移动端UI文件` / `shanpao` | 无 | 不是图库集 |
//!
//! 23 周年那套里三族的实际库号：地面 `0..=21`（22 个）、中层 `0..=35`（36 个）、
//! 表层 `0` + `1..=50` + `511..=517` + `5107`（58 个，其中 `Objects41`、`objects512..518`、
//! `objects5108` 是 64 字节空存根 —— 能登记，但取帧一律 `None`）。
//!
//! # 四、错误与降级口径
//!
//! - 结构性失败（根不存在 / 一个库都没登记 / 单库打开失败）→ [`AssetError`]；
//! - 帧级不可得（库不存在 / 帧号越界 / 空帧）→ [`SpriteSource::frame`] 返回 `None`；
//! - **绝不返回全透明假图**，也**绝不 panic**（含 `u32::MAX` 这类输入）。
//! - 解码期真错误在 `frame()` 里只能折成 `None`（trait 契约如此），但会记进
//!   [`WilSource::failures`]，不静默吞掉。

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod frame;
mod lib_paths;
mod wil_source;

pub use error::AssetError;
pub use frame::{Frame, SpriteSource};
pub use lib_paths::{Family, LibFile, LibInfo, LibPaths, LibRef, SkipReason, Skipped};
pub use wil_source::{WilSource, DEFAULT_CACHE_CAPACITY};

use std::path::Path;

/// 本机默认客户端根目录（实测存在：`data/` 下有 628 对 `.wzl/.wzx`）。
///
/// 写死的是**扫描出来的真实路径**，不是猜的；换机器时用 [`LibPaths::discover`] 传自己的根。
pub const DEFAULT_CLIENT_ROOT: &str = r"E:\GodM2\client\热血传奇23周年客户端";

/// 本机默认的客户端图集根目录（只读比对用，本 crate 不写它）。
pub const DEFAULT_CLIENT_ATLAS_ROOT: &str = r"E:\GodM2\GodotClient\assets";

/// 本机另一套客户端库目录：旧版 `.wil/.wix` 容器（`reference_full`）。
///
/// 实测 22 个 `.wil` + 21 个 `.wix`，其中 `StateItem2.wil` **没有**配对索引文件
/// ⇒ 按「必须成对」规则跳过并记录（见 [`LibPaths::skipped`]）。
pub const DEFAULT_CLIENT_WIL_ROOT: &str = r"E:\GodM2\client\reference_full\Resources";

/// 扫描 [`DEFAULT_CLIENT_ROOT`] 的便捷入口。
///
/// 失败语义与 [`LibPaths::discover`] 完全一致。
pub fn discover_default() -> Result<LibPaths, AssetError> {
    LibPaths::discover(Path::new(DEFAULT_CLIENT_ROOT))
}
