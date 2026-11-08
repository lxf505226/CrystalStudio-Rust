//! 印章**放置**：把资源面板里选中的那一枚 `.X` 真的贴到地图上。
//!
//! # 一、本模块只做「上层该做的事」，编辑语义一行都不重写
//!
//! 判据（锚点偏移 / 逐格越界裁剪 / 三族图号 / `bit15` 保留矩阵 / Type5 闸门 /
//! 整次放置 = 一步撤销）**全部**在 `miredit_core::stamp` 里，本模块只负责四件事：
//!
//! 1. **读盘**：`.X` 字节 → [`mir2_stamp::Stamp::parse`] → [`StampCellInput`] 数组；
//! 2. **注册**：`EditSession::register_stamp(id, cells)`（内核**不读盘**，格表必须由上层喂）；
//! 3. **落位**：`EditSession::place_stamp_by_id(id, x, y)`；
//! 4. **把结果折成一句中文**（成功回执 / 拒绝原因）与**待放置态**（[`Armed`]）。
//!
//! # 二、`.X → StampCellInput` 的适配**只有一份**，而且不在本 crate
//!
//! 面板缩略图那条路（`stamp_panel::thumbs::positioned_cells`）与放置那条路都走
//! 本模块的 [`cells_of`]；而 [`cells_of`] 只是一句转发 —— 唯一实现在
//! [`miredit_core::cells_from_stamp`]（web 的 `to_stamp_inputs` 走的也是它）。
//! 两边各写一份的话，"面板里看到的图"与"贴到地上的图"迟早会不一样（A-024 就是这么来的）。
//!
//! 适配口径（**真值见 `miredit_core::stamp_convert` 模块文档 §二/§三/§四**，本表只是索引）：
//!
//! | `StampCellInput` 字段 | `.X` 侧 | 处理（core 里） |
//! |---|---|---|
//! | `dx` / `dy` | `StampCell.x` / `.y`（`i32`，**可负**） | 原样 |
//! | `bk` | `back_image`（`i32`） | 低 16 位再掩 `0x7FFF`（高位是 C# 的阻挡标记/编辑器位，不解释） |
//! | `mid` | `middle_image`（`u16`） | 掩 `0x7FFF` |
//! | `fr` | `foreground.image`（`i16`） | 按位转 `u16` 再掩 `0x7FFF` |
//! | `bk_lib` / `mid_lib` / `fr_lib` | 三个 `i16` **全局库号** | **按号段换算成族内页号**（见下面 §二·五），不在号段 ⇒ 整枚拒绝 |
//!
//! # 二·五、库号是「全局 id」，必须先换算成族内页号（A-024，2026-11-07）
//!
//! `.X` 里的三个库号是 **C# `Assets` 表的全局 id**（`_upstream/CrystalStudio/Document.cs:71-72` 存的就是
//! `Cell.BackLibrary/MiddleLibrary/Foreground.Library`，而 C# 全程拿它们直接喂
//! `Assets.Get(library, …)`，见 `_upstream/CrystalStudio/StampPreview.cs:40`）。而本仓 `StampCellInput` /
//! `PositionedCell` 的三个库号是**地图格字节口径**（`btTiles` / `btsmTiles` / `btArea`
//! = 族内页号）⇒ 两者差一个**每族固定基址**：
//!
//! | 号段 | 地表 `Tiles` | 中层 `SmTiles` | 物体 `Objects` | 出处 |
//! |---|---|---|---|---|
//! | `Wemade`（`WemadeMir2` / 扁平目录兜底） | `0` | `1` | `2..=99`（第 k 页 = `2+k`） | `_upstream/CrystalStudio/Assets.cs:361-362` |
//! | `Shanda` | `100..=109` | `110..=119` | `120..=150` | `_upstream/CrystalStudio/Assets.cs:363-365` |
//!
//! ⇒ 族内页号 = 全局号 − 该族**号段下界**（`Wemade`：0/1/2；`Shanda`：100/110/120）。
//! **不是**「全局号原样当页号」—— 那样前景会去 `Objects3`（`surface(2)`）、中层去
//! `SmTiles2`（`middle(1)`），面板缩略图显示的不是这枚印章的画，落位还会把 `area` 写成 `2`
//! （A-024 的全部内容）。换算实现**只有一份**，在
//! [`miredit_core::stamp_convert::库号号段`]（本模块 [`cells_of`] 只是一句转发）；
//! `render_stamp` 里**没有**任何按值域猜库号的代码。
//!
//! ⚠ 本仓素材目前只有 `.wzl/.wil` ⇒ 号段恒 [`库号号段::Wemade`]，
//! 由 [`crate::mapdoc::Assets::号段`] 统一给出（`.lib/Shanda` 的接入点也在那里）。
//!
//! # 三、失败一律"地图一个字节都没动"
//!
//! 三道闸门的顺序是**读盘 → 注册 → 落位**，前两道失败时根本没有进入内核的写路径：
//!
//! | 失败 | 谁给原因 | 结果 |
//! |---|---|---|
//! | 文件读不出 / `.X` 解析失败 / 库号越界或不在号段 | 本模块 [`read`] | 中文原因，一格未写 |
//! | 空格表 / 超过 100 万格 / 注册表满 | 内核 `register_stamp` | 中文原因，一格未写 |
//! | Type5 有地表 / 全越界 / 未注册 | 内核 `plan_stamp` | 中文原因，一格未写 |
//!
//! 第三道里"部分越界"**不是失败**：内核逐格跳过并在 `PlaceOutcome::Placed.skipped`
//! 里报数，本模块把它写进回执（[`tx::STAMP_PLACE_SKIP_TMPL`]）。
//!
//! # 四、`Alt` 悬停预览只画轮廓
//!
//! [`outline_rect`] 把「锚点 + 印章包围盒」算成屏幕矩形，画的时候只描边、**不填任何像素**
//! （任务书 §三.5）。所以它只吃**包围盒**这一项真实数据（来自
//! `StampCatalog::resolve` 的 `bounds`，与 `Stamp::bounds()` 同源），不碰图库、不渲染缩略图 ——
//! 没有帧也照样能画出正确的框，绝不会出现"框的位置对、里面是假的图"。

use std::path::Path;

use egui::Rect;
use mir2_map::Map;
use mir2_stamp::Stamp;

use miredit_core::{PlaceOutcome, StampCellInput};

use crate::canvas_view::CanvasView;
use crate::edit::EditSession;
use crate::ui_text as tx;

/// **库号号段**（`.X` 全局 id → 族内页号的换算基址）—— 唯一实现已下沉到
/// [`miredit_core::stamp_convert`]（A-024）：本 crate **不留第二份范围表**，
/// 这里只做名字转发，好让既有调用点写 `stamp_place::库号号段`。
///
/// 换算规则与出处见 core 的 `库号号段` 文档（`Wemade`：`0/1/2` 基址；`Shanda`：`100/110/120`）。
/// 号段从 [`crate::mapdoc::Assets::号段`] 取（现役素材 `.wzl/.wil` ⇒ 恒 `Wemade`）。
pub use miredit_core::库号号段;

/// 一枚 `.X` 读盘后的**全部**结果：格表 + 包围盒。
///
/// 包围盒也一起返回，是因为待放置态的轮廓预览要用它（[`Armed::bounds`]），
/// 而 `resolve` 的缓存与本次读盘读的是同一个文件 —— 不必让调用方再查一次目录。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StampFile {
    /// 格表（相对锚点的偏移 + 三族图号/库号）。
    pub cells: Vec<StampCellInput>,
    /// 包围盒 `(min_x, min_y, max_x, max_y)`，**闭区间**，与 `Stamp::bounds()` 同源。
    pub bounds: (i32, i32, i32, i32),
}

/// 印章**待放置轮廓**的输入：`(锚点, 包围盒)`。
///
/// 单列一个类型别名是为了让 [`crate::canvas::CanvasCtx`] 的字段与
/// [`crate::app::MireditApp`] 的取值函数共用同一个名字 —— 两处各写一遍
/// `((u16, u16), (i32, i32, i32, i32))` 既难读，也会让 clippy 的 `type_complexity` 各报一条。
pub type Outline = ((u16, u16), (i32, i32, i32, i32));

/// 待放置态：**已经选中一枚印章，正等着用户点画布落位**。
///
/// 为什么要有这个态（而不是"选中即放置"）：选中是**浏览**动作（面板里一枚枚看过去），
/// 放置是**破坏性**动作（写地图 + 占一步撤销）。两者必须分开，否则用户翻页看缩略图
/// 就会在图上到处留印章。所以：选中 → 按 `Enter` 进入本态 → 点画布才落位。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Armed {
    /// 印章 id（`register_stamp` 的键，也是 `palette.json` 的主键）。
    pub id: String,
    /// 显示名（回执里用；数据，不是文案）。
    pub name: String,
    /// 包围盒（`None` = 目录里 `resolve` 失败 ⇒ 预览不画框，落位时会报读盘失败）。
    pub bounds: Option<(i32, i32, i32, i32)>,
    /// **格表**（A-011：进入待放置态时顺带读一次盘，Alt 预览要用它渲染半透明内容与受阻诊断）。
    ///
    /// `None` = 还没读 / 读失败 ⇒ 预览**降级成只画轮廓**（不画假像素）。
    /// 真正落位仍走 [`place`]（每次现读盘），本字段只服务预览。
    pub cells: Option<Vec<StampCellInput>>,
}

/// 放置成功的结果（内核 `PlaceOutcome::Placed` 的宿主侧镜像，只留两个数）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    /// **真正改动**的格数（= 那一步撤销里的格数）。
    pub cells: usize,
    /// 越界被跳过的格数。
    pub skipped: usize,
}

/// `.X` 的格表 → 内核的 [`StampCellInput`] 数组（**薄壳**：唯一实现在 core）。
///
/// # A-024（2026-11-07）：本 crate 不再自己搬字段
///
/// 以前这里有一份 410 行的字段映射 + 本地号段表，与 `miredit_core::cells_from_stamp`
/// （web 走的那条）**各写一份** ⇒ 两处口径迟早分叉（GUI 曾漏了全局号 → 页号的换算）。
/// 现在只剩一句转发：掩码口径、库号号段换算、「没有图的层不参与换算」、错误措辞
/// 全部由 [`miredit_core::cells_from_stamp`] 单点决定。
///
/// `号段` 由调用方从 [`crate::mapdoc::Assets::号段`] 取（现役 `.wzl/.wil` ⇒ `Wemade`）；
/// 渲染侧不参与号段判断。
pub fn cells_of(
    stamp: &Stamp,
    号段: 库号号段,
) -> Result<Vec<StampCellInput>, miredit_core::StampConvertError> {
    miredit_core::cells_from_stamp(stamp, 号段)
}

// ───────────────────── A-011：落位预览的**状态**（只读） ─────────────────────

/// 允许落位时的内容不透明度（上游 `_upstream/CrystalStudio/MapViewport.StampRendering.cs:61` 的 `0.55`）。
pub const 不透明度允许: f32 = 0.55;
/// 受阻（或有 error）时的内容不透明度（同处 `0.18`）。
pub const 不透明度受阻: f32 = 0.18;

/// 内容预览的两档不透明度（**只有这两个值**，不是渐变）。
#[must_use]
pub const fn 不透明度(允许: bool) -> f32 {
    if 允许 {
        不透明度允许
    } else {
        不透明度受阻
    }
}

/// Alt 落位预览的状态（A-011）：不透明度 + 内核的只读诊断。
///
/// 诊断本身在 [`miredit_core::check_stamp_placement`]（**单一实现**，GUI 不重算）；
/// 本结构只把"允许与否"折成上游的两个透明度档位。
#[derive(Clone, Debug, PartialEq)]
pub struct 预览状态 {
    /// 内容不透明度：[`不透明度允许`] 或 [`不透明度受阻`]。
    pub 不透明度: f32,
    /// 内核给的只读诊断（受阻格 + error）。
    pub 检查: miredit_core::StampPlacementCheck,
}

impl 预览状态 {
    /// 由内核诊断折出状态：`allowed()` ⇒ 0.55，否则（受阻或 error）⇒ 0.18。
    #[must_use]
    pub fn 由检查(检查: miredit_core::StampPlacementCheck) -> Self {
        let 允许 = 检查.allowed();
        Self {
            不透明度: 不透明度(允许),
            检查,
        }
    }
}

/// **Alt 预览状态**（`alt == false` ⇒ `None` = 不画）。
///
/// 判据全在内核（[`miredit_core::check_stamp_placement`]，只读）；这里只做三件界面的事：
/// ① Alt 门控；② 把 `allowed()` 折成 0.55 / 0.18；③ 原样带出逐格受阻给绘制层。
#[must_use]
pub fn preview_state(
    map: &Map,
    cells: &[StampCellInput],
    x: u16,
    y: u16,
    alt: bool,
) -> Option<预览状态> {
    if !alt {
        return None;
    }
    Some(预览状态::由检查(miredit_core::check_stamp_placement(
        map, cells, x, y,
    )))
}

/// 读一枚 `.X` 并换成内核要的格表；失败 ⇒ **带路径与原文的中文原因**。
///
/// 三种失败都走 [`tx::STAMP_READ_FAIL_TMPL`]（读不出 / 解析失败 / 库号不在号段），
/// 后半句都是"地图一个字节都没动"——这是有保证的事实：本函数只读，不碰会话。
///
/// `号段` 由调用方给（[`crate::mapdoc::Assets::号段`]）；库号换算的唯一实现在
/// [`miredit_core::cells_from_stamp`]，本函数只把它的错误原文翻成界面文案。
pub fn read(path: &Path, 号段: 库号号段) -> Result<StampFile, String> {
    let bytes = std::fs::read(path).map_err(|e| {
        tx::fill(
            tx::STAMP_READ_FAIL_TMPL,
            &[("path", &path.display().to_string()), ("err", &e.to_string())],
        )
    })?;
    let stamp = Stamp::parse(&bytes).map_err(|e| {
        tx::fill(
            tx::STAMP_READ_FAIL_TMPL,
            &[("path", &path.display().to_string()), ("err", &e.to_string())],
        )
    })?;
    let cells = cells_of(&stamp, 号段).map_err(|e| {
        tx::fill(
            tx::STAMP_READ_FAIL_TMPL,
            &[("path", &path.display().to_string()), ("err", &e.to_string())],
        )
    })?;
    Ok(StampFile {
        cells,
        bounds: stamp.bounds(),
    })
}

/// **读盘 → 注册 → 落位**的完整链路（GUI 与无窗验收共用这一条）。
///
/// 返回 `Ok(Placed)` = 真改了 `cells` 格；`Err(中文原因)` = **一格未写**。
///
/// 为什么每次都重新读盘：`.X` 在磁盘上可能被用户改过（CrystalStudio 也是每次放置前
/// 现读，`_upstream/CrystalStudio/PlacedStamps.cs:114-121`）。缓存格表会让"改了印章、贴出来还是旧的"。
///
/// `号段` 与 [`read`] 同一个来源（[`crate::mapdoc::Assets::号段`]）⇒
/// 「缩略图看到的」与「贴到地上的」用的是同一套库号换算。
pub fn place(
    ed: &mut EditSession,
    map: &mut Map,
    armed: &Armed,
    path: &Path,
    x: u16,
    y: u16,
    号段: 库号号段,
) -> Result<Placed, String> {
    let file = read(path, 号段)?;
    // 覆盖式注册：同一个 id 再注册 = 覆盖（内核的契约），正是"改完印章再贴"要的语义。
    ed.register_stamp(armed.id.clone(), file.cells)?;
    match ed.place_stamp_by_id(map, x, y, &armed.id) {
        PlaceOutcome::Placed { cells, skipped } => Ok(Placed { cells, skipped }),
        PlaceOutcome::Refused { reason } => Err(reason),
    }
}

/// 放置成功的回执（有越界跳过时换另一条模板，见 [`tx::STAMP_PLACE_SKIP_TMPL`] 的理由）。
pub fn receipt(name: &str, p: Placed) -> String {
    let n = p.cells.to_string();
    if p.skipped == 0 {
        tx::fill(tx::STAMP_PLACE_OK_TMPL, &[("name", name), ("n", &n)])
    } else {
        tx::fill(
            tx::STAMP_PLACE_SKIP_TMPL,
            &[("name", name), ("n", &n), ("k", &p.skipped.to_string())],
        )
    }
}

/// 放置失败的回执（原因来自内核或 [`read`]）。
pub fn fail_text(why: &str) -> String {
    tx::fill(tx::STAMP_PLACE_FAIL_TMPL, &[("why", why)])
}

/// 待放置态的提示行。
pub fn armed_text(name: &str) -> String {
    tx::fill(tx::STAMP_ARMED_TMPL, &[("name", name)])
}

/// 「锚点 + 印章包围盒」→ 屏幕矩形（**只用于画轮廓**，不产生任何像素）。
///
/// 包围盒是**闭区间**的格坐标（`mir2_stamp::Stamp::bounds` 的口径）：
/// 覆盖的格是 `anchor.x + min_x ..= anchor.x + max_x`（`y` 同理），可负 ⇒ 可能落在图外。
/// 本函数**不裁剪**到地图：越界部分照画（用户要能看见"这一角会掉出去"），
/// 绘制方靠 painter 的 clip 裁到画布矩形。
///
/// 反解：`cell_rect` 的 `min` 是格左上角，`max` 是右下角 ⇒ 左上取第一格的 `min`、
/// 右下取最后一格的 `max`，正好覆盖整块。
pub fn outline_rect(
    view: &CanvasView,
    canvas: Rect,
    anchor: (u16, u16),
    bounds: (i32, i32, i32, i32),
) -> Rect {
    let (min_x, min_y, max_x, max_y) = bounds;
    let ax = i32::from(anchor.0);
    let ay = i32::from(anchor.1);
    let tl = view.cell_rect(canvas, ax + min_x, ay + min_y);
    let br = view.cell_rect(canvas, ax + max_x, ay + max_y);
    Rect::from_min_max(tl.min, br.max)
}

/// 格表自己的包围盒（`(min_x, min_y, max_x, max_y)`，闭区间）—— 与 [`Stamp::bounds`] 同口径。
///
/// 用途：`Armed` 里 `bounds` 缺失（目录没给）时，预览仍能从格表算出框（只画轮廓 + 诊断）。
#[must_use]
pub fn 格表包围盒(cells: &[StampCellInput]) -> (i32, i32, i32, i32) {
    if cells.is_empty() {
        return (0, 0, 0, 0);
    }
    let mut b = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for c in cells {
        b.0 = b.0.min(c.dx);
        b.1 = b.1.min(c.dy);
        b.2 = b.2.max(c.dx);
        b.3 = b.3.max(c.dy);
    }
    b
}


#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};
    use miredit_core::EditLayer;
    use std::path::PathBuf;

    /// 取证根缺省值：`.X` 的取证来源（**整个测试模块里只出现这一处**）。
    /// 换机器用 `MIREDIT_UPSTREAM_ROOT`。
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

    /// 本机真实印章（CrystalStudio 自带范例；core 的验收门 2 用的也是它）。
    /// 缺取证根/缺文件 ⇒ `None`（公开仓/PR 的机器上没有 `.X` 素材，不算代码错）。
    fn 印章路径() -> Option<PathBuf> {
        let p = 取证根()?.join("Objects").join("mir").join("mir_00000.X");
        p.is_file().then_some(p)
    }

    /// 合成 8×8 `New14` 图：每格地表图号 = 序号 + 1（可区分）。
    fn 造图(w: u16, h: u16) -> Map {
        let mut bytes = vec![0u8; mir2_map::HEADER_LEN + 14 * (w as usize * h as usize)];
        bytes[0..2].copy_from_slice(&w.to_le_bytes());
        bytes[2..4].copy_from_slice(&h.to_le_bytes());
        for i in 0..(w as usize * h as usize) {
            let base = mir2_map::HEADER_LEN + i * 14;
            bytes[base..base + 2].copy_from_slice(&((i as u16) + 1).to_le_bytes());
            bytes[base + 12] = 4; // btTiles
        }
        Map::parse(&bytes).expect("合成图应能解析")
    }

    /// 一枚手工格表（不读盘）：2×2，三族都给非 0 值。
    fn 手印章() -> StampFile {
        StampFile {
            cells: vec![
                StampCellInput { dx: 0, dy: 0, bk: 11, mid: 12, fr: 13, bk_lib: 21, mid_lib: 22, fr_lib: 23, ..Default::default() },
                StampCellInput { dx: 1, dy: 0, bk: 14, mid: 15, fr: 16, bk_lib: 24, mid_lib: 25, fr_lib: 26, ..Default::default() },
                StampCellInput { dx: 0, dy: 1, bk: 17, mid: 18, fr: 19, bk_lib: 27, mid_lib: 28, fr_lib: 29, ..Default::default() },
                StampCellInput { dx: 1, dy: 1, bk: 20, mid: 21, fr: 22, bk_lib: 30, mid_lib: 31, fr_lib: 32, ..Default::default() },
            ],
            bounds: (0, 0, 1, 1),
        }
    }

    /// **注册 → 落位**这条链走通：格表进得去、`place_stamp_by_id` 能按 id 放到锚点上。
    #[test]
    fn 注册后按id落位() {
        let mut map = 造图(8, 8);
        let mut ed = EditSession::default();
        let f = 手印章();
        assert_eq!(ed.register_stamp("t", f.cells).expect("注册应成功"), 4);
        let out = ed.place_stamp_by_id(&mut map, 3, 3, "t");
        assert_eq!(out, PlaceOutcome::Placed { cells: 4, skipped: 0 });
        assert_eq!(map.cell(3, 3).unwrap().bk_img, 11);
        assert_eq!(map.cell(4, 4).unwrap().bk_img, 20);
        assert_eq!(ed.undo_len(), 1, "整次放置 = 一步撤销");
        assert!(ed.dirty(), "放置必须置脏");
    }

    /// **未注册的 id** ⇒ 拒绝 + 中文原因，地图一字节不动。
    #[test]
    fn 未注册被拒且不动地图() {
        let mut map = 造图(4, 4);
        let before = map.to_bytes();
        let mut ed = EditSession::default();
        let out = ed.place_stamp_by_id(&mut map, 0, 0, "nope");
        let PlaceOutcome::Refused { reason } = out else {
            panic!("未注册必须拒绝");
        };
        assert!(reason.contains("未注册"), "{reason}");
        assert_eq!(map.to_bytes(), before, "拒绝时地图必须逐字节不变");
        assert_eq!(ed.undo_len(), 0, "拒绝不该产生撤销步");
    }

    /// **读盘失败**给出中文原因（含路径），且一格未写。
    #[test]
    fn 读盘失败带中文原因() {
        let err = read(Path::new(r"E:\__no_such_stamp__.X"), 库号号段::Wemade)
            .expect_err("不存在必须失败");
        assert!(err.contains("读不出印章文件"), "{err}");
        assert!(err.contains("__no_such_stamp__.X"), "要点名路径：{err}");
        assert!(err.contains("地图一个字节都没动"), "{err}");
    }

    /// **真实印章**：读得出 25 格、包围盒 `(-1,-2,4,3)`（与 core 门 2 的三个数一致）。
    ///
    /// 这条同时钉住"适配口径没漂"：格数/包围盒变了就说明字段搬运改了。
    #[test]
    fn 真实印章格数与包围盒() {
        let p = match 印章路径() {
            Some(p) => p,
            None => {
                eprintln!("[跳过] 无取证根 MIREDIT_UPSTREAM_ROOT（或缺 Objects/mir/mir_00000.X）");
                return;
            }
        };
        let Ok(f) = read(&p, 库号号段::Wemade) else {
            eprintln!("[跳过] 本机没有印章样本 {}", p.display());
            return;
        };
        assert_eq!(f.cells.len(), 25, "mir_00000.X 是 25 格");
        assert_eq!(f.bounds, (-1, -2, 4, 3), "包围盒必须是 (-1,-2,4,3)");
        assert_eq!(f.cells.iter().filter(|c| c.fr != 0).count(), 8, "8 格带前景");
        assert!(
            f.cells.iter().all(|c| c.bk == 0 && c.mid == 0),
            "这枚是纯物件印章：地表/中层图号全 0"
        );
    }

    /// **`place()` 全链路**（读盘 → 注册 → 落位）在真实印章上跑通，回执含名称与格数。
    #[test]
    fn 全链路放置真实印章() {
        let p = match 印章路径() {
            Some(p) => p,
            None => {
                eprintln!("[跳过] 无取证根 MIREDIT_UPSTREAM_ROOT（或缺 Objects/mir/mir_00000.X）");
                return;
            }
        };
        let mut map = 造图(16, 16);
        let mut ed = EditSession::default();
        let armed = Armed {
            id: "mir/mir_00000".to_string(),
            name: "mir_00000".to_string(),
            bounds: Some((-1, -2, 4, 3)),
            cells: None,
        };
        // 锚点 (5,5) + 包围盒 (-1,-2,4,3) ⇒ x 4..=9 / y 3..=8，全在 16×16 内
        let out = place(&mut ed, &mut map, &armed, &p, 5, 5, 库号号段::Wemade).expect("应能放置");
        assert_eq!(out.skipped, 0, "锚点在图内，不该有越界格");
        assert!(out.cells > 0, "必须真的改了格");
        assert_eq!(ed.undo_len(), 1);
        let text = receipt(&armed.name, out);
        assert_eq!(text, format!("放置印章「mir_00000」{} 格", out.cells));
        // 带跳过的回执换一条模板（两个数都在）
        let t2 = receipt("x", Placed { cells: 3, skipped: 7 });
        assert_eq!(t2, "放置印章「x」3 格（7 格在地图外，已跳过）");
    }

    /// **全越界 ⇒ 拒绝**，且地图逐字节不变（门 2 的反面）。
    #[test]
    fn 全越界拒绝且不动地图() {
        let p = match 印章路径() {
            Some(p) => p,
            None => {
                eprintln!("[跳过] 无取证根 MIREDIT_UPSTREAM_ROOT（或缺 Objects/mir/mir_00000.X）");
                return;
            }
        };
        let mut map = 造图(4, 4);
        let before = map.to_bytes();
        let mut ed = EditSession::default();
        let armed = Armed { id: "mir/mir_00000".into(), name: "mir_00000".into(), bounds: None, cells: None };
        let err = place(&mut ed, &mut map, &armed, &p, 200, 200, 库号号段::Wemade)
            .expect_err("必须拒绝");
        assert!(err.contains("地图外"), "{err}");
        assert_eq!(map.to_bytes(), before, "拒绝时地图必须逐字节不变");
        assert!(!ed.dirty(), "拒绝不该置脏");
    }

    /// **轮廓矩形**：包围盒 `(-1,-2,4,3)` 在锚点 `(100,100)` ⇒ 覆盖 6×6 格。
    ///
    /// 6 列 = `100-1 ..= 100+4`、6 行 = `100-2 ..= 100+3`；再按 1× 缩放 × 48×32 像素。
    #[test]
    fn 轮廓矩形按包围盒算() {
        let view = CanvasView::default();
        let canvas = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0));
        let r = outline_rect(&view, canvas, (100, 100), (-1, -2, 4, 3));
        // pan = (0,0)、scale = 1 ⇒ 屏幕坐标 = 世界像素
        assert!((r.left() - 99.0 * 48.0).abs() < 0.01, "左边 {}", r.left());
        assert!((r.top() - 98.0 * 32.0).abs() < 0.01, "上边 {}", r.top());
        assert!((r.right() - 105.0 * 48.0).abs() < 0.01, "右边 {}", r.right());
        assert!((r.bottom() - 104.0 * 32.0).abs() < 0.01, "下边 {}", r.bottom());
        assert!((r.width() - 6.0 * 48.0).abs() < 0.01, "6 列宽");
        assert!((r.height() - 6.0 * 32.0).abs() < 0.01, "6 行高");
    }

    /// 轮廓矩形**不裁剪**：锚点贴着左上角时负偏移会画到画布外（用户要能看见"要掉出去"）。
    #[test]
    fn 轮廓不裁剪到地图() {
        let view = CanvasView::default();
        let canvas = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0));
        let r = outline_rect(&view, canvas, (0, 0), (-1, -2, 4, 3));
        assert!(r.left() < 0.0, "负偏移必须画到画布外：{}", r.left());
        assert!(r.top() < 0.0, "负偏移必须画到画布外：{}", r.top());
    }

    /// 放置后**当前编辑层与笔刷都不受影响**（印章写的是三族图号，不是"落笔"）。
    #[test]
    fn 放置不改当前层() {
        let mut map = 造图(8, 8);
        let mut ed = EditSession::default();
        ed.set_layer(EditLayer::Middle);
        let f = 手印章();
        ed.register_stamp("t", f.cells).unwrap();
        let _ = ed.place_stamp_by_id(&mut map, 2, 2, "t");
        assert_eq!(ed.layer(), EditLayer::Middle, "放置不该改当前编辑层");
    }

    // -----------------------------------------------------------------------
    // A-024：`.X` 的**全局库号** → **族内页号**（唯一换算处 = [`库号号段`]）
    // -----------------------------------------------------------------------

    /// 合成 12 字节/格的图（`Old12`：没有 `tiles`/`sm_tiles` 两个字节，但有 `area`）。
    fn 造图12(w: u16, h: u16) -> Map {
        let mut bytes = vec![0u8; mir2_map::HEADER_LEN + 12 * (w as usize * h as usize)];
        bytes[0..2].copy_from_slice(&w.to_le_bytes());
        bytes[2..4].copy_from_slice(&h.to_le_bytes());
        Map::parse(&bytes).expect("12 字节合成图应能解析")
    }

    /// 一枚合成印章（单格）。
    fn 合成印章(格: mir2_stamp::StampCellData) -> Stamp {
        Stamp {
            version: 2,
            cells: vec![mir2_stamp::StampCell { x: 0, y: 0, cell: 格 }],
        }
    }

    /// **适配层（GUI）与内核（core）的换算必须逐字段相等**（A-024 的「防再分叉」守卫）。
    ///
    /// A-024 之前 GUI 自己搬字段 + 自己算库号页号，core 另有一套 ⇒ 两边分叉，
    /// GUI 少了一步全局号换算。现在 [`cells_of`] 只是一句转发，本用例把它钉成
    /// **委托守卫**：一旦有人在本文件重新长出第二份映射（哪怕只多掩一次位），
    /// 与 core 的逐字段比较就会红。
    #[test]
    fn 适配层与内核换算逐字段相等() {
        let Some(p) = 印章路径() else {
            eprintln!("[跳过] 无取证根 MIREDIT_UPSTREAM_ROOT（或缺 Objects/mir/mir_00000.X）");
            return;
        };
        let Ok(bytes) = std::fs::read(&p) else {
            eprintln!("[跳过] 本机没有印章样本 {}", p.display());
            return;
        };
        let stamp = Stamp::parse(&bytes).expect("mir_00000.X 应能解析");
        let gui = cells_of(&stamp, 库号号段::Wemade).expect("GUI 适配应成功");
        let core = miredit_core::cells_from_stamp(&stamp, 库号号段::Wemade).expect("core 换算应成功");
        assert_eq!(gui, core, "GUI 适配与 core 换算必须逐字段相等（不许留第二份映射）");
        // 错误口径也必须同源：换一个号段，两边都得拒绝，且错误文案逐字相同
        let 号段错 = 库号号段::Shanda;
        let gui_err = cells_of(&stamp, 号段错).expect_err("Shanda 号段下必拒");
        let core_err =
            miredit_core::cells_from_stamp(&stamp, 号段错).expect_err("core 同样必拒");
        assert_eq!(gui_err.to_string(), core_err.to_string(), "错误文案必须同源");
    }

    /// **真实印章（25 格）三族库号必须落页 0**（`.X` 原文是全局号 0/1/2）。
    #[test]
    fn 真实印章三族库号落页0() {
        let Some(p) = 印章路径() else {
            eprintln!("[跳过] 无取证根 MIREDIT_UPSTREAM_ROOT（或缺 Objects/mir/mir_00000.X）");
            return;
        };
        let Ok(f) = read(&p, 库号号段::Wemade) else {
            eprintln!("[跳过] 本机没有印章样本 {}", p.display());
            return;
        };
        assert!(
            f.cells
                .iter()
                .all(|c| (c.bk_lib, c.mid_lib, c.fr_lib) == (0, 0, 0)),
            "全局号 0/1/2 必须分别换算成 Tiles/SmTiles/Objects 的**页 0**"
        );
        assert_eq!(f.cells.iter().filter(|c| c.fr != 0).count(), 8, "8 格带前景");
    }

    /// **落位到 `Old12`：`area` 字节 = 0**（不是全局号 2）—— A-024 的核心验收。
    #[test]
    fn 真实印章落位12字节档的area字节为0() {
        let Some(p) = 印章路径() else {
            eprintln!("[跳过] 无取证根 MIREDIT_UPSTREAM_ROOT（或缺 Objects/mir/mir_00000.X）");
            return;
        };
        let mut map = 造图12(16, 16);
        let mut ed = EditSession::default();
        let armed = Armed {
            id: "mir/mir_00000".to_string(),
            name: "mir_00000".to_string(),
            bounds: Some((-1, -2, 4, 3)),
            cells: None,
        };
        // 包围盒 (-1,-2,4,3) + 锚点 (5,5) ⇒ 覆盖 x 4..=9 / y 3..=8，全在 16×16 内
        let out = place(&mut ed, &mut map, &armed, &p, 5, 5, 库号号段::Wemade).expect("应能放置");
        assert_eq!(out.skipped, 0, "锚点在图内，不该有越界格");

        let 原 = map.to_bytes();
        let (h, 格长) = (16usize, 12usize);
        let mut 有前景 = 0;
        for x in 4..=9usize {
            for y in 3..=8usize {
                let off = mir2_map::HEADER_LEN + (x * h + y) * 格长;
                let fr = u16::from_le_bytes([原[off + 4], 原[off + 5]]);
                if fr & 0x7FFF == 0 {
                    continue;
                }
                有前景 += 1;
                assert_eq!(
                    原[off + 10], 0,
                    "({x},{y}) 的 `btArea`（off10）必须写成 0（Objects 页 0），不是全局号 2"
                );
            }
        }
        assert_eq!(有前景, 8, "这枚印章有 8 格前景，必须逐格都验到");
    }

    /// **合成印章（中层有图）：`New14` 的 `sm_tiles` = 0**（与 Old12 同一套换算，不另写一遍）。
    #[test]
    fn 合成印章落位14字节档的sm_tiles为0() {
        let 格 = mir2_stamp::StampCellData {
            back_library: 0,
            back_image: 0,
            middle_library: 1, // 全局号 1 = SmTiles
            middle_image: 7,
            foreground: Some(mir2_stamp::Front {
                library: 2, // 全局号 2 = Objects
                image: 9,
                frames: 0,
                tick: 0,
            }),
            ..Default::default()
        };
        let cells = cells_of(&合成印章(格), 库号号段::Wemade).expect("Wemade 号段内应换算成功");
        assert_eq!(
            (cells[0].bk_lib, cells[0].mid_lib, cells[0].fr_lib),
            (0, 0, 0),
            "0/1/2 三个全局号都要落页 0"
        );

        let mut map = 造图(4, 4); // 14 B/格 = New14
        let mut ed = EditSession::default();
        ed.register_stamp("t", cells).expect("注册应成功");
        assert_eq!(
            ed.place_stamp_by_id(&mut map, 1, 1, "t"),
            PlaceOutcome::Placed {
                cells: 1,
                skipped: 0
            }
        );
        let 原 = map.to_bytes();
        // (x=1, y=1) 在 4 列宽的图里 ⇒ 线性下标 = 1*4 + 1；每格 14 字节（New14）
        let off = mir2_map::HEADER_LEN + (4 + 1) * 14;
        assert_eq!(原[off + 13], 0, "(1,1) 的 `btsmTiles` 必须是 0（SmTiles 页 0）");
        assert_eq!(原[off + 10], 0, "(1,1) 的 `btArea` 必须是 0（Objects 页 0）");
        assert_eq!(
            原[off + 12], 4,
            "地表没有图 ⇒ `btTiles` 保持原值 4（印章不写无图层的库号）"
        );
    }

    /// **无图的层不参与换算**：库里那些历史常量（如 12B 档硬编码 `MiddleLibrary = 1`）
    /// 不该把整枚印章判成"号段不符"；反过来，**有图但号段不符**就必须拒绝。
    #[test]
    fn 无图层不参与换算_有图跨号段则拒绝() {
        // 只有前景的格（地表/中层图号 0）：库号 0/1 都在号段内，且都落页 0
        let 只有前景 = mir2_stamp::StampCellData {
            back_library: 0,
            back_image: 0,
            middle_library: 1,
            middle_image: 0,
            foreground: Some(mir2_stamp::Front {
                library: 2,
                image: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let ok = cells_of(&合成印章(只有前景), 库号号段::Wemade).expect("应成功");
        assert_eq!((ok[0].bk_lib, ok[0].mid_lib, ok[0].fr_lib), (0, 0, 0));

        // 有图但库号不在 Wemade 号段（120 是 Shanda 的 Objects 页 0）⇒ 必须拒绝，不许透传
        let 跨号段 = mir2_stamp::StampCellData {
            foreground: Some(mir2_stamp::Front {
                library: 120,
                image: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(
            cells_of(&合成印章(跨号段), 库号号段::Wemade).is_err(),
            "跨号段必须拒绝（原样透传会去 Objects121）"
        );
        // 同一枚换成 Shanda 号段 ⇒ 命中 Objects 页 0
        let ok = cells_of(&合成印章(跨号段), 库号号段::Shanda).expect("Shanda 号段内应成功");
        assert_eq!(ok[0].fr_lib, 0, "Shanda 120 → Objects 页 0");
    }

    /// **中层有图但库号 0（不在 Wemade 中层号段）⇒ 拒绝**
    #[test]
    fn 中层有图但库号越号段被拒() {
        let 中层缺号 = mir2_stamp::StampCellData {
            middle_library: 0,
            middle_image: 5,
            ..Default::default()
        };
        assert!(
            cells_of(&合成印章(中层缺号), 库号号段::Wemade).is_err(),
            "中层有图但库号不在号段 ⇒ 拒绝（不是静默写 0 号库）"
        );
    }

    /// **真实印章缩略图：`missing == 0`，且只向 `Objects` 页 0 要帧**（A-024 验收）。
    ///
    /// 用**记账图源**（对任何请求都给一帧 48×32 不透明图，并记下 `(族, 页, 帧号)`）：
    /// "有没有缺帧"因此不依赖本机图库的帧数（换成小图库也不会因帧号越界误报），
    /// 同时把"到底问了哪个库的哪一页"钉死 —— 这正是 A-024 的病根所在。
    #[test]
    fn 真实印章缩略图无缺帧且只问物体页0() {
        use std::cell::RefCell;

        use mir2_assets::{Frame, LibRef, SpriteSource};

        #[derive(Default)]
        struct 记账源 {
            请求: RefCell<Vec<(mir2_assets::Family, u16, u32)>>,
        }
        impl SpriteSource for 记账源 {
            fn frame(&self, lib: LibRef, index: u32) -> Option<Frame> {
                self.请求.borrow_mut().push((lib.family, lib.index, index));
                Some(Frame {
                    width: 48,
                    height: 32,
                    rgba: [255u8, 0, 0, 255].repeat(48 * 32),
                    ..Frame::default()
                })
            }
        }

        let Some(p) = 印章路径() else {
            eprintln!("[跳过] 无取证根 MIREDIT_UPSTREAM_ROOT（或缺 Objects/mir/mir_00000.X）");
            return;
        };
        let Ok(f) = read(&p, 库号号段::Wemade) else {
            eprintln!("[跳过] 本机没有印章样本 {}", p.display());
            return;
        };
        let 位置格: Vec<miredit_render::stamp::PositionedCell> = f
            .cells
            .iter()
            .map(|c| miredit_render::stamp::PositionedCell {
                dx: c.dx,
                dy: c.dy,
                bk: c.bk & 0x7FFF,
                mid: c.mid & 0x7FFF,
                fr: c.fr & 0x7FFF,
                bk_lib: c.bk_lib,
                mid_lib: c.mid_lib,
                fr_lib: c.fr_lib,
            })
            .collect();

        let 记账 = 记账源::default();
        let img = miredit_render::stamp::render_stamp(
            &位置格,
            &记账,
            &miredit_render::stamp::StampRenderOptions::thumbnail(46, 46),
        )
        .expect("记账源必然能渲染");
        assert_eq!(img.missing, 0, "记账源对任何请求都有帧 ⇒ 缺帧必须是 0");
        assert!(img.cells_drawn > 0, "必须真的画出东西");
        let 请求 = 记账.请求.borrow().clone();
        assert!(!请求.is_empty(), "必须真的问过帧（否则本用例是空转）");
        for (族, 页, _帧号) in 请求 {
            assert_eq!(
                (族, 页),
                (mir2_assets::Family::Surface, 0),
                "这枚印章只有前景图（库号全局 2）⇒ 只能问 Objects 页 0"
            );
        }
    }

    // ─────────────────── A-011：预览状态（透明度两档 + 只读诊断） ───────────────────

    /// 2×2 的合成印章格表（诊断用：不关心图号/库号，只关心 `(dx,dy)`）。
    fn 小印章() -> Vec<StampCellInput> {
        let mut v = Vec::new();
        for dy in 0..2 {
            for dx in 0..2 {
                v.push(StampCellInput {
                    dx,
                    dy,
                    ..Default::default()
                });
            }
        }
        v
    }

    /// **允许 ⇒ 0.55**；诊断留给内核（这里只验折档）。
    #[test]
    fn 预览允许时为五五() {
        let map = 造图(8, 8);
        let cells = 小印章();
        let s = preview_state(&map, &cells, 2, 2, true).expect("Alt 按住 ⇒ 有状态");
        assert!(s.检查.allowed(), "2×2 落在空位上应允许");
        assert!((s.不透明度 - 不透明度允许).abs() < 1e-6, "允许时必须 0.55");
        assert_eq!(不透明度(true), 0.55);
    }

    /// **受阻（占用）⇒ 0.18**，且受阻格被带出来。
    #[test]
    fn 预览受阻时为一八() {
        let mut map = 造图(8, 8);
        let i = miredit_core::index_of(&map, 3, 3).expect("图内格");
        map.cells[i].fr_img = 5;
        let s = preview_state(&map, &小印章(), 2, 2, true).expect("有状态");
        assert!(!s.检查.allowed());
        assert_eq!(s.检查.blocked.len(), 1, "只有 (3,3) 被占");
        assert!((s.不透明度 - 不透明度受阻).abs() < 1e-6, "受阻时必须 0.18");
        assert_eq!(不透明度(false), 0.18);
    }

    /// **error（空格表）⇒ 也是 0.18**（上游"落不下去就不透明地画"）。
    #[test]
    fn 预览有错误时也为一八() {
        let map = 造图(8, 8);
        let s = preview_state(&map, &[], 2, 2, true).expect("有状态");
        assert!(s.检查.error.is_some(), "空格表必须报 error");
        assert!((s.不透明度 - 不透明度受阻).abs() < 1e-6, "error 也要 0.18");
    }

    /// **没按 Alt ⇒ `None`**（既有的 Alt 门控保留）。
    #[test]
    fn 预览不按alt不返回() {
        let map = 造图(8, 8);
        assert!(preview_state(&map, &小印章(), 2, 2, false).is_none());
    }
}
