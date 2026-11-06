//! 单元格元数据（光照 / 门索引 / 门偏移）的**纯函数**口径（R12-F1）。
//!
//! 上游契约：
//! * 对话框与「只写改过的字段」：`CrystalStudio/MainWindow.axaml.cs:740-786`（`OpenCellProperties`）；
//! * 选区共同值：`MapViewport.cs:567-578`（`CommonMetadata`）；
//! * 批量落笔：`MapViewport.cs:579-597`（`ApplyMetadata`）。
//!
//! 本模块**只有纯函数**（不碰会话、不碰 IO），GUI / web / core 三端共用同一份口径：
//!
//! | 函数 | 用途 |
//! |---|---|
//! | [`clamp_area`] | 闭区间区域 → 图内区域（空 ⇒ `None`） |
//! | [`no_door_bytes_reason`] | 本格式有没有门字节（Mir3 两档**没有**） |
//!
//! 「共同值 / 混合」那一步在 GUI（`cell_props`）里算：它要读地图、要报"哪一格不同"，
//! 属于**取数**而非格式知识；core 只需要这里的夹紧与能力判据。

use mir2_format::FormatId;

/// 格区域：**闭区间** `(x0, y0, x1, y1)`，两端都含。
///
/// 与 [`crate::Command::SetSelection`] 的 `x0/y0/x1/y1` 同口径（本仓全链路闭区间；
/// 上游 C# 的 `Rect` 是 `(X, Y, Width, Height)` 开区间，两者在原地换算时差 1，
/// 见 `selection.rs` 的说明）。
pub type Area = (u16, u16, u16, u16);

/// 把区域夹进图内（`0..=w-1 × 0..=h-1`）；夹完为空 ⇒ `None`。
///
/// # 口径
///
/// 上游 `MapViewport.cs:585-586` 用
/// `for (y = Math.Max(0, (int)area.Y); y < Math.Min(Document.Height, area.Bottom); y++)`
/// 做**开区间**裁剪；本仓闭区间 ⇒ 等价的写法是「坐标各自 `min(尺寸-1)`，再判空」。
///
/// 颠倒的输入（`x0 > x1` 或 `y0 > y1`）按上游 `Rect` 的构造语义**规范化**（交换端点）——
/// 上游从 `Rect` 取值时不可能出现颠倒，规范化只是让"调用方算错端点"不至于静默变成
/// 「一个格都不改」。
///
/// 零尺寸图（`w == 0 || h == 0`）⇒ `None`：这种图任何区域都是空的。
#[must_use]
pub fn clamp_area(area: Area, w: u16, h: u16) -> Option<Area> {
    if w == 0 || h == 0 {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = area;
    if x0 > x1 {
        std::mem::swap(&mut x0, &mut x1);
    }
    if y0 > y1 {
        std::mem::swap(&mut y0, &mut y1);
    }
    let (x0, x1) = (x0.min(w - 1), x1.min(w - 1));
    let (y0, y1) = (y0.min(h - 1), y1.min(h - 1));
    (x0 <= x1 && y0 <= y1).then_some((x0, y0, x1, y1))
}

/// 本格式**没有门字节**的原因（`door_index` / `door_offset` 存不进磁盘）；有门字节 ⇒ `None`。
///
/// # 为什么 Mir3 两档没有
///
/// `mir2-format/src/formats/mir3_wemade.rs:61` 记 C# `MapCodec:432` 的注释
/// 「mir3 maps dont have doors」：那 3 字节被**当尾区原样保留**（落在 `Cell::tail`），
/// 既不由 `door_index`/`door_offset` 建模，writer 也不写它 ⇒ 在这两档上改门字段是**静默丢**。
///
/// 其余 7 档（`Old12` / `Shanda14` / `Shanda2012` / `AntiHack` / `Wemade2010` /
/// `Heroes34` / `CrystalCs`）的 writer 都会逐字节写出这两个字段 ⇒ `None`。
///
/// ⚠ 这是**能力判据**，不是"能不能写回"：九档都能写回（`FormatCaps::can_write` 全 `true`），
/// 只是这两档的格布局里根本没有这两个字段的位置。
#[must_use]
pub const fn no_door_bytes_reason(fmt: FormatId) -> Option<&'static str> {
    match fmt {
        FormatId::Mir3Wemade | FormatId::Mir3Shanda => Some(
            "本格式（Mir3）的格布局没有门字节（C# 原文「mir3 maps dont have doors」）\
             ，门索引 / 门偏移无处可写，已忽略",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **夹紧逐条**：正常 / 越界 / 颠倒 / 零尺寸。
    #[test]
    fn 区域夹紧() {
        // 正常：原样
        assert_eq!(clamp_area((1, 2, 3, 4), 8, 8), Some((1, 2, 3, 4)));
        // 右下越界：夹到 7/7（闭区间 ⇒ 尺寸-1）
        assert_eq!(clamp_area((3, 3, 99, 99), 8, 8), Some((3, 3, 7, 7)));
        // 左上越界：坐标是 u16 非负，不存在"负数"，只可能被 0 压住
        assert_eq!(clamp_area((0, 0, 0, 0), 8, 8), Some((0, 0, 0, 0)));
        // 颠倒：规范化成 (1,2)-(3,4)
        assert_eq!(clamp_area((3, 4, 1, 2), 8, 8), Some((1, 2, 3, 4)));
        // 零尺寸图：任何区域都空
        assert_eq!(clamp_area((0, 0, 0, 0), 0, 8), None);
        assert_eq!(clamp_area((0, 0, 0, 0), 8, 0), None);
        // 恰好整图
        assert_eq!(clamp_area((0, 0, 7, 7), 8, 8), Some((0, 0, 7, 7)));
    }

    /// **九种格式的门字节能力表**：只有两个 Mir3 档没有。
    #[test]
    fn 门字节能力逐格式() {
        let 九档 = [
            FormatId::Old12,
            FormatId::Shanda14,
            FormatId::Shanda2012,
            FormatId::AntiHack,
            FormatId::Wemade2010,
            FormatId::Heroes34,
            FormatId::Mir3Wemade,
            FormatId::Mir3Shanda,
            FormatId::CrystalCs,
        ];
        assert_eq!(九档.len(), 9, "九种格式都要在表里");
        for fmt in 九档 {
            let 原因 = no_door_bytes_reason(fmt);
            let 应当没有 = matches!(fmt, FormatId::Mir3Wemade | FormatId::Mir3Shanda);
            assert_eq!(
                原因.is_some(),
                应当没有,
                "{fmt:?} 的门字节能力判错（原因 = {原因:?}）"
            );
            if let Some(文本) = 原因 {
                assert!(文本.contains("mir3 maps dont have doors"), "{文本}");
            }
        }
    }
}
