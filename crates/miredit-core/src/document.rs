//! 文档：**双表示**（`raw` 字节真值 + `view` 编辑视图）+ 路径 + **脏标记** + `to_bytes()`。
//!
//! 抽取自 `miredit-gui/src/mapdoc.rs:22-44`（`MapDoc`）与
//! `miredit-gui/src/edit.rs:1326-1346`（`index_of` / `cell_at` / `cell_mut`）、
//! `edit.rs:1669-1720`（`ByteDiff` / `byte_diff`）。
//!
//! # 一、双表示：`raw` 是字节真值，`view` 是编辑/渲染视图
//!
//! ```text
//! raw : mir2_format::RawMap  ← 打开时 read_any_tolerant(bytes)；保存时由它出字节
//! view: mir2_map::Map        ← 编辑/渲染只认它（miredit-render 的入口签名就是 &mir2_map::Map）
//! ```
//!
//! - **打开**：`mir2_format::read_any_tolerant` → `RawMap` → [`crate::convert::view_from_raw`] → `Map`；
//! - **编辑**：只改 `view`（`paint` / `fill` / `undo` 一行都没改）；
//! - **保存**：[`Document::try_to_bytes`] 里先 [`crate::convert::sync_view_into_raw`] 把 `view`
//!   的格写回 `raw.cells`，再让 `raw.format` 对应的 writer 出字节，
//!   `header_raw` + `trailer` 原样带上。
//!
//! 为什么必须这样：`mir2_map::Map` 只认 12/14/36 三档（`mir2-map/src/cell.rs:96-104`），
//! 而九种格式的头部长度 / 格长 / 尾区各不相同（见 [`crate::convert`] 模块文档）。
//! 若强行让 `mir2_map::Map` 承载九种格式，就要改渲染层与 103 条既有测试 —— 那是另一件事。
//!
//! # 二、为什么"脏标记"存在这里，而不是再存一份
//!
//! 脏标记的真值只有一个（`Document::dirty`）。**谁能算它**：会话（[`crate::EditSession`]）
//! 拿 `pos != saved_pos || 笔画未结算` 推出来，然后调 [`Document::set_dirty`] 写回这一个字段
//! （`miredit-gui/src/edit.rs:727-729` 的判据原样搬过来）。**绝不两处各存一份**——
//! 两套真值迟早会出现"标题栏有 `*`、保存却什么都不写"这类自相矛盾。
//!
//! # 三、`orig_bytes` 为什么留着
//!
//! 验收要能回答"撤销之后是不是和磁盘上那份**逐字节**相同"（任务书 §六.4），
//! 以及"这一笔在全文件里差了几个字节"（§六.6）。`orig_bytes` 就是打开时读进来的那份字节，
//! **只读、只在 [`Document::diff`] 里被比对**，从不参与写回（写回永远走
//! [`Document::try_to_bytes`] 现算）。代价是一份地图大小的内存（`0.map` 5.6 MB），
//! 换来的是任何时刻都能做字节级自证。
//!
//! # 四、就地改 `view.cells`，不做覆盖层
//!
//! `mir2_map::Map::cells` 是 `pub Vec<Cell>`，**列优先**：`cells[x * height + y]` 对应 `(x, y)`
//! （`mir2-map/src/map.rs` §存储顺序）。本模块就地改 `cells[idx]`：
//!
//! - **不用覆盖层**（`HashMap<(u16,u16), Cell>`）：那会让"内存里的地图"和"文件里的字节"变成
//!   两套真值，保存时要再套回去，漏一个字段就会出现"看着改了、存下去没改"
//!   （`edit.rs:83-87` 原话）；
//! - 就地改的副作用可控：`sync_view_into_raw` 是逐格写回，改哪格就只影响哪格的字节区间。
//!
//! # 五、`view` 与 `raw` 谁可以被外部改
//!
//! `view` 是**编辑面**（`view_mut()` 拿得到），`raw` 是**只读真值**（只有 `raw()`）。
//! 唯一会写 `raw.cells` 的地方是 [`Document::try_to_bytes`]（写在一份 clone 上，
//! 不改文档本身）。这样"文档里的 `raw`"永远等于"磁盘上那份字节"，可以直接做逐字节自证。
//!
//! # 六、五层计数（2026-11-06 新增，见 [`crate::layer_counts`]）
//!
//! 文档缓存五个官方层计数（`_upstream/CrystalStudio/Document.LayerCounts.cs:5` 的等价物）：
//!
//! - **单格写**走 [`Document::write_view_cell`]（写视图 + O(1) 增量更新计数）；
//!   它是编辑路径的**唯一**逐格写入口（`paint` / `fill` / `stamp` / 选区 / 撤销）。
//! - **整块换视图**（`std::mem::swap(document.view_mut(), &mut map)`，GUI 的
//!   `mapdoc.rs:154` / `:194`、`edit.rs:322` / `:324`）绕过了上一个入口 ⇒
//!   换完**必须**调 [`Document::rebuild_layer_counts`]，否则计数停在旧视图上
//!   （旧值不会报错，只会显示错数）。**直接改 `view_mut().cells[…]` 的调用方同理。**
//! - 打开文件（[`Document::from_bytes`]）构造完立刻 `rebuild_layer_counts()` 一次。

use std::path::{Path, PathBuf};

use mir2_format::{format_by_id, FormatCaps, FormatError, FormatId, RawMap};
use mir2_map::{Cell, Map, MapError};

use crate::convert::{ground_paint_supported, sync_view_into_raw, view_cell_format, view_from_raw};
use crate::error::CoreError;
use crate::layer_counts::{cell_mask_ext, count_map, CountKeeper, LayerCounts, MaskContext};

// ─────────────────── 新建空白图：上游「创建」口径的常量与占位 ───────────────────

/// 空白文档的 `path` **占位**（[`Document::path`] 目前是非 `Option` 的 `PathBuf`）。
///
/// # 上游的 `path` 本来就是**可空**的
///
/// `MainWindow.MapTabs.cs:5`：
///
/// ```csharp
/// private sealed class OpenMapTab(MapEditorState state, string? path, string name)
/// ```
///
/// —— 注意 `string? path`。新建标签时传 `null`（`:50` 的 `AddMapTab(MapDocument document, string? path = null)`、
/// `:52` 的 `new OpenMapTab(..., path, $"Untitled {++untitledNumber}")`）；
/// 显示名走 `MapDisplayName`（`:125-131`）：
///
/// ```csharp
/// if (map.Path != null) return (System.IO.Path.GetFileName(map.Path));
/// string prefix = map.IsStamp ? "Untitled stamp " : "Untitled ";
/// return ... LocalText.Format(map.IsStamp ? "Untitled stamp {0}" : "Untitled {0}", number) : (map.Name);
/// ```
///
/// 窗口标题同理（`MainWindow.axaml.cs:1035`：`mapPath == null ? LocalText.T("Untitled") : Path.GetFileName(mapPath)`）。
///
/// ⇒ 「未关联文件」在上游是**一等状态**。本仓 [`Document::path`] 还是 `PathBuf`，
/// 只能用**空路径**当占位；判据统一走 [`Document::has_path`]，
/// **不许**在别处自己写 `is_empty()` / `== ""` / `file_name().is_none()` 各判一套。
///
/// ⚠ `path: PathBuf → Option<PathBuf>` 是跨 crate 的结构改造（core 6 处、gui 38 处、web 4 处，
/// 其中 15 处是「必须非空」需要语义决策），**必须用户拍板**，本单只做调研。
pub const BLANK_PATH: &str = "";

/// 空白文档的 [`Document::file_name`]。
///
/// 上游未关联文件的显示名就是 `"Untitled"`
/// （`Localization/Strings.json:3444-3446` 的 `Untitled_F59AB8D1`；`MainWindow.axaml.cs:1035`）。
///
/// ⚠ **不带序号**：上游页签名是 `$"Untitled {++untitledNumber}"`
/// （`MainWindow.MapTabs.cs:52`），序号来自**窗口级**计数器
/// （`MainWindow.MapTabs.cs:23` 的 `private int untitledNumber;`）——
/// [`Document`] 是文档级类型，拿不到也不该持有窗口状态 ⇒ 序号由宿主追加。
pub const UNTITLED_FILE_NAME: &str = "Untitled";

/// 上游新建窗口的单边**下限**（含）。
///
/// 真值：`_upstream/CrystalStudio/MapDimensions.cs:31-32`
/// （`width is < 16 or > 4096 || height is < 16 or > 4096` ⇒ `"Enter dimensions between 16 and 4096."`）。
pub const CREATION_MIN_DIMENSION: u16 = 16;

/// 上游新建窗口的单边**上限**（含）。真值同 [`CREATION_MIN_DIMENSION`]（`MapDimensions.cs:31`）。
pub const CREATION_MAX_DIMENSION: u16 = 4096;

/// 上游单张地图的格数上限（`_upstream/CrystalStudio/MapDimensions.cs:7` 的 `MaximumCells = 16_000_000`）。
///
/// 同值另见 `MapDimensions.cs:11-12` 的 `CanRoundTrip` 第三条、`LegacyMapReader.cs:50`。
pub const CREATION_MAX_CELLS: u64 = 16_000_000;

/// **整档快照**（字节层 + 视图）：尺寸变化的撤销/重做单位。
///
/// 为什么要连视图一起存：撤销一次裁剪要回到"旧尺寸 + 旧格 + 旧视图"。只存 `RawMap`
/// 虽然能用 [`crate::convert::view_from_raw`] 重建视图，但视图里还有**投影档位**
/// （`Map::format` / 视图头）这类不属于字节层的东西；存两份最省心，代价是内存约两倍地图。
///
/// 上游同形：`History.Resize` 压的就是 `MapRecoveryStore.Capture(document)` 整份快照
/// （`Document.cs` 的 `PushUndo(new([], null, state, Capture(document)))`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocSnapshot {
    /// 字节层真值（`format` / `header_raw` / `cells` / `trailer`）。
    pub raw: RawMap,
    /// 编辑视图（`width`/`height`/`format`/`header_raw`/`cells`）。
    pub view: Map,
}

/// 一张已解析的地图 + 它的出处信息。抽取自 `mapdoc.rs:22-44`。
#[derive(Debug)]
pub struct Document {
    /// **字节层真值**：`format` / `header_raw` / `trailer` / `cells`（`mir2-format/src/model.rs:305-344`）。
    ///
    /// 保存时由它出字节；它永远是"打开时那份字节 + 视图改过的格"。
    raw: RawMap,
    /// **编辑/渲染视图**：`mir2_map::Map`（`mir2-map/src/map.rs:28-41`）。
    ///
    /// 编辑只改它。它的 `format` 是**语义投影**（见 [`crate::convert::view_cell_format`]），
    /// 不是磁盘格长；它的 `header_raw` 是**视图头**，永不写回。
    view: Map,
    /// 打开时的文件路径（原样保留，用于保存与诊断）。
    pub path: PathBuf,
    /// 文件名（含扩展名，如 `0.map`）——标题栏、页签、状态栏都用它。
    pub file_name: String,
    /// 打开时的**磁盘原文件字节**（只读；见模块文档第三节）。
    orig_bytes: Vec<u8>,
    /// 脏标记（唯一真值；由 [`crate::EditSession`] 在每次改动后写回）。
    dirty: bool,
    /// `raw.trailer` 的字节数（= `raw.trailer_len()`）—— **「本工具没解释的剩余字节」**。
    ///
    /// 非 0 就必须告警（见 [`Document::edit_warning`]）：`mir2-format/src/model.rs:330-342`
    /// 明确要求「上层拿到 `trailer_len() != 0` 时必须提示用户：本工具按头部 `w·h` 解释，
    /// 剩余 N 字节未解释」。实测 67 张真实文件（`EM001.map` 等）全部 `8040` 字节。
    tolerant_trailer: usize,
    /// **五层计数缓存**（见模块文档第六节与 [`crate::layer_counts`]）。
    ///
    /// 唯一维护路径：[`Document::write_view_cell`]（增量）与
    /// [`Document::rebuild_layer_counts`]（全量）；**没有第三处**。
    counts: LayerCounts,
}

impl Document {
    /// 从磁盘读并解析一张 `.map`。
    ///
    /// 失败分两类，都带路径：读不出来 ⇒ [`CoreError::Io`]；读出来但九种格式都判不出来 ⇒
    /// [`CoreError::Map`]（判据全在 `mir2-format` 里，这里不重写）。
    pub fn load(path: &Path) -> Result<Document, CoreError> {
        let bytes = std::fs::read(path).map_err(|e| CoreError::Io {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
        Document::from_bytes(path, bytes)
    }

    /// **从字节层直接构造**（crate 内部 / 测试用）：视图由 `view_from_raw` 现算，
    /// `orig_bytes` 取该格式 writer 出的字节（"打开时那份"的等价物）。
    ///
    /// 为什么需要它：`from_bytes` 要先过 `read_any_tolerant` 的**格式判别**（魔数/标题），
    /// 而合成夹具的头部通常没有那些签名。本构造器跳过判别，只用于已经把 `RawMap` 拿在手里的
    /// 调用方（改尺寸的单元测试、将来可能的"从内存图新建"）。
    #[cfg(test)]
    pub(crate) fn from_raw(raw: RawMap) -> Document {
        let path = std::path::PathBuf::from(r"I:\__miredit_internal__\raw.map");
        let orig_bytes = format_by_id(raw.format).write(&raw).unwrap_or_default();
        let view = view_from_raw(&raw);
        let tolerant_trailer = raw.trailer_len();
        Document {
            raw,
            view,
            path,
            file_name: "raw.map".to_string(),
            orig_bytes,
            dirty: false,
            tolerant_trailer,
            counts: LayerCounts::default(),
        }
    }

    /// 从已有字节构造（网页版上传一份字节、或测试用内存图）。
    ///
    /// `bytes` 同时成为 [`Document::orig_bytes`]：构造那一刻"磁盘上是什么"就是它。
    ///
    /// # 为什么用 `read_any_tolerant` 而不是 `read_any`
    ///
    /// 真实语料里有 **67 张**「长度多一截」的文件（`client/热血传奇23周年客户端/Map/EM001.map`
    /// 等：头部声明 30×35 = 1050 格、`52 + 12×1050 = 12,652`，实际 20,692 字节）。
    /// 严格入口会按规格拒绝它们（`FormatError::LengthMismatch`），而编辑器**必须能打开**
    /// （验收门 1：1,158 张全部能 open）。宽松入口把那 8,040 字节原样放进 `raw.trailer`，
    /// **一个字节都不丢**，同时把「没解释的字节数」暴露在 [`Document::tolerant_trailer`] 上。
    pub fn from_bytes(path: &Path, bytes: Vec<u8>) -> Result<Document, CoreError> {
        let raw = mir2_format::read_any_tolerant(&bytes).map_err(|err: FormatError| CoreError::Map {
            path: path.to_path_buf(),
            err,
        })?;
        let view = view_from_raw(&raw);
        let tolerant_trailer = raw.trailer_len();
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        Ok(Document {
            raw,
            view,
            path: path.to_path_buf(),
            file_name,
            orig_bytes: bytes,
            dirty: false,
            tolerant_trailer,
            counts: LayerCounts::zero(),
        })
        // 构造完立刻全量数一次（`_upstream/CrystalStudio/Document.LayerCounts.cs:5` 的五个计数在
        // `MapDocument.Load` 的逐格 `Set` 里被累计，等价于这里的一次 O(n) 重建）。
        .map(|mut doc| {
            doc.rebuild_layer_counts();
            doc
        })
    }

    // ── 新建空白图（2026-11-08 新增；上游 `MapDocument(int,int)` 的等价物） ──

    /// 本文档**是否已关联磁盘文件**（`false` = [`Document::blank_in`] 造出来的空白文档）。
    ///
    /// 唯一判据：`path` 是不是 [`BLANK_PATH`] 那个占位。
    /// 为什么要有这个方法：[`Document::path`] 现在是 `PathBuf`（不是 `Option<PathBuf>`），
    /// 「未关联」只能用空路径表达 ⇒ 这个占位必须**单点定义**，否则各处会写出不同口径
    /// （有人判 `""`、有人判 `"."`、有人拿 `file_name().is_none()` 凑，迟早对不上）。
    ///
    /// 上游同形：`MainWindow.MapTabs.cs:127` 的 `if (map.Path != null)`。
    pub fn has_path(&self) -> bool {
        self.path.as_path() != Path::new(BLANK_PATH)
    }

    /// **新建一张空白地图文档**（`width × height`，磁盘格式 = [`FormatId::Old12`]）。
    ///
    /// 对应上游 `AddMapTab(new(256, 256))` —— `MainWindow.MapTabs.cs:105`：
    ///
    /// ```csharp
    /// if (mapTabs.Items.Count == 0) AddMapTab(new(256, 256));   // 关掉最后一个标签时补一张空白图
    /// ```
    ///
    /// 里的那个 `new MapDocument(256, 256)`（构造语义见 [`mir2_map::Map::blank`] 文档第一节）。
    ///
    /// # 与上游的三处**已知偏离**（不是对齐；逐条写清，等拍板）
    ///
    /// | # | 上游 | 本仓 | 原因 |
    /// |---|---|---|---|
    /// | 1 | 构造器**不选格式**，保存时永远写自己的 Crystal 格式（`Document.cs:90`、`:102`） | 默认写 [`FormatId::Old12`]（12 B/格） | `CrystalCs` 是**变长格**（26/38/35/33+）+ 尾块 + **0 真实样本** + `editable == false`（`mir2-format/src/model.rs:479`）⇒ 造得出、验不了。逐格式闸门见 [`Document::blank_in`] |
    /// | 2 | `path` 是 `string?`，新建时 `null`（`MainWindow.MapTabs.cs:5`、`:50`） | `path` 是 `PathBuf`，用 [`BLANK_PATH`]（空路径）占位 | `PathBuf → Option<PathBuf>` 是跨 crate 结构改造，须用户拍板（本单只做调研，见 [`BLANK_PATH`] 文档） |
    /// | 3 | 显示名 `Untitled {N}`（`MainWindow.MapTabs.cs:52`，`N` 来自窗口级 `untitledNumber`） | [`Document::file_name`] = [`UNTITLED_FILE_NAME`]（不带序号） | 序号是窗口状态（`MainWindow.MapTabs.cs:23`），文档级类型拿不到；序号由宿主追加 |
    ///
    /// # Errors
    ///
    /// 尺寸不合上游新建口径 ⇒ [`CoreError::Map`]（原因里带 `MapDimensions.cs` 行号）。
    /// 详见 [`Document::blank_in`]。
    pub fn blank(width: u16, height: u16) -> Result<Document, CoreError> {
        Document::blank_in(width, height, FormatId::Old12)
    }

    /// **按指定磁盘格式**新建一张空白地图文档（[`Document::blank`] 的带格式版）。
    ///
    /// # 一、支持的格式：`Old12` 与 `Shanda14`
    ///
    /// 判据三条**合取**，每条都能回指源码（不是"挑两个简单的"）：
    ///
    /// 1. **`caps().editable == true`** —— `mir2-format/src/model.rs:471-472` 的逐格式结论表里
    ///    只有这两种是 `true`（契约：`editable == true` ⟺ `editable_caveat` 是空串，
    ///    由 `mir2-format/src/registry.rs` 的 `editable与caveat必须一致` 钉住）。
    ///    空白图是**拿来编辑**的起点：起点格式若本仓自己都不承诺「改格后语义自洽」，
    ///    用户建完立刻收到一条常驻黄条，那是个荒谬的初始状态。
    /// 2. **格内无未建模尾区**（`TAIL_LEN == 0`）—— 有尾区的格式（`Shanda2012` 22 B、
    ///    `Mir3Shanda` 7 B、`Mir3Wemade` 4 B、`Wemade2010`/`Heroes34` 各 1 B、`CrystalCs` 变长）
    ///    新建时必须决定「这 N 个字节填什么」，而**上游没有任何新建路径给出过依据**
    ///    （C# 只有 `LoadMapType*` 读 + `SaveCore` 写自己那套）⇒ 填 0 就是猜值，违铁律。
    /// 3. **`detect_format` 能凭长度唯一认出**（`mir2-format/src/registry.rs:139-165`：
    ///    52 字节 `TMapHeader` 家族里 12 / 14 / 36 B 三档**只看长度、标题不参与**）
    ///    —— 保证写出的字节**能被本仓自己的 `read_any` 原样读回来**。
    ///    其余 6 种要靠头部签名（`AntiHack` 的 `0x15/0x32/0x41/0x31`、
    ///    `Mir3Shanda` 的 `(C) SNDA, MIR3`、`CrystalCs` 的 `C#` 魔数、`Mir3Wemade` 的
    ///    「首字节 0 + 头部 20/22/24 处的三元组」）或异或键（`AntiHack`/`Wemade2010` 的 `w ^ xor`）
    ///    才认得出 ⇒ 空白图必须先**编造**签名或键。
    ///
    /// ⇒ 支持集 = `{Old12, Shanda14}`；其余 7 种**显式拒绝并给中文原因**（不静默产出坏图）。
    ///
    /// # 二、尺寸口径：先按上游 `CreationError`，再交给 `mir2-map`
    ///
    /// 顺序与上游 `MapDimensions.CreationError` 一致（`_upstream/CrystalStudio/MapDimensions.cs:28-41`）：
    ///
    /// 1. `:31-32` 单边必须落在 `16..=4096`（`"Enter dimensions between 16 and 4096."`）；
    /// 2. `:33` `!CanRoundTrip` ⇒ 格数上限（`"A map supports at most 16,000,000 cells."`）。
    ///    这一条在窗口内**仍可能命中**：`4096×4096 = 16,777,216 > 16,000,000`；
    /// 3. `:34-39` 的 Province / Dungeon / Terrain 三条是**生成器专属**，
    ///    `MapCreationKind.Blank` 不适用（`MapCreationKind` 定义在 `MapDimensions.cs:3`：
    ///    `{ Blank, Terrain, Province, Dungeon }`；新建空白图传的就是 `Blank`，
    ///    见 `MainWindow.axaml.cs:461`）。
    ///
    /// 过了这两条之后才调 [`mir2_map::Map::blank`]（它内部是 `validate_dimensions`，
    /// 与 [`Map::parse`] 同一个单点）—— 那时尺寸已在 `16..=4096` 且 `w*h ≤ 16M`，
    /// 所以它只剩「防御性」意义（真命中说明两道门漂了）。
    ///
    /// # 三、字节层怎么来：走**既有**读入口，不另造一份 `RawMap` 构造逻辑
    ///
    /// `Map::blank` → `to_bytes()` → [`Document::from_bytes`]。
    /// 这样 `raw` 与 `view` **一定同源**（`from_bytes` 里 `read_any_tolerant` 会凭长度
    /// 认出 `Old12`/`Shanda14`，见 `registry.rs:139-165`），也省掉一份「手工拼 RawMap」
    /// 的第二套真值。构造完再用 `debug_assert` 自证两层逐格一致（见下）。
    ///
    /// # Errors
    ///
    /// - 格式不在支持集 ⇒ [`CoreError::Map`]（payload 是 [`FormatError::Inconsistent`]，
    ///   文案里点名格式与具体原因）；
    /// - 尺寸不合上游新建口径 ⇒ [`CoreError::Map`]（同上，文案带 `MapDimensions.cs` 行号）。
    pub fn blank_in(width: u16, height: u16, format: FormatId) -> Result<Document, CoreError> {
        let path = Path::new(BLANK_PATH);

        // ① 格式闸门（在尺寸之前：格式不支持与尺寸无关，先报更有用的那条）。
        if let Err(原因) = blank_gate(format) {
            return Err(CoreError::Map {
                path: path.to_path_buf(),
                err: FormatError::Inconsistent {
                    detail: format!(
                        "本仓还不能新建「{}」格式的空白图：{原因}",
                        format_by_id(format).name()
                    ),
                },
            });
        }

        // ② 上游 `CreationError(w, h, MapCreationKind.Blank)` 的两条（`MapDimensions.cs:31-33`）。
        if let Some(原因) = creation_error(width, height) {
            return Err(CoreError::Map {
                path: path.to_path_buf(),
                err: FormatError::Inconsistent { detail: 原因 },
            });
        }

        // ③ 视图层：空白图由 `mir2-map` 造（尺寸判据在那边单点实现，见 `Map::blank` 文档）。
        let view = Map::blank(width, height, view_cell_format(format))
            .map_err(|err| blank_size_error(path, format, err))?;

        // ④ 出字节 → 走既有读入口打开（保证 raw/view 同源，见本节第三节）。
        let mut doc = Document::from_bytes(path, view.to_bytes())?;

        // ⑤ 两层一致性自证：`raw → view` 的投影必须与 `Map::blank` 逐格相同。
        //    不等就说明两个 crate 的「全 0 格」口径已经漂了 —— 宁可炸，不许静默留两份真值。
        debug_assert_eq!(
            doc.view(),
            &view,
            "空白图的 raw→view 投影与 Map::blank 不一致（两个 crate 的空白格口径漂了）"
        );

        // ⑥ 文件名：上游未关联文件的显示名是 "Untitled"（见 `UNTITLED_FILE_NAME`）。
        //    `from_bytes` 是按路径推文件名的，空路径推出来是空串，所以这里必须显式覆盖。
        doc.file_name = UNTITLED_FILE_NAME.to_string();

        Ok(doc)
    }

    // ── 双表示的两侧 ──

    /// **字节层真值**（只读）。`format` / `header_raw` / `trailer` / `cells` 全在这里。
    ///
    /// ⚠ 它是**打开那一刻**的磁盘字节：编辑只改 [`Document::view`]，
    /// 改格写回 raw 只发生在 [`Document::try_to_bytes`] 的那份 clone 上
    /// （见模块文档第五节）。要「当前编辑结果的字节层」请用
    /// [`Document::raw_snapshot`]（`miredit-gui` 的 `MapDoc::raw` 就是它）。
    pub fn raw(&self) -> &RawMap {
        &self.raw
    }

    /// **以「当前编辑视图」为准的一份 raw 快照**（只读；`self` 一格不变）。
    ///
    /// 实现就是保存路径用的同一条 [`crate::convert::sync_view_into_raw`]（逐格按
    /// `FIELD_MAP` 写回：`bk_img`/`tiles`/`sm_tiles` 保高位、其余整字段覆盖，
    /// `tail` 长度不符时整段不动），所以「快照 == 现在保存会写出的那份格表」。
    /// `format`/`header_raw`/`trailer` 原样带过来（快照不重算头部）。
    ///
    /// # 为什么要它
    ///
    /// A-015「检查地图」是**只读诊断**，但必须看**用户眼前的**图，而不是打开时的图：
    /// 宿主（GUI）把编辑中的视图单独放在 `MapDoc::map` 里，`Document::view` 只在
    /// 每次编辑调用前后交换一次（`edit.rs` 的 `with_map`）⇒ 它是**上一次**的副本。
    /// 直接拿 [`Document::raw`] 去检查会把用户刚改的图号当成没改。
    ///
    /// # Errors
    ///
    /// 视图与 raw 的宽高/格数不一致 ⇒ [`ConvertError::Shape`]（**不猜**，调用方报错）。
    pub fn raw_snapshot(&self, view: &Map) -> Result<RawMap, crate::convert::ConvertError> {
        let mut raw = self.raw.clone();
        crate::convert::sync_view_into_raw(view, &mut raw)?;
        Ok(raw)
    }

    /// **字节层真值**（可变，**crate 内部**）：只有 `resize`（改尺寸）用它整体替换。
    ///
    /// 通用编辑路径**不许**用它 —— 改格要走 [`Document::write_view_cell`]（维护五层计数），
    /// 出字节要走 [`Document::try_to_bytes`]（在 clone 上同步视图）。
    pub(crate) fn raw_mut(&mut self) -> &mut RawMap {
        &mut self.raw
    }

    /// **编辑/渲染视图**（只读）。渲染层要的就是它。
    pub fn view(&self) -> &Map {
        &self.view
    }

    /// **编辑/渲染视图**（可变）—— 编辑路径（`paint` / `fill` / `undo`）走这条。
    ///
    /// ⚠ 改 `view.header_raw` / `view.format` / `view.width` **不会**影响保存结果：
    /// 保存走 `raw`，视图头与视图档位都是投影（见模块文档第一节）。
    /// 唯一有意义的改动是 `view.cells`。
    pub fn view_mut(&mut self) -> &mut Map {
        &mut self.view
    }

    // ── 五层计数（2026-11-06 新增；口径见 [`crate::layer_counts`]） ──

    /// 当前五层计数（官方 `LayerCount(MapLayer)` × 5 的缓存）。
    ///
    /// 由 [`Document::write_view_cell`] / [`Document::rebuild_layer_counts`] 维护；
    /// 直接改 `view_mut()` 的调用方必须自己调 `rebuild_layer_counts()`（模块文档第六节）。
    pub fn layer_counts(&self) -> LayerCounts {
        self.counts
    }

    /// **整块重建**五层计数（O(n)），返回重建后的结果。
    ///
    /// 两个调用点（**只有这两个**）：
    /// 1. [`Document::from_bytes`] 构造完之后（自动）；
    /// 2. 外部**整块换视图**之后 —— GUI 用
    ///    `std::mem::swap(document.view_mut(), &mut map)`（`mapdoc.rs:154` / `:194`、
    ///    `edit.rs:322` / `:324`）或直接改 `view_mut().cells[…]` 时，
    ///    绕过了 [`Document::write_view_cell`] ⇒ 必须调本方法把计数追平新视图。
    pub fn rebuild_layer_counts(&mut self) -> LayerCounts {
        self.counts = count_map(&self.raw, &self.view);
        self.counts
    }

    /// **写一格视图 + 增量更新计数**（编辑路径的**唯一**逐格写入口）。
    ///
    /// 返回 `true` = 真的写了（格值变了）；`false` = 没写、计数也没动，三种来源：
    /// 1. `(x, y)` 越界；
    /// 2. `after` 与当前格**逐字段相同**（no-op 不许动计数，也不许产生撤销步）；
    /// 3. 字节层与视图的格号对不上（[`crate::convert::ConvertError::Shape`] 类不变量被破坏）
    ///    —— 此时**宁可一格不写**，也不写一个「计数说不清」的格。
    pub fn write_view_cell(&mut self, x: u16, y: u16, after: Cell) -> bool {
        let Some(i) = index_of(&self.view, x, y) else {
            return false;
        };
        let before = self.view.cells[i];
        if before == after {
            return false;
        }
        let Some(raw_cell) = self.raw.cells.get(i) else {
            return false;
        };
        let ctx = MaskContext::from_raw(&self.raw);
        let before_mask = cell_mask_ext(raw_cell, &before, ctx, x, y);
        let after_mask = cell_mask_ext(raw_cell, &after, ctx, x, y);
        self.view.cells[i] = after;
        self.counts.apply(before_mask, after_mask);
        true
    }

    /// 借出「视图（可写）+ 计数更新器（可写）+ 字节层（只读）」三件**不重叠**的借用。
    ///
    /// 给撤销 / 重做用：`UndoStack::undo_with` 需要 `&mut Map` 写回旧格，同时要按
    /// 「每写一格」更新计数；若只借 `&mut Document`，两件事会撞在同一个 `&mut self` 上。
    pub(crate) fn view_and_keeper(&mut self) -> (&mut Map, CountKeeper<'_>) {
        let Document {
            raw,
            view,
            counts,
            ..
        } = self;
        (view, CountKeeper::new(raw, counts))
    }

    /// 格数组之前的**真头部字节**（九种格式各自长度不同，`mir2-format/src/model.rs:312-316`）。
    ///
    /// ⚠ 与 [`Document::view`]`().header_raw` 不同：那是定长 52 的**视图头**（永不写回）。
    pub fn raw_header(&self) -> &[u8] {
        &self.raw.header_raw
    }

    /// 格数组之后**没被解释**的字节（原样保留、原样写回）。
    pub fn trailer(&self) -> &[u8] {
        &self.raw.trailer
    }

    // ── 格式信息 ──

    /// 九种格式的标识（真值来自 `mir2-format` 的判别，不是猜的）。
    pub fn format_id(&self) -> FormatId {
        self.raw.format
    }

    /// 格式人读名（带格长与出处，例如 `"Shanda2012（36B/格，14B 字段 + 22B 未建模尾区）"`）。
    pub fn format_label(&self) -> &'static str {
        format_by_id(self.raw.format).name()
    }

    /// 该格式的能力声明（能不能写、能不能编辑、地表逐格落笔有没有效、碰撞极性…）。
    ///
    /// 依据 `mir2-format/src/model.rs:419-461`。
    pub fn caps(&self) -> FormatCaps {
        format_by_id(self.raw.format).caps()
    }

    /// 取一份**整档快照**（尺寸变化的撤销/重做用）。
    ///
    /// 只读：`self` 一格不变。
    #[must_use]
    pub fn snapshot(&self) -> DocSnapshot {
        DocSnapshot {
            raw: self.raw.clone(),
            view: self.view.clone(),
        }
    }

    /// **整体替换字节层**（宿主在**尺寸变化之后**同步保存源用）。
    ///
    /// 只做三件事：换 `raw`、按它重建视图（就地）、重算五层计数。**不校验、不置脏**。
    ///
    /// ⚠ 只许在「刚做完尺寸变化 / 尺寸变化的撤销重做」之后调用，且 `raw` 必须来自
    /// 同一个内核会话（[`crate::EditSession::document`]）—— 那是唯一知道新尺寸的权威来源。
    /// 通用编辑路径**不许**用它（改格要走 [`Document::write_view_cell`]）。
    pub fn replace_raw_after_resize(&mut self, raw: RawMap) {
        self.raw = raw;
        self.rebuild_view_in_place();
        self.rebuild_layer_counts();
    }

    /// 用整档快照还原（**就地**换视图字段，不换 `Map` 结构体）。
    ///
    /// 视图就地换的理由同 [`Document::replace_resized`]：宿主把同一个 `Map` 对象
    /// 换进换出，换结构体会让新尺寸的视图留在文档里、宿主拿到旧的那份。
    ///
    /// 脏标记**不在这里定**：撤销/重做后由 [`crate::EditSession::sync_dirty`] 按
    /// 「当前步号 vs 保存步号」重算（与本 crate 其它改格路径同一条纪律）。
    pub fn restore_snapshot(&mut self, snap: DocSnapshot) {
        self.raw = snap.raw;
        let v = snap.view;
        self.view.width = v.width;
        self.view.height = v.height;
        self.view.format = v.format;
        self.view.header_raw = v.header_raw;
        self.view.cells = v.cells;
        self.rebuild_layer_counts();
    }

    /// 宽 × 高（世界格）。来源：`mapdoc.rs:36-38`。
    pub fn size(&self) -> (u16, u16) {
        (self.raw.width, self.raw.height)
    }

    /// **视图档位名**（`Old12` / `New14` / `Ext36`）—— 注意这是 `mir2_map::CellFormat` 的
    /// 三档语义投影，**不是**磁盘格长（见 [`crate::convert::view_cell_format`]）。
    ///
    /// 真格式名请用 [`Document::format_label`]。
    pub fn format_name(&self) -> &'static str {
        self.view.format.name()
    }

    /// 磁盘单格字节数（变长格式如 `CrystalCs` 返回 `None`）。
    ///
    /// 与 [`Document::format_name`] 的区别：这个才是**文件里一格几字节**。
    pub fn disk_cell_len(&self) -> Option<usize> {
        self.caps().fixed_cell_len
    }

    // ── 未解释字节 / 告警 ──

    /// `raw.trailer` 的字节数 —— **「本工具没解释的剩余字节」**。
    ///
    /// 非 0 ⇒ 必须告警（见 [`Document::edit_warning`]）。实测那 67 张真实文件都是 `8040`。
    pub fn tolerant_trailer(&self) -> usize {
        self.tolerant_trailer
    }

    /// **常驻告警**（`None` = 没有需要提醒的事）。GUI/网页应当把它常驻显示。
    ///
    /// 两种来源，可能**同时**成立（用「；」连接）：
    ///
    /// 1. **未解尾块**（[`Document::tolerant_trailer`] `> 0`）：
    ///    「本图有 N 字节未解释尾块（按头部 w×h 解释）」
    ///    —— 依据 `mir2-format/src/model.rs` 对 [`mir2_format::RawMap::trailer`] 的强制要求。
    ///    **这条是真警告，任何改动都不许削弱它**（67 张 `EM001.map` 类文件尾块 8,040 字节，
    ///    若真实尺寸不是头部声明值，列优先展开会让整图错位）。
    /// 2. **格式不承诺「改格后语义自洽」**（`caps().editable == false`）：
    ///    文案**逐格式具体**取自 [`mir2_format::FormatCaps::editable_caveat`]，
    ///    说清「哪一段、多少字节、编辑时怎么处理」。
    ///
    /// # ⚠ 2026-11-05 订正（P1-9）：这里曾是一句**假话**
    ///
    /// 旧实现不管哪种格式都追加「本格式（X）未解区语义未知，编辑仅保证字节保真，
    /// 不保证动画/联动正确」。对 `Old12` 那是**事实错误** ——
    /// 它的 12 字节全部有名字（`mir2-format/src/formats/old12.rs` 字段表）、`tail` 恒空、
    /// [`crate::convert::unmodeled_fields`]`(Old12)` 为空、856 张真实样本往返 0 字节差异
    /// ⇒ 它**没有"未解区"**。
    /// 后果：1,051 / 1,158 张真实图（90.8%，含缺省打开的 `0.map`）一打开就挂一条常驻黄条，
    /// 用户很快学会无视它，而真正要看的第 1 条（尾块可能整图错位）就排在同一条黄条里被淹没。
    ///
    /// ⇒ 现在 `editable == true` 的格式（`Old12` / `Shanda14`）在来源 2 上**一个字都不出**；
    /// 其余 7 种各说各的具体原因（见 `mir2-format/src/formats/*.rs` 的 `caps()`）。
    ///
    /// ⚠ `editable == false` **不等于不能编辑**：本 crate 一律允许编辑，只是不承诺语义自洽。
    pub fn edit_warning(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if self.tolerant_trailer > 0 {
            parts.push(format!(
                "本图有 {} 字节未解释尾块（按头部 {}×{} 解释）——若真实尺寸不是头部声明值，\
                 列优先展开会让整图错位",
                self.tolerant_trailer, self.raw.width, self.raw.height
            ));
        }
        let caps = self.caps();
        if !caps.editable {
            // 契约（`mir2-format/src/registry.rs` 的 `editable与caveat必须一致` 钉住）：
            // `editable == false` ⟺ `editable_caveat` 非空。走到空串分支说明契约被破坏 ——
            // 此时宁可说「未给出具体原因」，也**不许**再编一句「未解区语义未知」那种套话。
            let 原因 = if caps.editable_caveat.is_empty() {
                "改格后语义自洽性未经确认（本格式未给出具体原因）"
            } else {
                caps.editable_caveat
            };
            parts.push(format!("本格式「{}」{}", self.format_label(), 原因));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("；"))
        }
    }

    // ── 字节 ──

    /// 打开时的文件字节数（诊断用；也是"真的读了这么多字节"的凭据）。
    pub fn orig_len(&self) -> usize {
        self.orig_bytes.len()
    }

    /// 打开时的磁盘原文件字节（只读视图）。见模块文档第三节。
    pub fn orig_bytes(&self) -> &[u8] {
        &self.orig_bytes
    }

    /// **保存写回的就是它**：`view → raw` 同步后由 `raw.format` 的 writer 出字节。
    ///
    /// # Panics
    ///
    /// 只在 [`Document::try_to_bytes`] 失败时 panic（会带上原因）。失败意味着
    /// 「`raw` 不再是它自己那次 `read` 的产物」或「有人把 `view` 的宽高改坏了」——
    /// 那是本 crate 的 bug，**宁可炸也不许静默回退到原字节**（回退＝假保存）。
    /// 生产路径（`save_session`）走的是 [`Document::try_to_bytes`]，失败会变成
    /// [`CoreError::Map`]（payload = [`FormatError::Inconsistent`]）。
    pub fn to_bytes(&self) -> Vec<u8> {
        self.try_to_bytes()
            .unwrap_or_else(|e| panic!("文档字节生成失败（这是本 crate 的 bug，不许静默回退）：{e}"))
    }

    /// 同 [`Document::to_bytes`]，但把失败如实报出来。
    ///
    /// 两条失败来源，都带明确原因，且都折成 [`CoreError::Map`]（payload 是
    /// [`FormatError`]，理由见 `error.rs` 模块文档最后一节）：
    /// 1. [`crate::convert::ConvertError::Shape`] —— 视图与 `raw` 的宽高/格数对不上
    ///    （包装成 [`FormatError::Inconsistent`]）；
    /// 2. [`FormatError`] —— writer 拒绝了某个值（例如该格式的图号只有 16 位）。
    ///
    /// ⚠ **绝不**回退到"原字节"：回退＝用户以为存上了，实际磁盘上还是旧内容。
    pub fn try_to_bytes(&self) -> Result<Vec<u8>, CoreError> {
        let mut raw = self.raw.clone();
        sync_view_into_raw(&self.view, &mut raw).map_err(|e| CoreError::Map {
            path: self.path.clone(),
            err: FormatError::Inconsistent {
                detail: e.to_string(),
            },
        })?;
        format_by_id(raw.format)
            .write(&raw)
            .map_err(|err| CoreError::Map {
                path: self.path.clone(),
                err,
            })
    }

    // ── 脏标记 ──

    /// 是否有未保存的修改。判据由会话算、存在这一个字段里（见模块文档第二节）。
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// 写脏标记（**会话是唯一写入点**；见模块文档第二节）。
    pub fn set_dirty(&mut self, v: bool) {
        self.dirty = v;
    }

    /// 标脏（改过一格就调一次）。
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// 标干净（保存成功后调）。
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// 当前内存地图 vs 磁盘原文件的逐字节差异。来源：`edit.rs:1690-1720`。
    ///
    /// 用的是 [`Document::to_bytes`]（= 真保存字节），不是 `view.to_bytes()`
    /// —— 后者对九种里的七种根本不是文件字节。
    pub fn diff(&self) -> ByteDiff {
        byte_diff_bytes(
            &self.to_bytes(),
            &self.orig_bytes,
            self.raw.header_raw.len(),
            self.disk_cell_len().unwrap_or(1).max(1),
        )
    }

    /// 该格式的「地表逐格落笔」是否有效（`false` 时地表笔刷必须被拒）。
    ///
    /// 九种里只有 `Mir3Wemade`（Type5）为 `false`；判据在
    /// [`crate::convert::ground_paint_supported`]。
    pub fn ground_paint_supported(&self) -> bool {
        ground_paint_supported(self.raw.format)
    }
}

// ─────────────────── 新建空白图：格式闸门 / 上游创建判据 / 错误折叠 ───────────────────

/// 上游 `MapDimensions.CreationError(width, height, MapCreationKind.Blank)` 的等价物。
///
/// # 上游原文（`_upstream/CrystalStudio/MapDimensions.cs:28-41`）
///
/// ```csharp
/// public static string? CreationError(int width, int height, MapCreationKind kind)
/// {
///     if (!Enum.IsDefined(kind)) return "Choose a map type.";                                   // :30
///     if (width is < 16 or > 4096 || height is < 16 or > 4096)
///         return "Enter dimensions between 16 and 4096.";                                        // :31-32
///     if (!CanRoundTrip(width, height)) return "A map supports at most 16,000,000 cells.";       // :33
///     if (kind == MapCreationKind.Province && (width < 128 || height < 128 || ...)) return ...;  // :34-35
///     if (kind == MapCreationKind.Dungeon  && (width < 64  || height < 64  || ...)) return ...;  // :36-37
///     if (kind == MapCreationKind.Terrain  && ...) return ...;                                   // :38-39
///     return null;                                                                               // :40
/// }
/// ```
///
/// `MapCreationKind` 定义在 `MapDimensions.cs:3`：`{ Blank, Terrain, Province, Dungeon }`。
/// 新建空白图传的是 `Blank`（`MainWindow.axaml.cs:461`、`NewMapWindow.cs:222-225`），
/// ⇒ **只受 `:30`-`:33` 约束**，后三条是生成器专属，本函数不实现（也不假装实现）。
///
/// # 与 `mir2_map::validate_dimensions` 的分工
///
/// | 判据 | 上游 | 允许范围 |
/// |---|---|---|
/// | 往返（`CanRoundTrip`） | `MapDimensions.cs:11-12` | `1..=32767` 且 `w*h ≤ 16M` |
/// | 新建（`CreationError(.., Blank)`） | `MapDimensions.cs:31-33` | **`16..=4096`** 且 `w*h ≤ 16M` |
///
/// 两者都要跑，顺序照上游（先窗口、后往返）—— 本函数是**窗口 + 格数**那一道，
/// 往返那一道在 [`mir2_map::validate_dimensions`] 里（由 `Map::blank` 调用）。
///
/// 返回 `None` = 可以新建；`Some(中文原因)` = 拒绝（原因里带上游 `MapDimensions.cs` 行号）。
fn creation_error(width: u16, height: u16) -> Option<String> {
    // ① `MapDimensions.cs:31-32`：`width is < 16 or > 4096 || height is < 16 or > 4096`
    if !(CREATION_MIN_DIMENSION..=CREATION_MAX_DIMENSION).contains(&width)
        || !(CREATION_MIN_DIMENSION..=CREATION_MAX_DIMENSION).contains(&height)
    {
        return Some(format!(
            "新建地图的宽高必须落在 {CREATION_MIN_DIMENSION}..={CREATION_MAX_DIMENSION} 之间，\
             收到 w={width} h={height}\
             （上游 MapDimensions.cs:31-32 的 `width is < 16 or > 4096 || height is < 16 or > 4096`）"
        ));
    }
    // ② `MapDimensions.cs:33`：`if (!CanRoundTrip(width, height))`
    //
    //    为什么窗口过了还要再判一次格数：`4096×4096 = 16,777,216 > 16,000,000`，
    //    单边合格但格数超限 —— 上游 `CreationError(4096, 4096, Blank)` 走的正是这一条。
    //    窗口内 `w,h ∈ 16..=4096`（远小于 32767、且非 0）⇒ `CanRoundTrip` 的三条件里
    //    只有「格数 ≤ 16M」这一条还可能不成立，故此处只判格数与上游等价。
    let cells = u64::from(width) * u64::from(height);
    if cells > CREATION_MAX_CELLS {
        return Some(format!(
            "新建地图最多 {CREATION_MAX_CELLS} 格，收到 {width}×{height} = {cells} 格\
             （上游 MapDimensions.cs:33 的 `!CanRoundTrip`，即 :11-12 的第三条；\
             注意单边 4096 合格但 4096×4096 已超格数）"
        ));
    }
    None
}

/// 空白文档**能不能按这个磁盘格式造出来**？`Ok(())` = 能；`Err(中文原因)` = 不能。
///
/// 判据三条合取，逐条依据见 [`Document::blank_in`] 文档第一节。逐格式落点：
///
/// | 格式 | `editable` | `TAIL_LEN` | 长度优先可认 | 结论 |
/// |---|---|---|---|---|
/// | `Old12` | true（`model.rs:471`） | 0（`formats/old12.rs:70`） | ✓（`registry.rs:150-157`） | **支持** |
/// | `Shanda14` | true（`model.rs:472`） | 0（`formats/shanda14.rs:79`） | ✓（`registry.rs:159-161`） | **支持** |
/// | `Shanda2012` | false（`model.rs:473`） | 22（`formats/shanda2012.rs:129`） | ✓（`registry.rs:163-165`） | 拒绝：尾区未建模 |
/// | `AntiHack` | false（`model.rs:474`） | 0 | ✗ 头部签名 + 异或键 | 拒绝：要编造签名与键 |
/// | `Wemade2010` | false（`model.rs:475`） | 1 | ✗ 异或键 | 拒绝：要编造异或键 |
/// | `Heroes34` | false（`model.rs:476`） | 1 | ✗ | 拒绝：尾区未建模 |
/// | `Mir3Wemade` | false（`model.rs:477`） | 4 | ✗ 头部三元组 + 背景块 | 拒绝：头部随尺寸变长 |
/// | `Mir3Shanda` | false（`model.rs:478`） | 7 | ✗ 标题魔数 | 拒绝：要编造标题 |
/// | `CrystalCs` | false（`model.rs:479`） | 变长 | ✗ `C#` 魔数 + 尾块 | 拒绝：变长格 + 0 真实样本 |
fn blank_gate(format: FormatId) -> Result<(), &'static str> {
    match format {
        FormatId::Old12 | FormatId::Shanda14 => Ok(()),
        FormatId::Shanda2012 => Err(
            "36 B/格，格内 22 字节尾区未建模（TAIL_LEN=22，语义未知）且 caps().editable == false\
             （mir2-format/src/model.rs:473）—— 新建时那 22 字节填什么，上游没有任何新建路径给出过依据",
        ),
        FormatId::AntiHack => Err(
            "宽高在头部 31/33/35 且与异或键异或存放（formats/antihack.rs:122-131 的 `w ^ xor`），\
             头部签名还要求 0x15/0x32/0x41/0x31（:114-120）—— 新建必须先编造一个异或键与一串签名字节",
        ),
        FormatId::Wemade2010 => Err(
            "宽高在头部 21/23/25 且与异或键异或存放（formats/wemade2010.rs:192-216），\
             格内还有 1 字节未建模尾区 —— 新建要先编造异或键，再猜那 1 个字节",
        ),
        FormatId::Heroes34 => Err(
            "格内 1 字节尾区未建模（TAIL_LEN=1，formats/heroes34.rs:62），\
             且背景图号是 u32、视图只装得下低 16 位 —— 新建填什么没有上游依据",
        ),
        FormatId::Mir3Wemade => Err(
            "头部长度随尺寸变（28 字节 + 半分辨率背景块，formats/mir3_wemade.rs:167-169），\
             宽高来自头部 20/22/24 处的三元组、还要求首字节为 0（:204-206）—— 空白图没有可固定的头",
        ),
        FormatId::Mir3Shanda => Err(
            "头部 40 字节且靠标题 `(C) SNDA, MIR3` 认格式（formats/mir3_shanda.rs:106-108），\
             格内还有 7 字节未建模尾区 —— 新建要先编造标题字符串",
        ),
        FormatId::CrystalCs => Err(
            "变长格（26/38/35/33+，由 `(ver, fr_count)` 决定）+ 尾部元数据块 + `C#` 魔数，\
             且本仓对它的 caps().editable == false、真实样本 0 张\
             （mir2-format/src/model.rs:434-448、:479）—— 造得出、验不了，不许静默产出坏图",
        ),
    }
}

/// 把 [`mir2_map::Map::blank`] 的尺寸错误折成 [`FormatError`]（**不新造措辞**）。
///
/// 三个尺寸变体与 `mir2-format` 的尺寸变体一一对应（两边都照 C#
/// `_upstream/CrystalStudio/MapDimensions.cs:11-12` 的 `CanRoundTrip` 三条件）：
/// `ZeroDimension → InvalidDimensions`、`DimensionTooLarge → DimensionTooLarge`、
/// `TooManyCells → TooManyCells`。这样 `miredit-web/src/error.rs:134-145` 的
/// `from_core` **不用改一个字**就能把「尺寸非法」报成 422。
///
/// ⚠ 走 [`Document::blank_in`] 时这两个变体其实**不可达**（窗口 `16..=4096` 已先拦掉），
/// 保留是为了「`mir2-map` 换了契约」时如实报，而不是把错误吞成一句含糊话。
fn blank_size_error(path: &Path, format: FormatId, err: MapError) -> CoreError {
    let err = match err {
        MapError::ZeroDimension { width, height } => FormatError::InvalidDimensions {
            format,
            width: i32::from(width),
            height: i32::from(height),
        },
        MapError::DimensionTooLarge {
            width,
            height,
            limit,
        } => FormatError::DimensionTooLarge {
            format,
            width: i64::from(width),
            height: i64::from(height),
            limit,
        },
        MapError::TooManyCells {
            width,
            height,
            cells,
            limit,
        } => FormatError::TooManyCells {
            format,
            width,
            height,
            cells,
            limit,
        },
        // `Map::blank` 不读字节 ⇒ 这两个变体在构造路径上不可达。真到了说明 mir2-map 改了契约。
        other @ (MapError::TooShort { .. } | MapError::LengthMismatch { .. }) => {
            FormatError::Inconsistent {
                detail: format!("空白图构造失败（mir2-map 的契约变了）：{other}"),
            }
        }
    };
    CoreError::Map {
        path: path.to_path_buf(),
        err,
    }
}

// ───────────────────────── 格访问 ─────────────────────────

/// `(x, y)` → `cells` 下标（列优先：`x * height + y`）；越界 `None`。来源：`edit.rs:1329-1334`。
pub fn index_of(map: &Map, x: u16, y: u16) -> Option<usize> {
    if x >= map.width || y >= map.height {
        return None;
    }
    Some(x as usize * map.height as usize + y as usize)
}

/// 只读取格（与 `Map::cell` 同语义，单独包一层便于本 crate 自证口径一致）。
/// 来源：`edit.rs:1337-1339`。
pub fn cell_at(map: &Map, x: u16, y: u16) -> Option<&Cell> {
    map.cell(x, y)
}

/// 可变取格 —— **本 crate 唯一的"可变访问器"**。来源：`edit.rs:1343-1346`。
///
/// 走 `Map::cells` 这个公开字段，**不需要**给 `mir2-map` 加 `cell_mut`
/// （那个 crate 正在别路验收，本单一个字都不许改）。
///
/// ⚠ **它绕过五层计数**：改完必须自己对 [`Document::rebuild_layer_counts`] 负责
/// （编辑路径请改用 [`Document::write_view_cell`]，那才是「写一格 + 计数增量」的入口）。
pub fn cell_mut(map: &mut Map, x: u16, y: u16) -> Option<&mut Cell> {
    let i = index_of(map, x, y)?;
    map.cells.get_mut(i)
}

// ───────────────────────── 逐字节差异（验收取证用） ─────────────────────────

/// 当前地图序列化后与"磁盘原文件"的差异统计。来源：`edit.rs:1669-1687`。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ByteDiff {
    /// 不同的字节数。
    pub bytes: usize,
    /// 涉及的格数（按格长折算）。
    pub cells: usize,
    /// 首个差异的文件偏移。
    pub first: Option<usize>,
    /// 头部（格数组之前的字节）里不同的字节数。
    pub header: usize,
}

impl ByteDiff {
    /// 是否逐字节相同。
    pub fn same(&self) -> bool {
        self.bytes == 0
    }
}

/// 逐字节比较 `map.to_bytes()` 与 `orig`；差异按格归并。来源：`edit.rs:1690-1720`。
///
/// ⚠ 只对 `mir2_map` 能完整表示的三档（`Old12` / `New14` / `Ext36`）成立 ——
/// 它的头部固定按 52 字节、格长按 `map.format.cell_size()` 折算。九种格式的通用版本是
/// [`byte_diff_bytes`]（[`Document::diff`] 走的是后者）。
///
/// 长度不等时按较长的一侧扫（多出的字节一律计差异），所以"截断/多写"也会被抓到。
pub fn byte_diff(map: &Map, orig: &[u8]) -> ByteDiff {
    byte_diff_bytes(
        &map.to_bytes(),
        orig,
        mir2_map::HEADER_LEN,
        map.format.cell_size().max(1),
    )
}

/// 通用版逐字节差异：给定**当前字节**与**原始字节**，按 `header_len` / `cell_size` 归并。
///
/// `cell_size` 传 0 时按 1 处理（变长格式没有固定格长，此时只统计字节数与首个偏移，
/// 格数一栏不可信 —— 调用方须自己知道这一点）。
pub fn byte_diff_bytes(now: &[u8], orig: &[u8], header_len: usize, cell_size: usize) -> ByteDiff {
    let size = cell_size.max(1);
    let total_cells = now.len().saturating_sub(header_len) / size + 1;
    let mut seen = vec![false; total_cells];
    let mut d = ByteDiff::default();
    let n = now.len().max(orig.len());
    for i in 0..n {
        let a = now.get(i).copied();
        let b = orig.get(i).copied();
        if a == b {
            continue;
        }
        d.bytes += 1;
        if d.first.is_none() {
            d.first = Some(i);
        }
        match i.checked_sub(header_len) {
            // 落在头部里的差异（正常编辑不会发生）
            None => d.header += 1,
            Some(rel) => {
                let cell = rel / size;
                if cell < total_cells && !seen[cell] {
                    seen[cell] = true;
                    d.cells += 1;
                }
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::造图;
    use mir2_map::CellFormat;

    /// 列优先下标必须与 `Map::cell` 同口径（否则改格会改到别的格上）。来源：`edit.rs:2123-2134`。
    #[test]
    fn 下标与取格同源() {
        let map = 造图(4, 3, CellFormat::New14);
        for y in 0..3u16 {
            for x in 0..4u16 {
                let idx = index_of(&map, x, y).expect("图内");
                assert_eq!(idx, x as usize * 3 + y as usize);
                assert_eq!(&map.cells[idx], map.cell(x, y).unwrap());
                // cell_mut 必须指到同一格
                let mut m = map.clone();
                assert_eq!(cell_mut(&mut m, x, y).copied(), Some(map.cells[idx]));
            }
        }
        assert_eq!(index_of(&map, 4, 0), None);
        assert_eq!(index_of(&map, 0, 3), None);
        let mut m = map.clone();
        assert!(cell_mut(&mut m, 4, 0).is_none(), "越界不许给可变引用");
    }

    /// 文档构造后必须**不脏**，`to_bytes()` 与输入逐字节相等（往返保真）。
    #[test]
    fn 构造后不脏且往返一致() {
        let map = 造图(3, 3, CellFormat::Ext36);
        let bytes = map.to_bytes();
        let doc = Document::from_bytes(Path::new(r"I:\x\3x3.map"), bytes.clone()).unwrap();
        assert!(!doc.dirty());
        assert_eq!(doc.to_bytes(), bytes);
        assert_eq!(doc.orig_len(), bytes.len());
        assert!(doc.diff().same(), "刚打开时 0 差异");
        assert_eq!(doc.file_name, "3x3.map");
        assert_eq!(doc.size(), (3, 3));
        assert_eq!(doc.format_name(), "Ext36");
        // 双表示两侧都能取到，且真值侧是 Shanda2012（36 B/格）
        assert_eq!(doc.format_id(), FormatId::Shanda2012);
        assert_eq!(doc.disk_cell_len(), Some(36));
        assert_eq!(doc.tolerant_trailer(), 0);
        assert_eq!(doc.raw_header().len(), 52, "Shanda2012 的真头部是 52 字节");
        assert_eq!(doc.trailer().len(), 0);
        // 视图档位 = 语义投影
        assert_eq!(doc.view().format, CellFormat::Ext36);
        assert_eq!(doc.view().cells.len(), 9);
    }

    /// 脏标记：`set_dirty` 写的就是那一个字段，`mark_clean` 清掉。来源：模块文档第二节。
    #[test]
    fn 脏标记只有一份() {
        let map = 造图(2, 2, CellFormat::Old12);
        let mut doc = Document::from_bytes(Path::new(r"I:\x\a.map"), map.to_bytes()).unwrap();
        assert!(!doc.dirty());
        doc.mark_dirty();
        assert!(doc.dirty());
        doc.mark_clean();
        assert!(!doc.dirty());
        doc.set_dirty(true);
        assert!(doc.dirty());
    }

    /// 逐字节差异统计：只改 2 格 ⇒ 差异落在 2 个格区间内，头部 0 字节差异。
    /// 来源：`edit.rs:2511-2526`。
    #[test]
    fn 差异按格统计() {
        let mut map = 造图(4, 4, CellFormat::Ext36);
        let orig = map.to_bytes();
        assert!(byte_diff(&map, &orig).same());

        let a = index_of(&map, 1, 2).unwrap();
        let b = index_of(&map, 3, 0).unwrap();
        map.cells[a].bk_img = 0x1234;
        map.cells[b].bk_img = 0x5678;
        let d = byte_diff(&map, &orig);
        assert_eq!(d.cells, 2, "应只涉及 2 个格");
        assert_eq!(d.header, 0, "52 字节头不许变");
        assert_eq!(d.bytes, 4, "每格只改 bk_img 的 2 字节");
        assert!(d.first.unwrap() >= mir2_map::HEADER_LEN);
    }

    /// 长度不等（多写 / 截断）也必须被抓到，不许静默判"相同"。
    #[test]
    fn 长度不等算差异() {
        let map = 造图(2, 2, CellFormat::Old12);
        let mut orig = map.to_bytes();
        orig.push(0);
        let d = byte_diff(&map, &orig);
        assert!(!d.same());
        assert_eq!(d.bytes, 1);
        assert_eq!(d.header, 0);
        assert_eq!(d.first, Some(map.to_bytes().len()), "差异就在多出来的那个字节");
    }

    /// 解析失败必须是 `Err(Map)`，带路径与具体原因（禁静默降级）。来源：`mapdoc.rs:176-190`。
    #[test]
    fn 解析失败带路径与原因() {
        let e = Document::from_bytes(Path::new(r"I:\x\bad.map"), vec![0u8; 51]).unwrap_err();
        match e {
            CoreError::Map { path, err } => {
                assert_eq!(path, Path::new(r"I:\x\bad.map"));
                assert!(err.to_string().contains("51"), "{err}");
            }
            other => panic!("应报 Map，实际 {other:?}"),
        }
    }

    /// 读不到文件 ⇒ `Err(Io)`，原因带系统原文。
    #[test]
    fn 读不到文件如实报错() {
        let p = Path::new(r"I:\__no_such_dir_miredit_core__\x.map");
        match Document::load(p) {
            Err(CoreError::Io { path, detail }) => {
                assert_eq!(path, p);
                assert!(!detail.is_empty(), "系统报错原文不许为空");
            }
            other => panic!("应报 Io，实际 {:?}", other.map(|d| d.file_name)),
        }
    }

    /// **出字节失败必须如实报，且绝不回退到原字节**（假保存比报错更糟）。
    ///
    /// 手法：把视图的宽高改坏（模拟"有人绕过 `Document` 动了 `view`"），
    /// 让 `sync_view_into_raw` 的形状校验失败。
    #[test]
    fn 出字节失败报格式层错误不回退() {
        let map = 造图(2, 2, CellFormat::Old12);
        let mut doc = Document::from_bytes(Path::new(r"I:\x\bad_shape.map"), map.to_bytes()).unwrap();
        doc.view_mut().width = 3;
        let e = doc.try_to_bytes().expect_err("形状不符必须报错");
        match e {
            CoreError::Map { path, err } => {
                assert_eq!(path, Path::new(r"I:\x\bad_shape.map"));
                assert!(
                    matches!(err, mir2_format::FormatError::Inconsistent { .. }),
                    "形状不符要折成 Inconsistent，实际 {err:?}"
                );
                let msg = err.to_string();
                assert!(msg.contains("形状不一致"), "{msg}");
                assert!(msg.contains('3') && msg.contains('2'), "信息要带两边尺寸：{msg}");
            }
            other => panic!("应报 Map（写方向），实际 {other:?}"),
        }
    }
}

/// 新建空白图的用例（`Map::blank` / `Document::blank` / `Document::blank_in`）。
///
/// 单独一个文件：`document.rs` 已过千行，空白图这套用例自带一个落盘往返 + 9 种格式的
/// 拒绝矩阵，塞回原模块会把「双表示 / 脏标记 / 字节差异」的用例挤散。
/// 挂法与 `map_check_tests.rs` / `resize_tests.rs` / `session_tests.rs` 一致（`#[path]` 挂回本模块，
/// 因此能用 `super::` 访问私有的 `creation_error` / `blank_gate`）。
#[cfg(test)]
#[path = "blank_tests.rs"]
mod blank_tests;
