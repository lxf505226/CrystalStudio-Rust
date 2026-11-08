//! **只读**地图检查（对齐上游 `CrystalStudio/MapCheck.cs:11-101@f84566b`；R32/R33 口径）。
//!
//! 只读：拿一张 [`RawMap`]（字节层真值）走一遍，产出「问题清单 + 总条数 + 未检查的层」，
//! 不改地图、不进撤销栈、不动脏标记。宿主先取一份 raw 快照再调（见 `MapDoc::raw`）。
//!
//! # 一、为什么入参是 `&RawMap` 而不是 `&Map`（R32 裁决）
//!
//! 上游的 `MapCheck.Run` 读三样东西：`cell.BackLibrary` / `cell.MiddleLibrary` /
//! `cell.Foreground.Library`（**库号**）+ `cell.BackImage` / `MiddleImage` /
//! `Foreground.Image`（**图号**）+ `BackImage & 0x60000000`（可走）。
//!
//! 本仓的**编辑视图** `mir2_map::Map` 只装得下三族的**页号**（`tiles`/`sm_tiles`/`area`）
//! 与三个图号，装不下：`bk_idx`/`mid_idx`/`fr_idx`（`convert.rs:255-265` 的 `LOST_IN_VIEW`）、
//! `flag`（Mir3 的可走位）、以及 `bk_img` 的高位（视图只承载低 16 位）。
//! ⇒ 用视图检查会**判错 Mir3 的可走性**、也拿不到 CrystalCs 的前景库号。
//! 字节层的 [`RawMap`] 九档全都拿得到，且 `Document::raw()` 本来就是公有只读 ⇒ 用它。
//!
//! # 二、库号：`MapLibRef.index` 是**族内页号**，不是 C# 全局号（R33 修正）
//!
//! 上游 `libraries` 字典的键是**扁平全局号**（`Assets.cs:360-372`）：
//! `0..=99` WemadeMir2、`100..=150` ShandaMir2、`190` AniTiles1、`200+` WemadeMir3、
//! `300+` ShandaMir3（Sanxiu 另有 `0/1000/2000` 三段）。而本仓图源
//! [`mir2_assets::LibRef`] 的键是**族 + 族内页号**（`lib_paths.rs:103-113`：
//! `index = 0` 是基名本身、`index > 0` 对应 `{Stem}{index+1}`）。
//!
//! 两者的换算（`Assets.cs:361-365` 逐条对得上 `lib_paths.rs` 的登记表）：
//!
//! | 族 | WemadeMir2 段 | ShandaMir2 段 | 族内页号 |
//! |---|---|---|---|
//! | 地面 | `Add(0, Tiles)`、`Add(100+i, Tiles{i+1})` i∈1..9 | 同 | **= `btTiles` 原值** |
//! | 中层 | `Add(1, Smtiles)`、`Add(110+i, SmTiles{i+1})` | 同 | **= `btsmTiles` 原值** |
//! | 物件 | `Add(2, Objects)`、`Add(i+1, Objects{i})` i∈2..98 | `Add(120+i, Objects{i+1})` i∈1..30 | **= `btArea` 原值** |
//!
//! ⇒ 「全局号 − 基址（`0/1/2` 或 `100/110/120`）」恰好等于地图字段原值。
//! 所以本模块**直接用字段值当页号**（`ground(tiles)` / `middle(sm_tiles)` / `surface(area)`），
//! **绝不**再 `+2 / +120 / +200 / +300` —— 加了就会去查错的图库（`Object2.lib` 一类）。
//! 与渲染侧同源：`miredit-render/src/compose.rs:308/335/365` 也是
//! `LibRef::ground(cell.tiles)` / `middle(cell.sm_tiles)` / `surface(cell.area)`。
//!
//! ## 三档不可查（显式 `未检查`，不猜 0）
//!
//! | 格式 | 为什么 `LibRef` 表达不了 |
//! |---|---|
//! | `Mir3Wemade` | 地面/中层/物件都是**具名槽**（`WemadeMir3\{state}\{Tilesc,Tiles30c,Smtilesc,Housesc,…,Object1c}`，`Assets.cs:366-370`），`LibPaths::classify_stem` 只认 `Tiles/SmTiles/Objects` + 数字前缀 ⇒ 这些基名**规则外**（`crystal_paths.rs:63` 的 `NotInThreeFamilies`）。另外 Type5 的**背图在块级**（3 B / 2×2 组，`decode_back_tiles`），格内没有地面库号。 |
//! | `Mir3Shanda` | 同上（`ShandaMir3\{names}{states}`，`Assets.cs:371`）。 |
//! | `CrystalCs` | 文件里存的是 **C# 全局号原值**（`LegacyMapReader.cs:580/584/594/615` 的 `BitConverter.ToInt16` 直接赋值），而全局号空间有**五段**（Wemade/Shanda/Mir3/AniTiles/Sanxiu `1000+·2000+`）⇒ 同一个数字在不同库里指不同文件，**无法证明**它等于哪个族的页号；且本机 1158 张图里 **0 张**带 `C#` 魔数（`mir2-format` 的 `FormatCaps::has_real_samples = false`）。⇒ 三族全部 `未检查`，**不探测**。 |
//!
//! # 三、`lib < 0`：`255` 哨兵算「必然缺库」，不算「未检查」
//!
//! C# 在 Mir3 两支把磁盘 `255` 读成 `-1`（`LegacyMapReader.cs:406/424/426/472/474/476`），
//! 而 `libraries` 字典的键恒 `>= 0` ⇒ 该格**必然** `Missing library`（`MapCheck.cs:29-30`）。
//! 这是**可证事实**，所以即使该格式整体「未检查」，这类格仍按缺库报
//! （[`LibSource::Missing`]）；正号槽位依旧是 `未检查`。
//!
//! Type5 的地面例外：它的背图库号**不在格内**（块级），`Cell::bk_idx` 恒 0 ⇒ 地面一律
//! `未检查`，不从格内字段臆测哨兵。
//!
//! ## 三·二、没有图库时：图号引用**整条不查**（上游 `libraries == null`）
//!
//! `MapCheck.Image` 的第一句是 `if (image == 0 || libraries == null) return;`（`:26`）
//! ⇒ 没有库表时上游**一条图号问题都不报**，连上面那种「必然缺库」的哨兵格也不报
//! （它在同一个 `return` 之后）。而**门两警告与不连通区照常**：那两段在 `:54-95`，
//! 根本不读 `libraries`。
//!
//! 本模块的行为由 [`LibProbe::has_libraries`] 决定：`false` ⇒ 图号引用整体跳过
//! （[`check_map`] 的 `查一层` 第一句就返回），门/BFS 一条不少。
//! 宿主用 [`NoLibProbe`] 表达「没有库表」，并在界面上追加官方那句
//! 「Load libraries to check image references.」。
//!
//! # 四、图号掩码 / 开门帧 / 可走（逐条对上 C#）
//!
//! | 项 | 上游 | 本模块 |
//! |---|---|---|
//! | 地面图号 | `cell.BackImage & 0x1FFFFFFF` | [`ground_image`]：先过该格式的 `csharp_back_image`（Wemade2010 是 `^0xAA38AA38`），再 `& 0x1FFF_FFFF` |
//! | 中层图号 | `cell.MiddleImage`（不掩） | [`middle_image`]：Wemade2010 要先 `^xor`（头 23..25） |
//! | 物件图号 | `front.Image & 0x7FFF` | [`objects_image`]：Wemade2010 先 `^xor`；Mir3 先 `+1`（`csharp_image`） |
//! | 开门帧 | `image != 0 && (Door & 0x7F) != 0 ⇒ image + DoorOffset` | [`open_door_image`] |
//! | 可走（Type0/2/3/4/7） | `BackImage & 0x60000000 == 0`（装载期把 bk 的 bit15 折成 `0x20000000`、fr 的折成 `0x40000000`） | `bk_img & 0x8000 == 0 && fr_img & 0x8000 == 0`（= 视图 [`mir2_map::Cell::is_walkable`]，与上游聚合两位等价） |
//! | 可走（`CrystalCs`） | 29/30 位是**持久化的碰撞位**（`CellCodec.Write` 整字写盘、`MapViewport.cs:505/546` 涂图号时保留 `& 0x60000000`）；装载期另把**前景** `bit15` 折成 `0x40000000`（仅 `ver > 1`） | `csharp_back_mark(bk_img, fr_img, ver) & 0x6000_0000 == 0`（`bit15` 只算图号，**不判阻挡**） |
//! | 可走（Mir3） | `flag&1==1` 才背景可走、`flag&2==2` 才前景可走（`LegacyMapReader.cs:436-437/494-495`） | `flag & 1 == 1 && flag & 2 == 2`（视图没有 `flag`，只能用 raw） |
//!
//! ## 有意差异 / 已知不精确（不静默）
//!
//! 1. **`Wemade2010`（Type1）的 bk `bit15`**：上游只把 Type0/2/3/4/7 的背景 `bit15`
//!    折进 `0x20000000`；Type1（`LegacyMapReader.cs:190-211`）**漏搬**（`wemade2010.rs`
//!    的 `FormatCaps.collision_note` 也把这条记成「9 种里唯一」）。本模块仍按磁盘 `bit15`
//!    判 ⇒ 该格式上比上游**更严格**（可能多报「不连通」）。
//!    为什么这样选：本仓画布的可走性就是 [`mir2_map::Cell::is_walkable`]，检查结论必须与
//!    用户看到的阻挡标记一致；该格式**零真实样本**，不拿未验证的上游特例去反向覆盖
//!    已在生产路径上跑了 1158 张图的判据。
//!
//!    ⚠ **`CrystalCs` 不在这个差异里**（R34 复核订正）：它的阻挡位是**持久化的**
//!    `bit29/bit30`（见第四节表格那一行），`bit15` 只是图号高位 ⇒ 按 `& 0x60000000`
//!    判才是上游口径；拿 `bit15` 判会把「图号 ≥ 32768」的格**误挡**。
//! 2. **`N0` 千分位**：上游 `"Disconnected walkable area: {0:N0} cells"` 会输出 `1,234`，
//!    本模块输出 `1234`（[`MapIssue`] 不带本地化格式化）。文案本身逐字同形。
//! 3. **`HasDoor` 的 `& 0x7F`**：上游 `cell.Door` 在 CrystalCs 里**已被 reader 掩过**
//!    （`LegacyMapReader.cs:627`），其余八档是原字节 ⇒ [`has_door`] 只对 `CrystalCs`
//!    再掩一次，其余原样（照抄 `MapPreview.cs:13` 的入参口径）。
//!
//! # 五、与上游逐条对照
//!
//! | 检查项 | 上游 | 本模块 |
//! |---|---|---|
//! | 图号引用（按 `(库, 图号)` 缓存） | `MapCheck.cs:24-41` | [`LibProbe`] 注入 + [`check_map`] 内 `HashMap` 缓存 |
//! | 问题文案里的 `Library {n}` | 上游 `:38` 打的是 **C# 扁平全局号** | 同样打全局号（[`csharp_back_lib`]/[`csharp_middle_lib`]/[`csharp_front_lib`]）；族内页号另存于 [`MapLibRef::index`] |
//! | 门格没有前景图 / 四周没有可走格 | `:54-59` | [`MSG_DOOR_NO_FOREGROUND`] / [`MSG_DOOR_NO_WALKABLE`] |
//! | 8 邻 BFS 断开区（**最大块不报**） | `:61-95` | [`MSG_DISCONNECTED`] |
//! | 只留前 1000 条、`total` 全量 | `:18-22` | [`MAP_CHECK_ISSUE_CAP`] |
//! | 逐格顺序（地面→中层→物件→开门帧→门两警告） | `:42-59` | [`check_map`] 单循环同序 |
//! | BFS 起点顺序（C# 行优先 `key = y*Width + x`） | `:61-64` | [`check_walkable_blocks`]（按 C# 键序，不是本仓列优先存储序） |
//!
//! # 六、不做
//!
//! 像素级校验、C# 数值对拍（无同语料基线）、后台线程（同步跑；真图 <1s，见
//! `miredit-gui` 的验收测试）。Crop/Expand 不属本模块。

use std::collections::HashMap;

use mir2_format::formats::{
    crystal_cs, heroes34, mir3_shanda, mir3_wemade, old12, shanda14, shanda2012, wemade2010,
};
use mir2_format::{Cell as RawCell, FormatId, RawMap};

/// 问题清单上限（上游 `MapCheck.cs:21`）。
pub const MAP_CHECK_ISSUE_CAP: usize = 1000;

/// 图族（= 上游 `Ground` / `Middle` / `Objects`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LibFamily {
    /// 地面层。
    Ground,
    /// 中层。
    Middle,
    /// 物件层。
    Objects,
}

impl LibFamily {
    /// 三族的固定顺序（报告里 `未检查` 就按这个顺序排）。
    pub const ALL: [LibFamily; 3] = [LibFamily::Ground, LibFamily::Middle, LibFamily::Objects];

    /// 中文层名（GUI 的「未检查」提示用）。
    pub const fn label(self) -> &'static str {
        match self {
            LibFamily::Ground => "地面层",
            LibFamily::Middle => "中层",
            LibFamily::Objects => "物件层",
        }
    }
}

/// 一个图库定位（= `mir2_assets::LibRef` 的同口径定义；本 crate **不依赖** `mir2-assets`）。
///
/// ⚠ 名字带 `Map` 前缀：本 crate 里已有一个 `LibRef`（图层/印章侧，见 `brush`），
/// 两者语义不同、不合并。
///
/// `index` 是**族内页号**（0 基）：`0` = 基名本身，`N > 0` = `{Stem}{N+1}`。
/// 与地图字段原值一致（见模块文档第二节）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapLibRef {
    /// 图族。
    pub family: LibFamily,
    /// 族内 0 基页号（= 地图格字段原值，**不是** C# 全局号）。
    pub index: u16,
}

impl MapLibRef {
    /// 地面层定位（`btTiles`）。
    pub const fn ground(index: u16) -> Self {
        MapLibRef {
            family: LibFamily::Ground,
            index,
        }
    }

    /// 中层定位（`btsmTiles`）。
    pub const fn middle(index: u16) -> Self {
        MapLibRef {
            family: LibFamily::Middle,
            index,
        }
    }

    /// 物件层定位（`btArea`）。
    pub const fn surface(index: u16) -> Self {
        MapLibRef {
            family: LibFamily::Objects,
            index,
        }
    }
}

/// 一帧的引用状态（上游四类 + 缺库）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageState {
    /// 正常。
    Ok,
    /// 空帧（索引存在但帧头无效）。
    Empty,
    /// 读不出 / 损坏。
    Unreadable,
    /// 图号越界（超出该库帧数）。
    OutOfRange,
    /// 库不存在。
    MissingLibrary,
}

/// 图库探针：**由调用方注入**（GUI 用 `mir2_assets` 实现，core 不新增生产依赖）。
pub trait LibProbe {
    /// `图号` 为 **1 基**（与地图格字段同口径；上游 `atlas.ReadHeader(image - 1)`）。
    ///
    /// 图号 `0` 由 [`check_map`] 先跳过，调用方不必处理。
    fn frame_state(&self, lib: MapLibRef, image_1based: u32) -> ImageState;

    /// 本次检查**有没有图库表**（上游 `libraries != null`）。
    ///
    /// `false` ⇒ 图号引用这一项**整条不查**（`MapCheck.Image` 的第一句就是
    /// `if (image == 0 || libraries == null) return;`，连「必然缺库」的哨兵格也不报）；
    /// **门两警告与不连通区照常**（上游那两段 `MapCheck.cs:54-95` 根本不看库表）。
    ///
    /// 缺省 `true`（既有实现者一行都不用改）；[`NoLibProbe`] 覆盖成 `false`。
    fn has_libraries(&self) -> bool {
        true
    }
}

/// **「没有图库」探针**：图号引用一项**整体不查**（等价于上游 `libraries == null`）。
///
/// `MapCheck.Image` 的第一句就是 `if (image == 0 || libraries == null) return;`
/// （`MapCheck.cs:26`）—— 没有图库表时上游**一条图号问题都不报**，只在对话框里追加
/// 「Load libraries to check image references.」。宿主（GUI / `--check-map`）在
/// `assets.source()` 为 `None` 时传本类型，报告里就只剩「门两警告 + 不连通区」。
///
/// ⚠ 为什么不让它返回 `MissingLibrary`：那会给每个非空图号刷一条假问题
/// （1000 条上限会把真问题挤掉），与上游语义也不符。
pub struct NoLibProbe;

impl LibProbe for NoLibProbe {
    fn frame_state(&self, _lib: MapLibRef, _image_1based: u32) -> ImageState {
        ImageState::Ok
    }

    /// **没有图库表** ⇒ 图号引用整条不查（`check_map` 在这一层就早退，连哨兵也不报）。
    fn has_libraries(&self) -> bool {
        false
    }
}

/// 某一层在**这一格**上的库结论。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibSource {
    /// 可查：族 + 族内页号。
    Page(MapLibRef),
    /// C# 算出 `-1`（磁盘 `255` 哨兵）⇒ 该层**必然缺库**（键恒 `>= 0` 的字典里没有它）。
    Missing,
    /// 该格式这一层 `LibRef` 表达不了（带原因；GUI 要如实显示，不探测）。
    Unchecked(&'static str),
}

/// 「Mir3 具名槽」未检查的原因（`LibRef` 三族规则外）。
pub const UNCHECKED_MIR3_NAMED_SLOTS: &str =
    "Mir3 的库是具名槽（Tilesc/Housesc/Smtilesc…），Tiles/SmTiles/Objects 三族页号表达不了";

/// Type5 地面未检查的原因（背图在块级，格内没有地面库号）。
pub const UNCHECKED_MIR3_BLOCK_GROUND: &str =
    "Mir3Wemade 的背图在块级（3 B / 2×2 组），格内没有地面库号；块级槽位同样是具名槽";

/// 「CrystalCs 全局号空间未知」未检查的原因。
pub const UNCHECKED_CRYSTALCS: &str =
    "CrystalCs 存的是 C# 全局库号（含 Mir3/Sanxiu 1000+·2000+ 多段空间），与族内页号无法证明同一空间；且零真实样本";

/// 「地图形状非法」未检查的原因（`cells.len() != 宽×高`）。
pub const UNCHECKED_SIZE_MISMATCH: &str = "地图尺寸与格数不符（宽×高 ≠ 格数），无法逐格检查";

/// 文案（与上游 `MapCheck.cs` 逐字同形）。
pub const MSG_LIB_MISSING: &str = "Missing library";
/// 图号越界。
pub const MSG_IMAGE_RANGE: &str = "Invalid image number";
/// 空帧。
pub const MSG_IMAGE_EMPTY: &str = "Empty image";
/// 读不出。
pub const MSG_IMAGE_UNREADABLE: &str = "Unreadable image";
/// 门格没有前景图。
pub const MSG_DOOR_NO_FOREGROUND: &str = "Door has no foreground image";
/// 门格四周没有可走格。
pub const MSG_DOOR_NO_WALKABLE: &str = "Door has no walkable neighbouring cell";
/// 断开的可走区域（前缀，其后接 `: N cells`）。
pub const MSG_DISCONNECTED: &str = "Disconnected walkable area";

/// 一条问题（上游 `MapIssue`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapIssue {
    /// 世界格 X。
    pub x: u16,
    /// 世界格 Y。
    pub y: u16,
    /// 说明（英文，与上游逐字同形）。
    pub message: String,
}

impl MapIssue {
    /// 一行展示：`(x, y)  说明`（上游 `MapIssue.ToString`）。
    #[must_use]
    pub fn line(&self) -> String {
        format!("({}, {})  {}", self.x, self.y, self.message)
    }
}

/// 检查报告：`total` 全量，`issues` 最多 [`MAP_CHECK_ISSUE_CAP`] 条。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapCheckReport {
    /// 清单（截断后的）。
    pub issues: Vec<MapIssue>,
    /// 真实总数。
    pub total: usize,
    /// **没查的层**（`(族, 原因)`，按 [`LibFamily::ALL`] 顺序，0..=3 条）。
    ///
    /// 非空 ⇒ 报告只是「查了能查的层」，GUI 必须把原因显示出来（不许假装全绿）。
    pub 未检查: Vec<(LibFamily, &'static str)>,
    /// **没有图库表**（上游 `libraries == null`）⇒ 图号引用**整项没查**。
    ///
    /// 与 [`MapCheckReport::未检查`] 分开记：那是「这一族的库号不可推导」，
    /// 这是「压根没有库表可比」（三个族都可能"可推导"，但图号一律没核）。
    /// 门两警告与不连通区**不在其中**：它们不依赖库表，照常查、照常报。
    pub 无图库: bool,
}

impl MapCheckReport {
    /// 被截掉的条数。
    #[must_use]
    pub fn truncated(&self) -> usize {
        self.total.saturating_sub(self.issues.len())
    }

    /// 某族的未检查原因（`None` = 这一族**查了**）。
    #[must_use]
    pub fn unchecked_reason(&self, family: LibFamily) -> Option<&'static str> {
        self.未检查
            .iter()
            .find(|(f, _)| *f == family)
            .map(|(_, r)| *r)
    }
}

// ───────────────────── 公式层：C# 全局库号（对照/取证用） ─────────────────────

/// C# 的前景库号（`Foreground.Library`，**扁平全局号**；`-1` = 无库哨兵）。
///
/// 九档逐条对齐（表 A，R32 §8.26a）：
///
/// | 格式 | 公式 | 源码 |
/// |---|---|---|
/// | `Old12` / `AntiHack` / `Wemade2010` / `Heroes34` | `area + 2` | `LegacyMapReader.cs:147/360/203/540` |
/// | `Shanda14` / `Shanda2012` | `area + 120` | `:247/298` |
/// | `Mir3Wemade` | `255 ⇒ -1`，否则 `area + 200`，再哨兵修正（图号 == 1 且 lib == 200 ⇒ -1） | `:424-431` |
/// | `Mir3Shanda` | `255 ⇒ -1`，否则 `area + 300`（`:484-485` 那条修正对本支是**死代码**） | `:476-485` |
/// | `CrystalCs` | `fr_idx` 原样（i16，无基址） | `:594/615` |
///
/// ⚠ **不是** [`MapLibRef::index`]：本函数给的是上游字典键，检查层不拿它去查图库
/// （换算见模块文档第二节）。
#[must_use]
pub fn csharp_front_lib(format: FormatId, cell: &RawCell) -> i16 {
    use FormatId as F;
    match format {
        F::Old12 => i16::try_from(old12::csharp_front_lib(cell.area)).unwrap_or(i16::MAX),
        F::AntiHack => i16::try_from(antihack_front(cell.area)).unwrap_or(i16::MAX),
        F::Wemade2010 => i16::try_from(wemade2010::csharp_front_lib(cell.area)).unwrap_or(i16::MAX),
        F::Heroes34 => i16::try_from(heroes34::csharp_front_lib(cell.area)).unwrap_or(i16::MAX),
        F::Shanda14 => i16::try_from(shanda14::csharp_front_lib(cell.area)).unwrap_or(i16::MAX),
        F::Shanda2012 => i16::from(cell.area) + shanda2012::FRONT_LIB_OFFSET as i16,
        F::Mir3Wemade => mir3_wemade::csharp_front_lib(cell.area, cell.fr_img),
        F::Mir3Shanda => {
            let lib = mir3_shanda::csharp_lib(cell.area);
            // 上游 `:484-485` 的哨兵修正；那是死代码（lib ≥ 300 或 −1）⇒ 调用不改变结果。
            mir3_shanda::csharp_dead_front_fix(cell.fr_img, lib)
        }
        F::CrystalCs => cell.fr_idx,
    }
}

/// C# 的背景库号（`cell.BackLibrary`，扁平全局号；`-1` = 无库）。
///
/// | 格式 | 公式 | 源码 |
/// |---|---|---|
/// | `Old12` / `AntiHack` / `Wemade2010` / `Heroes34` | 常量 `0` | `:135/348/194/531` |
/// | `Shanda14` / `Shanda2012` | `tiles + 100` | `:249/300` |
/// | `Mir3Wemade` | `255 ⇒ -1` 否则 `bk_idx + 200` | `:406` |
/// | `Mir3Shanda` | `255 ⇒ -1` 否则 `bk_idx + 300` | `:472` |
/// | `CrystalCs` | `bk_idx` 原样 | `:580` |
///
/// ⚠ **`Mir3Wemade` 的特例**：Type5 的背图库号来自**块级** 3 B 组
/// （`LegacyMapReader.cs:399-408`，本仓 `mir3_wemade::decode_back_tiles`），
/// 由 `:406` 抄进 2×2 四格的 `BackIndex`。本仓 `read` 不给格内 `bk_idx` 赋值
/// （恒 0）⇒ 本函数对 Type5 只是「把 C# 抄进格里的那个公式」复现一遍，
/// **检查层不用它**（地面一律 `未检查`，见 [`unchecked_reason`]）。
#[must_use]
pub fn csharp_back_lib(format: FormatId, cell: &RawCell) -> i16 {
    use FormatId as F;
    match format {
        F::Old12 | F::AntiHack | F::Wemade2010 | F::Heroes34 => 0,
        F::Shanda14 => i16::from(u8::try_from(cell.tiles).unwrap_or(u8::MAX))
            + shanda14::BACK_LIB_OFFSET as i16,
        F::Shanda2012 => i16::from(u8::try_from(cell.tiles).unwrap_or(u8::MAX))
            + shanda2012::BACK_LIB_OFFSET as i16,
        F::Mir3Wemade => mir3_wemade::csharp_lib(u8::try_from(cell.bk_idx).unwrap_or(255)),
        F::Mir3Shanda => mir3_shanda::csharp_lib(u8::try_from(cell.bk_idx).unwrap_or(255)),
        F::CrystalCs => cell.bk_idx,
    }
}

/// C# 的中层库号（`cell.MiddleLibrary`，扁平全局号）。
///
/// | 格式 | 公式 | 源码 |
/// |---|---|---|
/// | `Old12` / `AntiHack` / `Wemade2010` / `Heroes34` | 常量 `1` | `:136/349/196/533` |
/// | `Shanda14` / `Shanda2012` | `sm_tiles + 110` | `:250/301` |
/// | `Mir3Wemade` | `255 ⇒ -1` 否则 `mid_idx + 200`（**没有**前景那条哨兵修正） | `:426` |
/// | `Mir3Shanda` | `255 ⇒ -1` 否则 `mid_idx + 300` | `:474` |
/// | `CrystalCs` | `mid_idx` 原样 | `:584` |
#[must_use]
pub fn csharp_middle_lib(format: FormatId, cell: &RawCell) -> i16 {
    use FormatId as F;
    match format {
        F::Old12 | F::AntiHack | F::Wemade2010 | F::Heroes34 => 1,
        F::Shanda14 => i16::from(u8::try_from(cell.sm_tiles).unwrap_or(u8::MAX))
            + shanda14::MIDDLE_LIB_OFFSET as i16,
        F::Shanda2012 => i16::from(u8::try_from(cell.sm_tiles).unwrap_or(u8::MAX))
            + shanda2012::MIDDLE_LIB_OFFSET as i16,
        F::Mir3Wemade => mir3_wemade::csharp_lib(u8::try_from(cell.mid_idx).unwrap_or(255)),
        F::Mir3Shanda => mir3_shanda::csharp_lib(u8::try_from(cell.mid_idx).unwrap_or(255)),
        F::CrystalCs => cell.mid_idx,
    }
}

/// `AntiHack` 的前景库号（`area + 2`）。单独包一层只为让上面那张表与源码行号一一对位。
fn antihack_front(area: u8) -> u16 {
    mir2_format::formats::antihack::csharp_front_lib(area)
}

// ───────────────────────── 每格：层 → 库结论 ─────────────────────────

/// 某格式某一层**是否整层未检查**（与 [`layer_source`] 同源；`None` = 这一层可查）。
///
/// Type5 的地面单独一条原因（块级背图），其余按格式一条（见模块文档第二/三节）。
#[must_use]
pub fn unchecked_reason(format: FormatId, family: LibFamily) -> Option<&'static str> {
    use FormatId as F;
    match format {
        F::Mir3Wemade if family == LibFamily::Ground => Some(UNCHECKED_MIR3_BLOCK_GROUND),
        F::Mir3Wemade | F::Mir3Shanda => Some(UNCHECKED_MIR3_NAMED_SLOTS),
        F::CrystalCs => Some(UNCHECKED_CRYSTALCS),
        // 六档全部可查（`Old12`/`AntiHack`/`Wemade2010`/`Heroes34`/`Shanda14`/`Shanda2012`）
        F::Old12 | F::AntiHack | F::Wemade2010 | F::Heroes34 | F::Shanda14 | F::Shanda2012 => None,
    }
}

/// 某格式某一层在**这一格**上的库结论。
///
/// 六档可查 ⇒ [`LibSource::Page`]（页号 = 地图字段原值）；三档不可查 ⇒
/// 磁盘 `255` 哨兵那格给 [`LibSource::Missing`]，其余 [`LibSource::Unchecked`]。
#[must_use]
pub fn layer_source(format: FormatId, family: LibFamily, cell: &RawCell) -> LibSource {
    use FormatId as F;
    match format {
        // ── 六档可查：族内页号 = 地图字段原值（C# 全局号减基址后同值，见模块文档第二节）──
        F::Old12 | F::AntiHack | F::Wemade2010 | F::Heroes34 => match family {
            LibFamily::Ground => LibSource::Page(MapLibRef::ground(0)),
            LibFamily::Middle => LibSource::Page(MapLibRef::middle(0)),
            LibFamily::Objects => LibSource::Page(MapLibRef::surface(u16::from(cell.area))),
        },
        F::Shanda14 | F::Shanda2012 => match family {
            LibFamily::Ground => LibSource::Page(MapLibRef::ground(cell.tiles)),
            LibFamily::Middle => LibSource::Page(MapLibRef::middle(cell.sm_tiles)),
            LibFamily::Objects => LibSource::Page(MapLibRef::surface(u16::from(cell.area))),
        },
        // ── Mir3：具名槽；`255` 那格是**可证的**缺库（C# 读成 −1）──
        F::Mir3Wemade => match family {
            // 背图库在块级、格内 `bk_idx` 恒 0 ⇒ 不从格内臆测哨兵
            LibFamily::Ground => LibSource::Unchecked(UNCHECKED_MIR3_BLOCK_GROUND),
            LibFamily::Middle => mir3_sentinel(mir3_wemade::csharp_lib(
                u8::try_from(cell.mid_idx).unwrap_or(255),
            )),
            LibFamily::Objects => mir3_sentinel(mir3_wemade::csharp_front_lib(cell.area, cell.fr_img)),
        },
        F::Mir3Shanda => mir3_sentinel(match family {
            LibFamily::Ground => mir3_shanda::csharp_lib(u8::try_from(cell.bk_idx).unwrap_or(255)),
            LibFamily::Middle => mir3_shanda::csharp_lib(u8::try_from(cell.mid_idx).unwrap_or(255)),
            LibFamily::Objects => mir3_shanda::csharp_lib(cell.area),
        }),
        // ── CrystalCs：全局号空间无法证明 ⇒ 三族全未检查（不猜 0、不探测）──
        F::CrystalCs => LibSource::Unchecked(UNCHECKED_CRYSTALCS),
    }
}

/// `-1` 哨兵 ⇒ 必然缺库；正号槽位 ⇒ 未检查。
fn mir3_sentinel(csharp_lib: i16) -> LibSource {
    if csharp_lib < 0 {
        LibSource::Missing
    } else {
        LibSource::Unchecked(UNCHECKED_MIR3_NAMED_SLOTS)
    }
}

// ───────────────────────── 每格：图号 / 门 / 可走 ─────────────────────────

/// 逐格式的**头部常数**（只有两处真要读头部：XOR 键与 CrystalCs 的版本号）。
#[derive(Clone, Copy, Debug)]
struct 头部 {
    /// `AntiHack`（偏移 33）/ `Wemade2010`（偏移 23）的整字 XOR 键；其余格式恒 `0`。
    xor: i16,
    /// `CrystalCs` 的版本号（头部 `0..2`，i16 LE）；其余格式恒 `0`。
    ///
    /// 为什么要它：装载期只有 `ver > 1` 才把前景 `bit15` 折成 `0x40000000`
    /// （`LegacyMapReader.cs:598` / `:625`），可走判据要用到这个分支。
    crystal_ver: i16,
}

/// 从头部读一个 `i16`（小端）；不足两字节返回 `None`。
fn read_i16(header: &[u8], off: usize) -> Option<i16> {
    let b = header.get(off..off + 2)?;
    Some(i16::from_le_bytes([b[0], b[1]]))
}

/// 该图的两处**头部常数**：XOR 键（`AntiHack` 偏移 33 / `Wemade2010` 偏移 23）与
/// `CrystalCs` 的版本号（偏移 0）。其余七档恒 `0/0`。
///
/// 偏移量直接取 `mir2-format` 的**公有常量**（`antihack::XOR_OFFSET` /
/// `wemade2010::XOR_OFFSET`），不在本 crate 抄第二份数字；CrystalCs 的版本号在
/// `crystal_cs.rs` 里没有独立常量（它的 `read` 直接从 0 读），这里按 C# `:563-565` 取 `0..2`。
fn 取头部(format: FormatId, header: &[u8]) -> Option<头部> {
    use FormatId as F;
    let xor = match format {
        F::AntiHack => read_i16(header, mir2_format::formats::antihack::XOR_OFFSET)?,
        F::Wemade2010 => read_i16(header, wemade2010::XOR_OFFSET)?,
        _ => 0,
    };
    let crystal_ver = if format == F::CrystalCs {
        read_i16(header, 0)?
    } else {
        0
    };
    Some(头部 { xor, crystal_ver })
}

/// 12/14/36 档的 `bk_img` 在磁盘上只有 2 字节；越界（不该发生）按 `u16::MAX` 处理。
fn 低16(v: u32) -> u16 {
    u16::try_from(v).unwrap_or(u16::MAX)
}

/// 该格式把背景 `bit15` 折进 `0x20000000` 吗（`LegacyMapReader.cs:150/252/308/546`）。
///
/// `Wemade2010`（`:190-211`）与 `CrystalCs`（`:576-625`）**不折** —— 这是上游的漏搬，
/// 本模块的取图号/可走因此按「原样保留 `bit15`」处理（差异见模块文档第四节）。
const fn 折叠背景位(format: FormatId) -> bool {
    use FormatId as F;
    matches!(
        format,
        F::Old12 | F::AntiHack | F::Shanda14 | F::Shanda2012 | F::Heroes34
    )
}

/// 还原后的**背景图号位模式**（含 `bit15` 阻挡位，**未折叠**）。
fn 还原背景位(format: FormatId, cell: &RawCell, k: 头部) -> u32 {
    use FormatId as F;
    match format {
        // 12/14/36 档：磁盘 u16 原值（`AntiHack` 要先 XOR 还原）
        F::Old12 | F::Shanda14 | F::Shanda2012 => u32::from(低16(cell.bk_img)),
        F::AntiHack => u32::from(mir2_format::formats::antihack::csharp_image(
            低16(cell.bk_img),
            k.xor,
        )),
        // 15B 档：`Wemade2010` 整字 XOR（常量 `0xAA38AA38`，与头部键无关）
        F::Wemade2010 => wemade2010::csharp_back_image(cell.bk_img),
        F::Heroes34 => cell.bk_img,
        // Mir3：Type5 的背图在**块级**（格内没有）；Type6 是逐格 `(short)(盘值 + 1)`
        F::Mir3Wemade => 0,
        // Type6 的 C# 是 `(short)(ToInt16 + 1)` ⇒ **符号扩展**（`0x8000` 那类值要保高位补码）
        F::Mir3Shanda => i32::from(低16(cell.bk_img).wrapping_add(1) as i16) as u32,
        F::CrystalCs => cell.bk_img,
    }
}

/// 还原后的**前景图号位模式**（含 `bit15`；Mir3 两支已 `+1`）。
fn 还原前景位(format: FormatId, cell: &RawCell, k: 头部) -> u32 {
    use FormatId as F;
    match format {
        F::AntiHack => u32::from(mir2_format::formats::antihack::csharp_image(
            cell.fr_img,
            k.xor,
        )),
        F::Wemade2010 => u32::from(wemade2010::csharp_front_image(cell.fr_img, k.xor)),
        F::Mir3Wemade => u32::from(mir3_wemade::csharp_image(cell.fr_img)),
        F::Mir3Shanda => u32::from(mir3_shanda::csharp_image(cell.fr_img)),
        // `Old12` / `Shanda14` / `Shanda2012` / `Heroes34` / `CrystalCs`：磁盘原值
        F::Old12 | F::Shanda14 | F::Shanda2012 | F::Heroes34 | F::CrystalCs => {
            u32::from(cell.fr_img)
        }
    }
}

/// C# 内存里的 `BackImage`（`int`）：折叠位那五档把 `bit15` 搬成 `0x20000000`
/// （`LegacyMapReader.cs:150/252/308/546`），其余位原样（`Heroes34` 的高 16 位留着）。
fn 背景内存值(format: FormatId, cell: &RawCell, k: 头部) -> i32 {
    let bits = 还原背景位(format, cell, k);
    if 折叠背景位(format) && bits & 0x8000 != 0 {
        ((bits & 0x7FFF) | 0x2000_0000) as i32
    } else {
        bits as i32
    }
}

/// 地面图号（1 基；`0` = 空）= C# `cell.BackImage & 0x1FFFFFFF`。
///
/// 五档折叠位 `bit15` 之后掩码 ⇒ `v & 0x7FFF`；不折的两档（`Wemade2010` / `CrystalCs`）
/// `bit15` 留在图号里；`Heroes34` 的高 16 位也留着（C# 那边是 `(int)ToInt32`）。
fn ground_image(format: FormatId, cell: &RawCell, k: 头部) -> u32 {
    (背景内存值(format, cell, k) as u32) & 0x1FFF_FFFF
}

/// 中层图号（1 基；`0` = 空；上游 `cell.MiddleImage`，不掩码）。
fn middle_image(format: FormatId, cell: &RawCell, k: 头部) -> u32 {
    use FormatId as F;
    match format {
        F::AntiHack => u32::from(mir2_format::formats::antihack::csharp_image(
            cell.mid_img,
            k.xor,
        )),
        F::Wemade2010 => u32::from(wemade2010::csharp_middle_image(cell.mid_img, k.xor)),
        // Mir3 两支：`MiddleImage = ToUInt16 + 1`（`:427` / `:480`）
        F::Mir3Wemade => u32::from(mir3_wemade::csharp_image(cell.mid_img)),
        F::Mir3Shanda => u32::from(mir3_shanda::csharp_image(cell.mid_img)),
        _ => u32::from(cell.mid_img),
    }
}

/// 物件图号（1 基；`0` = 空）= C# `front.Image & 0x7FFF`。
fn objects_image(format: FormatId, cell: &RawCell, k: 头部) -> u32 {
    还原前景位(format, cell, k) & 0x7FFF
}

/// 开门帧图号：`fr != 0 && (door & 0x7F) != 0 ⇒ fr + door_offset`（1 基，同 C# `:52`）。
fn open_door_image(format: FormatId, cell: &RawCell, k: 头部) -> Option<u32> {
    let image = objects_image(format, cell, k);
    if image != 0 && (cell.door & 0x7F) != 0 {
        Some(image + u32::from(cell.door_offset))
    } else {
        None
    }
}

/// 门格判据（上游 `MapPreview.cs:13` 的入参口径）。
///
/// `CrystalCs` 的 `cell.Door` 在 C# 里**已被 reader 掩过**（`LegacyMapReader.cs:627`），
/// 其余八档是原字节 ⇒ 这里只对 `CrystalCs` 再掩一次 `0x7F`。
#[must_use]
pub fn has_door(format: FormatId, cell: &RawCell) -> bool {
    let door = if format == FormatId::CrystalCs {
        cell.door & 0x7F
    } else {
        cell.door
    };
    door != 0 || cell.door_offset != 0
}

/// C# 扁平全局库号（**消息文案用**；与 [`csharp_back_lib`] 等同一个真源）。
///
/// 六档可查格式上它恒等于「族内页号 + 基址」（Wemade 段 `0/1/2`、Shanda 段 `100/110/120`），
/// 三档未检查格式上它只是公式值（不用于查库）。上游文案 `Library {n}` 打的就是这个号。
fn 全局库号(format: FormatId, family: LibFamily, cell: &RawCell) -> i16 {
    match family {
        LibFamily::Ground => csharp_back_lib(format, cell),
        LibFamily::Middle => csharp_middle_lib(format, cell),
        LibFamily::Objects => csharp_front_lib(format, cell),
    }
}

/// 有前景图吗（上游 `Cell.HasForeground`：`Foreground is { } && (Image & 0x7FFF) != 0`）。
///
/// Mir3 两支的前景是**条件式**装进 `Foreground` 的（`LegacyMapReader.cs:441-443/499-501`：
/// `(frimage & 0x7FFF) > 0 && frindex > 0`）⇒ 哨兵格（`frindex == -1`）即使图号非 0
/// 也没有前景，必须照此判，否则会**漏报**「门没有前景图」。
fn has_foreground(format: FormatId, cell: &RawCell, k: 头部) -> bool {
    use FormatId as F;
    match format {
        F::Mir3Wemade => {
            mir3_wemade::csharp_front_lib(cell.area, cell.fr_img) > 0
                && (mir3_wemade::csharp_image(cell.fr_img) & 0x7FFF) != 0
        }
        F::Mir3Shanda => {
            mir3_shanda::csharp_lib(cell.area) > 0
                && (mir3_shanda::csharp_image(cell.fr_img) & 0x7FFF) != 0
        }
        _ => objects_image(format, cell, k) != 0,
    }
}

/// 可走判据（上游 `MapCheck.Walkable`：`BackImage & 0x60000000 == 0` 的等价物）。
///
/// - Mir3 两支：`flag & 1 == 1 && flag & 2 == 2`（`LegacyMapReader.cs:436-437/494-495`）；
/// - 其余非 Mir3、非 CrystalCs 档：**还原后的**背景与前景 `bit15` 都为 0（折叠位那五档与
///   上游逐格等价：上游把 `bit15` 搬成 `0x20000000/0x40000000`，`& 0x60000000 == 0`
///   正是「两位都为 0」）；
/// - `CrystalCs`：原生 `bit29/bit30`（见下方分支），`bit15` 只是图号高位。
///
/// 差异与理由见模块文档第四节（`Wemade2010` 的 bk `bit15` 上游不折 ⇒ 本仓更严格，
/// 不会误放行；`CrystalCs` 不在该差异内）。
fn walkable(format: FormatId, cell: &RawCell, k: 头部) -> bool {
    use FormatId as F;
    match format {
        F::Mir3Wemade => {
            let (back, front) = mir3_wemade::csharp_walk_bits(cell.flag);
            back && front
        }
        F::Mir3Shanda => {
            let (back, front) = mir3_shanda::csharp_walk_bits(cell.flag);
            back && front
        }
        // CrystalCs 的阻挡位是**持久化的** `bit29/bit30`，不是 `bit15`：
        // `CellCodec.Write` 把 `BackImage` 整字写盘（`_upstream/CrystalStudio/Document.cs:116`），
        // 涂地板时也只替换低 29 位、原样保留 `& 0x60000000`（`MapViewport.cs:505/546`），
        // 而 `bit15` 只是图号高位 ⇒ 判据必须是 `BackImage & 0x60000000`（`MapCheck.cs:100`）。
        // 装载期还会按 `ver > 1` 把**前景** `bit15` 折成 `0x40000000`（`:598`/`:625`），
        // 用 `csharp_back_mark` 一并算上 ⇒ 与 C# 内存里的 `BackImage` 逐位一致。
        F::CrystalCs => {
            (crystal_cs::csharp_back_mark(cell.bk_img, cell.fr_img, k.crystal_ver) & 0x6000_0000) == 0
        }
        _ => {
            (还原背景位(format, cell, k) & 0x8000) == 0
                && (还原前景位(format, cell, k) & 0x8000) == 0
        }
    }
}
// ───────────────────────────── 主入口 ─────────────────────────────

/// **只读**检查一张图（上游 `MapCheck.Run`）。
///
/// 逐格顺序与上游一致（地面 → 中层 → 物件 → 开门帧 → 门两警告），最后跑 8 邻 BFS。
/// `total` 是全量条数，`issues` 截到 [`MAP_CHECK_ISSUE_CAP`] 条。
///
/// 不碰任何可变状态：入参 `&RawMap` + `&dyn LibProbe`，返回一份新报告
/// （只读四项的钉子见 `map_check_tests.rs::检查只读不改文档状态`）。
#[must_use]
pub fn check_map(raw: &RawMap, probe: &dyn LibProbe) -> MapCheckReport {
    let format = raw.format;
    let (w, h) = (usize::from(raw.width), usize::from(raw.height));
    let mut 检查 = 检查器 {
        report: MapCheckReport::default(),
        probe,
        cache: HashMap::new(),
    };
    // 「有没有图库表」先落账（上游 `libraries == null` ⇒ 图号引用整项不查）
    检查.report.无图库 = !检查.probe.has_libraries();
    // 三族「整层未检查」先落账：即使后面一格都没走，GUI 也能如实显示。
    for family in LibFamily::ALL {
        if let Some(reason) = unchecked_reason(format, family) {
            检查.report.未检查.push((family, reason));
        }
    }
    if w == 0 || h == 0 || raw.cells.len() != w * h {
        // 形状非法：三族都没法逐格查（未检查列表改成形状原因，不留误导性的「可查」结论）
        检查.report.未检查 = LibFamily::ALL
            .iter()
            .map(|f| (*f, UNCHECKED_SIZE_MISMATCH))
            .collect();
        return 检查.report;
    }
    // 头部常数（AntiHack/Wemade2010 的 XOR 键、CrystalCs 的版本号）取不到 ⇒ 三层都不可查（不猜 0）
    let Some(k) = 取头部(format, &raw.header_raw) else {
        检查.report.未检查 = LibFamily::ALL
            .iter()
            .map(|f| (*f, UNCHECKED_NO_XOR_KEY))
            .collect();
        return 检查.report;
    };
    // 逐格：C# 的行优先键序（`key = y * Width + x`）—— 本仓是列优先存储，这里换算一次
    for key in 0..w * h {
        let (x, y) = (key % w, key / w);
        let cell = &raw.cells[x * h + y];
        let (ux, uy) = (x as u16, y as u16);
        // ① 地面
        检查.查一层(
            format,
            LibFamily::Ground,
            cell,
            ground_image(format, cell, k),
            "Ground",
            ux,
            uy,
        );
        // ② 中层
        let 中图 = middle_image(format, cell, k);
        if 中图 != 0 {
            检查.查一层(format, LibFamily::Middle, cell, 中图, "Middle", ux, uy);
        }
        // ③ 物件层 + 开门帧（上游 `:47-53`）
        let 物图 = objects_image(format, cell, k);
        if 物图 != 0 {
            检查.查一层(format, LibFamily::Objects, cell, 物图, "Objects", ux, uy);
            if let Some(开门) = open_door_image(format, cell, k) {
                检查.查一层(format, LibFamily::Objects, cell, 开门, "Open door", ux, uy);
            }
        }
        // ④ 门两警告（上游 `:54-59`）
        if has_door(format, cell) {
            if !has_foreground(format, cell, k) {
                检查.记(ux, uy, MSG_DOOR_NO_FOREGROUND.to_string());
            }
            if !任一邻可走(raw, format, k, x, y) {
                检查.记(ux, uy, MSG_DOOR_NO_WALKABLE.to_string());
            }
        }
    }
    // ⑤ 断开的可走区域（上游 `:61-95`）
    check_walkable_blocks(raw, format, k, &mut 检查);
    检查.report
}

/// 「AntiHack / Wemade2010 的图号要用头部 XOR 键，但头部不够长」。
const UNCHECKED_NO_XOR_KEY: &str =
    "该格式的图号要按头部 XOR 键还原（AntiHack 偏移 33 / Wemade2010 偏移 23），头部不足";
/// 一次检查的累加器：报告 + 探针 + 「按 `(库, 图号)` 缓存」的结论表（上游 `checkedImages`）。
struct 检查器<'a> {
    report: MapCheckReport,
    probe: &'a dyn LibProbe,
    cache: HashMap<(MapLibRef, u32), ImageState>,
}

impl 检查器<'_> {
    /// 记一条问题：`total` 全量 +1，清单未满 1000 才入列（上游 `MapCheck.cs:18-22` 的 `Add`）。
    fn 记(&mut self, x: u16, y: u16, message: String) {
        self.report.total += 1;
        if self.report.issues.len() < MAP_CHECK_ISSUE_CAP {
            self.report.issues.push(MapIssue { x, y, message });
        }
    }

    /// 查一层图号并落一条问题（上游 `MapCheck.Image` 的等价物）。
    #[allow(clippy::too_many_arguments)] // `self` + 7 个都是"这一格这一层"的输入
    fn 查一层(
        &mut self,
        format: FormatId,
        family: LibFamily,
        cell: &RawCell,
        image: u32,
        layer: &'static str,
        x: u16,
        y: u16,
    ) {
        // 上游 `:26`：`if (image == 0 || libraries == null) return;`
        //
        // ⚠ **没有图库时一条图号都不报**（连「必然缺库」的 Mir3 哨兵格也不报）：
        // 上游 `Image()` 在任何库号判断之前就先 `return` 了，而门两警告与不连通区
        // **不看 `libraries`**（那两段在 `:54-95`，与库表无关）⇒ 照常报。
        if image == 0 || !self.probe.has_libraries() {
            return;
        }
        let lib = match layer_source(format, family, cell) {
            LibSource::Page(lib) => lib,
            // `-1` 哨兵：键恒 `>= 0` 的字典里必然没有它 ⇒ 直接判缺库，**不探测**
            LibSource::Missing => {
                self.记(
                    x,
                    y,
                    format!("{layer}: {MSG_LIB_MISSING} · Library -1, Image {image}"),
                );
                return;
            }
            // 该格式这一层不可查 ⇒ 不探测、不报（原因已在 `report.未检查` 里）
            LibSource::Unchecked(_) => return,
        };
        let 态 = match self.cache.get(&(lib, image)) {
            Some(已) => *已,
            None => {
                let 态 = self.probe.frame_state(lib, image);
                self.cache.insert((lib, image), 态);
                态
            }
        };
        let 原因 = match 态 {
            ImageState::Ok => return,
            ImageState::MissingLibrary => MSG_LIB_MISSING,
            ImageState::OutOfRange => MSG_IMAGE_RANGE,
            ImageState::Empty => MSG_IMAGE_EMPTY,
            ImageState::Unreadable => MSG_IMAGE_UNREADABLE,
        };
        // 文案里的 `Library` 是 **C# 扁平全局号**（`Wemade` 段 `0/1/2+k`、`Shanda` 段
        // `100/110/120+k`），与上游 `MapCheck.cs:38` 逐字同形、可与 C# 报告直接对号；
        // 族内页号在 [`MapLibRef::index`] 里（`_ = lib` 只是为了把两者分开写清）。
        let 全局 = 全局库号(format, family, cell);
        let _ = lib;
        self.记(
            x,
            y,
            format!("{layer}: {原因} · Library {全局}, Image {image}"),
        );
    }
}

/// 3×3 里有没有可走格（不含自身；上游 `MapCheck.cs:56-58` 的双层循环）。
fn 任一邻可走(raw: &RawMap, format: FormatId, k: 头部, x: usize, y: usize) -> bool {
    let (w, h) = (usize::from(raw.width), usize::from(raw.height));
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            if walkable(format, &raw.cells[nx as usize * h + ny as usize], k) {
                return true;
            }
        }
    }
    false
}

/// 8 邻 BFS：**跳过最大一块**，其余每块报一行（上游 `MapCheck.cs:61-95`）。
///
/// 起点顺序照上游的行优先键序（`key = y * Width + x`）：它决定「同样大的两块谁是最大」，
/// 换成本仓的列优先序会让报出的块换一个（结论一样、行不同）。
fn check_walkable_blocks(raw: &RawMap, format: FormatId, k: 头部, 检查: &mut 检查器<'_>) {
    let (w, h) = (usize::from(raw.width), usize::from(raw.height));
    let mut 见过 = vec![false; w * h];
    let mut 块: Vec<(usize, usize)> = Vec::new(); // (C# 键, 格数)
    let mut 队列: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    for key in 0..w * h {
        if 见过[key] {
            continue;
        }
        见过[key] = true;
        if !walkable(format, &raw.cells[(key % w) * h + key / w], k) {
            continue;
        }
        let mut 数 = 0usize;
        队列.push_back(key);
        while let Some(当) = 队列.pop_front() {
            数 += 1;
            let (x, y) = (当 % w, 当 / w);
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let n = ny as usize * w + nx as usize;
                    if 见过[n] {
                        continue;
                    }
                    见过[n] = true;
                    if walkable(format, &raw.cells[(n % w) * h + n / w], k) {
                        队列.push_back(n);
                    }
                }
            }
        }
        块.push((key, 数));
    }
    // 最大块（并列取**先出现**的那块，同上游 `size > largest.Size` 的严格大于）
    let mut 最大: Option<usize> = None;
    for (i, (_, 数)) in 块.iter().enumerate() {
        if 最大.is_none_or(|m| 块[m].1 < *数) {
            最大 = Some(i);
        }
    }
    for (i, (起, 数)) in 块.iter().enumerate() {
        if Some(i) == 最大 {
            continue;
        }
        检查.记(
            (*起 % w) as u16,
            (*起 / w) as u16,
            format!("{MSG_DISCONNECTED}: {数} cells"),
        );
    }
}

#[cfg(test)]
#[path = "map_check_tests.rs"]
mod tests;
