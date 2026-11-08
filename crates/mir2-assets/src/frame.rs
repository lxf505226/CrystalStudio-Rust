//! 帧数据结构与「图库帧源」抽象。

use crate::lib_paths::LibRef;

/// 一帧解码结果：**行优先 RGBA8**（`rgba.len() == width * height * 4`，自顶向下）+ **锚点偏移**。
///
/// 字段与客户端图集 meta 的 `frames[i]` 一一对应：`width/height` 即 `w/h`，
/// `rgba` 即该帧 `region` 里那块像素的原始字节，`anchor` 即 `anchor: [px, py]`。
/// 逐像素比对（验收门 1）比的是前两者；`anchor` 由 `帧锚点与图集meta逐帧一致` 逐帧对账。
///
/// # `anchor` 是什么（三方同源，不是推测）
///
/// | 来源 | 字段 | 出处 |
/// |---|---|---|
/// | 二进制（WIL/WIX） | 帧头 `[4:6] = px i16` / `[6:8] = py i16` | `vendor/wil2atlas/src/wil_decoder.rs:206-209` |
/// | 二进制（WZL/WZX） | 块头 `[8:10] = px` / `[10:12] = py`（块头前多 4 字节 marker+unused） | `vendor/wil2atlas/src/wzl_decoder.rs:264-269` |
/// | 参考 C++ 客户端 | `TDXTextureInfo{ u16 nWidth; u16 nHeight; short px; short py; }` | `client/reference_full/Classes/Wil/WIL.h:50-56` |
/// | 官方 C# 编辑器 | `Sprite(Bitmap, int X, int Y)`，`X/Y` 读自帧记录 `[4:6]`/`[6:8]` | `_upstream/CrystalStudio/Assets.cs:86(未入库)` |
///
/// 语义 = **图片左上角相对锚点的绘制偏移**（与客户端一致：`DrawSprite(dx + px, dy + py)`，
/// 见 `GodotClient/game/resources/atlas_loader.gd:318-319` 的同款注释）。
///
/// # 覆盖范围：三条容器路径（`.wil` / `.wzl` / 原版 `.lib`）
///
/// [`crate::LibPaths`] 只登记 `.wzl/.wzx` 与 `.wil/.wix` 两种容器
/// （`lib_paths.rs:257-262`：数据文件只认 `wzl|wil`，索引文件只认 `wzx|wix`），
/// 其它扩展名一律忽略。官方 C# 编辑器读的 `.lib`（Crystal v2/v3、Sanxiu v5/v6）
/// 走**另一条独立路径** [`crate::CrystalLibPaths`] + [`crate::CrystalLibrary`]，
/// 不经过 `wil2atlas`。
///
/// 因此 `anchor` 有两个生产点，都指向**同一个字段**：
///
/// | 生产点 | 来源字段 |
/// |---|---|
/// | [`crate::WilSource::lookup_frame`] | WIL 帧头 `[4:6]` / WZL 块头 `[8:10]`（见上表） |
/// | [`crate::CrystalLibrary::decode_checked`] | `.lib` 帧头 `[4:6]`；Sanxiu 取元数据的 `x/y`（`_upstream/CrystalStudio/Assets.cs:86,163`） |
///
/// `.lib` 的帧头与 WIL **前 8 字节逐字段同构**（`_upstream/CrystalStudio/Assets.cs:86`），
/// 语义同一字段、位置相同，不需要另立字段。
///
/// # ⚠ 消费口径（谁该用、谁不许用）
///
/// 全仓**只有两个**消费点，且都有上游出处；除此之外**任何地方都不许**读本字段：
///
/// | 消费点 | 用不用 | 上游依据 |
/// |---|---|---|
/// | 光罩（地图光照） | **无条件用** | `_upstream/CrystalStudio/MapViewport.Presentation.cs:84-86(未入库)`：`offset = new(sprite.X, sprite.Y)`，不看库号/图号/尺寸 ⇒ `miredit-render` 的 `lighting::light_bounds` 直接用本字段 |
/// | 前景**加性混合**帧（`Front.Frames & 0x80 != 0`） | **用**（按分支条件） | `_upstream/CrystalStudio/MapSpriteDrawing.cs:43-52(未入库)` 的 `front && blend` 是**一条独立分支**：库 `14/27/100..198` ⇒ `px += X; py += 32-96 + Y`；否则先底边锚点，再按 **0-based 帧号** `2723..2732`（等价图号 2724..2733）⇒ `px += X; py += Y`。落地实现见 `miredit-render` 的 `compose.rs` §十（`混合锚点`） |
/// | **非混合**帧（表层普通帧 / 中层 / 地表） | **不许用** | 上游这类帧走格栅锚点（`MapSpriteDrawing.cs:53-55` 的 `tile` 判定与 `library == 28` 非混合特例另属台账 A-001，**未采纳**），本仓口径见 `compose.rs` §一的规则表 |
///
/// > **行号口径（2026-11-08 订正）**：本节 `MapSpriteDrawing.cs` 的行号一律按 `@6450da90` 及以后
/// > （`@a78affdf` 与 `@6450da90` 的该文件逐字节相同，blob `5ffc7fd7`）。**`@901dbe2f` 时代
/// > `front && blend` 分支在 `:19-28`**，`43-52` 在那版落在 `Composite` 方法体内 ——
/// > 旧注释把 `@6450da90` 的行号挂在了 `@901dbe2f` 标签上；本次只订正标签与行号归属，不改任何数值。
///
/// 两条边界（改这条口径前先回核上游）：
///
/// 1. **`front && blend` 之外一律不接**：混合帧用锚点是上游同名分支的组成部分
///    （A-018，`MapSpriteDrawing.cs:43-52`），**不是**「顺手把偏移接进三层合成」——
///    该函数其余分支一个 `X/Y` 都不加；
/// 2. **三格库分支只在 12 字节档**：`New14` / `Ext36` 视图档位里混着 `Shanda14`(+120)、
///    `Mir3*`(+200/+300) 与 `CrystalCs`，C# 库号推不出来 ⇒ 那两档只实现了不依赖库号的
///    帧号窗口分支（`compose.rs` §十.2）。
///
/// > **订正（2026-11-07）**：本节原文写「三层合成（地表/中层/表层）**不许用**，本字段
/// > 不得接进三层合成」。该表述**不准** —— 它把「非混合帧不使用」错误地推广成了「三层
/// > 合成一律不使用」。实为：**前景加性混合帧**必须用（否则加性混合帧落错位置，台账
/// > A-018），**非混合帧**仍不得用。原文的边界保留，只把适用范围收窄到「非混合帧」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// 帧宽（像素）。
    pub width: u32,
    /// 帧高（像素）。
    pub height: u32,
    /// 行优先 RGBA8 像素，长度 = `width * height * 4`。
    pub rgba: Vec<u8>,
    /// 锚点偏移 `(px, py)`：图片左上角相对锚点的绘制偏移，单位像素（可为负）。
    ///
    /// 与图集 meta 的 `frames[i].anchor = [px, py]` 同源同值；帧头里恒存在（定长字段），
    /// 不随库/帧变化，取不到只会是「该帧无效」而不是「该容器没有这个字段」。
    pub anchor: (i16, i16),
}

impl Default for Frame {
    /// 全空帧：0×0、无像素、锚点 `(0, 0)`。
    ///
    /// 存在的意义是让**别处的 `Frame { … }` 字面量**可以写成
    /// `Frame { width, height, rgba, ..Frame::default() }`，不必逐处补 `anchor`。
    fn default() -> Self {
        Frame {
            width: 0,
            height: 0,
            rgba: Vec::new(),
            anchor: (0, 0),
        }
    }
}

impl Frame {
    /// 像素字节数（`width * height * 4`）。
    pub fn byte_len(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    /// 取某像素的 RGBA；越界返回 `None`。
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let s = self.rgba.get(i..i + 4)?;
        Some([s[0], s[1], s[2], s[3]])
    }
}

/// 一帧的引用状态（A-015 图号检查）。
///
/// 对应上游 `MapCheck.cs` 的四类结论（正常 / 空帧 / 读不出 / 图号越界），外加"库不存在"。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageState {
    /// 正常。
    Ok,
    /// 索引存在但帧头无效（上游 `Empty image`）。
    Empty,
    /// 读不出 / 损坏（上游 `Unreadable image`）。
    Unreadable,
    /// 图号越界（上游 `Invalid image number`）。
    OutOfRange,
    /// 库不存在（上游 `Missing library`）。
    MissingLibrary,
}

/// 图库来源：把 `(库, 帧号)` 换成像素帧 / 元信息（地图渲染与印章缩略图共用）。
///
/// 三族索引空间互不相通（见 [`LibRef`]）；`frame_state` 是 A-015「检查地图」用的
/// 只读探针（能只读帧头的实现应覆盖缺省实现，避免为查一个图号解码整帧）。
/// 图库帧源抽象：按「族 + 族内索引」定位库，再按「0-based 帧号」取像素。
///
/// 契约（三条必须同时成立）：
///
/// 1. **不 panic**。任何输入（含 `u32::MAX`）都只返回 `Some` / `None`。
/// 2. **不返回假图**。`None` 明确表示「该库不存在 / 帧号越界 / 空帧（WIX 偏移 0 或尺寸越界）」，
///    绝不返回全透明占位图 —— 渲染层据此显示「缺图」而不是黑块。
/// 3. 返回的 RGBA 必须与客户端离线烘焙图集**逐字节一致**（同一解码器 + 同一调色板）。
pub trait SpriteSource {
    /// 取 `lib` 指定的图库的第 `index` 帧（0-based）；不可得返回 `None`。
    ///
    /// `lib` 用 [`LibRef`] 而不是单个整数：三族的索引空间互不相通，
    /// 详见 [`crate::LibPaths::resolve`] 的说明。
    fn frame(&self, lib: LibRef, index: u32) -> Option<Frame>;

    /// **一帧的引用状态**（A-015 图号检查用）：**只读元信息、不解码像素**。
    ///
    /// 默认实现退化成"取一次帧"（`Some ⇒ Ok`，`None ⇒ Unreadable`）—— 对任何实现者成立，
    /// 但会解码像素；能只读帧头的源应覆盖它（见 `WilSource` / `CrystalLibSource`）。
    fn frame_state(&self, lib: LibRef, index: u32) -> ImageState {
        if self.frame(lib, index).is_some() {
            ImageState::Ok
        } else {
            ImageState::Unreadable
        }
    }

    /// **只取锚点偏移**（不解码像素）；不可得返回 `None`。
    ///
    /// 语义与 [`Frame::anchor`] 完全一致（同一个帧、同一个字段），但**不产生像素**：
    /// 光罩定位只需要这一对数，为了拿它去解码一整张 RGBA 是纯浪费。
    ///
    /// # 为什么给默认实现
    ///
    /// 默认实现退化成「取帧再读 `anchor`」—— 对任何实现者都成立，且**不破坏**既有实现
    /// （新增方法带默认体，别处的 `impl SpriteSource` 一行都不用改）。
    /// 能拿到帧头元信息的高效实现（如 [`crate::WilSource`]）应覆盖它，走「只读帧头」的路。
    ///
    /// # 三条契约与 [`SpriteSource::frame`] 逐条相同
    ///
    /// 1. **不 panic**（含 `u32::MAX`）；
    /// 2. **不编造**：取不到就 `None`，绝不返回 `(0, 0)` 冒充「这一帧的锚点是零」——
    ///    「锚点确实是 `(0, 0)`」与「取不到」是两件事，调用方要能区分；
    /// 3. 返回的 `(px, py)` 必须等于同一帧 [`SpriteSource::frame`] 的 `anchor`（测试钉住）。
    fn anchor(&self, lib: LibRef, index: u32) -> Option<(i16, i16)> {
        self.frame(lib, index).map(|f| f.anchor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Default` 的锚点必须是 `(0, 0)`，且宽高为 0、无像素。
    ///
    /// 这条钉住的是**别处 19 处 `Frame { … }` 字面量的语义**：它们用
    /// `..Frame::default()` 省略锚点，等于声明「这些夹具的锚点是零」。
    /// 若哪天 `Default` 被改成别的值，三层合成/印章的既有测试会跟着变，必须先红在这里。
    #[test]
    fn 默认帧锚点为零且无像素() {
        let f = Frame::default();
        assert_eq!(f.anchor, (0, 0));
        assert_eq!((f.width, f.height), (0, 0));
        assert!(f.rgba.is_empty());
        assert_eq!(f.byte_len(), 0);
        assert_eq!(f.pixel(0, 0), None);
    }
}
