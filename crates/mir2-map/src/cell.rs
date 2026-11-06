//! 单元格布局与格长档位（`CellFormat` / `Cell`）。
//!
//! # 一、三档格长判别式（唯一判据 = 文件长度精确整除，禁标题字节）
//!
//! | 档位 | 单格字节 | 期望文件总长 |
//! |------|----------|--------------|
//! | [`CellFormat::Old12`] | 12 | `52 + 12*w*h` |
//! | [`CellFormat::New14`] | 14 | `52 + 14*w*h` |
//! | [`CellFormat::Ext36`] | 36 | `52 + 36*w*h` |
//!
//! 判据只看 `len` 与 `w*h` 的等式，**不看任何标题/魔法字节**，也不看扩展名。
//! 三档皆不成立 ⇒ 解析必须返回 [`crate::MapError::LengthMismatch`]，
//! 错误信息里同时列出三档期望长度与实际长度。
//!
//! 边界一：`w*h == 0` —— 长度判据**本身**不区分「0 格」（此时三档期望长度都等于 52），
//! 但 [`crate::Map::parse`] 在调用本判据**之前**就因 `w == 0 || h == 0` 返回
//! [`crate::MapError::ZeroDimension`]（与客户端 `map_loader.gd:94-95`、
//! C# `LegacyMapReader.cs:50` 同口径）⇒ 「52 字节的空图」不是合法地图。
//!
//! 边界二：`w*h > 16_000_000` —— 同样先于长度判据被 [`MAX_CELLS`] 拦下
//! ⇒ 超限文件得到的是 [`crate::MapError::TooManyCells`]，而不是「先分配再崩」。
//!
//! # 二、单元格字节偏移（三档共用前 12 字节，14/36 档再带两个库号字节）
//!
//! ```text
//! 偏移  字段            类型    说明
//!   0   wBkImg         u16     背景层图号
//!   2   wMidImg         u16     中间层图号
//!   4   wFrImg          u16     前景层图号
//!   6   btDoorIndex    u8      门索引
//!   7   btDoorOffset   u8      门偏移
//!   8   btAniFrame     u8      动画帧
//!   9   btAniTick      u8      动画节拍
//!  10   btArea          u8      区域
//!  11   btLight         u8      光照
//!  12   btTiles         u8      仅 14/36 档（Old12 档无此字节）
//!  13   btsmTiles       u8      仅 14/36 档（Old12 档无此字节）
//!  14   ...             [u8;22] 仅 36 档：尾区，语义未知，原样保存原样写回
//! ```
//!
//! # 三、`Ext36` 尾区（偏移 14..36）**不可假设恒零**
//!
//! 实测（2026-11-04，本机真实客户端资源）：
//!
//! - `client/热血传奇23周年客户端/Map/n0.map`（700×700 = 490000 格）全格扫描：
//!   **1660 格**尾区存在非零字节 = **0.3388%**；
//!   其中 **1658 格**在 off18/19/20 非零，另有 2 格（`(342,275)`、`(343,275)`）
//!   只在 off25..28 非零。
//! - 非零格空间分布：`x ∈ [297..396]`、`y ∈ [199..303]`（一条斜向带，不是单点噪声）。
//! - 对照组 `n3.map` / `n4.map` / `NALX.map` 尾区全零。
//!
//! ⇒ 尾区**既不能丢、也不能重置为 0**，更不能按「零」来推断字段语义。
//! 本库只做**原样保存 + 原样写回**，不解释、不命名、不做任何归一化。

/// 文件头（`TMapHeader`）字节数。
pub const HEADER_LEN: usize = 52;

/// 三档共用字段区的字节数（偏移 0..12）。
pub const COMMON_CELL_LEN: usize = 12;

/// 库号字节（`btTiles` / `btsmTiles`）在单元格内的起始偏移，仅 14/36 档有效。
pub const LIB_OFFSET: usize = COMMON_CELL_LEN;

/// 尾区在单元格内的起始偏移，仅 36 档有效。
pub const TAIL_OFFSET: usize = 14;

/// `Ext36` 档单格尾区（偏移 14..36）字节数。
pub const TAIL_LEN: usize = 22;

/// `Old12` 档单格字节数。
pub const OLD12_CELL_SIZE: usize = 12;

/// `New14` 档单格字节数。
pub const NEW14_CELL_SIZE: usize = 14;

/// `Ext36` 档单格字节数。
pub const EXT36_CELL_SIZE: usize = 36;

/// 单张地图允许的最大格数（`w * h`）—— **编辑器护栏，不是客户端真值**。
///
/// 口径来源（同值、同判据）：CrystalStudio 的 C#
/// `CrystalStudio/CrystalStudio/LegacyMapReader.cs:50`（`ValidateDimensions` 内
/// `if (Width <= 0 || Height <= 0 || count > 16_000_000) throw new
/// InvalidDataException("Invalid or excessive map dimensions.");`），
/// 同一数值也出现在 `Document.cs:58`。
///
/// 为什么需要这条护栏：本库的 [`Cell`] 是 **36 B 定长**（14 B 字段 + 22 B 尾区），
/// 而 `Old12` 档每格只有 12 B ⇒ 内存约为文件大小的 3 倍。没有上限时，一张 200 MB 的
/// 合法 12 B/格图会去申请约 600 MB，失败方式是 OOM 而不是明确报错。
///
/// 与客户端的关系：Godot 客户端 `map_loader.gd` **没有**这条上限（只判 `w/h > 0`），
/// 所以这是本库有意加的护栏，不是与客户端对齐的语义；上限值本身取自 C#（见上 `文件:行号`）。
pub const MAX_CELLS: usize = 16_000_000;

/// 格长档位（由文件长度唯一确定，见模块文档第一节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CellFormat {
    /// 单格 12 字节：只有前 12 字节共用字段。
    Old12,
    /// 单格 14 字节：12 字节共用字段 + `btTiles` + `btsmTiles`。
    New14,
    /// 单格 36 字节：14 字节字段 + 22 字节尾区（语义未知，原样保留）。
    Ext36,
}

impl CellFormat {
    /// 单格字节数。
    pub const fn cell_size(self) -> usize {
        match self {
            CellFormat::Old12 => OLD12_CELL_SIZE,
            CellFormat::New14 => NEW14_CELL_SIZE,
            CellFormat::Ext36 => EXT36_CELL_SIZE,
        }
    }

    /// 是否带库号字节（`btTiles` / `btsmTiles`）。
    pub const fn has_lib_bytes(self) -> bool {
        matches!(self, CellFormat::New14 | CellFormat::Ext36)
    }

    /// 是否带 22 字节尾区。
    pub const fn has_tail(self) -> bool {
        matches!(self, CellFormat::Ext36)
    }

    /// 档位名（用于日志/错误信息）。
    pub const fn name(self) -> &'static str {
        match self {
            CellFormat::Old12 => "Old12",
            CellFormat::New14 => "New14",
            CellFormat::Ext36 => "Ext36",
        }
    }

    /// 按「文件长度精确整除」判定档位；三档皆不成立返回 `None`。
    ///
    /// 判定顺序固定 `Old12 → New14 → Ext36`。本函数**只**看长度等式，不判尺寸合法性：
    /// `w*h == 0` 时三档同长 52，顺序上会落到 `Old12`。但正常入口 [`crate::Map::parse`]
    /// 在此之前就因 0 尺寸返回 [`crate::MapError::ZeroDimension`]、因超格数返回
    /// [`crate::MapError::TooManyCells`] ⇒ 这两个分支在解析路径上不可达。
    /// 保留纯长度语义是有意的：「唯一判据 = 文件长度精确整除」这条规则不许被
    /// 偷偷塞进第二套真相（尺寸合法性由 [`crate::Map::parse`] 单独把关）。
    pub fn detect(width: u16, height: u16, len: usize) -> Option<CellFormat> {
        let (old12, new14, ext36) = expected_lengths(width, height);
        let len = len as u64;
        if len == old12 {
            Some(CellFormat::Old12)
        } else if len == new14 {
            Some(CellFormat::New14)
        } else if len == ext36 {
            Some(CellFormat::Ext36)
        } else {
            None
        }
    }
}

/// 按 `w*h` 算出三档期望文件总长，顺序为 `(Old12, New14, Ext36)`。
///
/// 全程用 `u64` 计算：`u16::MAX * u16::MAX = 4294836225` 格，再乘 36 会溢出 `u32`，
/// 但不会溢出 `u64`。
pub fn expected_lengths(width: u16, height: u16) -> (u64, u64, u64) {
    let cells = width as u64 * height as u64;
    let head = HEADER_LEN as u64;
    (
        head + cells * OLD12_CELL_SIZE as u64,
        head + cells * NEW14_CELL_SIZE as u64,
        head + cells * EXT36_CELL_SIZE as u64,
    )
}

/// 一个地图格。
///
/// 字段与文件字节一一对应，不含任何推导值。`tiles` / `sm_tiles` 在 `Old12` 档恒为 0
/// 且写回时**不写出这两个字节**；`tail` 仅在 `Ext36` 档有意义，其余档恒为 `[0; 22]`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cell {
    /// 背景层图号（off 0）。
    pub bk_img: u16,
    /// 中间层图号（off 2）。
    pub mid_img: u16,
    /// 前景层图号（off 4）。
    pub fr_img: u16,
    /// 门索引（off 6）。
    pub door_index: u8,
    /// 门偏移（off 7）。
    pub door_offset: u8,
    /// 动画帧（off 8）。
    pub ani_frame: u8,
    /// 动画节拍（off 9）。
    pub ani_tick: u8,
    /// 区域（off 10）。
    pub area: u8,
    /// 光照（off 11）。
    pub light: u8,
    /// `btTiles`（off 12，仅 14/36 档；`Old12` 档恒 0）。
    pub tiles: u8,
    /// `btsmTiles`（off 13，仅 14/36 档；`Old12` 档恒 0）。
    pub sm_tiles: u8,
    /// 尾区 22 字节（off 14..36，仅 `Ext36` 档）。语义未知，**原样保存原样写回**。
    pub tail: [u8; TAIL_LEN],
}

impl Cell {
    /// 可走判定：背景层与前景层都不带 `0x8000` 位。
    ///
    /// 规格写法是 `((wBkImg & 0x8000) + (wFrImg & 0x8000)) == 0`。
    /// 这里改用等价的 `&&` 形式：`u16` 上 `0x8000 + 0x8000` 会溢出
    /// （release 下回绕成 0 ⇒ 把「两层都挡」误判成可走），`&&` 无此风险且语义完全相同。
    pub const fn is_walkable(&self) -> bool {
        (self.bk_img & 0x8000) == 0 && (self.fr_img & 0x8000) == 0
    }
}

/// 从单格原始字节解出 [`Cell`]。`raw.len()` 必须等于 `format.cell_size()`。
pub(crate) fn read_cell(format: CellFormat, raw: &[u8]) -> Cell {
    let mut tail = [0u8; TAIL_LEN];
    let (tiles, sm_tiles) = if format.has_lib_bytes() {
        (raw[LIB_OFFSET], raw[LIB_OFFSET + 1])
    } else {
        (0, 0)
    };
    if format.has_tail() {
        tail.copy_from_slice(&raw[TAIL_OFFSET..TAIL_OFFSET + TAIL_LEN]);
    }
    Cell {
        bk_img: u16::from_le_bytes([raw[0], raw[1]]),
        mid_img: u16::from_le_bytes([raw[2], raw[3]]),
        fr_img: u16::from_le_bytes([raw[4], raw[5]]),
        door_index: raw[6],
        door_offset: raw[7],
        ani_frame: raw[8],
        ani_tick: raw[9],
        area: raw[10],
        light: raw[11],
        tiles,
        sm_tiles,
        tail,
    }
}

/// 把 [`Cell`] 按档位追加到 `out` 尾部，字节数与 `format.cell_size()` 严格相等。
///
/// `Old12` 档**只写 12 字节**：`tiles` / `sm_tiles` / `tail` 即便被人为设成非零也不会落盘
/// （这两个字段在该档无对应字节，凭空补写会让文件变长、破坏往返保真）。
pub(crate) fn write_cell(cell: &Cell, format: CellFormat, out: &mut Vec<u8>) {
    out.extend_from_slice(&cell.bk_img.to_le_bytes());
    out.extend_from_slice(&cell.mid_img.to_le_bytes());
    out.extend_from_slice(&cell.fr_img.to_le_bytes());
    out.push(cell.door_index);
    out.push(cell.door_offset);
    out.push(cell.ani_frame);
    out.push(cell.ani_tick);
    out.push(cell.area);
    out.push(cell.light);
    if format.has_lib_bytes() {
        out.push(cell.tiles);
        out.push(cell.sm_tiles);
    }
    if format.has_tail() {
        out.extend_from_slice(&cell.tail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 档位字节数() {
        assert_eq!(CellFormat::Old12.cell_size(), 12);
        assert_eq!(CellFormat::New14.cell_size(), 14);
        assert_eq!(CellFormat::Ext36.cell_size(), 36);
        assert!(!CellFormat::Old12.has_lib_bytes());
        assert!(CellFormat::New14.has_lib_bytes());
        assert!(CellFormat::Ext36.has_lib_bytes());
        assert!(!CellFormat::New14.has_tail());
        assert!(CellFormat::Ext36.has_tail());
    }

    #[test]
    fn 判别式按长度精确整除() {
        // 30x35 = 1050 格
        assert_eq!(
            expected_lengths(30, 35),
            (52 + 12600, 52 + 14700, 52 + 37800)
        );
        assert_eq!(CellFormat::detect(30, 35, 12652), Some(CellFormat::Old12));
        assert_eq!(CellFormat::detect(30, 35, 14752), Some(CellFormat::New14));
        assert_eq!(CellFormat::detect(30, 35, 37852), Some(CellFormat::Ext36));
        // 差一字节即不成立
        assert_eq!(CellFormat::detect(30, 35, 12651), None);
        assert_eq!(CellFormat::detect(30, 35, 12653), None);
        assert_eq!(CellFormat::detect(30, 35, 14751), None);
        assert_eq!(CellFormat::detect(30, 35, 37851), None);
    }

    #[test]
    fn 零尺寸地图三档同长取old12() {
        // 纯长度语义：0×0 ⇒ 三档期望长度都是 52，顺序上落到 Old12。
        // ⚠ 这个分支在正式解析路径上**不可达**：Map::parse 先判 w/h 为 0 就报
        //    ZeroDimension（见 map.rs 的 `零尺寸一律报错...` 用例）。
        assert_eq!(expected_lengths(0, 0), (52, 52, 52));
        assert_eq!(CellFormat::detect(0, 0, 52), Some(CellFormat::Old12));
        assert_eq!(CellFormat::detect(0, 0, 53), None);
        // 单边为 0 同理（`52 + 0`）
        assert_eq!(expected_lengths(0, 7), (52, 52, 52));
        assert_eq!(expected_lengths(7, 0), (52, 52, 52));
    }

    #[test]
    fn 单元格读写三档自洽() {
        // 造一个各字段都非零、尾区也非零的格
        let mut cell = Cell {
            bk_img: 0x8123,
            mid_img: 0x4567,
            fr_img: 0x89AB,
            door_index: 1,
            door_offset: 2,
            ani_frame: 3,
            ani_tick: 4,
            area: 5,
            light: 6,
            tiles: 7,
            sm_tiles: 8,
            tail: [0u8; TAIL_LEN],
        };
        for (i, b) in cell.tail.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(1);
        }

        for fmt in [CellFormat::Old12, CellFormat::New14, CellFormat::Ext36] {
            let mut buf = Vec::new();
            write_cell(&cell, fmt, &mut buf);
            assert_eq!(buf.len(), fmt.cell_size(), "{fmt:?} 写出的字节数不对");
            let back = read_cell(fmt, &buf);
            assert_eq!(back.bk_img, cell.bk_img);
            assert_eq!(back.mid_img, cell.mid_img);
            assert_eq!(back.fr_img, cell.fr_img);
            assert_eq!(back.door_index, cell.door_index);
            assert_eq!(back.door_offset, cell.door_offset);
            assert_eq!(back.ani_frame, cell.ani_frame);
            assert_eq!(back.ani_tick, cell.ani_tick);
            assert_eq!(back.area, cell.area);
            assert_eq!(back.light, cell.light);
            if fmt.has_lib_bytes() {
                assert_eq!(back.tiles, cell.tiles);
                assert_eq!(back.sm_tiles, cell.sm_tiles);
            } else {
                assert_eq!(back.tiles, 0, "Old12 档 tiles 必须为 0");
                assert_eq!(back.sm_tiles, 0, "Old12 档 sm_tiles 必须为 0");
            }
            if fmt.has_tail() {
                assert_eq!(back.tail, cell.tail);
            } else {
                assert_eq!(back.tail, [0u8; TAIL_LEN], "{fmt:?} 档 tail 必须为 0");
            }
        }
    }

    #[test]
    fn 可走判定两层都看() {
        let mut c = Cell::default();
        assert!(c.is_walkable(), "两层都为 0 应可走");
        c.bk_img = 0x8000;
        assert!(!c.is_walkable(), "背景层带 0x8000 应不可走");
        c.bk_img = 0;
        c.fr_img = 0x8000;
        assert!(!c.is_walkable(), "前景层带 0x8000 应不可走");
        c.bk_img = 0x8000;
        assert!(
            !c.is_walkable(),
            "两层都带 0x8000 应不可走（不得因 u16 溢出误判）"
        );
        // 其它高位不影响
        c.bk_img = 0x7FFF;
        c.fr_img = 0x4000;
        assert!(c.is_walkable());
    }
}
