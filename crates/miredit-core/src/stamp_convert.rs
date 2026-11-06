//! `.X` 印章 → [`StampCellInput`] 的**唯一**转换口。
//!
//! 上游真值：`mir2-stamp/src/cell.rs` 的字段表（C# `Document.cs:7-32` 的 `Cell` 字段清单 +
//! `:84-98` 的 `CellCodec.Write(..., map: false)` 印章分支）；下游结构见 [`StampCellInput`]。
//!
//! # 一、为什么把这一步收进内核
//!
//! 2026-11-05 之前，「印章 id → 格表」由**每个调用方各写一份**（`miredit-gui/src/stamp_place.rs`、
//! `miredit-web/src/routes/stamps.rs`、以及本 crate 的 `tests/stamp_place.rs` 里那个 `map` 闭包）。
//! 三份实现各自决定「图号掩不掩 `0x7FFF`」「库号 `as u8` 还是报错」「五个视图装不下的字段填不填」，
//! ⇒ 同一个 `.X` 在不同入口能转出**不同的格表**，而且**没有任何机制**能发现这种漂移。
//!
//! 现在只有一个入口（本模块）。调用方只负责**读字节**（`std::fs::read` +
//! [`mir2_stamp::Stamp::parse`]），掩码 / 报错 / 字段搬运的口径全部钉在这里。
//!
//! # 二、字段映射（逐条可对照）
//!
//! | [`StampCellInput`] | `.X` 侧 `StampCellData` | 处理 |
//! |---|---|---|
//! | `dx` / `dy` | `StampCell::x` / `.y`（`i32`，**可负**） | 原样 |
//! | `bk` | `back_image`（`i32`） | 取低 16 位再 `& 0x7FFF`（见 §三） |
//! | `mid` | `middle_image`（`u16`） | `& 0x7FFF` |
//! | `fr` | `foreground.image`（`i16`） | 按位转 `u16` 再 `& 0x7FFF` |
//! | `bk_lib` | `back_library`（`i16`） | **全局 id → 族内页号**（按 [`库号号段`]，见 §四） |
//! | `mid_lib` | `middle_library`（`i16`） | 同上 |
//! | `fr_lib` | `foreground.library`（`i16`） | 同上；**无前景 ⇒ `0`** |
//! | `door` / `door_offset` | `door` / `door_offset`（`u8`） | 原样 |
//! | `light` | `light`（`u8`） | 原样 |
//! | `ani_frame` / `ani_tick` | `foreground.frames` / `.tick`（`u8`） | 原样；**无前景 ⇒ `0`** |
//! | `mid_ani_frame` / `mid_ani_tick` | `middle_frames` / `middle_tick`（`u8`） | 原样 |
//! | `tile_animation` / `tile_offset` | `tile_animation` / `tile_offset`（`i16`） | 按位转 `u16` |
//! | `tile_frames` | `tile_frames`（`u8`） | 原样 |
//!
//! ## 关于「若 `.X` 无对应字段就留 0」的那五个
//!
//! 任务书给 `mid_ani_frame` / `mid_ani_tick` / `tile_animation` / `tile_offset` / `tile_frames`
//! 留了退路：「`.X` 无对应字段就留 0 并注释」。**实测 `.X` 全都有对应字段**
//! （`mir2-stamp/src/cell.rs` 的 `StampCellData` 字段表；v2 格体偏移 `27..34`）
//! ⇒ 本模块**照搬真值、不填 0**。
//!
//! 它们搬不动的其实是**下游**：`mir2_map::Cell`（12/14/36 三档）没有这五个字段的位置，
//! 所以 `crate::stamp::plan_stamp` 会在有非 0 值时给一句中文说明（`convert.rs` 的 `LOST_IN_VIEW`）。
//! 这里若提前填 0，连「如实报告」都做不到 —— 那才是静默丢数据。
//!
//! # 三、三族图号一律 `& 0x7FFF`
//!
//! `bit15` 在 CrystalStudio 里是**阻挡/标记位**，不是图号的一部分：
//! 前景取 `Image & 0x7FFF` 见 `CrystalStudio/README.md:205-207` 与 `StampIdentifier.cs:84`；
//! 地表 `BackImage` 的高位（`0x20000000` / `0x40000000` = 低/高阻挡）更是在**第 29 位以上**。
//! ⇒ 三族统一掩掉 `bit15`，与 `crate::paint` 的写入口径（`v & 0x7FFF`）对齐，
//! 免得「转换时带进来的标记位」被 `plan_stamp` 当成图号写进地图。
//!
//! ⚠ 真实样本 `mir_00000.X` 的 25 格里 21 格 `back_image = 0x60000000`（**只有标记位、图号为 0**）：
//! 不掩码就会把 `0x60000000` 截成 `0x0000`（恰好也是 0）—— 掩不掩在这一格看不出差别，
//! 所以本模块的口径由 `stamp_convert` 的单元测试逐字段钉住，而不是靠这个样本自证。
//!
//! # 四、库号是**全局 id**：先按号段换算成族内页号，不在号段就报错（A-024）
//!
//! `.X` 的三个库号是 **C# 全局分配号**（`Assets` 表）；地图侧三个字节是**族内页号**。
//! 换算规则与出处全在 [`库号号段`]（`Wemade`：`0/1/2` 三个基址；`Shanda`：`100/110/120`）。
//!
//! 三条硬纪律：
//!
//! 1. **号段是入参**（[`cells_from_stamp`] 的第二个参数），不在本模块里按值域猜 ——
//!    调用方按素材格式显式给（本仓暂无 `.lib` 素材 ⇒ 恒 [`库号号段::Wemade`]）；
//! 2. **没有图的层不参与换算**（写 0）：12 B 档硬编码 `MiddleLibrary = 1` 这类历史常量
//!    不该把整枚印章判成"号段不符"；
//! 3. **不在号段一律报错**（[`StampConvertError::LibraryNotInBand`]），绝不「原样透传」——
//!    透传会让前景去 `Objects3`（`surface(2)`）、中层去 `SmTiles2`（`middle(1)`），
//!    画面变成**别的图**且不报错。这正是 A-024 修的那条：改前 `1/2` 被原样写进
//!    `btsmTiles`/`btArea`。
//!
//! 反向兼容提醒：改前 `lib_byte` 只查 `0..=255`，所以 `300` / `-1` / `256` 都报
//! [`StampConvertError::LibraryOutOfRange`]；现在它们仍是**报错**，只是错误变体与措辞换成
//! 「不在号段」（多带了号段范围），对外仍是 422（web 侧 `ApiError::from_stamp_convert`）。

use std::fmt;

use mir2_stamp::{Stamp, StampCellData};

use crate::stamp::StampCellInput;

/// 三族库号（地表 / 中层 / 物体）—— 只用于「哪一族的库号越界」这句话。
///
/// 三族的落点见 [`StampCellInput`] 的字段文档：`bk_lib` → `btTiles`、`mid_lib` → `btsmTiles`、
/// `fr_lib` → `btArea`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StampLibLayer {
    /// 地表库号（`.X` 的 `BackLibrary`）。
    Ground,
    /// 中层库号（`.X` 的 `MiddleLibrary`）。
    Middle,
    /// 物体库号（`.X` 的 `Foreground.Library`）。
    Front,
}

impl StampLibLayer {
    /// 中文族名，**逐字**取 `地表` / `中层` / `物体`（错误消息的固定格式，别改）。
    ///
    /// ⚠ 与 `crate::layer::NAME_GROUND`（`"地表图层"`）**不是**同一个词表：那三个是界面标题，
    /// 这里要的是能嵌进「第 N 格 / 地表 / 值」这句话的短名。
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ground => "地表",
            Self::Middle => "中层",
            Self::Front => "物体",
        }
    }
}

/// `.X` 三族库号所在的**全局号段**（= 作者客户端 `CrystalStudio` `Assets` 表的分配号）。
///
/// # 为什么必须有它（A-024）
///
/// `.X` 里的三个库号是 **C# 全局 id**（`Document.cs:86,94-96` 存的是
/// `BackLibrary` / `MiddleLibrary` / `Foreground.Library`，C# 全程拿它们直接喂
/// `Assets.Get(library, …)`，见 `StampPreview.cs:40`），而地图侧的 `btTiles` /
/// `btsmTiles` / `btArea` 是**族内页号**。两者差一个**每族固定基址**：
///
/// | 号段 | 地表 `Tiles` | 中层 `SmTiles` | 物体 `Objects` | 出处 |
/// |---|---|---|---|---|
/// | [`库号号段::Wemade`] | `0` | `1` | `2..=99` | `Assets.cs:361-362` |
/// | [`库号号段::Shanda`] | `100..=109` | `110..=119` | `120..=150` | `Assets.cs:363-365` |
///
/// 例：`.X` 前景库号 `2` ⇒ `Objects` 的**页 0**（不是 `Objects3`）；中层 `1` ⇒
/// `SmTiles` **页 0**（不是 `SmTiles2`）。
///
/// ⚠ **不许在别处按值域猜**（尤其 `miredit-render::stamp`）：号段是**入参**，
/// 由调用方按素材格式显式给（本仓暂无 `.lib` 素材 ⇒ 恒 [`库号号段::Wemade`]）。
///
/// ⚠ 未建模：Sanxiu（`AssetLibraryIds.cs:5-23`：地表 `0..` / 中层 `1000..` /
/// 物体 `2000..`）与 Mir3（`200+`/`300+`，是「每状态 15 个具名库」的槽位表，
/// 三族页号表达不了）⇒ 这两类库号会因「不在任何已建模号段」被拒绝，**不会错位**。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum 库号号段 {
    /// `WemadeMir2`（含上游的「扁平目录」兜底）：地表 `0`、中层 `1`、物体 `2..=99`。
    Wemade,
    /// `ShandaMir2`：地表 `100..=109`、中层 `110..=119`、物体 `120..=150`。
    Shanda,
}

impl 库号号段 {
    /// 该族在**本号段里的全局号范围**（闭区间），**第一个值就是换算基址**。
    ///
    /// | 号段 | 地表 | 中层 | 物体 | 出处 |
    /// |---|---|---|---|---|
    /// | [`库号号段::Wemade`] | `0` | `1` | `2..=99` | `Assets.cs:361-362`：`Add(0,Tiles)` / `Add(1,Smtiles)` / `Add(2,Objects)` / `Add(i+1,Objects{i})`（`i=2..99`） |
    /// | [`库号号段::Shanda`] | `100..=109` | `110..=119` | `120..=150` | `Assets.cs:363-365`：`Add(100,Tiles)`+`Add(100+i,Tiles{i+1})` / `Add(110,SmTiles)`+… / `Add(120,Objects)`+`Add(120+i,Objects{i+1})` |
    ///
    /// ⚠ `190`（`ShandaMir2\AniTiles1`）**不是三族页**，故落在范围外 ⇒ 会被拒绝。
    #[must_use]
    pub const fn 范围(self, layer: StampLibLayer) -> (i16, i16) {
        match (self, layer) {
            (Self::Wemade, StampLibLayer::Ground) => (0, 0),
            (Self::Wemade, StampLibLayer::Middle) => (1, 1),
            (Self::Wemade, StampLibLayer::Front) => (2, 99),
            (Self::Shanda, StampLibLayer::Ground) => (100, 109),
            (Self::Shanda, StampLibLayer::Middle) => (110, 119),
            (Self::Shanda, StampLibLayer::Front) => (120, 150),
        }
    }

    /// 全局库号 → **族内页号**（= 地图格字节 `btTiles`/`btsmTiles`/`btArea`）；
    /// 不在本号段 ⇒ `None`（**拒绝，不猜**）。
    ///
    /// 例（`Wemade`）：`0→0`（Tiles 页 0）· `1→0`（SmTiles 页 0）· `2→0`（Objects 页 0）·
    /// `3→1`（Objects2）· `99→97`（Objects98）；`100` 及以上不在本号段 ⇒ `None`。
    #[must_use]
    pub const fn 页号(self, layer: StampLibLayer, 全局: i16) -> Option<u8> {
        let (lo, hi) = self.范围(layer);
        if 全局 < lo || 全局 > hi {
            return None;
        }
        let 页 = 全局 - lo;
        if 页 < 0 || 页 > u8::MAX as i16 {
            return None;
        }
        Some(页 as u8)
    }

    /// 号段短名（错误消息用）。
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Wemade => "Wemade",
            Self::Shanda => "Shanda",
        }
    }
}

/// [`cells_from_stamp`] 的失败原因（手写 [`fmt::Display`] + [`std::error::Error`]，不引 `thiserror`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StampConvertError {
    /// 某一族的库号**不在给定号段里**（全局 id 号段不符 / 负数 / 未建模的 Sanxiu、Mir3 号段）。
    LibraryNotInBand {
        /// 格序号，**从 0 起**（= `stamp.cells` 的下标）；消息里显示 `index + 1`。
        index: usize,
        /// 哪一族。
        layer: StampLibLayer,
        /// `.X` 里的原始值（`i16`，未截断）。
        value: i16,
        /// 调用方给的号段。
        band: 库号号段,
    },
}

impl fmt::Display for StampConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LibraryNotInBand {
                index,
                layer,
                value,
                band,
            } => {
                let (lo, hi) = band.范围(*layer);
                write!(
                    f,
                    "第 {} 格 / {} / 库号 {value} 不在 {} 号段 {lo}..={hi} 内（拒绝原样透传：透传会取到别的图库）",
                    index + 1,
                    layer.label(),
                    band.label()
                )
            }
        }
    }
}

impl std::error::Error for StampConvertError {}

/// 把一枚 `.X` 印章**整枚**转成内核的格表（顺序、相对坐标、格数一律保持原样）。
///
/// 逐格映射见模块文档 §二；三族图号掩 `0x7FFF`（§三）；**库号按 `号段` 从全局 id
/// 换算成族内页号**，不在号段即报错（§四）。
///
/// # 错误
///
/// 任意一格**有图**的那一层库号不在 `号段` 里 ⇒ [`StampConvertError::LibraryNotInBand`]，
/// 消息含「第 N 格 / 地表|中层|物体 / 库号 / 号段范围」。
/// **整枚失败，不返回半份格表**（不做「跳过坏格继续」：那会让印章缺格而调用方不知情）。
///
/// 某一层**没有图**（图号掩 `0x7FFF` 后为 0）时该层库号写 0 且**不参与换算** ——
/// 落笔与渲染都只看有图的那几层，那种格上的历史库号（如 12 B 档硬编码的
/// `MiddleLibrary = 1`）不该把整枚印章判成"号段不符"。
///
/// # 例
///
/// ```no_run
/// use mir2_stamp::Stamp;
///
/// let bytes = std::fs::read("<dataRoot>/Objects/mir/mir_00000.X")?;
/// let stamp = Stamp::parse(&bytes)?;
/// // 本仓素材是 `.wzl/.wil`（Default 号段）⇒ 显式给 `Wemade`
/// let cells = miredit_core::cells_from_stamp(&stamp, miredit_core::库号号段::Wemade)?;
/// assert_eq!(cells.len(), 25);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn cells_from_stamp(
    stamp: &Stamp,
    号段: 库号号段,
) -> Result<Vec<StampCellInput>, StampConvertError> {
    let mut out = Vec::with_capacity(stamp.cells.len());
    for (index, cell) in stamp.cells.iter().enumerate() {
        out.push(cell_from_stamp(index, cell.x, cell.y, &cell.cell, 号段)?);
    }
    Ok(out)
}

/// 单格转换（[`cells_from_stamp`] 的实现体；`index` 是**0 起**的格序号，只用于报错）。
fn cell_from_stamp(
    index: usize,
    x: i32,
    y: i32,
    cell: &StampCellData,
    号段: 库号号段,
) -> Result<StampCellInput, StampConvertError> {
    let front = cell.foreground;
    // 「这一层有没有图」= 图号掩 `0x7FFF` 后非 0（与落笔/渲染同一判据）。
    let 有地表 = (cell.back_image & 0x7FFF) != 0;
    let 有中层 = (cell.middle_image & 0x7FFF) != 0;
    let 有前景 = front.is_some_and(|f| (f.image & 0x7FFF) != 0);
    Ok(StampCellInput {
        dx: x,
        dy: y,
        // 见模块文档 §三：`back_image` 是 `i32`，低 16 位里 bit15 是阻挡位，高位是编辑器标记。
        bk: (cell.back_image as u16) & 0x7FFF,
        mid: cell.middle_image & 0x7FFF,
        fr: front.map_or(0, |f| (f.image as u16) & 0x7FFF),
        bk_lib: lib_page(index, 号段, StampLibLayer::Ground, 有地表, cell.back_library)?,
        mid_lib: lib_page(index, 号段, StampLibLayer::Middle, 有中层, cell.middle_library)?,
        fr_lib: match front {
            Some(f) => lib_page(index, 号段, StampLibLayer::Front, 有前景, f.library)?,
            None => 0,
        },
        door: cell.door,
        door_offset: cell.door_offset,
        light: cell.light,
        ani_frame: front.map_or(0, |f| f.frames),
        ani_tick: front.map_or(0, |f| f.tick),
        // 这五个 `.X` 里真有字段（见模块文档 §二 末段），照搬；下游装不下时由 `plan_stamp` 如实报。
        mid_ani_frame: cell.middle_frames,
        mid_ani_tick: cell.middle_tick,
        tile_animation: cell.tile_animation as u16,
        tile_offset: cell.tile_offset as u16,
        tile_frames: cell.tile_frames,
    })
}

/// 库号 `i16`（**全局 id**）→ 族内页号；**没有图**的层恒 `0`（不参与换算）；
/// 有图但不在 `号段` ⇒ 报错（**不原样透传、不截断**，见模块文档 §四）。
fn lib_page(
    index: usize,
    号段: 库号号段,
    layer: StampLibLayer,
    有图: bool,
    value: i16,
) -> Result<u8, StampConvertError> {
    if !有图 {
        return Ok(0);
    }
    号段
        .页号(layer, value)
        .ok_or(StampConvertError::LibraryNotInBand {
            index,
            layer,
            value,
            band: 号段,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 取证根缺省值：`.X` 的取证来源（**本模块里只出现这一处**）。换机器用 `MIREDIT_UPSTREAM_ROOT`。
    const 缺省取证根: &str = r"I:\miredit\_upstream";

    /// 取证根：`MIREDIT_UPSTREAM_ROOT` → 缺省 [`缺省取证根`]；根不存在 ⇒ `None`。
    ///
    /// 只做 `var_os` + `PathBuf`，不引入任何第三方库。
    fn 取证根() -> Option<PathBuf> {
        let root = std::env::var_os("MIREDIT_UPSTREAM_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(缺省取证根));
        root.is_dir().then_some(root)
    }

    /// 读真实印章字节；**缺取证根或文件 ⇒ `None`**（公开仓/PR 的机器上没有 `.X` 素材，
    /// 那不算代码错 ⇒ 调用方打印 `[skip]` 并返回）。
    fn 读真实印章() -> Option<Vec<u8>> {
        let root = 取证根()?;
        let p = root.join("Objects").join("mir").join("mir_00000.X");
        if !p.is_file() {
            eprintln!("[skip] 缺文件 {}", p.display());
            return None;
        }
        match std::fs::read(&p) {
            Ok(b) => Some(b),
            Err(e) => panic!("读不到印章 {}：{e}", p.display()),
        }
    }

    fn i16_at(b: &[u8], at: usize) -> i16 {
        i16::from_le_bytes([b[at], b[at + 1]])
    }

    fn u16_at(b: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([b[at], b[at + 1]])
    }

    fn i32_at(b: &[u8], at: usize) -> i32 {
        i32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
    }

    /// 原始字节里的**一格**：字段名与 `.X` 侧同名，**不做任何掩码**
    /// —— 掩码是被测行为，必须在断言里显式写出，不能借用实现里的同一个表达式。
    #[derive(Debug, Clone, Copy)]
    struct 原始格 {
        x: i32,
        y: i32,
        back_library: i16,
        back_image: i32,
        middle_library: i16,
        middle_image: u16,
        fronts: u8,
        front_library: i16,
        front_image: i16,
        front_frames: u8,
        front_tick: u8,
        door: u8,
        door_offset: u8,
        middle_frames: u8,
        middle_tick: u8,
        tile_animation: i16,
        tile_offset: i16,
        tile_frames: u8,
        light: u8,
    }

    /// 从 `.X` 原始字节**逐格重解**一遍：**不经过** `mir2-stamp`、**不经过** [`cells_from_stamp`]。
    ///
    /// 布局照 `mir2-stamp/src/cell.rs` 模块文档 §一 的表（v2 印章分支）：
    /// 格起点 = `X(4) + Y(4)`，随后 `BackLibrary(2) BackImage(4) MiddleLibrary(2) MiddleImage(2)`、
    /// 前景个数(1)，有前景再加 `Library(2) Image(2) Frames(1) Tick(1)`，
    /// 然后 `Door(1) DoorOffset(1) MiddleFrames(1) MiddleTick(1) TileAnimation(2) TileOffset(2)
    /// TileFrames(1) Light(1)`。
    fn 逐格解字节(bytes: &[u8]) -> Vec<原始格> {
        assert_eq!(&bytes[0..2], b"C#", "魔数必须是 C#");
        let count = i32_at(bytes, 3);
        assert!(count > 0, "格数应 > 0，实测 {count}");
        let mut pos = 7usize;
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let c = &bytes[pos..];
            let fronts = c[18];
            assert!(fronts <= 1, "一格最多 1 个前景，实测 {fronts}");
            // 前景块之后才是 Door（`Document.cs:96` 的顺序）
            let tail = if fronts == 1 { 25 } else { 19 };
            out.push(原始格 {
                x: i32_at(c, 0),
                y: i32_at(c, 4),
                back_library: i16_at(c, 8),
                back_image: i32_at(c, 10),
                middle_library: i16_at(c, 14),
                middle_image: u16_at(c, 16),
                fronts,
                front_library: if fronts == 1 { i16_at(c, 19) } else { 0 },
                front_image: if fronts == 1 { i16_at(c, 21) } else { 0 },
                front_frames: if fronts == 1 { c[23] } else { 0 },
                front_tick: if fronts == 1 { c[24] } else { 0 },
                door: c[tail],
                door_offset: c[tail + 1],
                middle_frames: c[tail + 2],
                middle_tick: c[tail + 3],
                tile_animation: i16_at(c, tail + 4),
                tile_offset: i16_at(c, tail + 6),
                tile_frames: c[tail + 8],
                light: c[tail + 9],
            });
            pos += if fronts == 1 { 35 } else { 29 };
        }
        assert_eq!(pos, bytes.len(), "格数 × 单格字节 必须精确等于文件长度（尾部不许有冗余）");
        out
    }

    /// 按**规则**（模块文档 §二 的表 + §四 的号段换算）算出该格应得的 [`StampCellInput`]。
    ///
    /// 三族图号的 `& 0x7FFF` **在这里显式再写一遍**：如果实现漏了掩码，这条会红。
    ///
    /// 库号走**独立推导**：`.X` 原文 → 有没有图 → 按 `Wemade` 号段减基址。
    /// 与实现共用 [`库号号段::页号`] 是**有意**的（那是唯一真值表），但
    /// `(raw, 有无图) → 期望页号` 这一步在测试里显式写出，实现接错层也不会一起绿。
    fn 期望输入(g: &原始格) -> StampCellInput {
        let 有地表 = (g.back_image & 0x7FFF) != 0;
        let 有中层 = (g.middle_image & 0x7FFF) != 0;
        let 有前景 = g.fronts == 1 && (g.front_image & 0x7FFF) != 0;
        let 页 = |层: StampLibLayer, raw: i16, 有图: bool| -> u8 {
            if !有图 {
                0
            } else {
                库号号段::Wemade
                    .页号(层, raw)
                    .expect("样本库号应落在 Wemade 号段内")
            }
        };
        StampCellInput {
            dx: g.x,
            dy: g.y,
            bk: (g.back_image as u16) & 0x7FFF,
            mid: g.middle_image & 0x7FFF,
            fr: (g.front_image as u16) & 0x7FFF,
            bk_lib: 页(StampLibLayer::Ground, g.back_library, 有地表),
            mid_lib: 页(StampLibLayer::Middle, g.middle_library, 有中层),
            fr_lib: 页(StampLibLayer::Front, g.front_library, 有前景),
            door: g.door,
            door_offset: g.door_offset,
            light: g.light,
            ani_frame: g.front_frames,
            ani_tick: g.front_tick,
            mid_ani_frame: g.middle_frames,
            mid_ani_tick: g.middle_tick,
            tile_animation: g.tile_animation as u16,
            tile_offset: g.tile_offset as u16,
            tile_frames: g.tile_frames,
        }
    }

    /// 测试 1：真实 `mir_00000.X` —— 格数 25，`cells[0]` 的三族图号/库号与**直接读该格**一致，
    /// 并顺带把 25 格逐字段全比一遍（含 8 格带前景的 `fr` / `fr_lib` / `ani_frame` / `ani_tick`）。
    #[test]
    fn 真实印章二十五格_首格三族与直接读该格一致() {
        let Some(bytes) = 读真实印章() else {
            return;
        };
        let stamp = Stamp::parse(&bytes).expect("mir_00000.X 应能解析");
        let cells =
            cells_from_stamp(&stamp, 库号号段::Wemade).expect("样本库号应落在 Wemade 号段内");

        assert_eq!(stamp.cells.len(), 25, "README:211-215 记该文件 25 格");
        assert_eq!(cells.len(), 25, "转换不得增删格");

        let 原始 = 逐格解字节(&bytes);
        assert_eq!(原始.len(), 25);

        // ① 任务书点名的 `cells[0]`：三族图号 + 三族库号，逐个与直接读该格的结果比。
        assert_eq!(cells[0].bk, (原始[0].back_image as u16) & 0x7FFF);
        assert_eq!(cells[0].mid, 原始[0].middle_image & 0x7FFF);
        assert_eq!(cells[0].fr, (原始[0].front_image as u16) & 0x7FFF);
        // **两段断言**（A-024）：`.X` 原文是全局号 0/1/2，换算后都必须是**页 0**。
        assert_eq!(
            (原始[0].back_library, 原始[0].middle_library),
            (0, 1),
            "样本第 1 格的 `.X` 原文：地表 0（Tiles）、中层 1（SmTiles）"
        );
        assert_eq!(
            (cells[0].bk_lib, cells[0].mid_lib, cells[0].fr_lib),
            (0, 0, 0),
            "换算后三族都落页 0（地表无图 ⇒ 0；中层 1 → SmTiles 页 0；无前景 ⇒ 0）"
        );
        assert_eq!((cells[0].dx, cells[0].dy), (1, -2), "相对坐标可负");
        // 该格实测 `back_image = 0x60000000`：只有高位阻挡标记、图号为 0
        // ⇒ 掩码后必须是 0（这条同时证明高位标记没有漏进图号）。
        assert_eq!(原始[0].back_image, 0x6000_0000);
        assert_eq!(cells[0].bk, 0);

        // ② 全 25 格逐字段（掩码 + 号段换算口径见 `期望输入`）。
        for (i, (got, g)) in cells.iter().zip(原始.iter()).enumerate() {
            assert_eq!(*got, 期望输入(g), "第 {} 格与原始字节不符", i + 1);
        }

        // ③ 整枚的库号全落页 0（样本只有前景用得到库号：全局 2 → Objects 页 0）
        assert!(
            cells.iter().all(|c| c.fr_lib == 0),
            "8 格前景的全局号 2 必须全部换算成 Objects 页 0"
        );
    }

    /// **换算表逐条钉住**（`Wemade` / `Shanda` 两档；依据 `Assets.cs:361-365`）。
    #[test]
    fn 库号号段换算表() {
        use StampLibLayer::{Front, Ground, Middle};

        // Wemade：地表恒 0 / 中层恒 1 / 物体 2..=99
        assert_eq!(库号号段::Wemade.页号(Ground, 0), Some(0), "地表 0 → Tiles 页 0");
        assert_eq!(库号号段::Wemade.页号(Middle, 1), Some(0), "中层 1 → SmTiles 页 0");
        assert_eq!(库号号段::Wemade.页号(Front, 2), Some(0), "物体 2 → Objects 页 0");
        assert_eq!(库号号段::Wemade.页号(Front, 3), Some(1), "物体 3 → Objects2");
        assert_eq!(库号号段::Wemade.页号(Front, 99), Some(97), "物体 99 → Objects98");
        assert_eq!(库号号段::Wemade.页号(Front, 100), None, "Wemade 没有 100 号");
        assert_eq!(库号号段::Wemade.页号(Ground, 1), None, "Wemade 地表只有 0 号");
        assert_eq!(库号号段::Wemade.页号(Middle, 0), None, "Wemade 中层只有 1 号");

        // Shanda：100..=109 / 110..=119 / 120..=150
        assert_eq!(库号号段::Shanda.页号(Ground, 100), Some(0), "地表 100 → Tiles 页 0");
        assert_eq!(库号号段::Shanda.页号(Ground, 109), Some(9));
        assert_eq!(库号号段::Shanda.页号(Middle, 110), Some(0), "中层 110 → SmTiles 页 0");
        assert_eq!(库号号段::Shanda.页号(Middle, 119), Some(9));
        assert_eq!(库号号段::Shanda.页号(Front, 120), Some(0), "物体 120 → Objects 页 0");
        assert_eq!(库号号段::Shanda.页号(Front, 150), Some(30), "物体 150 → Objects31");
        assert_eq!(库号号段::Shanda.页号(Front, 151), None, "Shanda 没有 151 号");
        assert_eq!(库号号段::Shanda.页号(Front, 99), None, "Wemade 的 99 不在 Shanda 号段");

        // `190` = `ShandaMir2\AniTiles1`：不是三族页 ⇒ 两档都拒绝（不硬塞进 Objects）
        assert_eq!(库号号段::Shanda.页号(Front, 190), None);
        assert_eq!(库号号段::Wemade.页号(Front, 190), None);
        // 号段名（错误消息用）
        assert_eq!(库号号段::Wemade.label(), "Wemade");
        assert_eq!(库号号段::Shanda.label(), "Shanda");
    }

    /// **没有图的层不参与换算**：12 B 档硬编码的 `MiddleLibrary = 1` 之类不该被判成"号段不符"；
    /// 反过来，**有图但号段不符**必须拒绝。
    #[test]
    fn 无图层不参与换算_有图跨号段则拒绝() {
        let 只有前景 = StampCellData {
            back_library: 0,
            back_image: 0, // 地表无图
            middle_library: 1,
            middle_image: 0, // 中层无图
            foreground: Some(mir2_stamp::Front {
                library: 2,
                image: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let cells = cells_from_stamp(
            &Stamp {
                version: 2,
                cells: vec![mir2_stamp::StampCell { x: 0, y: 0, cell: 只有前景 }],
            },
            库号号段::Wemade,
        )
        .expect("无图层的库号不参与换算");
        assert_eq!((cells[0].bk_lib, cells[0].mid_lib, cells[0].fr_lib), (0, 0, 0));

        // 有图但库号是 Shanda 号段（120）⇒ Wemade 下拒绝，Shanda 下命中页 0
        let 跨号段 = StampCellData {
            foreground: Some(mir2_stamp::Front {
                library: 120,
                image: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let stamp = Stamp {
            version: 2,
            cells: vec![mir2_stamp::StampCell { x: 0, y: 0, cell: 跨号段 }],
        };
        let err = cells_from_stamp(&stamp, 库号号段::Wemade).expect_err("跨号段必须拒绝");
        assert!(
            matches!(err, StampConvertError::LibraryNotInBand { .. }),
            "{err:?}"
        );
        let msg = err.to_string();
        assert!(msg.contains("120") && msg.contains("Wemade"), "{msg}");
        assert!(msg.contains("2..=99"), "消息要带号段范围：{msg}");
        let ok = cells_from_stamp(&stamp, 库号号段::Shanda).expect("Shanda 号段内应成功");
        assert_eq!(ok[0].fr_lib, 0, "Shanda 120 → Objects 页 0");

        // 中层有图但库号 0（不在 Wemade 中层号段）⇒ 拒绝
        let 中层缺号 = StampCellData {
            middle_library: 0,
            middle_image: 5,
            ..Default::default()
        };
        let stamp = Stamp {
            version: 2,
            cells: vec![mir2_stamp::StampCell {
                x: 0,
                y: 0,
                cell: 中层缺号,
            }],
        };
        assert!(cells_from_stamp(&stamp, 库号号段::Wemade).is_err());
    }

    /// 测试 2：库号 300 / -1 / 256 都必须 `Err`，消息含「值 / 族名 / 1 起格号 / 号段范围」。
    #[test]
    fn 库号越号段必须报错_消息含格号族名与值() {
        let Some(bytes) = 读真实印章() else {
            return;
        };
        let mut stamp = Stamp::parse(&bytes).expect("解析");
        // 样本第 1 格的 `back_image` 只有高位标记（图号为 0）⇒ 地表「没有图」就不会参与换算。
        // 要测「有图 + 越号段 ⇒ 拒绝」，先把地表图号补上（否则测到的是"无图跳过"，不是拒绝）。
        stamp.cells[0].cell.back_image = 7;
        stamp.cells[0].cell.back_library = 300;

        let err = cells_from_stamp(&stamp, 库号号段::Wemade)
            .expect_err("库号 300 不在 Wemade 号段，必须拒绝");
        let msg = err.to_string();
        assert!(msg.contains("300"), "消息应含原始值 300：{msg}");
        assert!(msg.contains("第 1 格"), "消息应含 1 起的格号：{msg}");
        assert!(msg.contains("地表"), "消息应含族名：{msg}");
        assert_eq!(
            err,
            StampConvertError::LibraryNotInBand {
                index: 0,
                layer: StampLibLayer::Ground,
                value: 300,
                band: 库号号段::Wemade,
            }
        );

        // 手写的 `Display` + `std::error::Error` 真能当 trait object 用（不是只挂了个名字）。
        let dyn_err: &dyn std::error::Error = &err;
        assert_eq!(dyn_err.to_string(), msg);

        // 中层：负数同样不在号段（`-1 as u8 == 255`，截断会变成另一个库）。
        stamp.cells[0].cell.back_library = 0;
        stamp.cells[0].cell.middle_library = -1;
        // 中层无图 ⇒ 不参与换算；把图号补上才测得到拒绝
        stamp.cells[0].cell.middle_image = 3;
        let err = cells_from_stamp(&stamp, 库号号段::Wemade).expect_err("负库号必须拒绝");
        assert!(err.to_string().contains("中层"), "{err}");
        assert!(err.to_string().contains("-1"), "{err}");
        stamp.cells[0].cell.middle_library = 1;
        stamp.cells[0].cell.middle_image = 0;

        // 物体：找第一个带前景的格（样本有 8 格），把它的库号改成 256。
        let idx = stamp
            .cells
            .iter()
            .position(|c| c.cell.foreground.is_some())
            .expect("样本应有带前景的格");
        stamp.cells[idx]
            .cell
            .foreground
            .as_mut()
            .expect("刚查到带前景")
            .library = 256;
        let err = cells_from_stamp(&stamp, 库号号段::Wemade).expect_err("物体库号 256 必须拒绝");
        let msg = err.to_string();
        assert!(msg.contains("物体"), "{msg}");
        assert!(msg.contains("256"), "{msg}");
        assert!(msg.contains(&format!("第 {} 格", idx + 1)), "{msg}");
    }
}
