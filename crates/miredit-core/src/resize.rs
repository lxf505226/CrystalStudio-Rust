//! **地图尺寸变化**：工具 → 裁剪到选区（Crop）/ 地图扩边（Expand）（A-015，R36 口径）。
//!
//! 上游契约（逐条对照，`@901dbe2f`）：
//!
//! | 段 | 上游 | 本模块 |
//! |---|---|---|
//! | 裁剪边界 | `MapDocument.AlignSelection`（`Document.Crop.cs:26-36`）：`left/top = max(0, floor(v)) & ~1`、`right/bottom = min(尺寸, (ceil(v)+1) & ~1)`；`left>=right` ⇒ 抛「Select an area inside the map before cropping.」 | [`crop_bounds`]：闭区间选区 `(x0,y0,x1,y1)` ⇒ 半开边界 `[left,right)×[top,bottom)`，四边偶数对齐、夹在图内；空 ⇒ [`ResizeError::EmptySelection`] |
//! | 裁剪内容 | `MapDocument.Crop`（`:5-24`）：只搬 `Inside` 的格，坐标减 `(left, top)` | [`Document::replace_resized`] 的 `off=(left,top)` 分支 |
//! | 扩边前置 | `MapDimensions.CanExpand`（`MapDimensions.cs:23-31`）：四向 ≥0、**全偶数**、不能全 0；新尺寸单边 ≤32767 且格数 ≤16,000,000 | [`expand_target`] |
//! | 扩边内容 | `MapDocument.Expand`（`Document.Expand.cs:5-14`）：新图 `W+L+R × H+T+B`，旧格搬到 `(+left, +top)`，四周留空 | [`Document::replace_resized`] 的 `off=(-left,-top)` 分支 |
//! | 尺寸没变的裁剪 | `if (cropped.Width == Width && cropped.Height == Height) return false;`（`_upstream/CrystalStudio/MapViewport.Crop.cs:20`） | [`crate::EditSession::crop_to`] 同样早退：**不入撤销栈、不置脏** |
//! | 撤销 | `History.Resize`：压一条**整档快照**（`PushUndo(new([], null, state, Capture(document)))`） | [`crate::UndoStack`] 的 `StepKind::Resize`（整档 `RawMap` + `view`，可撤销**也可重做**） |
//!
//! # 支持范围（7 档；两档**显式拒绝**，不猜）
//!
//! | 格式 | 头部宽高 | 支持 |
//! |---|---|---|
//! | `Old12` / `Shanda14` / `Shanda2012` | 偏移 `0` / `2`（`tmap_dims`） | ✅ |
//! | `Wemade2010` | 偏移 `21` / `25`，值 = `新值 ^ xor`（`xor` 在 `23`，**不动**） | ✅ |
//! | `AntiHack` | 偏移 `31` / `35`，值 = `新值 ^ xor`（`xor` 在 `33`，**不动**） | ✅ |
//! | `Heroes34` | 偏移 `21` / `25`，**无 XOR** | ✅ |
//! | `Mir3Shanda` | 偏移 `16` / `18`，无 XOR | ✅ |
//! | `Mir3Wemade` | 头部**含随尺寸变化的半分辨率背景块**（`28 + 3·((w/2)+(w%2))·(h/2)`） | ❌ 拒绝 |
//! | `CrystalCs` | 变长格 + `trailer` 里的印章元数据/SHA256 链 | ❌ 拒绝 |
//!
//! ⚠ **R36 派单书第 4 条把 `AntiHack` 归进「off 0/2 无 XOR」那一组，那是错的**：
//! 上游 `LegacyMapReader.cs:332-340`（Type4）是 `offset = 31` → `w`、`offset += 2` → `xor`、
//! `offset += 2` → `h`、`Width = w ^ xor`，与 Type1（`Wemade2010`）**同一套**；
//! `mir2-format` 的 `antihack.rs:82-90`（`header_triplet` 读 `31/33/35`）与
//! `wemade2010.rs:145-153`（读 `21/23/25`）逐行对应。
//! 若按派单书写 `0/2`，会把宽高写进 TMapInfo 的**标题字节**、真正的宽高留在 `31/35` 不变
//! ⇒ 存出来的文件「头部说旧尺寸、格块是新尺寸」，自己都读不回来。
//! 本模块按源码实现 `31/35 ^ xor@33`，并有**逐字节测试**与 `dims()` 复核兜底。
//!
//! # `RawMap.trailer` 非空 ⇒ 一律拒绝
//!
//! 那段字节**没被解释**（实测 67 张 `EM001.map` 类文件各 8,040 字节，可能是真实格数据；
//! `CrystalCs` 的 trailer 是印章元数据 + SHA256 链，改尺寸后坐标全变 ⇒ 元数据必然失效）。
//! 宁可拒绝并说清原因，也不写出自相矛盾的文件。
//!
//! # 不做
//!
//! 不改 `mir2-format`（偏移表在本模块，且有 `dims()` 复核兜底）；不做印章元数据重写
//! （本仓的印章元数据只存在于 `CrystalCs` 的 `trailer`，而 `CrystalCs` 已整体拒绝）。

use std::fmt;

use mir2_format::{format_by_id, FormatId, MAX_CELLS, MAX_DIMENSION};
use mir2_map::Map;

use crate::convert::{sync_view_into_raw, view_from_raw};
use crate::document::Document;
use crate::selection::Selection;

/// 尺寸变化的结果（裁剪 / 扩边共用）。
///
/// `changed == false` ⇒ **尺寸没变**（选取覆盖整图）⇒ 什么都没做：不入撤销栈、不置脏、
/// 字节不变（上游 `MapViewport.CropSelection` 的 `cropped.Width == Width && … return false`
/// 同一条早退，本仓只是把它显式报出来）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResizeOutcome {
    /// 结果宽。
    pub width: u16,
    /// 结果高。
    pub height: u16,
    /// 是否真的改了尺寸。
    pub changed: bool,
}

/// 尺寸变化被拒的原因（**每一句都能直接上屏**）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResizeError {
    /// 该格式不支持改尺寸（含"头部结构随尺寸变化"的 `Mir3Wemade`、变长格的 `CrystalCs`）。
    UnsupportedFormat {
        /// 该图的格式。
        format: FormatId,
        /// 为什么不行（中文，可直接上屏）。
        why: &'static str,
    },
    /// `raw.trailer` 非空：那段字节没被解释，改尺寸会写出自相矛盾的文件。
    Trailer {
        /// 未解释的字节数。
        bytes: usize,
    },
    /// 新尺寸越界（单边 > 32767，或格数 > 16,000,000，或某边为 0）。
    Dimensions {
        /// 目标宽。
        width: u32,
        /// 目标高。
        height: u32,
    },
    /// 选区在图内没有可裁剪的格（全在图外 / 反向）。
    EmptySelection,
    /// 扩边四向全 0（上游 `(left|top|right|bottom) == 0` ⇒ 拒绝）。
    NothingToDo,
    /// 扩边量出现负数。
    NegativeAmount,
    /// 扩边量出现奇数（上游要求"以 2 格为单位"）。
    OddAmount,
    /// 视图与字节层形状不一致（内部护栏，理论上不可达）。
    Shape(String),
}

impl fmt::Display for ResizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResizeError::UnsupportedFormat { format, why } => {
                write!(f, "该格式不支持改尺寸（{}）：{why}", format.csharp_loader())
            }
            ResizeError::Trailer { bytes } => write!(
                f,
                "这张图有 {bytes} 字节未解释的尾块（可能是格数据或印章元数据），改尺寸会写出自相矛盾的文件"
            ),
            ResizeError::Dimensions { width, height } => write!(
                f,
                "新尺寸 {width}×{height} 越界（单边 ≤32767、总格数 ≤16,000,000）"
            ),
            ResizeError::EmptySelection => f.write_str("先在地图上框选要保留的部分再裁剪"),
            ResizeError::NothingToDo => f.write_str("四向扩边量不能全为 0"),
            ResizeError::NegativeAmount => f.write_str("扩边量不能是负数"),
            ResizeError::OddAmount => f.write_str("扩边量必须是偶数（地图按 2 格对齐）"),
            ResizeError::Shape(why) => write!(f, "视图与字节层形状不一致：{why}"),
        }
    }
}

impl std::error::Error for ResizeError {}

// ───────────────────────────── 纯计算：裁剪 / 扩边 ─────────────────────────────

/// 裁剪边界（上游 `MapDocument.AlignSelection`）：四边**对齐到偶数**、夹在 `[0,W]×[0,H]`。
///
/// 入参是本仓的**闭区间**选区（`x1`/`y1` 含端点）；返回**半开**边界
/// `(left, top, right, bottom)` ⇒ 裁剪后的尺寸 = `right-left × bottom-top`。
///
/// 逐条照抄上游（`Rect` 半开 + 构造时 `width = |dx|+1` ⇒ `Right = x1+1`）：
///
/// ```text
/// left   = max(0, floor(x0)) & ~1            = x0 & !1
/// top    = max(0, floor(y0)) & ~1            = y0 & !1
/// right  = min(W, (ceil(x1+1) + 1) & ~1)     = min(W, (x1+2) & !1)
/// bottom = min(H, (ceil(y1+1) + 1) & ~1)     = min(H, (y1+2) & !1)
/// ```
///
/// ⚠ 右/下边**往外**圆整（`(x1+2)&~1` 而不是 `(x1+1)&~1`）：这正是上游那行
/// `(Math.Ceiling(selection.Right) + 1) & ~1` 的结果 —— 1×1 选区在 (5,5) 上裁出 2×2。
///
/// # Errors
///
/// 边界退化成空（`left >= right` 或 `top >= bottom`）⇒ [`ResizeError::EmptySelection`]。
pub fn crop_bounds(
    width: u16,
    height: u16,
    sel: Selection,
) -> Result<(u16, u16, u16, u16), ResizeError> {
    let (w, h) = (u32::from(width), u32::from(height));
    let left = u32::from(sel.x0.min(sel.x1)) & !1;
    let top = u32::from(sel.y0.min(sel.y1)) & !1;
    let right = (u32::from(sel.x0.max(sel.x1)) + 2) & !1;
    let bottom = (u32::from(sel.y0.max(sel.y1)) + 2) & !1;
    let (left, top) = (left.min(w), top.min(h));
    let (right, bottom) = (right.min(w), bottom.min(h));
    if left >= right || top >= bottom {
        return Err(ResizeError::EmptySelection);
    }
    Ok((
        u16::try_from(left).unwrap_or(0),
        u16::try_from(top).unwrap_or(0),
        u16::try_from(right).unwrap_or(width),
        u16::try_from(bottom).unwrap_or(height),
    ))
}

/// 扩边后的尺寸（上游 `MapDimensions.CanExpand` 逐条对照）。
///
/// 判据：四向 ≥0、**全偶数**、不能全 0；新尺寸 `1..=32767` 且格数 ≤16,000,000。
///
/// # Errors
///
/// - 负数 ⇒ [`ResizeError::NegativeAmount`]；奇数 ⇒ [`ResizeError::OddAmount`]；
/// - 全 0 ⇒ [`ResizeError::NothingToDo`]；越界 ⇒ [`ResizeError::Dimensions`]。
pub fn expand_target(
    width: u16,
    height: u16,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> Result<(u16, u16), ResizeError> {
    let 四向 = [left, top, right, bottom];
    if 四向.iter().any(|v| *v < 0) {
        return Err(ResizeError::NegativeAmount);
    }
    if 四向.iter().any(|v| v & 1 != 0) {
        return Err(ResizeError::OddAmount);
    }
    if 四向.iter().all(|v| *v == 0) {
        return Err(ResizeError::NothingToDo);
    }
    let nw = i64::from(width) + i64::from(left) + i64::from(right);
    let nh = i64::from(height) + i64::from(top) + i64::from(bottom);
    check_dims(nw, nh)?;
    Ok((
        u16::try_from(nw).unwrap_or(width),
        u16::try_from(nh).unwrap_or(height),
    ))
}

/// 尺寸上限（上游 `CanRoundTrip` 的写侧等价物）：`1..=32767` 且格数 ≤ 16,000,000。
fn check_dims(width: i64, height: i64) -> Result<(), ResizeError> {
    let max_side = i64::from(MAX_DIMENSION);
    if width < 1
        || height < 1
        || width > max_side
        || height > max_side
        || width * height > MAX_CELLS as i64
    {
        return Err(ResizeError::Dimensions {
            width: u32::try_from(width).unwrap_or(0),
            height: u32::try_from(height).unwrap_or(0),
        });
    }
    Ok(())
}

// ───────────────────────── 头部尺寸：逐格式偏移表 ─────────────────────────

/// 头部里「宽 / 高」的位置与编码方式（**逐格式唯一表**）。
#[derive(Clone, Copy, Debug)]
struct DimsSlot {
    /// 宽所在的偏移（i16 小端）。
    w_at: usize,
    /// 高所在的偏移（i16 小端）。
    h_at: usize,
    /// XOR 键的偏移（`Some` ⇒ 宽高都要先与这个 i16 异或再写）。
    xor_key_at: Option<usize>,
}

/// 该格式**为什么不支持**改尺寸（`None` = 支持）。
fn unsupported_reason(format: FormatId) -> Option<&'static str> {
    use FormatId as F;
    match format {
        F::Mir3Wemade => Some(
            "头部里含随尺寸变化的半分辨率背景块（28 + 3·((w/2)+(w%2))·(h/2) 字节），\
             改尺寸要先重建整块（本单不做，且零真实样本）",
        ),
        F::CrystalCs => Some(
            "格长变长（26/38/35/33+，由版本与前景个数决定），且 trailer 里是印章元数据 + SHA256 链，\
             改尺寸会让元数据整体失效",
        ),
        _ => None,
    }
}

/// 该格式的头部尺寸槽位；`None` = 不支持改尺寸（原因见 [`unsupported_reason`]）。
///
/// | 格式 | 宽 / 高 | 依据（`mir2-format` 的读侧） |
/// |---|---|---|
/// | `Old12` / `Shanda14` / `Shanda2012` | `0` / `2` | `tmap_dims`（`old12.rs:108`、`shanda14.rs:126`、`shanda2012.rs:266`） |
/// | `Wemade2010` | `21` / `25`（`^xor@23`） | `header_triplet`（`wemade2010.rs:145-153`；C# `:174-181`） |
/// | `AntiHack` | `31` / `35`（`^xor@33`） | `header_triplet`（`antihack.rs:82-90`；C# `:332-340`） |
/// | `Heroes34` | `21` / `25`（无 XOR） | `i16_at(21)/i16_at(25)`（`heroes34.rs:73`；C# `:520-522`） |
/// | `Mir3Shanda` | `16` / `18`（无 XOR） | `i16_at(16)/i16_at(18)`（`mir3_shanda.rs:82`；C# `:460-464`） |
/// | `Mir3Wemade` / `CrystalCs` | — | 不支持（见 [`unsupported_reason`]） |
const fn dims_slot(format: FormatId) -> Option<DimsSlot> {
    use FormatId as F;
    match format {
        F::Old12 | F::Shanda14 | F::Shanda2012 => Some(DimsSlot {
            w_at: 0,
            h_at: 2,
            xor_key_at: None,
        }),
        F::Wemade2010 => Some(DimsSlot {
            w_at: 21,
            h_at: 25,
            xor_key_at: Some(23),
        }),
        // ⚠ **不是** `0/2`：Type4 与 Type1 同一套（`w@31`、`xor@33`、`h@35`）。
        F::AntiHack => Some(DimsSlot {
            w_at: 31,
            h_at: 35,
            xor_key_at: Some(33),
        }),
        F::Heroes34 => Some(DimsSlot {
            w_at: 21,
            h_at: 25,
            xor_key_at: None,
        }),
        F::Mir3Shanda => Some(DimsSlot {
            w_at: 16,
            h_at: 18,
            xor_key_at: None,
        }),
        F::Mir3Wemade | F::CrystalCs => None,
    }
}

/// 读头部的一个 `i16`（小端）。
fn read_i16(header: &[u8], at: usize) -> Option<i16> {
    let b = header.get(at..at + 2)?;
    Some(i16::from_le_bytes([b[0], b[1]]))
}

/// 写头部的一个 `i16`（小端）；越界返回 `false`。
fn write_i16(header: &mut [u8], at: usize, v: i16) -> bool {
    let Some(slot) = header.get_mut(at..at + 2) else {
        return false;
    };
    slot.copy_from_slice(&v.to_le_bytes());
    true
}

/// 把新宽高写回头部，并用 `mir2-format` 自己的 `dims()` **复核**。
///
/// 只动宽高两个 `i16`（XOR 档先与既有键异或），其余字节**一个都不动** ——
/// 标题、版本号、`xor` 键本身都保持原样。
///
/// # Errors
///
/// - 格式不支持 / 头部长度与该格式不符 ⇒ [`ResizeError::UnsupportedFormat`]；
/// - 写完之后 `dims()` 读不回 `(width, height)` ⇒ 同一变体（**绝不赌**）。
fn patch_dims(
    header: &mut [u8],
    format: FormatId,
    width: u16,
    height: u16,
) -> Result<(), ResizeError> {
    let Some(slot) = dims_slot(format) else {
        return Err(ResizeError::UnsupportedFormat {
            format,
            why: unsupported_reason(format).unwrap_or("该格式不支持改尺寸"),
        });
    };
    if header.len() != format_by_id(format).header_len() {
        return Err(ResizeError::UnsupportedFormat {
            format,
            why: "头部长度与该格式要求的长度不符",
        });
    }
    let key = match slot.xor_key_at {
        Some(at) => read_i16(header, at).ok_or(ResizeError::UnsupportedFormat {
            format,
            why: "头部取不到宽高 XOR 键",
        })?,
        None => 0,
    };
    let bad = || ResizeError::Dimensions {
        width: u32::from(width),
        height: u32::from(height),
    };
    let w = i16::try_from(width).map_err(|_| bad())?;
    let h = i16::try_from(height).map_err(|_| bad())?;
    if !write_i16(header, slot.w_at, w ^ key) || !write_i16(header, slot.h_at, h ^ key) {
        return Err(ResizeError::UnsupportedFormat {
            format,
            why: "头部写不下宽高（偏移越界）",
        });
    }
    // 复核：用格式自己的读侧把宽高读回来（表写错了这里当场拒绝）
    if format_by_id(format).dims(header) != Some((width, height)) {
        return Err(ResizeError::UnsupportedFormat {
            format,
            why: "写入宽高后该格式的 dims() 读不回来（头部槽位表与 format 不一致）",
        });
    }
    Ok(())
}

// ───────────────────────────── Document：改尺寸 ─────────────────────────────

impl Document {
    /// **改尺寸**（裁剪 / 扩边共用）：先把「当前编辑视图」同步进字节层，再按
    /// `(off_x, off_y)` 重排格，最后**重建视图 + 五层计数**并置脏。
    ///
    /// `off_x/off_y` 的语义是「新格 `(nx, ny)` 取旧格 `(nx+off_x, ny+off_y)`」：
    ///
    /// - **裁剪** `off = (left, top)`：新图左上角 = 旧图的 `(left, top)`；
    /// - **扩边** `off = (-left, -top)`：旧图整体右下移，四周是新空白格。
    ///
    /// 新图里落在旧图之外的格 = **空**（`Cell::default()` 字段 + 与该文件同长的全 0 尾区，
    /// 因为 writer 对尾区长度有硬校验）。头部宽高按 [`dims_slot`] 改写并自校验。
    /// `path` / `file_name` / `orig_bytes` **一概不动**（"打开时是什么"不因一次改尺寸而改写）。
    ///
    /// 视图**就地**改字段（不换 `Map` 结构体）：宿主的画布把同一个 `Map` 对象换进换出。
    ///
    /// # 失败即**什么都不写**
    ///
    /// 所有校验（格式 / 尾块 / 尺寸 / 头部槽位 / 视图形状）都在**任何赋值之前**完成，
    /// 失败时 `self` 一个字段都不变 ⇒ 上层可以直接把 [`ResizeError`] 当回执。
    ///
    /// # Errors
    ///
    /// 见 [`ResizeError`]。
    pub fn replace_resized(
        &mut self,
        width: u16,
        height: u16,
        off_x: i32,
        off_y: i32,
    ) -> Result<(), ResizeError> {
        let format = self.raw().format;
        if !self.raw().trailer.is_empty() {
            return Err(ResizeError::Trailer {
                bytes: self.raw().trailer.len(),
            });
        }
        if let Some(why) = unsupported_reason(format) {
            return Err(ResizeError::UnsupportedFormat { format, why });
        }
        check_dims(i64::from(width), i64::from(height))?;
        // ① 视图 → 字节层（与保存同一条映射；否则改尺寸会把未保存的编辑丢掉）
        let mut raw = self.raw().clone();
        sync_view_into_raw(self.view(), &mut raw).map_err(|e| ResizeError::Shape(e.to_string()))?;
        // ② 头部宽高（在副本上改；`patch_dims` 内部还会用 `dims()` 复核）
        let mut header = raw.header_raw.clone();
        patch_dims(&mut header, format, width, height)?;
        // ③ 重排格（列优先 `x*h + y`；新图空白格的尾区长度跟该文件一致）
        let tail_len = raw.cells.first().map_or(0, |c| c.tail.len());
        let (nw, nh) = (usize::from(width), usize::from(height));
        let mut cells: Vec<mir2_format::Cell> = Vec::with_capacity(nw * nh);
        for nx in 0..nw {
            for ny in 0..nh {
                let sx = nx as i32 + off_x;
                let sy = ny as i32 + off_y;
                let src = if sx >= 0
                    && sy >= 0
                    && sx < raw.width as i32
                    && sy < raw.height as i32
                {
                    raw.cells
                        .get(sx as usize * raw.height as usize + sy as usize)
                        .cloned()
                } else {
                    None
                };
                cells.push(src.unwrap_or_else(|| mir2_format::Cell::zeroed(tail_len)));
            }
        }
        // ④ 到这里才动 self（前面任何一步 `?` 都不会留下半改状态）
        raw.width = width;
        raw.height = height;
        raw.header_raw = header;
        raw.cells = cells;
        *self.raw_mut() = raw;
        self.rebuild_view_in_place();
        self.rebuild_layer_counts();
        self.mark_dirty();
        Ok(())
    }

    /// 按当前 `raw` 重建视图（**就地**改字段，不换结构体）。
    pub(crate) fn rebuild_view_in_place(&mut self) {
        let fresh: Map = view_from_raw(self.raw());
        let view = self.view_mut();
        view.width = fresh.width;
        view.height = fresh.height;
        view.format = fresh.format;
        view.header_raw = fresh.header_raw;
        view.cells = fresh.cells;
    }
}

#[cfg(test)]
#[path = "resize_tests.rs"]
mod tests;
