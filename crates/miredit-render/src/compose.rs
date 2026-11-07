//! 三层合成主流程（**全库唯一的「落格规则」实现处**）。
//!
//! # 一、落格规则（逐条对照客户端，改这里前先回核 `文件:行号`）
//!
//! | # | 规则 | 出处 |
//! |---|---|---|
//! | 1 | 世界格 = 48×32 px | `map_view.gd:9-10`（`Grobal.h:1980-1981` UNITX/UNITY）|
//! | 2 | 地表帧 = 96×64 px，覆盖 2×2 世界格，**落格 = 世界格 / 2** | `map_view.gd:11-13` |
//! | 3 | 地表**只铺偶偶格**（`x%2==0 && y%2==0`），其余格无地表 | `map_view.gd:12,199`（`GameScene.cpp:3545-3553`）|
//! | 4 | 地表像素位 = `(x/2)*96 = x*48`，纵向 `y*32` | `map_view.gd:13` |
//! | 5 | 中层：**tile 尺寸（48×32 / 96×64）且非混合 ⇒ 左上**；否则**底边**（`y_top = (y+1)*32 - 帧高`） | C# `MapSpriteDrawing.cs:24,55@f115352`（`.gd` 恒左上：`map_overlay_layer.gd:17,79`，A-001 起不跟） |
//! | 6 | 表层：同上式（`front=true, blend=false`）⇒ **tile 尺寸不位移**，非 tile 才底边；另有 `library==28` 且帧锚点非零 ⇒ **只叠锚点** | C# `MapSpriteDrawing.cs:53-55`（`.gd` 恒底边：`map_overlay_layer.gd:96-99`，A-001 起不跟） |
//! | 7 | 帧号 **1-based**，`0` = 无图，取图 `(img & 0x7FFF) - 1` | `map_loader.gd:51-52`、`map_overlay_layer.gd:20` |
//! | 8 | 库路由：`btTiles`→地面 / `btsmTiles`→中层 / `btArea`→表层 | `map_loader.gd:39-45`、`map_overlay_layer.gd:67,71,84,88` |
//!
//! # 二、层序与层内顺序
//!
//! - **层序**：地表 → 中层 → 表层（后画的压前画的）。
//!   出处：`map_view.gd:108-114`（`add_child` 顺序 = 绘制序：「中层先、表层后」）、
//!   `map_overlay_layer.gd:22-24`。
//! - **层内顺序**：**列优先**——`dx` 外层、`dy` 内层，同一列内 y 递增。
//!   出处：`map_overlay_layer.gd:46-53`（`for dx in cells: for dy in cells:`，注释
//!   `:14-16` 明确「绘制顺序 = 列优先 … 变尺寸物件互相遮挡由该顺序决定」）。
//!
//! # 二·五、落格算式的唯一实现处
//!
//! 上面表格里的锚点/帧号规则，代码上**只实现一次**：[`非加性锚点`]（规则 5/6 的完整算式，
//! 内含 [`左上锚点`] / [`底边锚点`] / [`tile尺寸`] / [`前景库号`]）、[`混合锚点`]（A-018 的
//! `front && blend` 分支）、[`帧号`]（规则 7）。地图渲染（本模块的三层循环）与
//! 印章渲染（`crate::stamp`）调的是**同一批函数** —— 改锚点只改一处，两边同时生效，
//! 不会出现「地图一套、印章另一套」的口径分裂。
//!
//! **循环级的落格前提也只有一份**：[`层取帧`]（偶偶格 + 库路由 + 帧号 + 门偏移 + 加性标志，
//! 规则 3/7/8 + §七）与 [`层放置`]（把取样的分支接到上面两个锚点算式上）。本模块的三层循环、
//! 窗内小地图的总览图（`crate::overview`）都调这两个函数 ⇒「1× 合成」与「1/2^level 总览」
//! 不可能各写一套落格口径。总览另需把位移换算到 level 尺度（`px / 2^level`），那是
//! `crate::overview` 自己的事，换算完仍走同一批锚点函数。
//!
//! ⚠ **与任务书 §三.4 的差异**：任务书写「表层按 y 排序绘制」（= 原始 C++ 的行优先，
//! `GameScene.cpp:3429-3431` 是 `j` 外层 / `i` 内层）。按「以源码为准」的口径，本实现取
//! **Godot 客户端的列优先**：截图对照的基准就是 Godot 客户端，层内顺序必须以它为准。
//! 两者只在**变尺寸表层帧互相重叠**时才看得出差别（0.map 表层多为 48×32，实际影响极小）。
//!
//! # 三、缺帧口径（禁假数据）
//!
//! - 该层图号为 `0` ⇒ **该层本来就没图**，跳过，**不计入 missing**（空地不是缺帧）；
//! - 图号非 0 但帧源返回 `None`（库没登记 / 帧号越界 / 空帧）⇒ **不画任何东西**，
//!   该层 `missing_* += 1`；**绝不画洋红/纯色占位图**（那是假数据）；
//! - 窗口落在图外（坐标 < 0 或 ≥ 宽高）⇒ 不画，也**不计入任何计数**；
//! - 关掉的层不画也不计数（见 [`crate::LayerMask`]）；
//! - `cells_drawn` 的口径是「**真的写了至少 1 个像素**」，不是「扫过了这一格」：
//!   同一格三层只算 1；缺帧、0×0 空帧、整帧落在画布外、帧落在画布内的像素全透明
//!   ⇒ **一个都不算落笔**（这些格确实没往画布上写任何东西）。判定只用
//!   [`crate::canvas`] 的 `Canvas::blit` 返回值，这里不另立第二套判据。
//!
//! # 四、刻意不做的三件事
//!
//! 1. **不做动画帧**：`btAniFrame`/`btAniTick`（`GameScene.cpp:3482-3508`）需要
//!    「当前动画计数」这个运行期状态；Godot 客户端当前也只取静态基准帧
//!    （`map_overlay_layer.gd:20`「本文件不碰」）⇒ 本库保持同一口径（静态帧）。
//!    ⚠ **门偏移不再属于这一类**：它由 [`RenderOptions::doors_open`] 显式开关控制，
//!    默认关（= 静态帧），开关见 §七。
//! 2. **不复制客户端的图库白名单**（`map_layers.gd:24-31` 的 TILE_LIBS/SM_LIBS/OBJ_LIBS）：
//!    本库以「图库登记表」（`mir2_assets::LibPaths`）为唯一事实来源 —— 登记表里没有的库，
//!    取帧自然返回 `None` 并计 missing，不会出现「白名单里写了但磁盘没有」的第二份真相。
//! 3. ~~**不做光照的视觉差异**~~ **已实现**（2026-11-05）：`btLight` 现在有消费点了 ——
//!    全屏遮罩 + 逐格光罩，见 §八。默认档（白天）仍是「什么都不做」，输出逐字节不变。
//!
//! # 五、已知与客户端实现的差异（如实登记，不静默）
//!
//! - 地表帧若**不是** 96×64：本库按**帧原始尺寸**贴（与 C++ 原版 `postRender` 一致）；
//!   Godot 侧走 `TileSetAtlasSource`，格子固定 96×64（`ground_tileset.gd:35,205`），
//!   尺寸不符时会被按格裁切/留白。实测 0.map / 41.map 的地表帧都是 96×64，不触发。
//! - 原始 C++ `renderBridge` 只画 **48×32** 的表层帧（`GameScene.cpp:3513`
//!   `nWidth == UNITX && nHeight == UNITY` 才 postRender），Godot 移植版没有这条限制
//!   （`map_overlay_layer.gd:96-100` 按帧高做底边对齐）⇒ 本库跟 `.gd`（截图对照基准）。

//! # 六、阻挡标记（独立通道，默认关闭）
//!
//! 三层画完之后，若 [`CollisionOverlay`] 有通道开着，就**逐格**叠加低/高位阻挡标记
//! （判定与视觉见 [`crate::collision`]，口径汇总见 crate 文档 §七）。三条硬口径：
//!
//! 1. **默认关闭**：`overlay.low == false && overlay.high == false` ⇒ 整个环节跳过，
//!    输出与「没有这个功能」逐字节相同（旧函数就是这么转调的）；
//! 2. **通道独立**：`low`/`high` 各自与判定相与，只开一个时另一个的格**一个像素都不染**；
//! 3. **不计入 `cells_drawn`**：标记走 `cells_low_blocked` / `cells_high_blocked`，
//!    `cells_drawn` 的口径始终只统计三层精灵的落笔（见 §三）。
//!
//! # 七、门开帧偏移（`doors_open`，**只动前景层的帧号**）
//!
//! 客户端原文（`GameScene.cpp:3328-3388`）：
//!
//! ```text
//! fridx = (wFrImg & 0x7FFF) + 动画步进
//! if ((btDoorOffset & 0x80) > 0 && (btDoorIndex & 0x7F) > 0) fridx += (btDoorOffset & 0x7F)
//! fridx -= 1
//! GetObjs(btArea, fridx)                 // :3341 库号 = btArea，:3388 取帧
//! ```
//!
//! 本库（静态帧、0-based 帧号，[`帧号`] 已减 1）等价写法：
//!
//! ```text
//! 索引 = 帧号(cell.fr_img)                                  // = (fr_img & 0x7FFF) - 1
//! if options.doors_open && (cell.door_index & 0x7F) != 0
//!     索引 += cell.door_offset & 0x7F                       // 见 [`门偏移`]
//! ```
//!
//! 四条硬口径：
//!
//! 1. **默认关**：`doors_open == false` ⇒ [`门偏移`] 恒返回 `0`，`索引` 与改动前同一个值
//!    ⇒ 输出逐字节不变（这是「不改既有 API 签名」的兼容性来源）；
//! 2. **只在前景层加**：地表（`bk_img`）/ 中层（`mid_img`）**没有门**，两处代码一行都不碰；
//!    门帧与闭门帧同属 `Objects` 库（`GameScene.cpp:3341,3388`），`Tiles`/`SmTiles` 无关；
//! 3. **必须 `& 0x7F`**：`door_offset` 高位的 `0x80` 是「运行期已开门」标志，不是帧偏移；
//!    全量实测 **43 张图 / 1,764 格**已置位，其中 **1,229 格**同时有前景图与非 0 门索引
//!    （**1,173 格库号为 0 = `Objects` 真实存在**）⇒ 整字节相加会 `+128+low` 画错帧。
//!    详见 [`RenderOptions`] 的类型文档 §三/§四（含「为什么不跟 C# 编辑器」）；
//! 4. **`door_index & 0x7F == 0` 不加**：没有门的格即便 `door_offset` 非 0 也不加。
//!
//! 门开帧取不到（库缺 / 帧号越界 / 空帧）时与其它缺帧同口径：不画、计 `missing_front`
//! —— 绝不回退去画闭门帧（那会静默撒谎，说「门开了」却显示关着的门）。
//!
//! # 八、地图光照（`lighting`，**三层之后、标记之前**）
//!
//! 官方 `MapViewport.Presentation.cs` 的绘制序（同一个 `DrawPresentation` 里从上往下）：
//!
//! ```text
//! :31-32  PushClip(map)                     // 裁到地图矩形
//! :34-35  DrawImage(DarknessMask())         // ← 全屏黑遮罩 + 逐格光罩
//! :40-42  CollisionMarkers.Draw(...)        // ← 阻挡标记：在遮罩之后 ⇒ 不被压暗
//! :43-50  lightMarker / doorMarker          // ← 黄/青标记：同样在遮罩之后
//! ```
//!
//! 本模块的落点与之一致：三层循环结束 ⇒ `crate::lighting::apply` ⇒ 阻挡标记。
//! 因此**阻挡标记不会被压暗**（若把遮罩放在标记之后，就与官方相反了）。
//!
//! 三条硬口径（细节全在 [`crate::lighting`] 的模块文档）：
//!
//! 1. **默认（白天）零成本**：`darkness == 0` ⇒ `apply` 第一行返回，画布一个字节都不动；
//! 2. **裁在地图矩形内**：离屏画布本身就是地图矩形（`cells_w*48 × cells_h*32`），
//!    官方在屏幕上多出来的视口边缘在本库不存在 ⇒ 恒等裁剪；
//! 3. **不计入 `cells_drawn`**：遮罩不是「格子落笔」，与阻挡标记同一条纪律（§六.3）。
//! 4. **光罩位置用前景帧的 `anchor`，三层合成不用**：官方光罩无条件加 `sprite.X/Y`
//!    （`Presentation.cs:84-86`），画精灵却只在特例下加（`MapSpriteDrawing.cs:19-31`）
//!    ⇒ 本模块只把 `src` 递给 `lighting::apply` 取偏移，三个落格循环一行都不碰它。
//!
//! # 九、前景加性混合（`Front.Frames & 0x80`，**只管表层**）
//!
//! 上游 `CrystalStudio/MapSpriteDrawing.cs:5@6450da90`：
//!
//! ```text
//! internal static bool FrontBlend(byte frames) => (frames & 0x80) != 0;   // :5
//! ```
//!
//! 被 `FrontBlend` 选中的帧**不做覆盖，而做加性叠加**（`MapViewport.cs:381`：
//! `dc.DrawImage(blend ? MapSpriteDrawing.Additive(...) : s.Bitmap, bounds)`）。
//! 合成公式在 `MapSpriteDrawing.cs:68-96` 的 `Composite`（Pbgra32 预乘空间）：
//! RGB 逐通道 `min(255, dst+src)`、alpha **不参与加法**取 `max(旧 dst.a, max(加后 RGB))`。
//! 本仓的画布是 straight RGBA8 ⇒ 预乘/反预乘在 [`crate::canvas`] 里显式做，
//! 舍入口径与「待回源」标注见 `Canvas::blit_additive` 的文档。
//!
//! 四条硬口径：
//!
//! 1. **只在前景层**：本模块只有 §表层这个循环看 `cell.ani_frame` 的 `0x80`；
//!    地表/中层一行都不碰。中层那套 `MiddleBlend`（`MapSpriteDrawing.cs:8`）对
//!    **12/14 B 档作废**：这两档的 `.map` 里**没有**中层动画字节
//!    （`mir2-format::Cell::mid_ani_frame` 恒填 `0`）⇒ `MiddleFrames(frames) ≡ 0`
//!    ⇒ `MiddleBlend` 永不成立。本单不动中层；
//! 2. **与门偏移正交**：`ani_frame` 决定「怎么混」，`门偏移` 决定「取第几帧」，
//!    两者互不影响（门偏移的实现与文档见 §七）；
//! 3. **不进缓存键**：混合方式是**每帧现算**的 `u8` 高位判断，不是 `RenderOptions`
//!    之类的显式选项 ⇒ [`FrameCache`] 的键（库 + 帧号）不需要、也不许变；
//! 4. **`cells_drawn` 口径不变**：仍只认「真的写了至少 1 个像素」，判定来源换成
//!    `blit` / `blit_additive` 各自的返回值（两者共用同一份裁剪骨架，口径逐条相同）。
//!
//! ⚠ **未覆盖**（如实登记）：印章渲染（`crate::stamp`，`StampCellInput` 无前景 frames
//! 高位语义）、缩略图、总览（`MapViewport.Overview.cs` / `MinimapExporter.cs` 那几处
//! 上游也传 `FrontBlend`，本仓尚未接线）。混合帧的**落格锚点**见 §十（已实现，但只到
//! 12 字节档的库分支）。

//! # 十、前景加性混合帧的**落格锚点**（`Bounds(..., blend: true)`）
//!
//! §九 只决定「怎么混」，本节决定「混合帧落在哪」。上游 `Bounds` 里 `front && blend`
//! 是一条**独立分支**，与格栅锚点互斥（`MapSpriteDrawing.cs:43-52@6450da90`；本地未入库
//! 快照 `_upstream/CrystalStudio/MapSpriteDrawing.cs`，sha256 `0AEB302F…`）：
//!
//! ```text
//! double px = x * 48.0, py = y * 32.0;                      // 世界格 → 像素
//! else if (front && blend)                                  // :43
//! {
//!     if (library is 14 or 27 || library is > 99 and < 199) // :45
//!     { px += sprite.X; py += 32 - 96 + sprite.Y; }         // :46 ← 三格基线（96 是常量，与帧高无关）
//!     else
//!     {
//!         py += 32 - sprite.Height;                         // :49 ← 底边锚点
//!         if (image is >= 2723 and <= 2732)
//!         { px += sprite.X; py += sprite.Y; }               // :50 ← 帧号窗口
//!     }
//! }
//! ```
//!
//! 三条必须逐字照抄的口径（错一条就与上游不一致）：
//!
//! 1. **`image` 是 0-based 帧号，不是图号**：上游传进 `Bounds` 的实参来自
//!    `MapPreview.FrontImage`（`MapPreview.cs:8`：`(front.Image & 0x7FFF) - 1`）⇒ 窗口
//!    `2723..2732` 对应的**图号是 2724..2733**。台账 A-018 把窗口写成「图号 2723..2732」
//!    是**简写错位（差 1）**，本实现按 0-based 帧号。
//! 2. **门偏移参与取帧、也参与这个判断**：上游把 `FrontImage(...)` 的结果（含
//!    `doorsOpen` 时的 `DoorOffset` 相加；本仓掩 `0x7F` 的口径见 §七 硬口径 3）直接传给 `Bounds`
//!    （`MapViewport.cs:287`）⇒ 本实现用**加过 [`门偏移`] 之后**的 `索引` 做窗口判断，
//!    不用 `帧号(cell.fr_img)`。（本仓不做动画帧，§四.1；上游那一步 `animationFrame/…`
//!    的叠加以外不涉及本规则。）
//! 3. **`py += 32 - 96 + Y` 里的 96 是三格基线常量**：别改成 `32 - 帧高`，那会让这一支
//!    退化成普通底边锚点。
//!
//! ## 十.1 本仓实现
//!
//! 落格算式**只写一次**：[`混合锚点`]。`render` 的前景循环在
//! `cell.ani_frame & 0x80 != 0` 时调它、否则仍走 [`底边锚点`] ⇒ **非混合帧逐字节不变**
//! （§九 的等价单测与本节的「非混合帧」用例双向钉住）。
//!
//! ## 十.2 三格库分支：只在 **12 字节档**实现（与上游的差异，如实登记）
//!
//! 上游的 `library` 是「C# 侧前景库号」，本仓地图格里存的是**原样字节** `btArea`
//! （`mir2_map::Cell::area`），两者换算**按格式**不同：
//!
//! | 上游格式 | 前景库号 | 出处 |
//! |---|---|---|
//! | `Old12` | `btArea + 2` | `LegacyMapReader.cs:147` |
//! | `AntiHack`（12 B） | `btArea + 2` | `LegacyMapReader.cs:360` |
//! | `Shanda14` / `Shanda2012` | `btArea + 120` | `LegacyMapReader.cs:247` / `:298` |
//! | `Mir3Wemade` / `Mir3Shanda` | `btArea + 200 / + 300` | `LegacyMapReader.cs:424` / `:476` |
//!
//! 渲染层的视图档位 `mir2_map::CellFormat` **只有三档**：`Old12`（= 上面两种 12 B 格，
//! 两者都是 `+2`，见 `miredit-core/src/convert.rs` 的投影表）与 `New14` / `Ext36`；后两档
//! 里同时挤着 `Shanda14`(+120)、`Shanda2012`(+120)、`Mir3Wemade`(+200)、`Mir3Shanda`(+300)
//! 与 `CrystalCs`（格内根本没有 `area` 字节）—— **光凭 `map.format` 推不出 `library`**。
//!
//! ⇒ `Old12` 档**完整实现**三格库分支；`New14` / `Ext36` 档**只实现**帧号窗口那一支
//! （它不依赖库号，任何档位都成立），三格库分支**未覆盖**。这不是漏做，是「推不出就不猜」：
//! [`三格库分支`] 在非 12 字节档显式返回 `false`，单测
//! `混合帧_14字节档不做三格库分支` 把这条口径钉死。
//!
//! ## 十.3 语料实测（2026-11-07，只读字节，`E:\GodM2\GodotClient\maps` 61 张 / 2,760,946 格）
//!
//! | 图 | 档 | 加性混合格（有前景图号） | 落哪一支 | 本改动 |
//! |---|---|---|---|---|
//! | `0.map` | 12 | **40**（全部 `fr_img=2724` ⇒ 索引 `2723`、`area=0` ⇒ 库号 `2`） | 帧号窗口 | **位移 (−51, −113)**（帧 100×100、锚点 `(−51,−113)`；`py = y*32 − 68 − 113`） |
//! | `30.map` | 12 | 5（`area=29` ⇒ 库号 `31`；索引 817/982/1681/1895/2305） | 都不命中 | **不动**（既有输出保持） |
//! | 14 B 档 6 张 | 14 | 202（`area` 0/19/40/41/42/218） | 若按 `+120` 则全落三格库分支 | **不动**（未覆盖，见十.2） |
//!
//! ## 十.4 与 A-001 的边界（别把两条并成一条）
//!
//! A-001 管的是**非混合帧**的格栅锚点例外（`anchored && (!tile || blend)` 的 `tile` 判定、
//! `library == 28` 且非混合的特例，`MapSpriteDrawing.cs:53-55`）—— 那条要跟 `.gd` 还是跟
//! C# 仍是**产品裁定**，本单**不碰**。本节只覆盖 `front && blend` 这一条上游分支，而 `.gd`
//! 客户端**根本没有加性混合**（`map_overlay_layer.gd:96-100` 只有底边锚点，`map_view.gd` /
//! `MapLayers` 里没有 `FrontBlend` 的对应物）⇒ 混合帧这一类的唯一参照就是 C#，不存在
//! 「跟 .gd」的第二解。两者因此是**可分离**的两次裁定。

//! # 十一、A-001：非加性帧的格栅锚点（**用户裁定：跟 C#**，2026-11-07）
//!
//! 上游 `MapSpriteDrawing.cs:24,53-55@f115352`（本地快照 `_upstream/CrystalStudio/`）：
//!
//! ```text
//! 24: bool tile = (sprite.Width == 48 && sprite.Height == 32) || (sprite.Width == 96 && sprite.Height == 64);
//! 53: else if (front && library == 28 && (sprite.X != 0 || sprite.Y != 0))
//! 54: { px += sprite.X; py += sprite.Y; }
//! 55: else if (anchored && (!tile || blend)) py += 32 - sprite.Height;
//! ```
//!
//! 调用点：中层 `MapViewport.cs:268`（`anchored: true, front: false, blend: MiddleBlend`）、
//! 表层 `:287`（`anchored: true, front: true, blend: FrontBlend`）、
//! 地表 `:364-365`（总览/小地图 `anchored: false`）。
//!
//! 实现：[`非加性锚点`] 是这条算式**唯一**的实现处（内含 [`tile尺寸`] / [`前景库号`] /
//! [`左上锚点`] 的等价算式），三层循环分别传 [`层别::地表`] / [`层别::中层`] / [`层别::表层`]。
//! 加性帧（`ani_frame & 0x80`）**不进**这条路，仍走 [`混合锚点`]（A-018，一行未改）。
//!
//! ## 十一.1 与 `.gd` 的取舍（为什么改）
//!
//! 旧口径跟 Godot 客户端（中层恒左上、表层恒底边，`map_overlay_layer.gd:79,96-99`）。
//! 用户 2026-11-07 裁定「A-001 跟 C#」⇒ 96×64 的**表层 tile** 帧不再被当成"大件"钉底边
//! （这正是 A-001 的原始症状）；48×32 的 tile 帧因为 `32 - 32 == 0` 数值上不变。
//!
//! ## 十一.2 语料实测（2026-11-07，只读字节，`E:\GodM2\GodotClient\maps`）
//!
//! 61 张图 / **2,760,946 格** / 解出 **169,503 帧**（`WilSource` 真帧尺寸；扫描脚本用完已删）：
//!
//! | 分类 | 格数 | 本改动是否动像素 |
//! |---|---|---|
//! | 表层·96×64（tile、非加性） | **3** | **是**（`py: -32 → 0`，整帧下移 32 px；帧索引 3967/3987/3988） |
//! | 表层·48×32（tile、非加性） | 51,631 | 否（`32-32=0`，两条分支同址） |
//! | 表层·非 tile（非加性） | 63,341 | 否（改前改后都底边） |
//! | 表层·加性（A-018） | 228 | 否（不走本路径） |
//! | 表层·`library == 28` 特例 | **0** | 否（语料里没有「12 B 档 + `area=26` + 帧锚点非零」的格） |
//! | 中层·非 tile | **1** | **是**（240×192 帧：改前左上 `py=0` ⇒ 改后底边 `py = 32-192 = -160`） |
//! | 中层·tile | 54,299 | 否（仍左上） |
//!
//! ⇒ 真实语料里锚点**真的变**的只有 **4 格**（表层 3 + 中层 1），其余 16.9 万帧逐像素不变。
//!
//! ⚠ **中层混合帧不可达**：`MiddleBlend` 要读「中层动画帧」字节，而 `mir2_map::Cell` 没有
//! 这个字段（12 B/14 B 档 C# 侧 `MiddleFrames` 恒 0；36 B 档本仓未解析）⇒ 渲染循环传
//! [`中层帧缺省`]（恒 0 ⇒ `MiddleBlend` 恒 false）。锚点函数已按 `blend` 参数写好并有单测
//! （`非加性_中层混合帧即使tile也底边`）；视图补上该字节后无需改本模块，只需把实参换掉。

use mir2_assets::{LibRef, SpriteSource};
use mir2_map::{CellFormat, Map};

use crate::canvas::Canvas;
use crate::collision;
use crate::frames::FrameCache;
use crate::types::{
    CollisionOverlay, LayerMask, RenderError, RenderOptions, RenderStats, Rendered, Viewport,
    CELL_H, CELL_W,
};

/// 一次渲染的实际实现（已通过校验）。
pub(crate) fn render(
    map: &Map,
    src: &dyn SpriteSource,
    vp: &Viewport,
    mask: &LayerMask,
    overlay: CollisionOverlay,
    options: RenderOptions,
) -> Result<Rendered, RenderError> {
    let (width, height) = vp.pixel_size().ok_or_else(|| 校验错误(vp))?;
    let mut canvas = Canvas::new(width, height);
    let mut cache = FrameCache::new(src);
    let mut stats = RenderStats::default();

    let cells_w = vp.cells_w as i32;
    let cells_h = vp.cells_h as i32;
    // 「至少有一层**真的**往画布写了像素」的格标记（`cells_drawn` 的口径）。
    //
    // ⚠ 只认 `Canvas::blit` 的返回值，**不看「这一格扫过了没有」**：
    //   图号为 0（该层本来就没图）、帧取不到（缺帧，计 missing_*）、
    //   帧与画布无交集（整帧落在画布外）、0×0 空帧、帧落在画布内的像素全透明
    //   —— 这些一律**不算**落笔（渲染像素本来就没被改，统计不该说它画了）。
    let mut touched = vec![false; (cells_w * cells_h) as usize];

    if mask.ground {
        // ---- 地表：只铺偶偶格（map_view.gd:199）；落格 = 世界格/2，像素位 = x*48 / y*32 ----
        for dx in 0..cells_w {
            let x = vp.origin_x + dx;
            for dy in 0..cells_h {
                let y = vp.origin_y + dy;
                let Some(cell) = 取格(map, x, y) else {
                    continue; // 图外：不画也不计数
                };
                // 「只铺偶偶格」与「该层取哪一帧」在 [`层取帧`] 里只有一份：
                // 奇格返回 `None`（**不是缺帧**，不计 missing），其余格照旧
                // （`GameScene.cpp:3549-3553` / 模块文档 §一 规则 3）。
                let Some(取样) = 层取帧(层别::地表, x, y, cell, options) else {
                    continue;
                };
                match cache.get(取样.库, 取样.索引) {
                    Some(f) => {
                        // 落格算式在 [`层放置`]（地表 `anchored = false` ⇒ 永远左上，
                        // `MapViewport.cs:364-365`）。
                        let (px, py) = 层放置(
                            层别::地表,
                            dx,
                            dy,
                            map.format,
                            cell.area,
                            &取样,
                            f.width,
                            f.height,
                            f.anchor,
                        );
                        // 只有真的写下像素才算这格落笔（缺帧/空帧/全透明帧都不算）
                        if canvas.blit(&f, px, py) {
                            touched[(dx * cells_h + dy) as usize] = true;
                        }
                    }
                    None => stats.missing_ground += 1,
                }
            }
        }
    }

    if mask.middle {
        // ---- 中层：逐格（含奇格）。锚点按 C#：tile 尺寸左上、其余底边（见 [`层放置`]）----
        for dx in 0..cells_w {
            let x = vp.origin_x + dx;
            for dy in 0..cells_h {
                let y = vp.origin_y + dy;
                let Some(cell) = 取格(map, x, y) else {
                    continue;
                };
                let Some(取样) = 层取帧(层别::中层, x, y, cell, options) else {
                    continue; // 该格没有中层图
                };
                match cache.get(取样.库, 取样.索引) {
                    Some(f) => {
                        // 同上：以「真的写了像素」为准
                        let (px, py) = 层放置(
                            层别::中层,
                            dx,
                            dy,
                            map.format,
                            cell.area,
                            &取样,
                            f.width,
                            f.height,
                            f.anchor,
                        );
                        if canvas.blit(&f, px, py) {
                            touched[(dx * cells_h + dy) as usize] = true;
                        }
                    }
                    None => stats.missing_middle += 1,
                }
            }
        }
    }

    if mask.front {
        // ---- 表层：逐格。非加性帧按 C#：tile 尺寸左上、非 tile 底边（`library==28` 特例只叠锚点）----
        for dx in 0..cells_w {
            let x = vp.origin_x + dx;
            for dy in 0..cells_h {
                let y = vp.origin_y + dy;
                let Some(cell) = 取格(map, x, y) else {
                    continue;
                };
                // 取帧与混合方式都在 [`层取帧`]：门开帧偏移**只有前景层有**
                // （`options.doors_open == false` 时 [`门偏移`] 恒 0 ⇒ 索引与改动前同一个值，
                // 见模块文档 §七），加性判据是 [`表层混合`]（`Front.Frames & 0x80`，
                // 上游 `MapSpriteDrawing.cs:5@6450da90` 的 `FrontBlend`）。
                //
                // 与门偏移**正交**：门只改「取第几帧」，混合方式只看 `ani_frame` 的高位，
                // 两者互不影响（模块文档 §九）。
                let Some(取样) = 层取帧(层别::表层, x, y, cell, options) else {
                    continue; // 该格没有表层图
                };
                match cache.get(取样.库, 取样.索引) {
                    Some(f) => {
                        // 锚点在 [`层放置`]：加性帧走上游 `front && blend` 那条独立分支
                        // （§十），非加性帧仍是「tile 不位移 / 非 tile 底边」（§一 规则 6）。
                        // 二者互斥 ⇒ §十 的锚点**不会**漏到普通帧上。
                        //
                        // ⚠ 传进去的是**加过门偏移**的 `索引`：上游把
                        // `MapPreview.FrontImage(...)` 的结果同时用于取帧与帧号窗口判断
                        // （`MapViewport.cs:287`），门偏移因此参与「落不落进 2723..2732」。
                        let (px, py) = 层放置(
                            层别::表层,
                            dx,
                            dy,
                            map.format,
                            cell.area,
                            &取样,
                            f.width,
                            f.height,
                            f.anchor,
                        );
                        // 大件帧的锚点可能在画布上方，但底边总落在格底；
                        // 只有真的写了像素（非空帧、非全透明）才算落笔
                        let 落笔 = if 取样.加性 {
                            canvas.blit_additive(&f, px, py)
                        } else {
                            canvas.blit(&f, px, py)
                        };
                        if 落笔 {
                            touched[(dx * cells_h + dy) as usize] = true;
                        }
                    }
                    None => stats.missing_front += 1,
                }
            }
        }
    }

    stats.cells_drawn = touched.iter().filter(|t| **t).count() as u32;
    stats.frames_decoded = cache.decoded();

    // ---- 地图光照：三层**全部画完之后**才盖遮罩（模块文档 §八）----
    //
    // 位置照官方 `MapViewport.Presentation.cs:34-35`：遮罩画在精灵场景之后、
    // 阻挡/灯光/门标记之前（`Presentation.cs:40-50`）⇒ **标记不被压暗**。
    // `options.lighting == MapLighting::Day` 时这里立即返回，画布一个字节都不动。
    //
    // 传 `src` 只为取前景帧的**锚点偏移**（官方 `Presentation.cs:84-86` 的 `sprite.X/Y`）：
    // 光罩要它、三层合成**不要**它（`MapSpriteDrawing.cs:19-31` 里 anchor 只在特例下生效）
    // ⇒ 这个偏移不会回流到上面三个循环，三层像素与本改动前逐字节相同。
    crate::lighting::apply(&mut canvas, map, vp, mask.front, options.lighting, src);

    // ---- 阻挡标记：三层画完之后再叠（C# `MapViewport.Presentation.cs:40-42` 的位置：
    //      地图精灵之后、灯光/门标记之前）----
    //
    // 遍历口径与三层一致（列优先）；格与格不重叠，所以这里不存在层内顺序问题。
    // 图外格不画也不计数（与 §三 同一条纪律）。
    if overlay.any() {
        for dx in 0..cells_w {
            let x = vp.origin_x + dx;
            for dy in 0..cells_h {
                let y = vp.origin_y + dy;
                let Some(cell) = 取格(map, x, y) else {
                    continue; // 图外：不画也不计数
                };
                let (低, 高) = collision::draw_cell(
                    &mut canvas,
                    overlay,
                    cell,
                    dx * CELL_W as i32,
                    dy * CELL_H as i32,
                );
                // 只统计「真的画出了像素」的格（alpha == 0 时这里是 false）
                if 低 {
                    stats.cells_low_blocked += 1;
                }
                if 高 {
                    stats.cells_high_blocked += 1;
                }
            }
        }
    }

    Ok(Rendered {
        width,
        height,
        rgba: canvas.into_rgba(),
        stats,
        // 本函数只产 1× 图（`vp.scale != 1.0` 在前面就被拒了）；多倍率是
        // `scale.rs` 的后处理，它拿到 1× 结果后自己改写这个字段。
        scale: crate::types::RenderScale::One,
    })
}

/// 把 `(x, y)` 世界格取成 [`mir2_map::Map`] 的格；负坐标 / 超 `u16` 一律 `None`。
///
/// （返回值的生命周期按 elision 规则取自 `map`，与原先显式写 `'m` 等价。）
///
/// `crate::lighting` 也调它（光源可能落在视口外，同样要走「图外 = 没有」这条口径）。
#[inline]
pub(crate) fn 取格(map: &Map, x: i32, y: i32) -> Option<&mir2_map::Cell> {
    let cx = u16::try_from(x).ok()?;
    let cy = u16::try_from(y).ok()?;
    map.cell(cx, cy)
}

// ---------------------------------------------------------------------------
// 落格口径的**唯一实现处**（地图渲染与印章渲染共用，见 `stamp.rs`）
//
// 这三个函数是「像素位怎么算」的单一真相：`compose.rs` 的三层循环与
// `stamp.rs` 的印章合成都只调它们，**任何一处都不许再手写一遍同样的算式**。
// 出处与 `compose.rs` 模块文档 §一 的表格逐条对应（改这里前先回核源码）。
// ---------------------------------------------------------------------------

/// 左上角锚点：帧左上角 = `(dx*48, dy*32)`。
///
/// 出处：地表 `map_view.gd:13`（`x*48` / `y*32`）、中层 `map_overlay_layer.gd:79`
/// （「左上角锚定、居中=false」）。地表与中层**共用同一个算式**（模块文档 §一 规则 4/5）。
#[inline]
pub(crate) fn 左上锚点(dx: i32, dy: i32) -> (i32, i32) {
    (dx * CELL_W as i32, dy * CELL_H as i32)
}

/// 底边锚点：帧**底边**钉在格底，`top = (dy+1)*32 - 帧高`。
///
/// 出处：`map_overlay_layer.gd:18-19,96-99`；原作 `GameScene.cpp:3515`（模块文档 §一 规则 6）。
#[inline]
pub(crate) fn 底边锚点(dx: i32, dy: i32, 帧高: u32) -> (i32, i32) {
    (dx * CELL_W as i32, (dy + 1) * CELL_H as i32 - 帧高 as i32)
}


// ─────────────────── A-001：非加性帧的格栅锚点（`MapSpriteDrawing.cs:24,53-55`） ───────────────────

/// 帧是不是**「tile 尺寸」**：上游 `MapSpriteDrawing.cs:24@6450da90`
/// `(sprite.Width == 48 && sprite.Height == 32) || (sprite.Width == 96 && sprite.Height == 64)`。
///
/// 为什么单拎出来：这个判定在 `Bounds` 里**只出现一次**却被两条分支共用，散在循环里写成
/// `f.width == 48 && f.height == 32 || …` 迟早漏一个括号。
#[inline]
pub(crate) fn tile尺寸(帧宽: u32, 帧高: u32) -> bool {
    matches!((帧宽, 帧高), (48, 32) | (96, 64))
}

/// 中层混合：上游 `MapSpriteDrawing.cs:8@6450da90`
/// `frames != 255 && (frames & 15) is 8 or 10`。
///
/// ⚠ 本仓 `mir2_map::Cell` **没有「中层动画帧」字节**（12 B/14 B 档 C# 侧
/// `MiddleFrames` 恒 `0`；36 B 档我们只把它留在 `Cell::tail` 里没解析）⇒ 渲染循环里
/// 传 [`中层帧缺省`]，本函数在现有视图下恒 `false`。一旦视图补上该字节，锚点函数已经就绪。
#[inline]
pub(crate) fn 中层混合(frames: u8) -> bool {
    frames != 255 && matches!(frames & 15, 8 | 10)
}

/// 表层加性混合：上游 `MapSpriteDrawing.cs:5@6450da90` 的 `FrontBlend`（`frames & 0x80 != 0`）。
///
/// 与 [`中层混合`] 是两套判据（前者看高位、后者看低 4 位），**不要合并**。
#[inline]
pub(crate) fn 表层混合(frames: u8) -> bool {
    frames & 0x80 != 0
}

/// 本仓视图里**能拿到的**中层动画帧字节 —— 恒 `0`，理由见 [`中层混合`]。
pub(crate) const 中层帧缺省: u8 = 0;

/// 前景库号（C# `Foreground.Library`）在**本仓能算的档**里换算；算不出来就 `None`。
///
/// 换算表与 [`三格库分支`] 同源（模块文档 §十.2）：`Old12`/`AntiHack` 都是 `btArea + 2`
/// （`LegacyMapReader.cs:147` / `:360`），故视图档 [`CellFormat::Old12`] 能给值；
/// `New14` / `Ext36` 里同时挤着 `+120` / `+200` / `+300` / CrystalCs（无 `area`）
/// ⇒ **推不出就不猜**。
#[inline]
pub(crate) fn 前景库号(格式: CellFormat, 库号原值: u8) -> Option<i32> {
    matches!(格式, CellFormat::Old12).then(|| i32::from(库号原值) + 2)
}

/// **非加性帧**的落格锚点：上游 `MapSpriteDrawing.cs:53-55@6450da90`。
///
/// ```text
/// else if (front && library == 28 && (sprite.X != 0 || sprite.Y != 0))   // :53
/// { px += sprite.X; py += sprite.Y; }                                     // :54 ← 只叠锚点
/// else if (anchored && (!tile || blend)) py += 32 - sprite.Height;        // :55 ← 底边锚点
/// ```
///
/// # 参数（与上游一一对应）
///
/// | 参数 | 上游 | 说明 |
/// |---|---|---|
/// | `dx` / `dy` | `x` / `y` | **视口内**格坐标（乘 48/32 前） |
/// | `格式` / `库号原值` | `library` | 只有能算出前景库号的档才可能命中 `library == 28`，见 [`前景库号`] |
/// | `帧宽` / `帧高` | `sprite.Width/Height` | 决定 [`tile尺寸`] 与 `32 - Height` |
/// | `锚点` | `sprite.X/Y` | 帧头锚点（[`Frame::anchor`]） |
/// | `front` | `front` | 地表 / 中层为 `false`；表层为 `true` |
/// | `锚定` | `anchored` | 地表 `false`（`MapViewport.cs:364-365` 总览/小地图），中层/表层 `true`（`:268` / `:287`） |
/// | `混合` | `blend` | 表层：加性帧走 [`混合锚点`]（A-018），**不进本函数**；中层：[`中层混合`] |
///
/// # 三条口径（别合并）
///
/// 1. `library == 28` 分支**先于**底边锚定，且只在 `front` 上成立 ⇒ 中层/地表永不命中；
/// 2. 底边锚定条件是 `!tile || blend` —— **tile 帧默认不锚定**（96×64 也属于 tile），
///    这正是 A-001 的原始症状（96×64 表层被当成"大件"钉了底边，实际该贴左上）；
/// 3. `48×32` 的 tile 帧：`32 - 32 == 0` ⇒ 两条分支数值相同（像素不变），
///    只有 `96×64` 与「非 tile」才真的动像素。
// 9 个参数是**逐字对齐上游** `Bounds(sprite, x, y, library, image, anchored, front, blend, format)`
// 的实参表（`MapSpriteDrawing.cs:19-22`）：拆成结构体反而看不出与上游的一一对应关系。
// 与 `stamp_convert::cells_from_stamp` 同类处置（那条也是"参数就是上游签名"）。
#[allow(clippy::too_many_arguments)]
#[inline]
pub(crate) fn 非加性锚点(
    dx: i32,
    dy: i32,
    格式: CellFormat,
    库号原值: u8,
    帧宽: u32,
    帧高: u32,
    锚点: (i16, i16),
    层: 层别,
    混合: bool,
) -> (i32, i32) {
    let (ax, ay) = (i32::from(锚点.0), i32::from(锚点.1));
    let (px, mut py) = 左上锚点(dx, dy);
    // `:53-54`：`front && library == 28 && (X != 0 || Y != 0)` ⇒ 只叠锚点（不做底边锚定）
    if 层.是表层()
        && 前景库号(格式, 库号原值) == Some(28)
        && (ax != 0 || ay != 0)
    {
        return (px + ax, py + ay);
    }
    // `:55`：`anchored && (!tile || blend)` ⇒ 底边锚点（`py += 32 - 帧高`）
    if 层.锚定() && (!tile尺寸(帧宽, 帧高) || 混合) {
        py += CELL_H as i32 - 帧高 as i32;
    }
    (px, py)
}

/// 三层在 `Bounds` 里的两个开关（`anchored` / `front`），由层决定而不是调用方随手传。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum 层别 {
    /// 地表：`anchored = false`（`MapViewport.cs:364-365`）⇒ 永远左上。
    地表,
    /// 中层：`anchored = true, front = false`（`:268`）。
    中层,
    /// 表层非加性：`anchored = true, front = true`（`:287`）。
    表层,
}

impl 层别 {
    /// 上游 `front` 实参。
    #[inline]
    fn 是表层(self) -> bool {
        matches!(self, Self::表层)
    }

    /// 上游 `anchored` 实参。
    #[inline]
    fn 锚定(self) -> bool {
        !matches!(self, Self::地表)
    }
}

/// 某一层在某一格上「取哪个库的哪一帧、用哪种混合」——**三层循环与总览共用的唯一实现**。
///
/// 出自 [`层取帧`]；它把三处散落在循环里的判断（偶偶格、库路由、帧号、门偏移、
/// 加性标志）收成一份，`crate::overview` 的三层循环与 [`render`] 的三层循环都调它
/// ⇒ 「窗内小地图的总览图」和「1× 合成图」不可能各写一套落格前提。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct 层取样 {
    /// 图库路由：地表 = `btTiles` / 中层 = `btsmTiles` / 表层 = `btArea`（模块文档 §一 规则 8）。
    pub(crate) 库: LibRef,
    /// 0-based 帧号（表层**已加**门偏移，见 [`门偏移`]）。
    pub(crate) 索引: u32,
    /// 表层加性混合（[`表层混合`]）；地表 / 中层恒 `false`。
    pub(crate) 加性: bool,
    /// 非加性分支的 `blend` 实参：中层走 [`中层混合`]（本仓视图恒 `false`，见该函数文档）；
    /// 其余恒 `false`。加性分支不看它（[`混合锚点`] 不带 `blend` 参数）。
    pub(crate) 混合: bool,
}

/// 该层在这一格上取哪一帧：`None` = **该层在此格本来就没图**（不是缺帧，调用方不得计 missing）。
///
/// # 参数
///
/// | 参数 | 说明 |
/// |---|---|
/// | `层` | 三层之一；决定库路由与「地表只铺偶偶格」 |
/// | `x` / `y` | **世界格**坐标（不是视口内 `dx/dy`）：偶偶格判据用的是世界坐标 |
/// | `cell` | 该格的三个图号 / 库号 / 动画字节 |
/// | `options` | 目前只用 [`RenderOptions::doors_open`]（只影响表层帧号）|
///
/// # 三条口径（与改动前的三层循环逐条对应）
///
/// 1. **地表只铺偶偶格**：`x & 1 != 0 || y & 1 != 0` ⇒ `None`。出处 `map_view.gd:199`、
///    `GameScene.cpp:3549-3553`（模块文档 §一 规则 3）；奇格**不取格、不取帧、不计 missing**。
/// 2. **图号为 0 ⇒ `None`**（[`帧号`]）—— 空地不是缺帧（模块文档 §三）。
/// 3. **门偏移只加在表层**（[`门偏移`] 在 `doors_open == false` 时恒 0），
///    且**必须在取帧之前加**（否则取错帧、`混合锚点` 的帧号窗口也会判错）。
#[inline]
pub(crate) fn 层取帧(
    层: 层别,
    x: i32,
    y: i32,
    cell: &mir2_map::Cell,
    options: RenderOptions,
) -> Option<层取样> {
    match 层 {
        层别::地表 => {
            if x & 1 != 0 || y & 1 != 0 {
                return None; // 奇格不铺地表
            }
            Some(层取样 {
                库: LibRef::ground(u16::from(cell.tiles)),
                索引: 帧号(cell.bk_img)?,
                加性: false,
                混合: false,
            })
        }
        层别::中层 => Some(层取样 {
            库: LibRef::middle(u16::from(cell.sm_tiles)),
            索引: 帧号(cell.mid_img)?,
            加性: false,
            // 本仓视图没有中层动画帧字节 ⇒ 恒 false（见 [`中层混合`] / [`中层帧缺省`]）
            混合: 中层混合(中层帧缺省),
        }),
        层别::表层 => Some(层取样 {
            库: LibRef::surface(u16::from(cell.area)),
            索引: 帧号(cell.fr_img)? + 门偏移(cell, options),
            加性: 表层混合(cell.ani_frame),
            混合: false,
        }),
    }
}

/// 该层这一帧该贴的**像素位置**：**三层循环与总览共用的唯一实现**。
///
/// 两条分支互斥：
///
/// | 取样 | 走的算式 | 出处 |
/// |---|---|---|
/// | `加性 == true`（只有表层可能） | [`混合锚点`] | `MapSpriteDrawing.cs:43-52@6450da90` |
/// | 否则 | [`非加性锚点`]（`anchored` / `front` 由 [`层别`] 决定） | `MapSpriteDrawing.cs:53-55` |
///
/// ⚠ 参数是**上游 `Bounds(...)` 的实参表**（9 个），拆成结构体反而看不出与源码的
/// 一一对应关系；与 [`非加性锚点`] 的 `#[allow]` 同一处置。
#[allow(clippy::too_many_arguments)]
#[inline]
pub(crate) fn 层放置(
    层: 层别,
    dx: i32,
    dy: i32,
    格式: CellFormat,
    库号原值: u8,
    取样: &层取样,
    帧宽: u32,
    帧高: u32,
    锚点: (i16, i16),
) -> (i32, i32) {
    if 取样.加性 {
        混合锚点(dx, dy, 格式, 库号原值, 取样.索引, 帧高, 锚点)
    } else {
        非加性锚点(dx, dy, 格式, 库号原值, 帧宽, 帧高, 锚点, 层, 取样.混合)
    }
}

/// **前景加性混合帧**的锚点：上游 `MapSpriteDrawing.cs:43-52@6450da90` 的 `front && blend` 分支。
///
/// 与 [`底边锚点`] 的关系：非混合帧**一律**走 [`底边锚点`]，本函数只被
/// `cell.ani_frame & 0x80 != 0` 的分支调用（模块文档 §十.1）。
///
/// # 参数
///
/// | 参数 | 含义 | 上游对应 |
/// |---|---|---|
/// | `dx` / `dy` | **视口内**格坐标（与 [`底边锚点`] 同口径，乘 48/32 前） | `x` / `y` |
/// | `格式` | 视图档位，只用于判「库号能不能算」（§十.2） | —— |
/// | `库号原值` | 格内 `btArea` 原样字节 | `library`（= `btArea + 2`，见 [`三格库分支`]） |
/// | `索引` | **加过门偏移之后**的 0-based 帧号 | `image`（`MapPreview.cs:8`） |
/// | `帧高` | 解码帧高 | `sprite.Height` |
/// | `锚点` | 帧头 `(px, py)` = [`Frame::anchor`] | `sprite.X` / `sprite.Y` |
///
/// # 为什么 `索引` 必须是「加过门偏移」的那个
///
/// 上游把 `MapPreview.FrontImage(...)`（含 `doorsOpen` 时的 `DoorOffset`）一次算好后
/// 同时用于取帧和 `Bounds` 的 `image is >= 2723 and <= 2732`
/// （`MapViewport.cs:287`）⇒ 门偏移会改变「落不落进帧号窗口」。本实现与取帧用同一个值。
#[inline]
pub(crate) fn 混合锚点(
    dx: i32,
    dy: i32,
    格式: CellFormat,
    库号原值: u8,
    索引: u32,
    帧高: u32,
    锚点: (i16, i16),
) -> (i32, i32) {
    let (ax, ay) = (i32::from(锚点.0), i32::from(锚点.1));
    if 三格库分支(格式, 库号原值) {
        // 上游 `:46`：`px += X; py += 32 - 96 + Y`（96 = 三格基线常量，**不是**帧高）
        return (dx * CELL_W as i32 + ax, dy * CELL_H as i32 + 32 - 96 + ay);
    }
    // 上游 `:48-51`：其余一律「底边锚点」，只有**帧号窗口** `2723..=2732` 再叠锚点。
    //
    // 窗口按 **0-based 帧号**判（上游实参是 `MapPreview.FrontImage` 的返回值
    // `(fr_img & 0x7FFF) - 1`）⇒ 等价图号是 2724..2733。
    let (px, py) = 底边锚点(dx, dy, 帧高);
    if (2723..=2732).contains(&索引) {
        (px + ax, py + ay)
    } else {
        (px, py)
    }
}

/// 上游 `library is 14 or 27 || library is > 99 and < 199`（`MapSpriteDrawing.cs:45`）
/// 在**本仓原值空间**里的等价判据。模块文档 §十.2 是完整推导，这里只留结论与边界。
///
/// # 为什么只有 12 字节档
///
/// 上游 `library` 是 C# 侧前景库号，等于「格内 `btArea` + 格式常量」：`Old12`/`AntiHack`
/// 是 `+2`（`LegacyMapReader.cs:147` / `:360`），`Shanda14`/`Shanda2012` 是 `+120`
/// （`:247` / `:298`），`Mir3*` 是 `+200`/`+300`（`:424` / `:476`）。
/// 视图档位 [`CellFormat`] 只有三档，`New14` / `Ext36` 里同时混着 `+120` / `+200` /
/// `+300` 三种格式 ⇒ **推不出库号就不猜**，这两档直接返回 `false`（只保留不依赖库号的
/// 帧号窗口分支）。将来若要让 14/36 档也跟上，必须先让渲染层拿到真实格式
/// （例如扩 `mir2_map::Map` 的格式信息），不能在本函数里按档位硬猜。
///
/// # 边界值（单测逐条钉住）
///
/// `库号 = btArea + 2 ∈ {14, 27} ∪ (99, 199)` ⇒ 原值 `btArea ∈ {12, 25} ∪ [98, 196]`；
/// `btArea == 97`（库号 99）与 `197`（库号 199）**都不在**窗口内（上游是开区间）。
#[inline]
fn 三格库分支(格式: CellFormat, 库号原值: u8) -> bool {
    if !matches!(格式, CellFormat::Old12) {
        return false;
    }
    let Some(库号) = 前景库号(格式, 库号原值) else {
        return false;
    };
    库号 == 14 || 库号 == 27 || (库号 > 99 && 库号 < 199)
}

/// 图号 → **0-based 帧号**：`0` = 该层没有图（返回 `None`，**不算缺帧**），
/// 否则取低 15 位再减一（最高位是「不可通过」标记，不参与取图）。
///
/// 出处：`map_loader.gd:51-52`、`map_overlay_layer.gd:20`；原作 `GameScene.cpp:3575-3577`
/// （模块文档 §一 规则 7）。
///
/// # 为什么是 `pub`（2026-11-05 收口）
///
/// 本 crate 之外还有一处**必须**用同一个帧号：`miredit-gui` 导出时的扩边计算
/// （`export.rs` 的 `tile_margins`）要按「真正会画出去的那一帧」的尺寸算外溢 ——
/// 帧号算错一位就取错帧、取错尺寸，块边界上丢像素。它原先因为本函数是 `pub(crate)`
/// 取不到而**照抄过一份**（抄件注释写着「改口径必须两边一起改」）⇒ 两份必然漂。
/// 现提成 `pub` 并由本 crate 再导出（`miredit_render::帧号`），**全仓只此一份**。
///
/// 与 [`门偏移`] 同一处置：**掩码 `0x7FFF` 这种魔法数不是从公开常量推得出来的**
/// （对比 `CELL_W` / `CELL_H` —— 那种「抄了也漂不了」的算式不必共享）。
#[inline]
pub fn 帧号(img: u16) -> Option<u32> {
    let 图号 = img & 0x7FFF;
    if 图号 == 0 {
        None
    } else {
        Some(u32::from(图号 - 1))
    }
}

/// 门开帧偏移：`doors_open && (door_index & 0x7F) != 0` 时取 `door_offset & 0x7F`，
/// 其余情况（含**默认的 `doors_open == false`**）恒为 `0`。
///
/// **全仓唯一一份门偏移实现**（2026-11-05 起 `pub`，并由本 crate 再导出为
/// `miredit_render::门偏移`）。`miredit-gui` 的导出扩边（`export.rs` 的 `tile_margins`）
/// 直接调它：那边原先因为本函数是 `pub(crate)` 而**照抄过一份**，抄件注释写着
/// 「改口径两边一起改」。两份必然漂，而**漂移的症状正是「画布开门、导出闭门」**
/// —— 同一个 bug 修好一次就够了，不许再靠人工同步。
///
/// # 两个条件缺一不可
///
/// | 条件 | 为什么 |
/// |---|---|
/// | `options.doors_open` | 客户端看「运行期开合位」，本库无运行期状态 ⇒ 由调用方显式给（= 编辑器「开启门」勾选态）。关着时本函数恒 0 ⇒ 输出与改动前逐字节相同 |
/// | `(door_index & 0x7F) != 0` | 没有门的格（`door_index` 低 7 位为 0）即便 `door_offset` 非 0 也**不加**。出处：`GameScene.cpp:3367-3370` 的 `if ((btDoorIndex & 0x7F) > 0)` |
///
/// # 为什么是 `& 0x7F` 而不是整字节
///
/// `door_offset` 的高位 `0x80` 是「**运行期已开门**」标志位，不是帧偏移的一部分。
/// 全量实测 1097 张 `.map`：**43 张图 / 1,764 格**的 `door_offset` 已置 `0x80`，
/// 其中 **1,229 格**同时有前景图与非 0 门索引（**1,173 格库号 `area == 0` ⇒ `Objects`
/// 库真实存在，会真的画出来**）。照官方编辑器
/// `CrystalStudio/MapPreview.cs:6-10@901dbe2f(未入库)` 的整字节相加，
/// 这些格会加 `128 + low` ⇒ 画出完全无关的帧。逐条数据见 [`RenderOptions`] 的类型文档 §三。
///
/// # 为什么跟客户端、不跟官方编辑器
///
/// 客户端 `client/reference_full/Classes/scenes/GameScene.cpp:3367-3370`（新档 `m_MArr2`；
/// 旧档 `:3375-3378` 逐字同构）原文：
///
/// ```text
/// if ((btDoorOffset & 0x80) > 0)              // :3367 —— 门开合状态看 0x80
///     if ((btDoorIndex & 0x7F) > 0)           // :3369 —— 没有门的格不加
///         fridx = fridx + (btDoorOffset & 0x7F);   // :3370 —— 偏移只取低 7 位
/// ```
///
/// **两处都掩了位**。而官方 C# 编辑器同一处写成
/// `image + (doorsOpen && (cell.Door & 0x7F) != 0 ? cell.DoorOffset : 0)`
/// （`CrystalStudio/MapPreview.cs:6-10@901dbe2f(未入库)`）—— `DoorOffset` **整字节相加、
/// 没有掩 `0x80`**，那是它在这批数据上的偏差（会画错上面 1,229 格里的每一格）。
/// 本仓一律以**客户端字节语义**为准（与 [`CollisionOverlay`] 那节「判定按 `TMap.cpp:339`、
/// 只借 C# 的视觉样式」是同一条纪律）。
///
/// ⚠ 唯一的**有意替换**：客户端用 `door_offset & 0x80` 判「此刻开着没」，本库改用入参
/// `options.doors_open`（本库没有运行期状态；编辑器里就是菜单「视图 → 开启门」的勾选态）。
/// 即「**门开没开的判据**换成了开关，**偏移取值必须掩 0x80** 这一条照客户端」。
///
/// # 上界
///
/// [`帧号`] 最大 `0x7FFE`（32766），加 `0x7F`（127）仍在 `u32` 内，无溢出风险。
#[inline]
pub fn 门偏移(cell: &mir2_map::Cell, options: RenderOptions) -> u32 {
    if options.doors_open && (cell.door_index & 0x7F) != 0 {
        u32::from(cell.door_offset & 0x7F)
    } else {
        0
    }
}

/// 把 [`Viewport`] 的校验失败翻译成对应的 [`RenderError`]。
fn 校验错误(vp: &Viewport) -> RenderError {
    if vp.scale != 1.0 {
        return RenderError::UnsupportedScale {
            scale_milli: (vp.scale as f64 * 1000.0) as i64,
        };
    }
    if vp.cells_w == 0 || vp.cells_h == 0 {
        return RenderError::ZeroCells;
    }
    RenderError::DimensionOverflow {
        width: vp.cells_w as u64 * CELL_W as u64,
        height: vp.cells_h as u64 * CELL_H as u64,
    }
}

// ---------------------------------------------------------------------------
// 单测：**混合方式只由 `ani_frame` 的 0x80 位决定**（模块文档 §九）
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_assets::Frame;

    /// 造一张 `w×h` 的合成 `.map`：52 B 文件头 + 每格 12 B 格体（字段偏移同
    /// `old12.rs:126-139`：`fr_img` 在 4..6、`ani_frame` 在 8）。
    fn 合成图(w: u16, h: u16, 填: impl Fn(u16, u16, &mut [u8])) -> Map {
        let mut bytes = vec![0u8; 52 + 12 * (w as usize * h as usize)];
        bytes[0..2].copy_from_slice(&w.to_le_bytes());
        bytes[2..4].copy_from_slice(&h.to_le_bytes());
        for y in 0..h {
            for x in 0..w {
                let base = 52 + 12 * (y as usize * w as usize + x as usize);
                填(x, y, &mut bytes[base..base + 12]);
            }
        }
        Map::parse(&bytes).expect("合成图应能解析")
    }

    /// 一张格：`fr_img = 1`（⇒ 帧号 0），前景动画字节由 `ani` 给。
    fn 一格图(ani: u8) -> Map {
        合成图(1, 1, |_, _, 格| {
            格[4..6].copy_from_slice(&1u16.to_le_bytes()); // fr_img
            格[8] = ani; // ani_frame（`Front.Frames`）
        })
    }

    /// 定帧源：任意请求都返回同一张 48×32 半透明灰。
    ///
    /// 选半透明灰是因为它能让两条混合路径**必然分叉**：
    /// `over` 在全透明底上给 `(200,200,200,128)`，加性混合给 `(255,255,255,100)`。
    struct 定帧源(Frame);

    impl SpriteSource for 定帧源 {
        fn frame(&self, _lib: LibRef, _index: u32) -> Option<Frame> {
            Some(self.0.clone())
        }
    }

    fn 渲染一格(map: &Map, src: &dyn SpriteSource) -> (Vec<u8>, RenderStats) {
        let vp = Viewport::new(0, 0, 1, 1);
        let 只前景 = LayerMask {
            ground: false,
            middle: false,
            front: true,
        };
        let 图 = render(
            map,
            src,
            &vp,
            &只前景,
            CollisionOverlay::none(),
            RenderOptions::default(),
        )
        .expect("1×1 合成图应能渲染");
        (图.rgba, 图.stats)
    }

    /// **`ani_frame & 0x80 == 0` ⇒ 输出必须与 `blit` 逐字节相同**（防误改默认路径）。
    ///
    /// 同时钉住「低 7 位不参与混合判断」：`0x00` 与 `0x05` 都必须等于 `blit` 的结果。
    #[test]
    fn 加性混合_高位不置位时与blit逐字节相同() {
        let 帧 = Frame {
            width: 48,
            height: 32,
            rgba: [200u8, 200, 200, 128].repeat(48 * 32),
            ..Frame::default()
        };
        let src = 定帧源(帧.clone());

        // 参照：同一个帧走 `Canvas::blit`（帧 48×32 贴在 (0,0)，1×1 格 = 48×32 px）
        let mut 参照 = Canvas::new(48, 32);
        assert!(参照.blit(&帧, 0, 0));
        let 参照 = 参照.into_rgba();

        for ani in [0x00u8, 0x05] {
            let (rgba, stats) = 渲染一格(&一格图(ani), &src);
            assert_eq!(
                rgba, 参照,
                "ani_frame=0x{ani:02X}（0x80 未置位）必须与 `blit` 逐字节相同"
            );
            assert_eq!(stats.cells_drawn, 1, "ani_frame=0x{ani:02X}：落笔口径不变");
        }
    }

    /// **`ani_frame & 0x80 != 0` ⇒ 走加性混合**，且结果等于 `Canvas::blit_additive`。
    #[test]
    fn 加性混合_高位置位时走加性() {
        let 帧 = Frame {
            width: 48,
            height: 32,
            rgba: [200u8, 200, 200, 128].repeat(48 * 32),
            ..Frame::default()
        };
        let src = 定帧源(帧.clone());

        let mut 参照 = Canvas::new(48, 32);
        assert!(参照.blit_additive(&帧, 0, 0));
        let 参照 = 参照.into_rgba();

        let (rgba, stats) = 渲染一格(&一格图(0x80), &src);
        assert_eq!(rgba, 参照, "0x80 置位必须走 `blit_additive`");
        assert_eq!(stats.cells_drawn, 1, "落笔口径不变（加性也真的写了像素）");
        // 与默认路径必须不同（否则本用例对分派不敏感 ⇒ 假通过）
        let mut 默认 = Canvas::new(48, 32);
        assert!(默认.blit(&帧, 0, 0));
        assert_ne!(
            rgba,
            默认.into_rgba(),
            "同一帧走加性混合必须与覆盖混合不同（半透明灰源在全透明底上必然分叉）"
        );
        println!("[加性] 0x80 置位：首像素 {:?}", &rgba[0..4]);
    }

    // -----------------------------------------------------------------------
    // §十 前景加性混合帧的**落格锚点**（A-018）：上游 `MapSpriteDrawing.cs:43-52`
    // -----------------------------------------------------------------------
    //
    // 这一组用例的共同手法：用「只有一个不透明像素的 16×8 帧」把「帧落在哪」化成
    // **一个像素的坐标断言** —— 锚点差多少 px，断言就红在哪一格，比逐字节 diff 好读。

    /// 一格可定点：前景图号 / 动画字节 / 前景库号原值（`area`）都由调用方给。
    fn 指定格图(fr_img: u16, ani: u8, area: u8) -> Map {
        合成图(1, 1, |_, _, 格| {
            格[4..6].copy_from_slice(&fr_img.to_le_bytes()); // fr_img
            格[8] = ani; // ani_frame
            格[10] = area; // btArea（前景库号原值，C# 侧 +2）
        })
    }

    /// 14 字节档（`New14`）的同款合成图：前 12 字节字段位与 12 档逐字节相同，只是步长 14。
    fn 合成图14(fr_img: u16, ani: u8, area: u8) -> Map {
        let mut bytes = vec![0u8; 52 + 14];
        bytes[0..2].copy_from_slice(&1u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&1u16.to_le_bytes());
        let 格 = &mut bytes[52..66];
        格[4..6].copy_from_slice(&fr_img.to_le_bytes());
        格[8] = ani;
        格[10] = area;
        let map = Map::parse(&bytes).expect("14 字节档合成图应能解析");
        assert_eq!(
            map.format,
            CellFormat::New14,
            "夹具必须真的是 14 字节档，否则下面的档位口径用例是空转"
        );
        map
    }

    /// 16×8 单点帧：只有 `(7,3)` 一个不透明红点，其余全透明；锚点由调用方给。
    fn 单点帧(锚点: (i16, i16)) -> Frame {
        let (w, h) = (16u32, 8u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let i = ((3 * w + 7) * 4) as usize;
        rgba[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
        Frame {
            width: w,
            height: h,
            rgba,
            anchor: 锚点,
        }
    }

    /// 渲染 1×1 视口（画布 48×32），只开前景层；`options` 由调用方给。
    fn 渲染一格带选项(
        map: &Map,
        src: &dyn SpriteSource,
        options: RenderOptions,
    ) -> (Vec<u8>, RenderStats) {
        let 只前景 = LayerMask {
            ground: false,
            middle: false,
            front: true,
        };
        let 图 = render(
            map,
            src,
            &Viewport::new(0, 0, 1, 1),
            &只前景,
            CollisionOverlay::none(),
            options,
        )
        .expect("1×1 合成图应能渲染");
        (图.rgba, 图.stats)
    }

    /// 取画布上 `(x, y)` 的 RGBA（越界返回 `None`；画布宽固定 48）。
    fn 像素(rgba: &[u8], x: i32, y: i32) -> Option<[u8; 4]> {
        const 宽: i32 = 48;
        let 高 = (rgba.len() / 4) as i32 / 宽;
        if x < 0 || y < 0 || x >= 宽 || y >= 高 {
            return None;
        }
        let i = ((y * 宽 + x) * 4) as usize;
        Some([rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]])
    }

    /// 「这一格画了东西」的红点在不在 `(x, y)`。
    fn 红点在(rgba: &[u8], x: i32, y: i32) -> bool {
        像素(rgba, x, y) == Some([255, 0, 0, 255])
    }

    /// **A-018 主用例**：帧号窗口 `2723..=2732` 的混合帧必须叠上帧锚点。
    ///
    /// 夹具对齐 `0.map` 的实测格（`fr_img=2724` ⇒ 索引 `2723`、`area=0` ⇒ 库号 `2`），
    /// 但把帧缩成 16×8、锚点换成 `(-5,-6)` 以便画布装得下：
    ///
    /// ```text
    /// 改前（底边锚点）：px = 0,            py = 32 - 8 = 24  ⇒ 红点 (7, 27)
    /// 改后（叠锚点）  ：px = 0 + (-5) = -5, py = 24 + (-6) = 18 ⇒ 红点 (2, 21)
    /// ```
    /// 两个位置互不重叠 ⇒ 断言对「加没加锚点」敏感，不是假通过。
    #[test]
    fn 混合帧_帧号窗口内必须叠上帧锚点() {
        let src = 定帧源(单点帧((-5, -6)));
        for (fr_img, 落窗口) in [(2724u16, true), (2733, true), (2723, false), (2734, false)] {
            let (rgba, stats) = 渲染一格带选项(
                &指定格图(fr_img, 0x80, 0),
                &src,
                RenderOptions::closed(),
            );
            let 索引 = (fr_img & 0x7FFF) - 1;
            if 落窗口 {
                assert!(
                    红点在(&rgba, 2, 21),
                    "fr_img={fr_img}（索引 {索引}）在窗口 2723..=2732 内 ⇒ 必须叠 (-5,-6) 锚点到 (2,21)"
                );
                assert!(
                    !红点在(&rgba, 7, 27),
                    "fr_img={fr_img}（索引 {索引}）：不能同时又落在**没叠锚点**的位置 (7,27)"
                );
            } else {
                assert!(
                    红点在(&rgba, 7, 27),
                    "fr_img={fr_img}（索引 {索引}）在窗口外 ⇒ 只走底边锚点，红点应在 (7,27)"
                );
                assert!(
                    !红点在(&rgba, 2, 21),
                    "fr_img={fr_img}（索引 {索引}）：窗口外不许叠锚点"
                );
            }
            assert_eq!(stats.cells_drawn, 1, "fr_img={fr_img}：落笔口径不变");
        }
    }

    /// **12 字节档的三格库分支**：上游 `library is 14 or 27 || > 99 and < 199`。
    ///
    /// 原值 `btArea = 库号 - 2` ⇒ 命中 `{12, 25} ∪ [98, 196]`；边界 `97`(库号 99)、
    /// `197`(库号 199) 是**开区间外侧**，必须不命中。
    ///
    /// 夹具锚点取 `(5, 70)`（三格基线 `py = 0 + 32 - 96 + 70 = 6`）：红点落在 `(12, 9)`；
    /// 不走这一支时红点在 `(7, 27)`（`py = 32 - 8 = 24`）—— 两处不重叠。
    /// 图号取 `101`（索引 `100`）**故意在帧号窗口外**，好让「是不是库号驱动的」一目了然。
    #[test]
    fn 混合帧_12字节档三格库分支命中与边界() {
        let src = 定帧源(单点帧((5, 70)));
        for (area, 命中) in [
            (12u8, true),  // 库号 14
            (25, true),    // 库号 27
            (98, true),    // 库号 100（开区间内侧）
            (196, true),   // 库号 198（开区间内侧）
            (97, false),   // 库号 99（区间外）
            (197, false),  // 库号 199（区间外）
            (0, false),    // 库号 2（= 0.map 那 40 格的真实值）
        ] {
            let (rgba, _) = 渲染一格带选项(
                &指定格图(101, 0x80, area),
                &src,
                RenderOptions::closed(),
            );
            let 库号 = u16::from(area) + 2;
            if 命中 {
                assert!(
                    红点在(&rgba, 12, 9),
                    "area={area}（库号 {库号}）命中三格库分支 ⇒ 红点应在 (12,9)"
                );
                assert!(!红点在(&rgba, 7, 27), "area={area}：不能同时落在底边锚点位置");
            } else {
                assert!(
                    红点在(&rgba, 7, 27),
                    "area={area}（库号 {库号}）不在 `14 / 27 / (99,199)` 内 ⇒ 只走底边锚点"
                );
                assert!(!红点在(&rgba, 12, 9), "area={area}：不该吃到三格基线");
            }
        }
    }

    /// **14 字节档不做三格库分支**（模块文档 §十.2 的口径闹钟）。
    ///
    /// `New14` 视图档位里混着 `Shanda14`(+120) / `Mir3*`(+200/+300) / `CrystalCs`，
    /// 推不出 C# 库号 ⇒ 只保留不依赖库号的帧号窗口分支，三格库分支**不实现**。
    /// 若哪天打通了 14 档的库号换算，**本用例必须一起改**（届时它会红，这是设计好的）。
    #[test]
    fn 混合帧_14字节档不做三格库分支() {
        let src = 定帧源(单点帧((5, 70)));
        let (rgba, _) = 渲染一格带选项(&合成图14(101, 0x80, 12), &src, RenderOptions::closed());

        assert!(
            红点在(&rgba, 7, 27),
            "14 字节档未实现三格库分支 ⇒ 红点应在底边锚点位置 (7,27)"
        );
        assert!(
            !红点在(&rgba, 12, 9),
            "14 字节档**不许**按 12 档的 +2 口径吃三格基线（库号推不出来就别猜）"
        );
    }

    /// **帧号窗口用的是「加过门偏移之后」的索引**（模块文档 §十 口径 2）。
    ///
    /// 上游把 `MapPreview.FrontImage(...)`（含 `doorsOpen` 时的 `DoorOffset`）一次算好后
    /// 同时用于取帧与 `image is >= 2723 and <= 2732`（`MapViewport.cs:287`）⇒
    /// 同一个格：闭门时索引 `2623`（窗口外）、开门后索引 `2623+100 = 2723`（窗口内）。
    #[test]
    fn 混合帧_门偏移会改变是否落进帧号窗口() {
        let src = 定帧源(单点帧((-5, -6)));
        let 格 = 合成图(1, 1, |_, _, 格| {
            格[4..6].copy_from_slice(&2624u16.to_le_bytes()); // fr_img ⇒ 索引 2623
            格[6] = 1; // door_index & 0x7F != 0
            格[7] = 100; // door_offset & 0x7F = 100
            格[8] = 0x80; // ani_frame：混合
            格[10] = 0; // area ⇒ 库号 2
        });
        let (闭门, _) = 渲染一格带选项(&格, &src, RenderOptions::closed());
        assert!(
            红点在(&闭门, 7, 27),
            "闭门：索引 2623 在窗口外 ⇒ 只走底边锚点"
        );
        let (开门, _) = 渲染一格带选项(&格, &src, RenderOptions::all_doors_open());
        assert!(
            红点在(&开门, 2, 21),
            "开门：索引 2623+100=2723 落进窗口 ⇒ 必须叠 (-5,-6) 锚点"
        );
    }

    /// **非混合帧一律不加锚点**（A-002 已复核输出的回归钉子）。
    ///
    /// 即使库号命中三格分支、帧锚点非零，只要 `ani_frame & 0x80 == 0`，落格就必须与
    /// **改动前**逐字节相同：这里的参照是 `Canvas::blit` 在 [`底边锚点`] 位置的输出。
    #[test]
    fn 非混合帧_带非零锚点也不加锚点() {
        let 帧 = 单点帧((5, 70));
        let src = 定帧源(帧.clone());

        let mut 参照 = Canvas::new(48, 32);
        assert!(参照.blit(&帧, 0, 32 - 8));
        let 参照 = 参照.into_rgba();

        // area=12（库号 14）在 12 档里**是**三格库分支的命中值 —— 但非混合帧不吃它。
        for (ani, 说明) in [(0x00u8, "无动画字节"), (0x05, "半透明帧，低位非 0")] {
            let (rgba, stats) =
                渲染一格带选项(&指定格图(101, ani, 12), &src, RenderOptions::closed());
            assert_eq!(
                rgba, 参照,
                "ani=0x{ani:02X}（{说明}）非混合 ⇒ 必须与 `blit` 在底边锚点位置逐字节相同"
            );
            assert_eq!(stats.cells_drawn, 1, "ani=0x{ani:02X}：落笔口径不变");
        }
    }

    // -----------------------------------------------------------------------
    // §十一 A-001：非加性帧的格栅锚点（`MapSpriteDrawing.cs:24,53-55`，跟 C#）
    // -----------------------------------------------------------------------
    //
    // 上游原文（`_upstream/CrystalStudio/MapSpriteDrawing.cs`）：
    //   24: bool tile = (sprite.Width == 48 && sprite.Height == 32) || (sprite.Width == 96 && sprite.Height == 64);
    //   53: else if (front && library == 28 && (sprite.X != 0 || sprite.Y != 0))
    //   54: { px += sprite.X; py += sprite.Y; }
    //   55: else if (anchored && (!tile || blend)) py += 32 - sprite.Height;
    // 调用点：中层 `MapViewport.cs:268`（anchored:true, front:false, blend=MiddleBlend）、
    //        表层 `:287`（anchored:true, front:true, blend=FrontBlend）、
    //        地表 `:364-365`（总览/小地图 anchored:false）。

    /// 任意尺寸的**单点帧**：只有 `(7,3)` 一个不透明红点，其余全透明。
    fn 单点帧尺寸(w: u32, h: u32, 锚点: (i16, i16)) -> Frame {
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let i = ((3 * w + 7) * 4) as usize;
        rgba[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
        Frame {
            width: w,
            height: h,
            rgba,
            anchor: 锚点,
        }
    }

    /// 指定中层图号的 1×1 合成图（12 B 档：`mid_img` 在 2..4）。
    fn 指定中层图(mid_img: u16) -> Map {
        合成图(1, 1, |_, _, 格| {
            格[2..4].copy_from_slice(&mid_img.to_le_bytes()); // mid_img
        })
    }

    /// 只开某一层渲染 1×1 视口（画布 48×32）。
    fn 渲染一格单层(map: &Map, src: &dyn SpriteSource, 层: 层别) -> Vec<u8> {
        let mask = match 层 {
            层别::地表 => LayerMask {
                ground: true,
                middle: false,
                front: false,
            },
            层别::中层 => LayerMask {
                ground: false,
                middle: true,
                front: false,
            },
            层别::表层 => LayerMask {
                ground: false,
                middle: false,
                front: true,
            },
        };
        render(
            map,
            src,
            &Viewport::new(0, 0, 1, 1),
            &mask,
            CollisionOverlay::none(),
            RenderOptions::default(),
        )
        .expect("1×1 合成图应能渲染")
        .rgba
    }

    /// **① 表层 96×64（tile）⇒ 左上；改前（底边）会跌到画布外**。
    ///
    /// 改前：`py = 32 - 64 = -32` ⇒ 红点 `(7, -29)`（画布外，看不见）；
    /// 改后：`py = 0` ⇒ 红点 `(7, 3)`。两个位置**一个可见一个不可见** ⇒ 断言对锚点敏感。
    #[test]
    fn 非加性_表层96x64是tile不位移() {
        assert!(tile尺寸(96, 64), "96×64 必须是 tile");
        // 改前的位置确实在画布外（这就是 A-001 的原始症状：大件 tile 被钉了底边）
        let 旧 = 底边锚点(0, 0, 64);
        assert_eq!(旧.1, -32, "改前 py 应是 -32");
        assert!(旧.1 + 3 < 0, "改前红点 y = {} 在画布外", 旧.1 + 3);

        let src = 定帧源(单点帧尺寸(96, 64, (0, 0)));
        let rgba = 渲染一格单层(
            &合成图(1, 1, |_, _, 格| 格[4..6].copy_from_slice(&1u16.to_le_bytes())),
            &src,
            层别::表层,
        );
        assert!(红点在(&rgba, 7, 3), "tile 表层必须贴左上（红点 (7,3)）");
    }

    /// **② 表层 48×32（tile）⇒ 左上**（数值上等价于改前的底边：`32 - 32 == 0`）。
    #[test]
    fn 非加性_表层48x32是tile不位移() {
        assert!(tile尺寸(48, 32));
        assert_eq!(底边锚点(0, 0, 32), 左上锚点(0, 0), "48×32 两条分支数值相同");
        let src = 定帧源(单点帧尺寸(48, 32, (0, 0)));
        let rgba = 渲染一格单层(
            &合成图(1, 1, |_, _, 格| 格[4..6].copy_from_slice(&1u16.to_le_bytes())),
            &src,
            层别::表层,
        );
        assert!(红点在(&rgba, 7, 3), "48×32 表层仍是左上");
    }

    /// **③ 表层非 tile（100×100）⇒ 底边锚点**（这条与改前**相同**，是回归）。
    #[test]
    fn 非加性_表层非tile走底边() {
        assert!(!tile尺寸(100, 100));
        let 落 = 非加性锚点(0, 0, CellFormat::Old12, 0, 100, 100, (0, 0), 层别::表层, false);
        assert_eq!(落, (0, 32 - 100), "非 tile 必须底边锚定（py = 32-100）");
    }

    /// **④ 中层 48×32 ⇒ 左上**（改前我方恒左上；C# 的 tile 分支也左上 ⇒ 不变）。
    #[test]
    fn 非加性_中层48x32贴左上() {
        let src = 定帧源(单点帧尺寸(48, 32, (0, 0)));
        let rgba = 渲染一格单层(&指定中层图(1), &src, 层别::中层);
        assert!(红点在(&rgba, 7, 3), "中层 tile 必须贴左上");
    }

    /// **⑤ 中层非 tile（16×8）⇒ 底边锚点**（改前我方是左上 ⇒ **真的换了像素**）。
    ///
    /// 改前：红点 `(7, 3)`；改后：`py = 32 - 8 = 24` ⇒ 红点 `(7, 27)`。
    #[test]
    fn 非加性_中层非tile走底边() {
        let src = 定帧源(单点帧尺寸(16, 8, (0, 0)));
        let rgba = 渲染一格单层(&指定中层图(1), &src, 层别::中层);
        assert!(红点在(&rgba, 7, 27), "中层非 tile 必须底边锚定（红点 (7,27)）");
        assert!(!红点在(&rgba, 7, 3), "改前的左上位置必须空了（否则本用例对锚点不敏感）");
    }

    /// **⑥ 中层 `MiddleBlend`（frames=8/10）⇒ 即使 tile 也底边**。
    ///
    /// 用 96×64 的 tile：`blend` 成立时 `py = 32 - 64 = -32`，与左上（0）**差 32 px**。
    /// ⚠ 本仓视图没有中层动画帧字节 ⇒ 渲染循环传 [`中层帧缺省`]（恒 0），
    /// 所以这条在**函数层**验证；视图补上该字节后无需改动本函数。
    #[test]
    fn 非加性_中层混合帧即使tile也底边() {
        assert!(中层混合(8) && 中层混合(10), "MiddleBlend: (frames & 15) is 8 or 10");
        assert!(!中层混合(0), "0 不是混合");
        assert!(!中层混合(9), "9 不是混合");
        assert!(!中层混合(255), "255 是哨兵，不是混合");
        assert!(!中层混合(0x80), "只有高位（低 4 位 = 0）不触发");
        assert!(
            中层混合(0x80 | 8),
            "低 4 位 = 8 ⇒ 仍触发：上游只看低 4 位，高位既不加也不减"
        );
        let 混合 = 中层混合(8);
        let 落 = 非加性锚点(0, 0, CellFormat::Old12, 0, 96, 64, (0, 0), 层别::中层, 混合);
        assert_eq!(落, (0, -32), "混合帧即使 tile 也要底边（py = 32-64）");
        assert_ne!(落, 左上锚点(0, 0), "必须与左上不同，否则本用例白测");
    }

    /// **⑦ 地表 96×64 ⇒ 仍左上**（上游地表 `anchored: false`）。
    #[test]
    fn 非加性_地表不锚定() {
        let src = 定帧源(单点帧尺寸(96, 64, (0, 0)));
        let rgba = 渲染一格单层(
            &合成图(1, 1, |_, _, 格| 格[0..2].copy_from_slice(&1u16.to_le_bytes())), // bk_img
            &src,
            层别::地表,
        );
        assert!(红点在(&rgba, 7, 3), "地表永远左上（anchored:false）");
        let 落 = 非加性锚点(0, 0, CellFormat::Old12, 0, 96, 64, (0, 0), 层别::地表, true);
        assert_eq!(落, 左上锚点(0, 0), "地表连 blend 都不该影响锚点");
    }

    /// **⑧ 加性路径回归（A-018）**：`ani_frame & 0x80` 的帧**仍走** [`混合锚点`]，
    /// 逐字节等于在该位置的 `Canvas::blit_additive`；即使帧是 tile（96×64）也仍底边。
    #[test]
    fn 加性路径不受本改动影响() {
        let 帧 = 单点帧尺寸(96, 64, (0, 0));
        let src = 定帧源(帧.clone());
        let map = 合成图(1, 1, |_, _, 格| {
            格[4..6].copy_from_slice(&1u16.to_le_bytes()); // fr_img = 1 ⇒ 帧号 0
            格[8] = 0x80; // FrontBlend
        });
        // A-018 的锚点算式（库号 2 ⇒ 不命中三格库分支、帧号 0 不在窗口 ⇒ 纯底边）
        let 期望锚点 = 混合锚点(0, 0, CellFormat::Old12, 0, 0, 64, (0, 0));
        assert_eq!(期望锚点, (0, 32 - 64), "加性帧仍是底边（A-018 不动）");
        let mut 参照 = Canvas::new(48, 32);
        // 帧顶在 -32 ⇒ 只有 y ∈ [32,64) 的帧行可见（红点在帧内 (7,3)，这里看不见）
        assert!(!参照.blit_additive(&帧, 期望锚点.0, 期望锚点.1), "该位置整帧在画布外");
        let rgba = 渲染一格单层(&map, &src, 层别::表层);
        assert_eq!(
            rgba,
            参照.into_rgba(),
            "加性路径必须与「同一帧、同一锚点、blit_additive」逐字节相同"
        );
    }

    /// **`library == 28` 特例**（`MapSpriteDrawing.cs:53-54`）：只叠帧锚点，**不做底边锚定**。
    ///
    /// 库号 28 ⇔ `area = 26`（12 B 档 `+2`）；锚点 `(5, 7)` ⇒ 落点 `(5, 7)`，
    /// 而普通非 tile 的底边锚点会是 `(0, 32-帧高)`。
    #[test]
    fn 非加性_库28特例只叠锚点() {
        assert_eq!(前景库号(CellFormat::Old12, 26), Some(28));
        assert_eq!(前景库号(CellFormat::New14, 26), None, "14 档推不出库号 ⇒ 不猜");
        let 落 = 非加性锚点(0, 0, CellFormat::Old12, 26, 16, 8, (5, 7), 层别::表层, false);
        assert_eq!(落, (5, 7), "库 28 且锚点非零 ⇒ 只叠锚点");
        // 锚点全 0 ⇒ 回到普通分支（底边）
        let 落0 = 非加性锚点(0, 0, CellFormat::Old12, 26, 16, 8, (0, 0), 层别::表层, false);
        assert_eq!(落0, (0, 32 - 8), "锚点为 0 ⇒ 不命中 :53 的条件");
        // 中层/地表即使库号 28 也不命中（`:53` 要求 front）
        let 中 = 非加性锚点(0, 0, CellFormat::Old12, 26, 16, 8, (5, 7), 层别::中层, false);
        assert_eq!(中, (0, 32 - 8), "中层不许命中 front 专有分支");
        let 地 = 非加性锚点(0, 0, CellFormat::Old12, 26, 16, 8, (5, 7), 层别::地表, false);
        assert_eq!(地, (0, 0), "地表永远左上");
    }
}

