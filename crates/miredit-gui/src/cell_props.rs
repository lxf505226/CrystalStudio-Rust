//! 单元格属性模态的**取数与校验**（R12-F1：光照 / 门索引 / 门偏移）。
//!
//! 上游真值：
//! * 对话框与「只写改过的字段」：`_upstream/CrystalStudio/MainWindow.axaml.cs:740-784`
//!   （`OpenCellProperties`：三字段初值 = `CommonMetadata(...)?.ToString() ?? ""`，
//!   提交时 `box.Text == initialValues[box] ? null : parsed`）；
//! * 选区共同值：`_upstream/CrystalStudio/MapViewport.cs:567-578`（`CommonMetadata`：按 `area` clamp 后遍历，
//!   任两格不同 ⇒ `null`）；
//! * 落笔：`_upstream/CrystalStudio/MapViewport.cs:579-597`（`ApplyMetadata`，实现在
//!   [`miredit_core::EditSession::apply_metadata`]）。
//!
//! 本模块只有**纯函数**（不画、不碰会话），好让 `cargo test --lib` 不开窗就能验：
//!
//! | 函数 | 对应上游 |
//! |---|---|
//! | [`共同值`] / [`初值`] | `CommonMetadata` + `common?.ToString() ?? ""` |
//! | [`解析输入`] | `byte.TryParse`（空白 ⇒ `null`） |
//! | [`决定写入`] | `Text == initialValues[Text] ? null : parsed` |
//! | [`头行`] | `Cell {x}, {y}` / `Apply to Selected Cells` |
//!
//! 字段语义：三个都是 `0..=255` 的格属性（`mir2_map::Cell` 的 `light` / `door_index` /
//! `door_offset`）。门两字段在 **Mir3 两档**上没有字节可写，由 [`字段::可用`] 判
//! （原因见 [`crate::ui_text::CELL_PROPS_NO_DOOR`]）。

use mir2_map::Cell;

use crate::ui_text as tx;

/// 三个可编辑字段（顺序 = 上游对话框的 Light / Door Index / Door Offset）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum 字段 {
    /// 光照（`Cell::light`，0..=255）。
    光照,
    /// 门索引（`Cell::door_index`，0..=255）。
    门索引,
    /// 门偏移（`Cell::door_offset`，0..=255）。
    门偏移,
}

impl 字段 {
    /// 全部字段，**按上游对话框的先后顺序**。
    pub const 全: [Self; 3] = [Self::光照, Self::门索引, Self::门偏移];

    /// 面板里的下标（0..=2；焦点/矩形数组都用它）。
    #[must_use]
    pub const fn 下标(self) -> usize {
        match self {
            Self::光照 => 0,
            Self::门索引 => 1,
            Self::门偏移 => 2,
        }
    }

    /// 字段标题（官方 `"Light (0–255)"` / `"Door Index (0–255)"` / `"Door Offset (0–255)"`）。
    #[must_use]
    pub const fn 标签(self) -> &'static str {
        match self {
            Self::光照 => tx::CELL_PROPS_LIGHT,
            Self::门索引 => tx::CELL_PROPS_DOOR,
            Self::门偏移 => tx::CELL_PROPS_OFFSET,
        }
    }

    /// 读该格这一字段的真值。
    #[must_use]
    pub const fn 读(self, c: &Cell) -> u8 {
        match self {
            Self::光照 => c.light,
            Self::门索引 => c.door_index,
            Self::门偏移 => c.door_offset,
        }
    }

    /// 这一字段是不是**门字段**（要门字节才能落盘）。
    #[must_use]
    pub const fn 是门字段(self) -> bool {
        !matches!(self, Self::光照)
    }

    /// 本格式能不能编辑这一字段；不能 ⇒ `Err(原因)`（原因原样上屏）。
    ///
    /// Mir3 两档的格布局里没有门字节（`mir2-format/src/formats/mir3_wemade.rs:61` 记
    /// C# 的「mir3 maps dont have doors」）⇒ 那两个框**禁用**并显示原因，
    /// 绝不做一个"点了没反应"的假输入框。
    pub fn 可用(self, 有门字节: bool) -> Result<(), &'static str> {
        if self.是门字段() && !有门字节 {
            return Err(tx::CELL_PROPS_NO_DOOR);
        }
        Ok(())
    }
}

/// 一个输入框的解析结果。
///
/// 为什么不用 `Result<Option<u8>, ()>`：`()` 不携带原因（clippy `result_unit_err` 会拦），
/// 而这个三态本来就比 `Result` 更贴上游 —— 上游是 `bool Parse(out byte? value)`：
/// 「空白 ⇒ `true` + `null`」「数字 ⇒ `true` + 值」「非法 ⇒ `false`」。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum 取值 {
    /// 空白 / 与初值逐字相同 ⇒ **这一字段不动**（上游传 `null`）。
    不动,
    /// 解析成功 ⇒ 把该字段设成这个值（`0` 也是"设成 0"，不是"不动"）。
    设为(u8),
    /// 非法（负数 / 超 255 / 小数 / 文字）⇒ **整个提交被拒**、一格不写。
    非法,
}

/// 选区共同值：**全部相同** ⇒ `Some(值)`；有差异或区域为空 ⇒ `None`（上游的 Mixed）。
///
/// `area` 是闭区间格坐标，内部按 [`miredit_core::clamp_area`] 夹进图内（与落笔同一份口径）。
#[must_use]
pub fn 共同值(map: &mir2_map::Map, area: miredit_core::Area, f: 字段) -> Option<u8> {
    let (x0, y0, x1, y1) = miredit_core::clamp_area(area, map.width, map.height)?;
    let mut 值: Option<u8> = None;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let c = map.cell(x, y)?;
            let v = f.读(c);
            match 值 {
                Some(got) if got != v => return None,
                _ => 值 = Some(v),
            }
        }
    }
    值
}

/// 输入框初值文本：共同值 ⇒ 十进制；Mixed（或空区域）⇒ **空串**（官方 `?? ""`）。
#[must_use]
pub fn 初值(map: &mir2_map::Map, area: miredit_core::Area, f: 字段) -> String {
    match 共同值(map, area, f) {
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// 区域夹紧后的格数（`0` = 空区域/全在图外）。
#[must_use]
pub fn 格数(map: &mir2_map::Map, area: miredit_core::Area) -> usize {
    match miredit_core::clamp_area(area, map.width, map.height) {
        Some((x0, y0, x1, y1)) => {
            (usize::from(x1) - usize::from(x0) + 1) * (usize::from(y1) - usize::from(y0) + 1)
        }
        None => 0,
    }
}

/// 解析一个输入框：**空白 ⇒ [`取值::不动`]**；`0..=255` ⇒ [`取值::设为`]；
/// 其余（负数 / 超界 / 小数 / 非数字）⇒ [`取值::非法`]（官方 `byte.TryParse` 失败）。
///
/// ⚠ `"0"` 必须解析成 [`取值::设为`]`(0)`（"把值设成 0"），**不是** [`取值::不动`]
/// —— 这两者在 Mixed 选区里是相反的结果。
#[must_use]
pub fn 解析输入(s: &str) -> 取值 {
    let t = s.trim();
    if t.is_empty() {
        return 取值::不动;
    }
    match t.parse::<u8>() {
        Ok(v) => 取值::设为(v),
        Err(_) => 取值::非法,
    }
}

/// 与初值**逐字**比较后决定写什么：没改动 ⇒ [`取值::不动`]；改动了 ⇒ [`解析输入`]。
///
/// 为什么逐字比较（而不是比值）：上游比的也是 `TextBox.Text` 原文（`_upstream/CrystalStudio/MainWindow.axaml.cs:777-779`），
/// 于是"Mixed 的空框仍是空框"⇒ 不动，「把 Mixed 填成 9」⇒ 写 9，
/// 「把已经相同的 9 再敲一遍 9」⇒ 原文没变 ⇒ **不写**（不产生无意义的撤销步）。
#[must_use]
pub fn 决定写入(当前: &str, 初值: &str) -> 取值 {
    if 当前 == 初值 {
        return 取值::不动;
    }
    解析输入(当前)
}

/// 对话框头行：`area` 夹紧后恰好 1 格 ⇒ 「单元格 x,y」；否则 ⇒「应用到选区 N 格」。
///
/// 上游是 `area.Width == 1 && area.Height == 1 ? "Cell {x},{y}" : "Apply to Selected Cells"`；
/// 本仓把后者的格数也写上（上游那句不告诉用户影响多少格，多给这一个数不改变语义：
/// 格数由同一份夹紧口径算出）。
#[must_use]
pub fn 头行(map: &mir2_map::Map, area: miredit_core::Area) -> String {
    let n = 格数(map, area);
    if n == 1 {
        let (x0, y0, _, _) =
            miredit_core::clamp_area(area, map.width, map.height).unwrap_or((0, 0, 0, 0));
        return tx::fill(
            tx::CELL_PROPS_HEADER_CELL_TMPL,
            &[("x", &x0.to_string()), ("y", &y0.to_string())],
        );
    }
    tx::fill(tx::CELL_PROPS_HEADER_AREA_TMPL, &[("n", &n.to_string())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_map::CellFormat;

    /// 合成 4×4 图：每格 `light = 列优先格序 + 1`、`door_index = 7`、`door_offset = 0`。
    ///
    /// 「每格 light 不同」是故意的：这样"选区共同值 = None（Mixed）"才有真事可测。
    fn 造图(w: u16, h: u16) -> mir2_map::Map {
        let mut bytes = vec![0u8; mir2_map::HEADER_LEN + 12 * (w as usize * h as usize)];
        bytes[0..2].copy_from_slice(&w.to_le_bytes());
        bytes[2..4].copy_from_slice(&h.to_le_bytes());
        for i in 0..(w as usize * h as usize) {
            let base = mir2_map::HEADER_LEN + i * 12;
            bytes[base + 6] = 7; // door_index
            bytes[base + 11] = (i as u8) + 1; // light
        }
        let m = mir2_map::Map::parse(&bytes).expect("合成图应能解析");
        assert_eq!(m.format, CellFormat::Old12);
        m
    }

    /// **共同值**：单格 ⇒ 该值；一行里 light 各不相同 ⇒ None；门索引相同 ⇒ 有共同值。
    #[test]
    fn 共同值三种情形() {
        let m = 造图(4, 4);
        // 单格（1,1）：light = 列优先 index + 1 = 1*4 + 1 + 1 = 6
        assert_eq!(共同值(&m, (1, 1, 1, 1), 字段::光照), Some(6));
        assert_eq!(共同值(&m, (1, 1, 1, 1), 字段::门索引), Some(7));
        assert_eq!(共同值(&m, (1, 1, 1, 1), 字段::门偏移), Some(0));
        // 一整行 light 各不相同 ⇒ Mixed
        assert_eq!(共同值(&m, (0, 0, 3, 0), 字段::光照), None, "不同 ⇒ None");
        // 同一行的门索引全是 7 ⇒ 仍能给出共同值
        assert_eq!(共同值(&m, (0, 0, 3, 0), 字段::门索引), Some(7));
        // 全越界 ⇒ 夹紧后是右下角 1 格（不是空）
        assert_eq!(共同值(&m, (50, 50, 60, 60), 字段::门索引), Some(7));
        // 零尺寸区域不存在（闭区间 + 夹紧只会缩到 1 格）；真正的"空"由 core 的
        // `clamp_area(w == 0 || h == 0) == None` 提供，gui 侧无法造 0×0 图（解析器拒），
        // 所以这里直接钉 core 的那条判据，保证 `共同值` 的 `?` 分支有据可依。
        assert_eq!(miredit_core::clamp_area((0, 0, 0, 0), 0, 0), None);
    }

    /// **初值文本**：共同值 ⇒ 十进制；Mixed ⇒ 空串（官方 `?? ""`）。
    #[test]
    fn 初值文本混合为空串() {
        let m = 造图(4, 4);
        assert_eq!(初值(&m, (0, 0, 0, 0), 字段::门索引), "7");
        assert_eq!(初值(&m, (0, 0, 3, 3), 字段::光照), "", "混合必须是空串");
        assert_eq!(
            初值(&m, (0, 0, 3, 3), 字段::门偏移),
            "0",
            "0 要显示成 \"0\"，不是空"
        );
    }

    /// **解析输入**：空白 ⇒ 不动；0/255 ⇒ 设为；越界/负数/小数/文字 ⇒ 非法。
    #[test]
    fn 解析输入逐条() {
        assert_eq!(解析输入(""), 取值::不动);
        assert_eq!(
            解析输入("   "),
            取值::不动,
            "空白等同空（官方 IsNullOrWhiteSpace）"
        );
        assert_eq!(解析输入("0"), 取值::设为(0), "0 是「设成 0」，不是「不动」");
        assert_eq!(解析输入("255"), 取值::设为(255));
        assert_eq!(解析输入(" 9 "), 取值::设为(9), "两侧空白要吃掉");
        for bad in ["256", "-1", "1.5", "abc", "0x10", "１"] {
            assert_eq!(解析输入(bad), 取值::非法, "{bad:?} 必须被拒");
        }
    }

    /// **只写改过的字段**（上游 `box.Text == initialValues[box] ? null : parsed`）。
    #[test]
    fn 决定写入只认改动() {
        // 没改（含"两个空串相等"）⇒ 不写
        assert_eq!(决定写入("", ""), 取值::不动);
        assert_eq!(决定写入("7", "7"), 取值::不动);
        // 改了 ⇒ 写
        assert_eq!(决定写入("8", "7"), 取值::设为(8));
        assert_eq!(
            决定写入("0", ""),
            取值::设为(0),
            "Mixed 填空填成 0 ⇒ 要写 0"
        );
        // 改成了非法值 ⇒ 拒（不写）
        assert_eq!(决定写入("300", "7"), 取值::非法);
    }

    /// **头行**：1 格 ⇒ 单元格 x,y；多格 ⇒ 选区 N 格；越界按夹紧后的格数算。
    #[test]
    fn 头行两种写法() {
        let m = 造图(4, 4);
        assert_eq!(头行(&m, (1, 2, 1, 2)), "单元格 1,2");
        assert_eq!(头行(&m, (0, 0, 1, 1)), "应用到选区 4 格");
        // 越界夹紧：2×2 图上的 (1,1)-(9,9) ⇒ 1 格
        let small = 造图(2, 2);
        assert_eq!(头行(&small, (1, 1, 9, 9)), "单元格 1,1");
    }

    /// **Mir3 两档的门字段不可编辑**（有门字节时三个字段都能用）。
    #[test]
    fn 门字段可用性() {
        for f in 字段::全 {
            assert_eq!(f.可用(true), Ok(()), "{f:?} 在有门字节的格式上应可用");
        }
        assert_eq!(字段::光照.可用(false), Ok(()), "光照与门字节无关");
        assert_eq!(字段::门索引.可用(false), Err(tx::CELL_PROPS_NO_DOOR));
        assert_eq!(字段::门偏移.可用(false), Err(tx::CELL_PROPS_NO_DOOR));
        // 顺序/下标与上游一致
        assert_eq!(
            字段::全.map(字段::下标),
            [0, 1, 2],
            "三个字段的下标必须连续（焦点与矩形数组按它索引）"
        );
    }
}
