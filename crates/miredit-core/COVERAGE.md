# 覆盖度盘点：`miredit-gui/src/edit.rs` → `miredit-core`

> 盘点日期：2026-11-05（系统时间 `2026-11-05 00:0x`）。
> 对象：`crates/miredit-gui/src/edit.rs`（**4,280 行**，其中实现 1–2086 行、测试 2089–4280 行）
> 对照：`crates/miredit-core/src/**`（lib 83 条 + 集成 15 条 = **98 条**测试）
> 口径：**先盘点、后动手**。本文件是迁移前的判据；迁移完成后在文末追加"迁移结果"。

---

## 〇、一句话结论

**算法层（`bit15` 矩阵 / 填充 BFS / 撤销 / 原子保存 / 命令）core 已经 100% 覆盖**，
**薄适配层可以直接建**。但有三处**卡住**，必须先裁定才能动 `edit.rs`：

| 编号 | 卡点 | 影响 | 建议 |
|---|---|---|---|
| **P1** | `miredit_core::Tool` 缺 `from_index` / `index` / `name` | `screen_text.rs:145` 的 `ed.tool().name()` 编译不过（该文件**不在白名单**） | 允许我在 `core/src/paint.rs` 补 3 个方法 + 5 条工具名常量 |
| **P2** | `core::EditSession::footprint` 只吃自己的地图尺寸；`pick_cell` 只吃自己的地图 | gui 的 `footprint(x,y,w,h)` / `pick_cell(&Map,x,y)` 是 `&self` + 外部 `&Map`，**无法**把宿主地图换进内核 | 允许我在 `core/src/brush.rs` 抽出两个纯函数 `footprint_of` / `pick_cell_of`（session 转调，行为不变）；否则 gui 侧要各抄一份（共 ~18 行） |
| **P3** | `Family` / `LibRef` 改成 `pub use mir2_assets::{…}` | `Family::label()` 会从「地表图层/中层图层/物体」变成「地面/中层/表层」⇒ **4 条测试的逐字文案断言必红**（core `族名与层名同源`、core `拒绝原因可上屏`、gui `族不匹配拒绝且一字节不动`、gui `阻挡层拒绝图库笔刷`） | **建议本条不做**（详见 §五） |

另有两条**验收门本身不可达**，见 §六。

---

## 一、`edit.rs` 的 public 项逐条判定

判定口径：
- ✅ **完全等价**：core 有同名同义项，可直接 `pub use`；
- ⚠️ **有但要适配**：core 有，但类型/签名/归属不同，需要在 gui 侧包一层；
- ❌ **没有**：core 没有，需要补进 core 或明确留在 gui。

### 1.1 `EditLayer`（gui `edit.rs:127-360`）

| # | gui 项 | core 位置 | 判定 |
|---|---|---|---|
| 1 | `enum EditLayer { Ground, Middle, Front, LowCollision, HighCollision }` | `layer.rs:89` | ✅ 变体名/顺序/`Default`/`PartialEq` 全同（core 另加 `Hash`） |
| 2 | `EditLayer::ALL: [EditLayer; 5]` | `layer.rs:105` | ✅ |
| 3 | `EditLayer::IMAGE: [EditLayer; 3]` | `layer.rs:117` | ✅ |
| 4 | `row()` | `layer.rs:120` | ✅ |
| 5 | `from_row()` | `layer.rs:131` | ✅ |
| 6 | `is_collision()` | `layer.rs:143` | ✅ |
| 7 | `family() -> Option<Family>` | `layer.rs:150` | ⚠️ 返回 **core 的** `Family`（gui 现用 `mir2_assets::Family`） |
| 8 | `pick_family() -> Family` | `layer.rs:164` | ⚠️ 同上 |
| 9 | `name()` | `layer.rs:173` | ✅ **值逐字相同**（core 自带 `NAME_*` 常量）；⚠️ 文案源从 `tx::LAYER_NAMES` 变成 core 常量（已由 core 测试 `层名与界面文案同源` 钉住） |
| 10 | `key()` | `layer.rs:188` | ✅（core 另加逆映射 `from_key`） |
| 11 | `img_field()` | `layer.rs:213` | ✅ |
| 12 | `lib_field()` | `layer.rs:222` | ✅ |
| 13 | `writes_lib_byte()` | `layer.rs:232` | ✅ |
| 14 | `img_of()` | `layer.rs:240` | ✅ |
| 15 | `img_of_has_collision_bit()` | `layer.rs:257` | ✅ |
| 16 | `set_img()` | `layer.rs:268` | ✅ |
| 17 | `lib_byte_of()` | `layer.rs:282` | ✅ |
| 18 | `set_lib_byte()` | `layer.rs:294` | ✅ |

### 1.2 常量 / 自由函数

| # | gui 项 | core 位置 | 判定 |
|---|---|---|---|
| 19 | `fn family_name(Family) -> &'static str`（`edit.rs:366`） | 无同名；`Family::label()` 值相同 | ⚠️ **留 gui**：gui 版走 `palette::family_label`（`tx::LAYER_GROUND` 等），与 core 常量逐字相同；gui 测试 `层名与族名同源` 依赖它 |
| 20 | `TOOL_BRUSH_IDX = 0`（`:373`） | 无 | ❌ **留 gui**：`icons::TOOLS` 的图标下标，纯 UI 布局常量 |
| 21 | `TOOL_BUCKET_IDX = 2`（`:378`） | 无 | ❌ 留 gui（同上；`app.rs:343` 直接用） |
| 22 | `TOOL_ERASER_IDX = 3`（`:380`） | 无 | ❌ 留 gui |
| 23 | `TOOL_PICKUP_IDX = 6`（`:382`） | 无 | ❌ 留 gui |
| 24 | `COLLISION_BIT = 0x8000` | `layer.rs:54` | ✅ |
| 25 | `SPRITE_MAX = 0x7FFF` | `layer.rs:60` | ✅ |
| 26 | `fn palette_index_to_sprite(u32) -> Option<u16>`（`:471`） | `brush.rs:149` | ✅ |
| 27 | `fn index_of(&Map,x,y) -> Option<usize>`（`:1329`） | `document.rs:143` | ✅ |
| 28 | `fn cell_at(&Map,x,y) -> Option<&Cell>`（`:1337`） | `document.rs:152` | ✅ |
| 29 | `fn cell_mut(&mut Map,x,y) -> Option<&mut Cell>`（`:1343`） | `document.rs:160` | ✅ |
| 30 | `fn lib_ref_of(EditLayer,u16) -> LibRef`（`:1352`） | `brush.rs:134` | ⚠️ 返回 **core 的** `LibRef` |
| 31 | `fn fill_key(EditLayer,&Cell) -> FillKey`（`:1559`） | `fill.rs:106` | ✅ |
| 32 | `fn line_cells((u16,u16),(u16,u16)) -> Vec<(u16,u16)>`（`:1640`） | `session.rs:717` | ✅ |
| 33 | `fn byte_diff(&Map,&[u8]) -> ByteDiff`（`:1690`） | `document.rs:190` | ✅ |
| 34 | `fn lib_text(Option<CellFormat>,u8) -> String`（`:2059`） | 无 | ❌ **留 gui**：纯界面文案（`Old12` 档显示 `—`） |
| 35 | `fn brush_lib_text(Option<CellFormat>,&Brush) -> String`（`:2071`） | 无 | ❌ 留 gui（同上；`app.rs:449/1043/1224` 直接用） |
| 36 | `fn dirty_text(bool) -> &'static str`（`:2079`） | 无 | ❌ 留 gui（`VAL_YES`/`VAL_NO`） |
| 37 | `fn print_check(&DocState,Option<&[u8]>,&Path)`（`:2017`） | 无 | ❌ 留 gui（`DocState` + `tx::DIAG_EDIT_CMP_*` 打印） |

### 1.3 `Tool`（`:394-455`）

| # | gui 项 | core 位置 | 判定 |
|---|---|---|---|
| 38 | `enum Tool { Brush, Bucket, Eraser, Pickup, Unwired }` | `paint.rs:46` | ✅ 变体名/顺序/`Default` 全同 |
| 39 | `Tool::from_index(usize)`（`:411`） | 无 | ❌ **需补 core**（见 P1） |
| 40 | `Tool::index()`（`:422`） | 无 | ❌ **需补 core**（见 P1） |
| 41 | `Tool::name()`（`:433`） | 无（core 只有 `key()`） | ❌ **需补 core**：`screen_text.rs:145` 的 `ed.tool().name()` 依赖它，而 `screen_text.rs` **不在白名单** |
| 42 | `Tool::is_wired()` | `paint.rs:63` | ✅ |
| 43 | `Tool::uses_brush_size()` | `paint.rs:88` | ✅ |

### 1.4 笔刷 / 取格 / 变更 / 结果

| # | gui 项 | core 位置 | 判定 |
|---|---|---|---|
| 44 | `struct Brush { family, img, lib_byte, lib, from }` | `brush.rs:169` | ✅ 字段名/顺序/语义全同；⚠️ `family`/`lib` 的类型变成 core 的 |
| 45 | `Brush::sprite()` | `brush.rs:191` | ✅ |
| 46 | `Brush::blocked()` | `brush.rs:196` | ✅ |
| 47 | `Brush::family_name()` | `brush.rs:201` | ✅（值同 `family_name(Family)`） |
| 48 | `struct PickedCell { x, y, bk_img, mid_img, fr_img }` | `brush.rs:211` | ✅ |
| 49 | `PickedCell::img_of(EditLayer)` | `brush.rs:226` | ✅ |
| 50 | `struct Change { x, y, before, after }` | `undo.rs:42` | ✅ |
| 51 | `struct Step { seq, changes }` | `undo.rs:55` | ✅（core 另派生 `PartialEq`） |
| 52 | `UNDO_LIMIT = 100` | `undo.rs:63` | ✅ |
| 53 | `enum PaintOutcome { Write, NoOp, LayerMismatch, PaletteBrushOnCollision }` | `paint.rs:95` | ✅ |
| 54 | `fn paint(fmt, cell, tool, brush, layer) -> PaintOutcome`（gui **私有**，`:1393`） | `paint.rs:132`（**pub**） | ✅ |
| 55 | `enum FillOutcome { Filled, AlreadyFilled, UnsupportedLayer, NoBrush, OutOfMap, LayerMismatch, TooLarge }` | `fill.rs:237` | ✅ |
| 56 | `FillOutcome::is_refusal()` | `fill.rs:260` | ✅ |
| 57 | `FillOutcome::cells()` | `fill.rs:272` | ✅（core 另加 `note()`，**文案与 gui 模板逐字相同**） |
| 58 | `enum FillKey { Ground, Middle, Collision, Unsupported }` | `fill.rs:73` | ✅ |
| 59 | `fn neighbours(...)`（私有，`:1590`） | `fill.rs:143`（私有） | ✅ |
| 60 | `fn layer_note(...)`（私有，`:1617`） | `session.rs:692`（私有） | ⚠️ 文案来源不同（gui 走 `tx::fill` 模板、core 走 `format!`），**逐字相同**（见 §四.2 对照） |
| 61 | `struct ByteDiff { bytes, cells, first, header }` | `document.rs:169` | ✅ |
| 62 | `ByteDiff::same()` | `document.rs:181` | ✅ |
| 63 | `struct FillRecord { cells, layer }` | `session.rs:56` | ✅ |

### 1.5 `EditSession`（`:585-1324`，**这是唯一需要"包一层"的类型**）

gui 的会话**不持有地图**：每个改格方法都收 `&mut Map`（`app.rs` 把 `DocState.map` 借进来）。
core 的会话**持有 `Document`**（`session.rs:65-89`），方法不收地图。

⇒ gui 的 `EditSession` 只能变成**薄包装**：内部 `core: miredit_core::EditSession`，
每次调用前后用 `std::mem::swap` 把宿主地图换进/换出（O(1)，只换 `Vec` 指针，不拷 5.9 MB）。

| # | gui 方法 | core 等价 | 判定 |
|---|---|---|---|
| 64 | `tool()` | `session.rs:137` | ✅ |
| 65 | `brush()` | `:142` | ✅ |
| 66 | `layer()` | `:147` | ✅ |
| 67 | `note()` | `:154` | ✅ |
| 68 | `brush_size()` | `:159` | ✅ |
| 69 | `last_fill()` | `:165` | ✅ |
| 70 | `clear_note()` | `:219` | ✅ |
| 71 | `undo_len()` | `:183` | ✅ |
| 72 | `redo_len()` | `:188` | ✅ |
| 73 | `pos()` | `:193` | ✅ |
| 74 | `saved_pos()` | `:198` | ✅ |
| 75 | `dirty()` | `:204` | ✅（core 真值在 `Document::dirty`，判据同 `pos != saved_pos \|\| 笔画未结算`） |
| 76 | `last_save()` | `:170` | ✅ |
| 77 | `save_error()` | `:175` | ✅ |
| 78 | `stroke_active()` | `:209` | ✅ |
| 79 | `last_cell()` | `:214` | ✅ |
| 80 | `select(i: usize) -> bool` | `set_tool(Tool) -> bool`（`:226`） | ⚠️ 收下标 ⇒ 包一层 `Tool::from_index`（依赖 P1） |
| 81 | `set_layer(EditLayer)` | `:235` | ✅ |
| 82 | `set_brush_size(usize) -> usize` | `:251` | ✅ |
| 83 | `grow_brush_size(isize) -> usize` | `:257` | ✅ |
| 84 | `footprint(x,y,w,h) -> Vec<(u16,u16)>` | `:280`（**只吃 x,y**，用自己地图的宽高） | ❌ **签名不同** ⇒ 见 P2 |
| 85 | `refresh_layer_note()`（私有） | `:299`（私有） | ✅ |
| 86 | `mark_saved()` | `:264` | ✅ |
| 87 | `note_save_ok()` | `:270` | ✅ |
| 88 | `note_save_err(String)` | `:275` | ✅ |
| 89 | `begin()` | `:317` | ✅ |
| 90 | `touch(&mut Map,x,y) -> usize` | `touch(x,y)`（`:328`） | ⚠️ 换图 |
| 91 | `touch_footprint(&mut Map,x,y) -> usize` | `:386` | ⚠️ 换图 |
| 92 | `end() -> usize` | `:399` | ✅ |
| 93 | `apply_once(&mut Map,x,y) -> usize` | `:409` | ⚠️ 换图 |
| 94 | `paint_stroke_once(&mut Map,x,y) -> usize` | `:420` | ⚠️ 换图 |
| 95 | `paint_stroke(&mut Map,Option<(u16,u16)>,(u16,u16)) -> usize` | `:432` | ⚠️ 换图 |
| 96 | `fill(&mut Map,x,y) -> FillOutcome` | `:453` | ⚠️ 换图 |
| 97 | `refuse_fill(..)`（私有） | `:555`/`:561` | ✅ |
| 98 | `pick_cell(&Map,x,y) -> Option<PickedCell>` | `:572`（`&self`，只吃 x,y） | ❌ **`&self` + 外部 `&Map`，无法换图** ⇒ 见 P2 |
| 99 | `pick_at(&Map,x,y) -> Option<Brush>` | `:589` | ⚠️ 换图 |
| 100 | `set_brush_from_palette(mir2_assets::LibRef,u32)` | `:616`（收 **core 的** `LibRef`） | ⚠️ 需 4 行类型转换（`mir2_assets::LibRef` → `core::LibRef`） |
| 101 | `undo(&mut Map) -> usize` | `:634` | ⚠️ 换图 |
| 102 | `redo(&mut Map) -> usize` | `:643` | ⚠️ 换图 |
| 103 | `Default for EditSession`（`:634-656`） | core **没有** `Default`（会话必须持文档） | ❌ **gui 侧必须实现**：占位文档 = 1×1 `Old12` 地图（`statusbar.rs:350`、`text_fit.rs:521` 都用 `EditSession::default()`） |

### 1.6 脚本驱动（gui 独有）

| # | gui 项 | core 位置 | 判定 |
|---|---|---|---|
| 104 | `enum EditCmd { Layer, Pickup, Brush, Erase, Stroke, Fill, Size, Undo, Redo, Check }`（`:1726`） | `cmd::Command`（9 变体，`cmd.rs:51`） | ⚠️ **部分映射**，见 §三.2 |
| 105 | `struct ScriptReport { edited_cells, steps, undo, redo, dirty }`（`:1751`） | 无 | ❌ 留 gui（脚本回执） |
| 106 | `fn apply_script(&mut DocState, &mut EditSession, &[EditCmd]) -> ScriptReport`（`:1767`） | `cmd::apply(&mut Session, Command) -> Result<Outcome, CoreError>` | ⚠️ gui 版要**打印 `tx` 模板行 + 驱动 `DocState`**，只能"翻译后调 core"；`Command` 缺 `Erase`/`Check` |

---

## 二、`edit.rs` 的 64 条测试逐条判定

gui `edit.rs` 内 `#[test]` **64 条**（gui 全 crate 207 条，其余 143 条在别的模块）。
下表"core 等价断言"一栏指**core 里已有的同名/同义断言**（含集成测试 `crates/miredit-core/tests/`）。

| # | gui 测试（行号） | core 等价断言 | 判定 |
|---|---|---|---|
| 1 | `下标与取格同源`（2123） | `document.rs::下标与取格同源` | ✅ |
| 2 | `吸管取到图号与库号`（2138） | `session_tests.rs::吸管取到图号与库号` | ✅ |
| 3 | `越界吸管不动笔刷`（2159） | `session_tests.rs::越界吸管不动笔刷` | ✅ |
| 4 | `画笔写图号保留地表bit15必红`（2183） | `session_tests.rs::画笔写图号保留地表bit15` + `tests/bit15_matrix.rs::门2_改前必红` | ✅ |
| 5 | `画笔只改地表层`（2210） | `paint.rs::只改当前层字段` | ⚠️ core 版只比 `Cell` 字段，gui 版还钉 `walkable` 前后一致 + `bit15` 前后相同 |
| 6 | `画笔在old12档不动库号字节`（2255） | `paint.rs::十二字节档不动库号` | ✅ |
| 7 | `橡皮只清图号保留阻挡位`（2269） | `paint.rs::橡皮两个分支` | ✅ |
| 8 | `没取笔刷时画笔不落格`（2283） | `session_tests.rs::落笔零格必有原因`（"没有笔刷"支） | ✅ |
| 9 | `未接线工具不改地图`（2299） | `paint.rs::其它工具不改地图` | ✅ |
| 10 | `一次拖动算一步`（2319） | `undo.rs::一次拖动算一步` | ✅ |
| 11 | `撤销重做逐字节还原`（2346） | `undo.rs::一次拖动算一步`（含 `to_bytes()` 相等）+ `session_tests` | ⚠️ core 分散在多条，无同名整条 |
| 12 | `脏标记随保存与撤销变化`（2376） | `session_tests.rs::脏标记随保存与撤销变化` | ✅ |
| 13 | `保存失败原因保留`（2398） | 无 | ❌ core 无等价（`note_save_err` 有，但没有"失败原因不因保存成功消失"的断言） |
| 14 | `撤销栈上限一百步`（2410） | `undo.rs::撤销栈上限一百步` | ✅ |
| 15 | `新的一步清空重做栈`（2427） | `undo.rs::新的一步清空重做栈` | ✅ |
| 16 | `值没变不记步`（2443） | `paint.rs::值没变也给write` | ⚠️ core 只钉"paint 仍返回 Write"，没钉"会话不记步"（`undo.rs::空步不入栈` 补上了后者） |
| 17 | `未结算的笔画也算脏`（2458） | `undo.rs::未结算的笔画也算脏` + `session_tests.rs::未结算笔画也算脏` | ✅ |
| 18 | `重开一笔会先结算上一笔`（2474） | `undo.rs::重开一笔先结算上一笔` | ✅ |
| 19 | `划线插值覆盖路径`（2497） | `session_tests.rs::划线插值覆盖路径` | ✅ |
| 20 | `差异按格统计`（2511） | `document.rs::差异按格统计` | ✅ |
| 21 | `库号显示按档位`（2530） | 无 | ❌ **留 gui**（测的是 `lib_text` 这个界面文案函数） |
| 22 | `工具下标与图标表一致`（2539） | 无 | ❌ **留 gui**（测 `icons::TOOLS` 下标 ↔ `Tool`；依赖 P1 的 `from_index`/`index`/`name`） |
| 23 | `脚本驱动跑通闭环`（2560） | `cmd.rs::落笔回执` / `拖动回执与撤销` | ⚠️ core 测的是单命令，gui 测的是"多命令 + `DocState` + `Check`"整条脚本 |
| 24 | `无地图脚本不做任何事`（2606） | 无 | ❌ **留 gui**（`DocState.doc == None` 是宿主状态） |
| 25 | `素材帧索引落格写图号加一`（2663） | `brush.rs::帧索引加一且上限按图号判` | ✅ |
| 26 | `零号帧落格写成图号一`（2683） | 同上 | ✅ |
| 27 | `图号上限按图号判而非按索引`（2699） | 同上 | ✅ |
| 28 | `落格后渲染取到的正是对应帧`（2717） | 无 | ❌ **留 gui**（`miredit-render` + `mir2_assets::SpriteSource` 集成） |
| 29 | `吸管反显图号不再减一`（2757） | 无（core 只钉 `img`，没钉状态栏文案） | ⚠️ 半留 gui：`b.img == 905` 由 core 覆盖，**状态栏那一行文案**只有 gui 能测 |
| 30 | `真图库下渲染出的是第九零四帧像素`（2786） | 无 | ❌ **留 gui**（真图库 + 真渲染逐像素） |
| 31 | `三族落笔只动本层字节`（2890） | `paint.rs::只改当前层字段` + `layer.rs::bit15保留矩阵` | ⚠️ core 无"逐字节偏移集合"断言 |
| 32 | `族不匹配拒绝且一字节不动`（2938） | `paint.rs::族不匹配拒绝` + `session_tests.rs::族不匹配拒绝且一字节不动` | ✅（但 gui 版还**逐字**断言 `note` 文案） |
| 33 | `橡皮按层只清当前层`（2972） | `paint.rs::橡皮两个分支` | ⚠️ core 版逐层比 `Cell`，gui 版还钉字节偏移 |
| 34 | `十二字节档三族只写图号`（3009） | `paint.rs::十二字节档不动库号` | ⚠️ 同上 |
| 35 | `吸管按当前层取笔刷`（3032） | `session_tests.rs::吸管取到图号与库号` + `brush.rs::整格三族按层取` | ⚠️ core 无"整格三族 + 按层取 + 落笔只写本层"整条 |
| 36 | `层名与族名同源`（3077） | `layer.rs::层名与界面文案同源` + `brush.rs::族名与层名同源` | ✅ |
| 37 | `三族撤销重做逐字节还原`（3121） | `undo.rs::整格快照能还原所有字段` + `一次拖动算一步` | ⚠️ core 无"三档 × 三族"矩阵式整条 |
| 38 | `阻挡层置位只动bit15`（3172） | `paint.rs::阻挡层画笔只置位` | ⚠️ core 只比 `Cell`，gui 版钉"差异只有 1 个字节" |
| 39 | `阻挡层清位只动bit15`（3224） | `paint.rs::橡皮两个分支` | ⚠️ 同上 |
| 40 | `阻挡位与walkable一致`（3267） | `paint.rs::笔刷自带bit15不传播` | ⚠️ core 只钉一格 `is_walkable`，gui 版钉置位/清位往返 |
| 41 | `阻挡层拒绝图库笔刷`（3296） | `session_tests.rs::阻挡层笔刷闸门` + `paint.rs::阻挡层画笔只置位` | ✅（gui 版还逐字断言 `note`） |
| 42 | `阻挡层接受地图吸来的笔刷`（3329） | `session_tests.rs::阻挡层笔刷闸门` | ✅ |
| 43 | `吸管在阻挡层取同一格内容`（3344） | `brush.rs::阻挡层吸管族按字段` | ⚠️ core 只钉族，gui 钉"与地表层吸到的一模一样" |
| 44 | `阻挡层撤销重做逐字节还原`（3371） | `undo.rs::整格快照能还原所有字段` | ⚠️ core 无"三档 × 两位"矩阵 |
| 45 | `脚本可切当前编辑层`（3395） | `cmd.rs::层名非法就报错不改状态` | ⚠️ core 走 `Command::SetLayer`，gui 走 `EditCmd::Layer` |
| 46 | `尺寸夹到三到三十一的奇数`（3437） | `brush.rs::尺寸夹到三到三十一的奇数` + `session_tests.rs::尺寸不改地图也不进撤销栈` | ✅ |
| 47 | `尺寸三档单点覆盖格数`（3471） | `session_tests.rs::尺寸三档单点覆盖格数` | ✅ |
| 48 | `尺寸在边缘正确裁剪`（3494） | `session_tests.rs::尺寸在边缘正确裁剪` | ✅ |
| 49 | `拖动整条轨迹按尺寸落`（3522） | `session_tests.rs::拖动整条轨迹按尺寸落` | ✅ |
| 50 | `填充只填同号连通块`（3541） | `session_tests.rs::填充只填同号连通块` + `fill.rs::收块只读且四邻连通` | ✅ |
| 51 | `填充撤销一步逐字节还原`（3617） | `session_tests.rs::填充撤销一步逐字节还原` + `tests/fill_912.rs::门4` | ✅ |
| 52 | `填充超上限一格不写`（3645） | `fill.rs::超上限停在收块阶段` + `session_tests` | ✅ |
| 53 | `填充不跨图边界`（3675） | `fill.rs::越界起点报错` | ⚠️ core 无"地表 step=2 vs 中层 step=1 格数差别"整条 |
| 54 | `填充阻挡层只置位`（3707） | `session_tests.rs::填充阻挡层只置位` | ✅ |
| 55 | `填充物体层被拒`（3734） | `session_tests.rs::填充三种拒绝` | ✅ |
| 56 | `填充没笔刷被拒`（3748） | 同上 | ✅ |
| 57 | `填充匹配键不看重阻挡位`（3764） | `fill.rs::匹配键逐层` | ✅ |
| 58 | `三档合成图画笔写图号保留bit15`（3850） | `tests/bit15_matrix.rs::门1_bit15保留矩阵三族各一条` | ⚠️ core 走会话路径，gui 版钉"逐字节 2 字节差异" |
| 59 | `三档真实地图画笔写图号保留bit15`（3897） | `tests/paint_3x3.rs::三档真图写图号都保留bit15` | ✅ |
| 60 | `三族写图号bit15保留矩阵`（3954） | `paint.rs::写图号bit15保留矩阵` + `layer.rs::bit15保留矩阵` | ✅ |
| 61 | `填充九百一十二格全部保留bit15`（4011） | `tests/fill_912.rs::门3_填充912格全部保留bit15` | ✅ |
| 62 | `画笔与填充改图号不改walkable`（4064） | `tests/fill_912.rs::门3`（含 `walkable` 变化 0） | ⚠️ core 无"三族画笔 + 填充"合一条 |
| 63 | `拖动画笔与脚本落笔保留bit15`（4160） | 无 | ❌ **留 gui**（`apply_script` + `EditCmd` 路径覆盖；core 侧 `cmd.rs` 只测单命令） |
| 64 | `图号层写图号不影响阻挡层置位清位`（4233） | 无 | ❌ **留 gui**（跨"图号层会话 + 阻挡层会话"的组合回归） |

### 2.1 汇总

| 判定 | 条数 | 说明 |
|---|---|---|
| ✅ 完全等价 | **27** | 迁移后仍留在 gui（只改 `use`/类型名），断言不动 |
| ⚠️ core 有但要适配 / 粒度不同 | **26** | 迁移后仍留在 gui；core 版覆盖了**同一条语义**，gui 版是更细的字节/组合断言 |
| ❌ core 没有 | **11** | 见 §三 |
| **合计** | **64** | 与 `grep '#[test]' edit.rs` 一致 |

> ⚠️ **重要**：**一条测试都不用删**。上表 ❌ 的 11 条都是"gui 宿主层"的东西（文案、图标下标、渲染集成、`DocState` 驱动、跨会话组合），
> 它们本来就该留在 gui —— 迁移只改它们调用的**实现**（`use` 与类型名），断言内容一个字不动。

---

## 三、❌ 清单与处置

### 3.1 需要补进 core 的（**要放行**）

| 缺什么 | 为什么 gui 侧绕不过去 | 处置 |
|---|---|---|
| `Tool::from_index(usize)` / `Tool::index()` / `Tool::name()` | ① `screen_text.rs:145` 调 `ed.tool().name()`，该文件**不在白名单**；② `edit.rs::select(i)` 与测试 `工具下标与图标表一致` 要 `from_index`/`index` | **补 core `paint.rs`**（+ 5 条工具名常量，值同 `tx::TOOL_*`）。属纯映射，无行为变化 |
| `footprint_of(brush_size, x, y, w, h)` | gui 的 `footprint(&self, x, y, w, h)` 是 `&self` 且**只拿到宽高、拿不到地图**，无法把宿主地图换进内核 | **补 core `brush.rs`**（`EditSession::footprint` 转调，行为不变）；否则 gui 抄 12 行 |
| `pick_cell_of(&Map, x, y) -> Option<PickedCell>` | gui 的 `pick_cell(&self, map: &Map, …)` 是 `&self` + 外部 `&Map`（`app.rs:434`），**无法** `mem::swap`（那要 `&mut`） | **补 core `brush.rs`**（`EditSession::pick_cell` 转调，行为不变）；否则 gui 抄 6 行 |

> 三条都是**纯函数抽取 / 纯映射**，不碰任何判据；补完后 core 自己的 98 条测试**一条不改**。

### 3.2 明确**留在 gui** 的（判为宿主/文案层，不迁）

| 留在 gui 的项 | 原因 |
|---|---|
| `TOOL_*_IDX`（4 个图标下标） | 属于 `icons::TOOLS` 的 UI 布局常量，`app.rs:343/1028` 直接用 |
| `family_name()`、`lib_text()`、`brush_lib_text()`、`dirty_text()` | 界面文案（`tx::VAL_UNKNOWN` 等），core 不许依赖 gui 文案表 |
| `EditCmd` / `ScriptReport` / `apply_script` | 脚本驱动 = 命令行契约（打印 `[编辑] …` / `EDIT_RESULT` 行）+ 驱动 `DocState`；core 的 `cmd::apply` 是**单命令 + 结构化回执**，两者粒度不同 |
| `print_check` | 诊断打印（`DocState` + 磁盘原文件字节） |
| `EditSession::default()` | core 的会话必须持文档；gui 的"无地图会话"是宿主概念 ⇒ gui 侧用 1×1 `Old12` 占位文档实现 |
| 11 条 ❌ 测试（见 §2.1） | 见上表；断言内容不动 |

### 3.3 `EditCmd` → `Command` 的映射表（能翻的翻，翻不了的说清）

| `EditCmd` | `miredit_core::Command` | 结论 |
|---|---|---|
| `Layer(EditLayer)` | `SetLayer { layer: l.key() }` | ✅ 可翻 |
| `Pickup(x,y)` | `Pick { x, y }` | ✅ 可翻 |
| `Brush(x,y)` | `Paint { x, y }`（= `paint_stroke_once`） | ✅ 可翻 |
| `Stroke(a,b)` | `Stroke { from, to }` | ✅ 可翻 |
| `Fill(x,y)` | `Fill { x, y }` | ✅ 可翻（`Outcome.changed` = `FillOutcome::cells()`，`note` = 拒绝原因） |
| `Size(n)` | `SetBrushSize { size: n as u16 }` | ✅ 可翻（`n` 恒 ≤ 31） |
| `Undo(n)` | 循环 `Undo` ×n | ✅ 可翻 |
| `Redo(n)` | 循环 `Redo` ×n | ✅ 可翻 |
| **`Erase(x,y)`** | **无 `Command::Erase`** | ⚠️ 只能 `set_tool(Eraser)` + `paint_stroke_once` 直调会话 |
| **`Check`** | **无** | ⚠️ 纯宿主诊断（要磁盘原文件字节），留 gui |

⇒ 若坚持"`EditCmd` 全部经 `Command::apply`"，需要给 core 加 `Command::Erase`（**第 4 项要放行的改动**）。

---

## 四、迁移设计（薄适配层）

### 4.1 类型全部从 core re-export

```rust
pub use miredit_core::{
    byte_diff, cell_at, cell_mut, fill_key, index_of, lib_ref_of, line_cells,
    palette_index_to_sprite, paint, Brush, ByteDiff, Change, EditLayer, Family, FillKey,
    FillOutcome, FillRecord, LibRef, PaintOutcome, PickedCell, Step, Tool, COLLISION_BIT,
    SPRITE_MAX, UNDO_LIMIT,
};
```

### 4.2 文案逐字对照（**这是"行为不变"的硬判据**）

core 的 `note` 文案与 gui 的 `tx` 模板**逐字相同**（已逐条比对）：

| 场景 | gui 模板 | core 生成 | 一致 |
|---|---|---|---|
| 族不匹配 | `EDIT_LAYER_MISMATCH_TMPL` = `笔刷族（{brush}）与当前图层（{layer}）不一致` | `session.rs:345` `format!("笔刷族（{}）与当前图层（{}）不一致", …)` | ✅ |
| 阻挡层收图库笔刷 | `EDIT_COLLISION_BRUSH_TMPL` = `笔刷来自图库（{brush}），{layer}只改 0x8000 位、不写图号` | `session.rs:354` | ✅ |
| 填充层不支持 | `FILL_ERR_LAYER` | `fill.rs:289` | ✅ |
| 填充没笔刷 | `FILL_ERR_NO_BRUSH` | `fill.rs:293` | ✅ |
| 填充越界 | `FILL_ERR_OUT_OF_MAP` = `点击格 ({x},{y}) 在地图外` | `fill.rs:296` / `session.rs:468` | ✅ |
| 填充超上限 | `FILL_ERR_TOO_LARGE` | `fill.rs:301` | ✅ |

⚠️ **前提**：`{brush}` 必须填「地表图层 / 中层图层 / 物体」，即 `Family::label()` 现在这套值。
**这正是 P3 会打破的东西**（见 §五）。

### 4.3 会话包装（`mem::swap`）

```rust
pub struct EditSession { core: miredit_core::EditSession }

impl EditSession {
    /// 把宿主地图换进内核会话（O(1)，只换 Vec 指针）→ 调用 → 换回
    fn with_map<R>(&mut self, map: &mut Map, f: impl FnOnce(&mut miredit_core::EditSession) -> R) -> R {
        std::mem::swap(&mut self.core.document_mut().map, map);
        let r = f(&mut self.core);
        std::mem::swap(&mut self.core.document_mut().map, map);
        r
    }
}
```
`Document.map` 是 `pub`（`document.rs:45`），`Document::path` 也是 `pub` ⇒ 换图/换路径都不需要改 core。

### 4.4 `save.rs`

gui 的 `save_atomic(path, bytes) -> Result<PathBuf, String>` / `SaveOutcome`（含 `NoMap`/`Failed`）签名不变，
内部委托 `miredit_core::save_atomic` + `fsync_parent`，并把 `CoreError::Save { stage, .. }` 按阶段折回
**原来的 `tx::SAVE_ERR_*` 文案**（`Temp`/`Sync`/`Rename` 可逐字还原；`Verify` 阶段 core 只给
"写入不完整：期望 N 字节，读回 M 字节"，需要按阶段用 `detail` 兜底 —— 该分支实际不可达，迁移后如实记录）。

### 4.5 `mapdoc.rs`

`DocState` / `MapDoc` / `Assets` **保留**（管"打开哪张图 / 库在哪"，属宿主决策）；
底层 `MapDoc` 的解析改成 `miredit_core::Document::from_bytes`（`Map::parse` 的包装，判据仍在 `mir2-map`）。

---

## 五、P3：`Family` / `LibRef` re-export —— **不建议现在做**

任务书要求 `core/src/lib.rs` 改成 `pub use mir2_assets::{Family, LibRef}`。技术上可行，但**有语义冲突**：

| 项 | `miredit_core::brush::Family::label()`（现在） | `mir2_assets::Family::label()`（re-export 后） |
|---|---|---|
| `Ground` | `地表图层` | `地面` |
| `Middle` | `中层图层` | `中层` |
| `Surface` | `物体` | `表层` |

`label()` 被这些地方消费：
- `core/src/brush.rs:201` `Brush::family_name()`；
- `core/src/session.rs:346/355`（族不匹配 / 阻挡层拒绝的 note 文案）；
- `core/src/fill.rs:298`（`FillOutcome::note` 的族不匹配文案）。

⇒ re-export 会让上面 6 条文案全部从「地表图层」变成「地面」，直接打破：

| 会红的测试 | 断言 |
|---|---|
| `core/src/brush.rs::族名与层名同源` | `assert_eq!(Family::Ground.label(), EditLayer::Ground.name())` |
| `core/src/fill.rs::拒绝原因可上屏` | `assert!(note.contains("地表图层") …)` |
| `gui edit.rs::族不匹配拒绝且一字节不动` | `assert_eq!(ed.note(), Some(tx::fill(EDIT_LAYER_MISMATCH_TMPL, …)))`（**逐字**） |
| `gui edit.rs::阻挡层拒绝图库笔刷` | `assert_eq!(ed.note(), Some(tx::fill(EDIT_COLLISION_BRUSH_TMPL, …)))`（**逐字**） |

这违反"断言内容不许放宽"，而且会**改掉用户可见的状态栏文案**。

**三个选项，请裁定：**

| 选项 | 代价 |
|---|---|
| **A（推荐）**：本条不做。gui 侧仍然"不留副本"——它 `pub use miredit_core::{Family, LibRef}`，副本只在 core 里一份 | 0 风险；但 core 与 `mir2-assets` 各有一份 `Family`/`LibRef`（字段/变体/`stem`/`index` 完全同口径） |
| **B**：做 re-export，同时改 `core/src/brush.rs`（`family_name` 改走本地映射）+ `core/src/fill.rs` + 2 条 core 测试断言 | 要再动 2 个 core 文件（brush.rs / fill.rs）+ 改 2 条 core 断言（`contains("地表图层")` → `contains("地面")`，属"改断言"） |
| **C**：做 re-export，并给 `mir2-assets` 的 `Family::label()` 换值 | ❌ 会污染 `mir2-assets`（`lib_paths.rs:79-85` 的「地面/中层/表层」是它自己的日志口径），且 `mir2-assets` 不在白名单 |

---

## 六、验收门的两处**基线不符**（必须先裁定）

### 6.1 `cargo clippy -p miredit-gui --all-targets -- -D warnings` —— **迁移前就已经红**

实测（`2026-11-05 00:1x`，`CARGO_INCREMENTAL=0`、`CARGO_TARGET_DIR=I:\miredit-gui-target`）：

```
CLIPPY_EXIT=101
error: could not compile `miredit-gui` (lib) due to 34 previous errors
error: could not compile `miredit-gui` (lib test) due to 45 previous errors
```

45 条错误的分布（**全部在我无权改的文件里**）：

| 文件 | 条数 | 在白名单 |
|---|---|---|
| `theme.rs` | 29 | ❌ |
| `canvas_view.rs` | 11 | ❌ |
| `editrun.rs` | 4 | ❌ |
| `metrics.rs` | 3 | ❌ |
| `icons.rs` | 1 | ❌ |
| `mapdoc.rs` | 1 | ✅ |
| `text_fit.rs` | 1 | ❌ |

错误种类（按 clippy 打印的 `error:` 首行归类，共 45 条）：

| 条数 | 首行文案 |
|---|---|
| 29 | `digits of hex, binary or octal literal not in groups of equal size` |
| 5 | `field assignment outside of initializer for an instance created with Default::default()` |
| 3 | `this operation has no effect` |
| 2 | `doc list item without indentation` |
| 1 | `empty line after doc comment` |
| 1 | `large size difference between variants`（`mapdoc.rs:47` 的 `Assets`） |
| 1 | `the loop variable \`i\` is used to index \`TOOLS\`` |
| 1 | `this assertion has a constant value` |
| 1 | `this function has too many arguments (8/7)` |
| 1 | `this operation will always return zero. This is likely not the intended outcome` |

⇒ **门禁 2 的 clippy 半条不可达**（除非放行我去改 `theme.rs`/`canvas_view.rs`/`editrun.rs`/`metrics.rs`/`icons.rs`/`text_fit.rs`）。
**请裁定**：(a) 放行修这 6 个文件；(b) 把门禁改成"迁移不新增 clippy 错误"（我给出迁移前后两份错误集合比对）；(c) 其他。

### 6.2 `cargo metadata` 成员数 —— **基线是 8，不是 7**

```
$ cargo metadata --no-deps --format-version 1 | … 'members=', len(...)
members= 8
  mir2-map / mir2-assets / miredit-render / miredit-gui /
  mir2-stamp / mir2-format / miredit-core / miredit-web
```
`tools/miredit/Cargo.toml` 的 `members` 就是 8 条（含 `miredit-web`）。
⇒ 门禁 5 应按"**迁移前后都等于 8**"判。

---

## 七、迁移前基线（已实测，供迁移后逐项对照）

### 7.1 测试

| 命令 | 结果 |
|---|---|
| `cargo test -p miredit-gui --lib` | **207 passed / 0 failed**（1.70s） |
| `cargo test -p miredit-core` | lib **83** + `bit15_matrix` 4 + `fill_912` 2 + `paint_3x3` 3 + `save_atomic` 5 + doc 1 = **98 passed / 0 failed** |

### 7.2 构建

| 命令 | 结果 |
|---|---|
| `cargo build -p miredit-gui --release`（`cargo clean -p miredit-gui --release` 后重建） | **exit 0，rustc warning 0** |
| `cargo clippy -p miredit-gui --all-targets -- -D warnings` | **exit 101，45 条**（见 §6.1） |

### 7.3 字节级回归（CLI，`--edit-*` + `--save`，`0.map` 副本）

| 用例 | 命令 | 输出 |
|---|---|---|
| ① 3×3 笔刷 | `--palette-pick 9 --edit-brush 0,0 --edit-check --save` | `改动 4 格` / `比对：5 字节 / 4 个格 / 首差 @52 / 头部 0`；`bk_img(0,0) 0x873C → 0x800A`；bit15 前后均置位 |
| ② 填充 | `--palette-pick 0 --edit-fill 358,0 --edit-check --save` | `改动 912 格` / `比对：1824 字节 / 912 个格`；912 格 `bk_img` 全 `0x8001`；`bit15` 变化 **0**；其它字段变化 0 |
| ③ 撤销 | `--palette-pick 9 --edit-brush 0,0 --edit-fill 358,0 --edit-undo 2 --edit-check --save` | `撤销 2 步 → 还原 916 格`；`比对：与磁盘原文件逐字节相等（5880052 字节）`；`saved=skip` |
| ④ 保存原子性 | 正常：①②③ 均 `saved=ok`；失败：把 `<map>.miredit-tmp` 换成目录 | `[保存] 失败：写临时文件 … 失败：拒绝访问。(os error 5)`，`EXIT=3`，原文件 SHA256 不变 |

### 7.4 截图

| 项 | 值 |
|---|---|
| 命令 | `miredit-gui --map E:\GodM2\GodotClient\maps\0.map --shot <png> --frames 2` |
| 尺寸 | 1500×950，2,278,440 字节 |
| sha256（连拍两次一致） | `dc001fe4216d4c4cb57fbfff2c9ea921501722ab8d62edaed109c6557a890f0` |

> ⚠️ `--shot` 必须**脱离管道**启动（`Start-Process`）；挂在 `Tee-Object` 后面会卡死（实测 300s 无输出）。

---

## 八、迁移结果（2026-11-05 00:2x ~ 00:3x 实跑）

### 8.1 改了哪些文件

| 文件 | 白名单 | 改了什么 |
|---|---|---|
| `crates/miredit-core/src/paint.rs` | **放行（P1）** | `Tool` 补 `from_index` / `index` / `name`；新增 5 条工具名常量 + 4 个 `TOOL_*_IDX`；新增测试 `工具下标与名字往返一致` |
| `crates/miredit-core/src/brush.rs` | **放行（P2）** | 抽 `footprint_of` / `pick_cell_of` 两个纯函数；`Family` 补「与 `mir2_assets::Family` 的关系」整节；新增测试 `族与mir2_assets按序号一一对应` / `落笔范围居中且裁剪` / `吸管整格吃外部地图` |
| `crates/miredit-core/src/session.rs` | **放行（P2）** | `footprint` / `pick_cell` 转调上面的纯函数；`pick_at` 抽出 `brush_from_cell` 并新增 `pick_at_from(&Map, …)`（宿主地图只借不换） |
| `crates/miredit-core/src/cmd.rs` | **放行（第 4 项）** | 新增 `Command::Erase { x, y }`；`命令集封闭九条` → `命令集封闭十条`（9→10）；新增测试 `擦除命令` |
| `crates/miredit-core/src/lib.rs` | 白名单 | 只补 re-export（新增的常量与两个纯函数）；`Family`/`LibRef` 仍是 core 自有定义（**P3 = 选 A，不 re-export**） |
| `crates/miredit-core/Cargo.toml` | 白名单 | 加 `[dev-dependencies] mir2-assets`（**仅测试**：跨 crate 的族集合一致性只能靠真依赖来钉） |
| `crates/miredit-gui/Cargo.toml` | 白名单 | 加 `miredit-core` 依赖 |
| `crates/miredit-gui/src/edit.rs` | 白名单 | **重写成薄适配层**（见 8.2） |
| `crates/miredit-gui/src/save.rs` | 白名单 | 原子写委托 `miredit_core::save_atomic`；`temp_path_for` / `fsync_parent` / `DirSync` / `TEMP_SUFFIX` 改为 re-export |
| `crates/miredit-gui/src/mapdoc.rs` | 白名单 | `read_map` 委托 `miredit_core::Document::load`；新增 `core_err_text` 把结构化错误折回**逐字相同**的旧文案 |

> ⚠ **未改**（等放行）：`crates/miredit-web/src/json.rs:65` 的 `Command` 穷举 match 因新增
> `Command::Erase` 而 `E0004` 编译失败（**miredit-web 不在白名单**）。这是"契约变更的消费方"，
> 详见 §8.7。

### 8.2 `edit.rs`：从 4,280 行降到 **3,172 行**（实现段 2,087 → 980 行）

| 段 | 迁移前 | 迁移后 |
|---|---|---|
| 模块文档 + 实现 | 2,087 行 | **980 行** |
| 测试（`mod tests`） | 2,193 行 | 2,192 行（**只改 `use` / 类型名 / 1 个测试助手**） |
| 合计 | **4,280 行** | **3,172 行**（−1,108） |

**删掉的重复实现**（全部改为 `pub use miredit_core::…`，本文件零定义）：

| 被删的块 | 原行数（约） | 现在 |
|---|---|---|
| `enum EditLayer` + `impl`（18 项） | 260 | `pub use` |
| `enum Tool` + `impl` + 4 个 `TOOL_*_IDX` | 85 | `pub use`（下标常量定义搬进 core） |
| `Brush` / `PickedCell` / `Change` / `Step` / `UNDO_LIMIT` | 130 | `pub use` |
| `palette_index_to_sprite` / `lib_ref_of` / `index_of` / `cell_at` / `cell_mut` | 45 | `pub use` |
| `PaintOutcome` + `paint()` | 130 | `pub use` |
| `FillOutcome` / `FillKey` / `fill_key` / `neighbours` / `line_cells` | 175 | `pub use` |
| `ByteDiff` + `byte_diff` | 55 | `pub use` |
| `EditSession` 本体（字段 + 40 个方法 + 填充 BFS + 撤销栈 + `layer_note`） | **~740** | **~370 行的包装**（每个方法一行委托） |
| `COLLISION_BIT` / `SPRITE_MAX` | 10 | `pub use` |

**仍然留在 gui 的（共 ~230 行，全部有理由）**：

| 留在 gui | 行数 | 原因 |
|---|---|---|
| `EditSession` 包装 + `with_map` + `apply_cmd` | ~90 | 宿主把地图放在 `DocState` 里、内核会话自己持文档 ⇒ 必须有一层 `mem::swap` 桥 |
| `family_name` / `lib_text` / `brush_lib_text` / `dirty_text` | ~40 | 界面文案（`tx::VAL_UNKNOWN` 等），core 不许依赖 gui 文案表 |
| `core_lib` / `assets_lib` | ~30 | 两个 `LibRef` 之间的字段搬运（**判据仍在 core**，这里只换外壳） |
| `EditCmd` / `ScriptReport` / `apply_script` | ~250 | 脚本驱动：打印 `[编辑] …` 契约行 + 驱动 `DocState`；能翻成 `Command` 的 8 条（`Layer`/`Pickup`/`Brush`/`Erase`/`Stroke`/`Size`/`Undo`/`Redo`）都翻了，`Fill` 因要 `FillOutcome` 直调会话、`Check` 是宿主诊断 |
| `print_check` | ~40 | 宿主诊断（要磁盘原文件字节） |

### 8.3 测试

| 命令 | 迁移前 | 迁移后 | 判据 |
|---|---|---|---|
| `cargo test -p miredit-gui --lib` | 207 / 0 failed | **207 / 0 failed** | ≥207 ✅ **条数不减** |
| `cargo test -p miredit-core` | 98 / 0 failed | **103 / 0 failed**（lib 88 + 集成 14 + doc 1） | ≥98 ✅ **+5** |

**gui 的 207 条一条没删、一条没放宽**。因类型搬家改动的**只有 3 类、共 8 处**：

| 处数 | 改法 | 断言变了吗 |
|---|---|---|
| 6 | `lib_ref_of(layer, n)` → `图库(layer, n)`（新加的**测试专用**助手，内部就是 `assets_lib(lib_ref_of(…))`） | ❌ 没变 |
| 1 | `assert_eq!(b.lib, Some(mir2_assets::LibRef::middle(22)))` → `Some(LibRef::middle(22))` | ❌ 没变（同一个值的两个写法） |
| 1 | `family_name(fam.family)` → `crate::palette::family_label(fam.family)`（`fam` 是 `mir2_assets::LibRef`，而 `family_name` 现在收内核的 `Family`） | ❌ 没变（同一个函数的旧入口） |

**core 侧**：新增 5 条（`工具下标与名字往返一致` / `落笔范围居中且裁剪` / `吸管整格吃外部地图` /
`族与mir2_assets按序号一一对应` / `擦除命令`），改 1 条名字（`命令集封闭九条` → `十条`，断言 9→10，
**是这次放行的契约变更本身，不是放宽**）。

### 8.4 字节级回归（迁移前 vs 迁移后，逐项对照）

| # | 项 | 迁移前 | 迁移后 | 结论 |
|---|---|---|---|---|
| ① | 3×3 笔刷 `(0,0)` | `改动 4 格`；`5 字节 / 4 格 / 首差 @52 / 头部 0` | 同左 | **逐字相同** |
| ① | `bk_img(0,0)` | `0x873C → 0x800A`（bit15 保留） | 同左 | **相同** |
| ② | 填充 `(358,0)` | `改动 912 格`；`1824 字节 / 912 格` | 同左 | **逐字相同** |
| ② | 填充后取值 | 912 格全 `0x8001`；bit15 变化 **0**；其它字段变化 0 | 同左 | **相同** |
| ③ | 撤销 2 步 | `还原 916 格`；`与磁盘原文件逐字节相等（5880052 字节）`；`saved=skip` | 同左 | **逐字相同** |
| ④ | 保存失败（临时路径被目录占住） | `EXIT=3`；`[保存] 失败：写临时文件 … 拒绝访问。(os error 5)`；原文件 SHA256 不变 | 同左 | **逐字相同** |

**文件级对照（sha256）**：

| 产物 | 迁移前 | 迁移后 | 相同 |
|---|---|---|---|
| `brush.map`（笔刷后） | `C6EFCC210FADFCC766F3AC0D51D4CDD5791AADA1924409EB0588136C0FAB230E` | 同左 | ✅ |
| `fill.map`（填充后） | `BEB6A3906C5DEA929B02227BB193E6C448CA2CB7B7F8A8CB982244094AE85974` | 同左 | ✅ |
| `undo.map`（撤销后） | `183E9D545E284275D7955E56DFC4AF301E8AF9E54C46470A4C7817EE7C5742E7`（= 原文件） | 同左 | ✅ |
| `fail.map`（保存失败后） | `183E9D545E…`（= 原文件，一字节未动） | 同左 | ✅ |

**stderr 文本对照**：4 个用例的完整输出（去掉目录名 `miredit-baseline` / `miredit-after` 的差异）
**逐字符相同**（`text_same=True` ×4）。

### 8.5 截图

| 项 | 迁移前 | 迁移后 |
|---|---|---|
| 尺寸 / 字节数 | 1500×950 / 2,278,440 | 1500×950 / 2,278,440 |
| sha256 | `dc001fe4216d4c4cb57fbfff2c9ea921501722ab8d62edaed109c6557a890f0` | `dc001fe4216d4c4cb57fbfff2c9ea921501722ab8d62edaed109c6557a890f0` |

⇒ **像素逐字节相同** ✅

### 8.6 门禁

| # | 门禁 | 结果 |
|---|---|---|
| 1a | `cargo test -p miredit-gui --lib` ≥207 / 0 failed | **207 / 0** ✅ |
| 1b | `cargo test -p miredit-core` ≥98 / 0 failed | **103 / 0** ✅ |
| 2a | `cargo build -p miredit-gui --release`（clean 后重建） | **exit 0 / rustc warning 0** ✅ |
| 2b | `cargo clippy -p miredit-gui --all-targets -- -D warnings` | exit 101，**但错误集合与迁移前逐条相同**：`NEW = 0`、`GONE = 0`、`error:` 文案多重集**完全相同**（47 行）⇒ 按裁定的 **(b) 不新增** 判定 **通过** ✅ |
| 3 | 4 项字节级回归 | 全部逐字节/逐字符相同 ✅ |
| 4 | 截图 sha256 | 相同 ✅ |
| 5 | `cargo metadata` 成员数 | **8**（迁移前 8，迁移后 8）✅ |
| 6 | 本文件 | 已补 §八 |

### 8.7 ✅ 已放行并修复：`miredit-web` 的 `Command` 穷举 match

`crates/miredit-web/src/json.rs:65` 的 `EditCmd::name()` 因新增 `Command::Erase` 报 `E0004`。
2026-11-05 父代理**只放行 1 行**，已在 `json.rs:70` 补：

```rust
EditCmd::Core(Command::Erase { .. }) => "Erase",
```

三条门禁（实测）：

| 命令 | 结果 |
|---|---|
| `cargo check -p miredit-web` | **EXIT=0**，warning 0 |
| `cargo check --workspace` | **EXIT=0**，warning 0（改前这条是红的） |
| `cargo test -p miredit-web` | **17 passed / 0 failed** |

改完后复核其余门禁**未受影响**：gui `207/0`、core `103/0`、release `EXIT=0 / warning 0`、
`cargo metadata` 仍 **8**。

⚠ **遗留一处（未授权、未改，如实登记）**：`json.rs:82` 的 `touched_cell()`
**没有** `Command::Erase` 分支 —— 它落到 `_ => None`，于是 `/api/events` 对"擦除"给出的
`rerender` 区域是 `None`（前端整屏重取，功能不受影响，只是少了区域提示）。
补法是再加一行 `| EditCmd::Core(Command::Erase { x, y }) => Some((*x, *y))`，
但**超出本次 1 行授权**，留给你决定。

### 8.8 其它未做 / 已知差异

1. **P3（`Family`/`LibRef` re-export `mir2_assets`）按裁定选 A，未做**；已按要求补了
   `brush.rs` 的对照说明 + 测试 `族与mir2_assets按序号一一对应`（钉住变体数 / index / stem，
   并显式断言两边 `label()` **故意不同**）。
2. **`SaveStage::Verify`（写入不完整）的文案**：core 只给
   `写入不完整：期望 N 字节，读回 M 字节`，本仓模板要 `{want}`/`{got}` 两个数 ⇒ 剥前缀后套
   `临时文件 {tmp} 写入不完整（期望 N 字节，读回 M 字节）`。该分支在本地 SSD 上不可达
   （写→刷→读回之间不会短写），**如实记录**；`Temp`/`Sync`/`Rename` 三档与迁移前逐字相同（已实测）。
3. **`mapdoc.rs` 的 `large_enum_variant`（`Assets`）** 是既有 clippy 债，本轮**没顺手修**
   （不在本次范围，且要动 `Assets` 的形状）；它与迁移前同一条，`NEW=0` 判定不受影响。
4. **`Outcome` 表达力不足**（父代理已记一笔）：`FillOutcome::AlreadyFilled` 与"被拒绝"在
   `Outcome` 里都折成 `changed == 0`，所以 `EditCmd::Fill` 只能直调会话。将来给 `Outcome`
   加 `reason` 字段后，这条可以再收口。


---

## 九、`mir2-format` 接入（2026-11-05 00:5x ~ 01:1x）：从"只认 12/14/36 三档"到"9 种全通"

> 依据：`docs` 侧任务书「让 `miredit-core` 接入 `mir2-format`」；本节只记**结果与实测数**，
> 设计细节见 `src/convert.rs` 模块文档（字段映射表 / 位宽处理 / 信息丢失清单）。

### 9.1 做了什么（双表示）

| 层 | 类型 | 角色 |
|---|---|---|
| 字节层真值 | `mir2_format::RawMap` | `format` / `header_raw` / `trailer` / `cells`；保存时由它出字节 |
| 编辑/渲染视图 | `mir2_map::Map` | 编辑（paint/fill/undo）与渲染（`miredit-render` 的入口签名）只认它 |

`Document` 同时持有两者：**打开** `read_any_tolerant` → `RawMap` → 逐格投影成 `Map`；
**编辑**只改 `view`；**保存**把 `view` 按位宽写回 `raw.cells`，再由该格式 writer 出字节
（`header_raw` + `trailer` 原样带上）→ 原子写盘。

- 新增 `src/convert.rs`：`FIELD_MAP`（12 条字段映射）/ `MapKind`（Exact / Low16 / Low8 / Tail22）/
  `lost_fields`（逐格式信息丢失清单）/ `view_cell_format`（视图档位语义投影）/
  `writes_layer_lib_byte`（库号字节写不进去的三种格式）/ `restore_unwritten_lib_byte`。
- `Document` 新增：`raw()` / `view()` / `view_mut()` / `raw_header()` / `trailer()` /
  `format_id()` / `format_label()` / `caps()` / `disk_cell_len()` / `tolerant_trailer()` /
  `edit_warning()` / `try_to_bytes()` / `ground_paint_supported()`。
- `EditSession` 新增：`edit_warning()` / `ground_paint_supported()`；`touch` / `fill` 各加两道
  「格式能力闸门」（地表闸门、库号字节闸门）。
- `FillOutcome` 新增 `FormatCannotPaintGround`（Type5 地表填充拒绝）。
- 依赖：`Cargo.toml` 加 `mir2-format = { path = "../mir2-format" }`（白名单仍无第三方）。

### 9.2 实测（门 1/2/5，`tests/formats_9.rs`）

| 项 | 结果 |
|---|---|
| 样本 | `GodotClient/maps` 61 + `热血传奇23周年客户端/Map` 1,097 = **1,158** |
| 门 1 全部能 open | **1,158 / 1,158**，失败 **0** |
| 按格式分布 | `Old12` **856**（含 67 张带未解释尾块）· `Shanda14` **107** · `Shanda2012` **195** |
| 门 2 未改即存 | **0 字节差异**（逐张 `open → save_atomic 写副本 → 读回比对`） |
| 门 5 带尾块 67 张 | 尾块分布 `{8040: 65, 9840: 2}`；改一格后**只有目标格 bk_img 的 1~2 字节变**、尾块原样 |

⚠ **口径订正（旧记 / 任务书都写"67 张 8,040 B"）**：实测 **65 张 8,040**、**2 张 9,840**
（`T218.map` / `T318.map`：头部 30×30、文件 20,692 字节 ⇒ `52 + 12×900 = 10,852`，
`20,692 − 10,852 = 9,840`）。逐文件核对 `文件长度 == 头部 + 格长×格数 + 尾块`，全部成立。

### 9.3 实测（门 3/4/6）

| 门 | 样本 | 证据 |
|---|---|---|
| 3 | `0.map`(Old12) | 格起 52，差异 2 处 `[(52,0x3C→0x0A),(53,0x87→0x80)]`，`bk 0x873C→0x800A`，`bit15` 保留 |
| 3 | `41.map`(New14) | 格起 52，差异 1 处 `[(52,0x16→0x0A)]`，`bk 0x8016→0x800A`，`bit15` 保留 |
| 3 | `n0104.map`(Ext36) | 格起 52，差异 1 处 `[(52,0x00→0x0A)]`，`bk 0x8000→0x800A`，`bit15` 保留 |
| 4 | `n0155.map` `(5,0)` | 格起 9052、尾区 9066..9088（**2 字节非零**）改一格后**逐字节原样** |
| 6 | 合成 Type5 4×2（146 字节） | 地表落笔 ⇒ `changed=0` + 「Type5 背图在块级，逐格写 bk_img 无效…地图一个字节都没动」；填充同拒；文件**一字节不动** |

### 9.4 门禁（本机实测）

| 命令 | 结果 |
|---|---|
| `cargo test -p miredit-core` | **121 passed / 0 failed**（lib 100 + 集成 20 + doctest 1）—— 改前 103，新增 18 条 |
| `cargo clippy -p miredit-core --all-targets -- -D warnings` | **EXIT=0** |
| `cargo build -p miredit-core` | **EXIT=0** |
| `cargo check --workspace --exclude miredit-gui` | **EXIT=0** |
| `cargo check --workspace` / `-p miredit-gui` | ❌ **红，但不是本单引入**：`miredit-gui/src/app.rs:949` 调 `dock_right::draw` 少传 `stamps` 参数（`dock_right.rs:27` 已改成 7 参）。**在途改动**证据：同一小时内该 crate 的报错集合变了三次（`draw_active_asset` 5↔6 参 → `cannot find function draw_stamp_active` → 只剩 1 条 arity），且 `miredit-gui` 源码里已有「2026-11-05：`miredit_core::Document` 的 `map` 字段改成了私有的」的适配注释 ⇒ 那边在同步适配本单的 API。 |
| `cargo metadata` 成员数 | **8**（未变） |

### 9.5 未做 / 未验证（如实登记）

1. **6 种格式零真实样本**（`CrystalCs` / `Mir3Wemade` / `Mir3Shanda` / `AntiHack` / `Wemade2010` / `Heroes34`）：
   只有合成样本覆盖（与 `mir2-format` 的结论一致），"能打开能写回"靠 writer 的往返契约，**没有真实语料验证**。
2. **信息丢失（编辑不到，但写回原样保留）**：`bk_idx` / `mid_idx` / `fr_idx` / `fr_count` / `flag` /
   `mid_ani_frame` / `mid_ani_tick` / `tile_frames` / 长度 ≠ 22 的整段 `tail`。
   **三档真实格式（Old12/Shanda14/Shanda2012）+ AntiHack 的丢失清单为空**（`lost_fields` 有测试钉住）。
3. **`view.header_raw` 是视图头**：前 4 字节强制 `w`/`h`（满足 `mir2_map::Map` 的不变量），其余照抄
   `raw.header_raw` 前缀、不足零填；对 52 字节头的三种格式是恒等，**对另外六种是视图专用合成，永不写回**。
4. **`view.format` 是语义投影**（按"格内有没有库号字节"+ "有没有 22 B 定长尾区"），**不是磁盘格长**；
   磁盘格长请用 `Document::disk_cell_len()`。
5. **`Document.path` 仍是 `PathBuf`（非任务书的 `Option<PathBuf>`）**：改成 Option 会连锁
   `save_session` / `SaveOutcome` / `CoreError` 三个签名，收益为 0（本 crate 的文档一律来自文件或
   显式传入的字节），故保持原样。
6. **`CoreError` 没有新开「出字节失败」变体**：`miredit-web/src/error.rs:117-128` 对 `CoreError` 是穷尽
   match 而本单禁止改 `miredit-web`，新变体会让整个 workspace 编译不过 ⇒ 复用 `CoreError::Map`
   （payload 就是 `FormatError`，写方向的拒绝是 `FormatError::Inconsistent`），并把该变体的语义与
   `Display` 放宽到**读/写两个方向**（`地图格式层失败（读/写）`），不留"说是解析其实是写"的谎话。

---

## 10. 印章放置（2026-11-05 新增）

把一枚 `.X` 印章贴进地图：**可撤销（整次一步）**、**可保存（逐字节可自证）**、**越界逐格裁剪**、
**格式能力闸门**。代码落在新模块 `src/stamp.rs`（`session.rs` 只留薄壳），文件级行号见各函数文档。

### 10.1 新增 API

| 项 | 位置 | 说明 |
|---|---|---|
| `StampCellInput` | `src/stamp.rs` | 相对偏移 `dx/dy`（可负）+ 三族图号 + 三个库号字节 |
| `PlaceOutcome` | `src/stamp.rs` | `Placed { cells, skipped }` / `Refused { reason }`（中文原因，禁静默） |
| `plan_stamp`（crate 内） | `src/stamp.rs` | **先算后写**的纯函数：拒绝时一格不写 |
| `EditSession::place_stamp` | `src/session.rs` | 锚点 + 格表 ⇒ 写视图 + **一步**撤销（`UndoStack::commit`） |
| `EditSession::register_stamp` / `remove_stamp` / `stamp_ids` / `stamp_cells` / `place_stamp_by_id` | `src/session.rs` | 印章 id → 格表（core **零 IO**，读盘由上层做） |
| `Tool::from_key` | `src/paint.rs` | 工具键唯一真源（4 个键，严格小写、不收别名、`unwired` 拒收） |
| `cmd::set_tool_by_key` | `src/cmd.rs` | 非法键 ⇒ `Ok(Outcome{changed:0, note})`，不改状态 |
| `Command::SetTool` / `Command::PlaceStamp` | `src/cmd.rs` | 命令集 **10 → 12 条**（契约变更，web/gui 已同步） |
| `STAMP_MAX_CELLS` / `STAMP_REGISTRY_MAX` | `src/stamp.rs` | 1,000,000 格/枚（同 C# `Document.cs:140`）/ 10,000 个 id（本仓护栏） |

### 10.2 三条与"想当然"不同的裁定（都有出处）

1. **越界 = 逐格跳过，不是整枚拒绝**：官方 `CanPlaceStamp`（`PlacedStamps.cs:61`）是整枚拒绝，
   但官方自己另有一条逐格判据 `CheckStampPlacement`（`:80` 对出界格 `blocked.Add(…MapBoundary); continue;`）。
   任务书 §三.2 要求逐格 ⇒ 取后者，`skipped` 如实计数；**全部越界**才拒绝。
2. **图号 0 的族整族不写**（含库号字节）：`PlacedStamps.cs:90-92` 的 `if(a.BackImage!=0){…}` /
   `if(a.MiddleImage!=0){…}` / `if (a.Foreground is { } front) …`；`Document.cs:31` 的 `Cell::Empty`
   把"全 0"定义为空。若照"三族无条件写"，贴一枚纯物件印章会顺手把目标图的地表清成 0。
3. **`CropStamp` 不在 `mir2-format` 里**（全 crate grep 0 命中）：真身是 C# 的
   `CrystalStudio/CrystalStudio/Document.cs:126-136`，做的是**造印章时**的居中裁剪（`!s.Cell.Empty`
   + 包围盒中心 + 偶偶对齐），**不是**放置时的地图边界裁剪 ⇒ 本模块只借它"空格不算内容"的口径。

### 10.3 实测（门 2~7，`cargo test -p miredit-core --test stamp_place -- --nocapture`）

| 门 | 样本 | 证据 |
|---|---|---|
| 2 | `mir_00000.X`（780 B）贴 `0.map` 副本 `(100,100)` | 25 格 / 包围盒 `(-1,-2,4,3)` / 8 格带前景；**改动 8 格、越界 0、差异 15 字节、差异格 8、头部差异 0**；`bk 0x8003→0x8003`、`bk 0x8004→0x8004`（`bit15` 保留）、`fr 0x0000→0x1E0x` |
| 3 | 同上，撤销一次 | `to_bytes()` 与原文件**逐字节相等**（5,880,052 B）；保存后再撤销 ⇒ 字节同样回到原文件，但**判脏**（盘上是带印章那份） |
| 2b | 同一枚印章贴"三族都带 `bit15`"的 12×12 合成图 | 25 格全部保留 `bk/fr` 的 `bit15`；中层只在"该族未写"时保留旧值；撤销逐字节还原 |
| 4 | 同一枚印章贴 `(0,0)` | 图内 18 格 / 真改 5 格 / **越界跳过 7 格**（独立推导同值）；差异 10 字节且全部落在图内印章格 |
| 5 | 贴 `(700,700)`（图外） | `Refused`「印章全部 25 格都在地图外…一格未写」；`to_bytes()` 与原文件相等、保存走 `Clean`、盘上不动 |
| 6 | 合成 Type5 4×2 | 带 `bk≠0` ⇒ `Refused`「…不支持逐格写地表背图（Type5 的背图在 4 格块里…）这枚印章有 2 格 bk≠0…一个字节都没动」；只带中层/物体 ⇒ 放行 2 格 |
| 7 | 8×8 合成图，两格印章同点两次 | 第二次 `cells=0` 且**不产生空撤销步**；撤销一次逐字节回到放之前；换一枚盖同点 ⇒ 后者胜，撤销逐级回退（2 步） |

### 10.4 门禁（本机实测，2026-11-05）

| 命令 | 结果 |
|---|---|
| `cargo test -p miredit-core` | **145 passed / 0 failed**（lib 117 + 集成 27 + doctest 1）—— 改前 121，**一条没少**，新增 24 条 |
| `cargo build -p miredit-core` | **EXIT=0** |
| `cargo clippy -p miredit-core --all-targets -- -D warnings` | **EXIT=0** |
| `cargo check --workspace --exclude miredit-gui --exclude miredit-web` | **EXIT=0** |
| `cargo check --workspace` | **EXIT=0**（web/gui 已同步 `SetTool` / `PlaceStamp` 两条臂） |
| `cargo clippy -p miredit-web --all-targets -- -D warnings` | **EXIT=0**（本单曾因 `cmd.rs` 的 `push_str("；")` 挡过它，已改 `push('；')`） |

### 10.5 未做 / 未验证（如实登记）

1. **真实印章只覆盖物体族**：`mir_00000.X` 的 25 格里 `back_image` / `middle_image` **全 0**
   （纯物件印章）⇒ 门 2 的"真实数据"只验到物体层 + `bit15`；地表/中层与**三个库号字节**的语义
   由 `src/stamp.rs` 的单元测试（`三族图号与库号都写` / `库号写不进去要说明` / `十二字节档库号忽略`）覆盖，
   **没有第二枚带地表库号的真实印章样本**。
2. **`0.map` 是 12 B/格**（无库号字节）⇒ 真实放置路径验不到库号字节落盘；库号字节只在
   `Shanda14` / `Shanda2012` / `CrystalCs` / `Mir3*` 上才有字节可写，这几种**本机无真实印章+地图组合**。
3. **印章旋转/翻转、多帧前景的 `Frames/Tick`**：本单只写三族图号 + 库号字节，
   `foreground.frames/tick` / `door` / `light` / `tile_*` **一律不写**（任务书 §三.3 的字段清单之外）。
4. **`Command::PlaceStamp` 的 `stamp` 只是 id**：core 不读盘（`mir2-stamp` 仅在 dev-dependencies），
   格表由上层 `register_stamp` 注册；未注册 ⇒ 明确拒绝并列出已注册 id。


---

## 11. 矩形选区 + 剪贴板（2026-11-05 新增，审计 P0-4）

九个功能点（设/取消选区、复制、剪切、粘贴、删除、填充选区）**共用一套选区结构**。
代码落在新模块 `src/selection.rs`（纯数据 + 纯计划），`session.rs` 只留 8 个薄方法 + 1 个回执访问器，
`cmd.rs` 加 7 条命令（命令集 12 → 19）。真值全部来自
`CrystalStudio/CrystalStudio/MapViewport.cs` 与 `CrystalStudio/CrystalStudio/RegionCopy.cs`。

### 11.1 新增 API

| 项 | 位置 | 说明 |
|---|---|---|
| `Selection` | `src/selection.rs:158` | 闭区间 `{x0,y0,x1,y1}`（字段 pub；反向坐标由 `xs()/ys()` 规范化） |
| `Selection::{width,height,count,contains,clamp_to,iter}` | `src/selection.rs:194/201/207/213/226/249` | 边界语义 + **入口裁剪一次**（复刻 `MapViewport.cs:390`/`:399`） |
| `Clipboard` | `src/selection.rs:267` | 格表 + 宽高（字段私有）；`cells()` 行优先、含空格 |
| `Clipboard::{width,height,cells,is_empty,from_cells}` | `src/selection.rs:275/280/285/290/298` | `from_cells` 校验 `w×h == cells.len()` |
| `Clipboard::capture`（crate 内） | `src/selection.rs:343` | 整块都抓；三族只存低 15 位（`bit15` 是阻挡位，粘贴永远保留**目标格**的）；**按 `vis` 过滤隐藏层**（见 §12） |
| `clear_plan`（crate 内） | `src/selection.rs:445` | **先算后写**的清空计划（三族按层清 + 库号字节写 0）；`vis` = 逐族闸门（剪切按可见性、删除传 `[true;3]`，见 §12） |
| `EditSession::{selection,set_selection,clear_selection,clipboard}` | `src/session.rs:899/911/935/940` | 选区是 UI 状态：不进撤销栈、不置脏 |
| `EditSession::{copy_selection,cut_selection,paste_clipboard}` | `src/session.rs:956/1006/1044` | 复制 / 剪切（复制+清空）/ 粘贴（复用 `plan_stamp`） |
| `EditSession::{delete_selection,fill_selection}` | `src/session.rs:1084/1156` | 删除 / 填充选区，各 **一步撤销** |
| `clear_selection_now`（私有） | `src/session.rs:1118` | 清空的唯一实现（算计划 → 写 → 一步撤销），**不拼文案** |
| `EditSession::last_clear` | `src/session.rs:237` | 清空回执（`cut_selection` 的返回值按契约是 `(宽,高)`，装不下格数） |
| `Command::{SetSelection,ClearSelection,CopySelection,CutSelection,PasteSelection,DeleteSelection,FillSelection}` | 变体 `src/cmd.rs:161/172/177/181/186/194/199`；`apply` 分支 `:346/371/381/395/416/437/446` | 命令集 12 → **19** 条 |

### 11.2 C# 真值（选区语义，逐条行号）

| # | 事实 | 源码 |
|---|---|---|
| 1 | 选区可空；`null` = 没有选区 | `MapViewport.cs:102`、`:107` |
| 2 | 起点必须图内、终点夹到图内 ⇒ **存下来的选区永远整块在图内** | `:390`、`:399` |
| 3 | 存 `(左,上,宽,高)`，宽高 = 端点差 **+1** | `:400` |
| 4 | **`Right`/`Bottom` 不含端点**（半开）；所有迭代都是 `x < r.Right` / `y < r.Bottom` | `:115`、`:540`、`:543`、`:546` |
| 5 | ⇒ 派单说的「闭区间」与源码**等价**：`x1 = Right - 1`，格集合完全相同 | 第 3 + 第 4 条合成 |
| 6 | 选区**不进撤销栈、不置脏**（与 `History` 并列存进 `MapEditorState`） | `:147`、`:152` |
| 7 | 复制**整块都抓**（空格也抓），`previewCells` 才过滤 | `RegionCopy.cs:21-27` |
| 8 | 粘贴**逐族判断**（图号为 0 的族不写）+ **保留目标格阻挡位** | `:78-87` |
| 9 | `ClearSelection` **只清当前层、不清库号字节** | `MapViewport.cs:546` |
| 10 | `CutSource` **三族一起清、库号字节写 0** | `RegionCopy.cs:45-69`（`:54`、`:59`、`:63`） |
| 11 | `FillSelection` 逐格调 `Apply`，而 `Apply` 的地表 `step=2` 且对齐偶偶格 | `MapViewport.cs:540` + `:474-475` |

### 11.3 与官方的四处**有意差异**（都写进注释，不是悄悄改）

1. **库号字节**：删除与剪切统一按 `CutSource`（`RegionCopy.cs:54/59`）清库号，
   而不是按 `ClearSelection`（`MapViewport.cs:546`，它不清）。裁定理由：`img=0` 时库号无意义，
   两个命令语义相同却行为不同是历史包袱。
2. **粘贴越界**：官方**整块拒绝**（`RegionCopy.cs:71/74/120`），本仓**逐格跳过**并在 `skipped` 报数
   —— 与 `stamp.rs` §二 同一条理由（官方自己也有逐格判据 `PlacedStamps.cs:80`）。
3. **粘贴锚点**：官方按源奇偶重算（`RegionCopy.cs:70`），本仓直接用调用方给的 `(x,y)`
   —— 本仓地表是逐格写（`session.rs:22-26`），没有"奇偶格"概念，重算只会让位置错开 1 格。
4. **`FillSelection` 的地表步长**：C# `step=2` 会让 3×3 选区溢出成 4×4（`:474-475`），
   本仓取 **step=1 严格填满选区**；代价是地表奇格写进去不显示（`compose.rs:107-116` 只铺偶偶格），属无害。

### 11.4 实测（门 2~7，`cargo test -p miredit-core --test selection_clipboard -- --nocapture --test-threads=1`）

| 门 | 样本 | 证据 |
|---|---|---|
| 2 | `0.map` 副本，源块 `(390,64)..(392,66)` → 粘到 `(200,200)` | 剪贴板 9 格：带地表 9 / 带中层 9 / 带物体 1；**改动 9 格、越界 0、差异 29 字节、差异格 9、头部差异 0**；三族图号逐格等于源、`bk/fr` 的 `bit15` 保留、中层 `bit15` 清 0 |
| 2 补 | 8×8 `Shanda14` 合成图（三族 + 三个库号可区分） | 9 格 `btTiles`/`btsmTiles`/`btArea` **逐格搬运**；差异 45 字节且只落在目标 3×3 |
| 3 | 同一源块剪切后保存 | **差异 20 字节、差异格 9、头部 0 差异**；每格 `bk &= 0x8000`、`mid = 0`、`fr &= 0x8000`，动画/门/光照/尾区不动 |
| 4 | 粘贴后撤销 / 剪切后撤销 | 各与操作前 `to_bytes()` **逐字节相等**（5,880,052 B） |
| 5 | 8×8 合成图 | 选区 `(6,6,99,99)` 裁成 `(6,6,7,7)`；粘贴到 `(7,7)` ⇒ `skipped=8`、改动 1 格；粘到 `(100,100)` ⇒ `Refused`「…全部 9 格都在地图外…一格未写」 |
| 6 | 8×8 合成图，笔刷取自 `(0,0)`（图号 1） | 选区 3×3 ⇒ **9 格全变**、一步撤销、撤销逐字节还原；保存后差异只落在选区 |
| 7 | 合成 Type5（`Mir3Wemade` 4×2） | 剪贴板含 `bk≠0` ⇒ `Refused`「粘贴被拒绝：…不支持逐格写地表背图…2 格 bk≠0…一个字节都没动」；把地表清 0 后 ⇒ 放行 2 格 |

### 11.5 实测修掉的一个真缺陷：回执自相矛盾（2026-11-05 02:1x）

**现象**（web 那路实测原文）：

```text
剪切 5×4 格（20 格被清空，已进剪贴板）；剪切 0 格（5×4）已进剪贴板；
选区 (20,20)..(24,23) 里没有可清的内容（三族图号都是 0），未改动
```

同一句里既说「20 格被清空」（假）又说「未改动」。**两个根因**：
1. `cut_selection` 把「复制段」与「删除段」两段回执拼在一起；
2. `cmd.rs` 又自己拼了一段摘要，再追加 `session.note()`（第三段）；
3. `last_clear` 在 `delete_selection` 早退（0 格）时**没被更新** ⇒ 沿用了上一次删除的 20。

**修法**：`cut_selection` 只出**一句** —— `剪切 W×H 格（已复制 N 格进剪贴板；清空 M 格）`，
`M == 0` 时明写「选区内容本来就是空的」；`delete_selection` 的成功回执与"没有可清的内容"**互斥**；
`last_clear` 在**每条退出路径**上都写；`cmd.rs` 对这两条命令**只透传**不再二次拼接；
告警（库号字节写不进去）统一用「另：」隔开，不与"改了几格"混成一句。
回归门：`tests/selection_clipboard.rs::附_剪切回执不出假话`（先 `DeleteSelection` 再 `CutSelection`
⇒ `changed == 0`、note 不含「被清空」/「未改动」）。

### 11.6 门禁（本机实测，2026-11-05 02:2x）

| 命令 | 结果 |
|---|---|
| `cargo test -p miredit-core` | **164 passed / 0 failed**（lib 126 + 集成 37 + doctest 1）—— 改前 145，**一条没少**，新增 19 条（lib 9 + 集成 10） |
| `cargo build -p miredit-core` | **EXIT=0** |
| `cargo clippy -p miredit-core --all-targets -- -D warnings` | **EXIT=0** |
| `cargo check --workspace --exclude miredit-gui --exclude miredit-web` | **EXIT=0** |
| `cargo check --workspace` | **EXIT=0**（web 那路已同步 7 条臂：`json.rs:152-158`、`:182-192`、`:213-217`、`:302-322`） |

### 11.7 未做 / 未验证（如实登记）

1. **`RegionCopy.cs:55` 的地表动画三字节**（`TileAnimation/TileOffset/TileFrames`）没清：
   它们只在 36 B 档存在，在本 crate 里落在**不透明的 22 字节尾区** `Cell::tail`，
   按猜测清尾区会写坏未知字节（`convert.rs` 的 `FIELD_MAP` 第 12 条）。
2. **`RegionCopy.cs:59` 的 `MiddleFrames/MiddleTick`** 没清：14 B 档里 CrystalStudio 把那两个字节
   读进 `FrontAnimationFrame/Tick`（`LegacyMapReader.cs:243-244`、`:257-258`），`MiddleAnimationFrame` 恒 0
   （`Document.cs:19-20`、`:63`）⇒ C# 自己也没有可清的中层动画；本 crate 的 `ani_frame/ani_tick`
   就是那两个字节，照 C# 的实际行为不动。
3. **选区/剪贴板不进撤销栈**：撤销**不会**恢复选区（C# 的 `MapEditorState` 同理，`:147`/`:152`）。
4. **剪贴板不跨会话**：`Clipboard` 只活在 `EditSession` 里；网页版要持久化得自己序列化格表
   （`Clipboard::from_cells` 提供了回流入口）。
5. **`miredit-gui` 未接**：本单只做内核，GUI 侧 7 条命令的按钮/快捷键接线不属本单。

---

## 12. 复制按图层可见性过滤（2026-11-05 新增）

审计补口：§11 的复制**不看图层可见性**，而 C# 的 `CaptureSelection` 是**按 `VisibleLayers` 定格**的。
真值全部来自 `CrystalStudio/CrystalStudio/RegionCopy.cs`（本轮逐行复核 `:3`、`:103-110`、`:78-82`）。

### 12.1 C# 原文（三条结论，先取证后动手）

```csharp
103:     private void CaptureSelection(bool cut)
104:     {
105:         if (Selection is not Rect r) { Message?.Invoke(LocalText.T("Select a region with M first")); return; }
106:         if (!VisibleLayers.Any(v => v) && !CollisionVisible) { Message?.Invoke(LocalText.T("Show at least one layer to copy")); return; }
107:         copiedRegion = new(Document, (int)r.X, (int)r.Y, (int)r.Width, (int)r.Height, VisibleLayers[0], VisibleLayers[1], VisibleLayers[2], LowCollisionVisible, HighCollisionVisible);
108:         if (cut) { CancelPickup(); copiedRegion.CutSource(Document, History, InvalidateCell); }
```
```csharp
  3: // A detached snapshot: layer choices are captured when copying, not when pasting.
 78:     bool pasteGround = ground && (source.BackImage & 0x1FFFFFFF) != 0;
 79:     bool pasteMiddle = middle && source.MiddleImage != 0;
 80:     bool pasteObjects = objects && source.HasForeground;
 82:     if (!pasteGround && !pasteMiddle && !pasteObjects && !pasteCollision) continue;
```

| # | 结论 | 依据 |
|---|---|---|
| 1 | 复制**按可见性过滤**：`VisibleLayers[0..2]` 在**复制瞬间**拍进快照 | `:107`；`:3` 的注释 |
| 2 | 三层全关 ⇒ **拒绝**（`"Show at least one layer to copy"`），**不建快照、不产生历史** | `:106` 的提前 `return` |
| 3 | **粘贴不看当前可见性**：`Paste` 只读快照里定格的 flag | `:3`、`:78-82` |

⚠ 细节订正（与"复制就裁剪 cells"的直觉不同）：C# **不裁剪 `cells` 数组**（`:24` 整格全 `Copy`），
它是把「哪些层参与」定格成三个布尔 flag，粘贴/剪切时按 flag 跳过。

### 12.2 新增 API

| 项 | 位置 | 说明 |
|---|---|---|
| `EditSession::visible_layers` | `src/session.rs:878` | 返回 `[bool; 3]`，顺序 `[地表, 中层, 物体]`（= `VisibleLayers[0..2]`） |
| `EditSession::set_visible_layers` | `src/session.rs:889` | 只影响**之后**的复制；不改地图、不进撤销栈、不置脏 |
| 字段 `visible: [bool; 3]` | 声明 `src/session.rs:119`、缺省 `src/session.rs:152`（`[true; 3]`） | 纯 UI 状态 |
| `Clipboard::capture` 加参数 `vis: [bool; 3]` | `src/selection.rs:343`（过滤表见该函数文档 §可见性过滤） | 隐藏层字段填 0 |
| `clear_plan` 加参数 `vis: [bool; 3]` | `src/selection.rs:445` | 隐藏的层**不清**（`RegionCopy.cs:52/57/61` 的 `if (ground/middle/objects && …)`） |
| `copy_selection` 三层全关闸门 | `src/session.rs:964-978` | 拒绝 + 中文原因，剪贴板/地图/撤销栈全不动 |
| `cut_selection` 传当前可见性 | `src/session.rs:1012` | 剪切只清可见层（否则"复制滤掉、剪切却删掉"） |
| `delete_selection` 传 `[true; 3]` | `src/session.rs:1086` | 删除键的 C# 对照 `MapViewport.cs:546` 不判可见性 ⇒ 仍三族全清 |

**实现口径**：在**复制时**就把隐藏层的字段抹成 0（而不是粘贴时查当前可见性）——
本 crate 的 `StampCellInput` 没有布尔 flag，但粘贴侧的判据本来就是「该族有值才写」
（`stamp.rs` 的 `合并粘贴附加字段`）⇒ 抹 0 与 C# 的快照 flag **逐字节等价**。

### 12.3 三条与 C# 的口径差（都写进注释，不是悄悄改）

| # | 差在哪 | 为什么 |
|---|---|---|
| 1 | 「三层全关」判据**不含碰撞层**（C# 是 `!VisibleLayers.Any(v => v) && !CollisionVisible`） | 本 crate 的剪贴板**不抓阻挡位**（`capture` 只取低 15 位，`bit15` 由粘贴时目标格保留矩阵决定）⇒ 三层全关就等于"没有任何可抓的东西" |
| 2 | `light` **不随层过滤** | 它不是任何一族的字段（C# `Cell.Light` 是整格属性），且粘贴路径根本不写它（`RegionCopy.cs:84-86` 无 `Light`） |
| 3 | 隐藏的物体层连带把 `door`/`door_offset`/`ani_frame`/`ani_tick` 抹 0 | 门与前景动画属于物体层（`RegionCopy.cs:86` 的 `if (pasteObjects) { target.Door = … }`） |

### 12.4 实测（4 条，`src/session_tests.rs`）

| 用例 | 位置 | 钉住什么 |
|---|---|---|
| `复制三层全开抓满三族` | `src/session_tests.rs:493` | 3×3 ⇒ 剪贴板 9 格，三族图号全非 0；复制不产生撤销步 |
| `复制隐藏物体层只滤物体族` | `src/session_tests.rs:516` | 关物体层 ⇒ `fr`/`fr_lib`/`door`/`door_offset`/`ani_*` 全 0，`bk`/`mid` 逐字段不变；反向再验关地表 / 关中层 |
| `复制三层全关拒绝且不动任何状态` | `src/session_tests.rs:573` | 拒绝 + 原因（含「全部隐藏」与源码出处）+ `undo_len` 不增 + 字节不动 + 旧剪贴板原样保留 + 可见性/选区状态不变 |
| `剪切按可见性只清可见层` | `src/session_tests.rs:603` | 关物体层再剪切 ⇒ 剪贴板物体族全 0、地图物体层**原样**、可见两层被清、撤销一步逐字节还原 |

### 12.5 门禁（本机实测，2026-11-05 13:1x）

| 命令 | 结果 |
|---|---|
| `cargo test -p miredit-core` | **176 passed / 0 failed**（lib 132 + 集成 42 + doctest 2）—— 改前 **172**，新增 4 条，**一条没少** |
| `cargo clippy -p miredit-core --all-targets -- -D warnings` | **EXIT=0**（无告警） |
| `cargo check --workspace --exclude miredit-gui --exclude miredit-web` | **EXIT=0** |

⚠ **环境坑（不是本轮改动引起，已实测复现两次）**：`tests/formats_9.rs:68-73` 的 `临时目录(tag)` 用的是
**固定目录名**（`%TEMP%\miredit_core_formats9_gate12`，无 PID），两个 `cargo test -p miredit-core` 同时跑会互相
`remove_dir_all` / 抢同一个 `copy.map.miredit-tmp` ⇒ 症状是「读回 N 字节 ≠ 期望」或「系统找不到指定的文件 (os error 2)」。
清掉 `%TEMP%\miredit_core_formats9_*` 后单跑通过；随后三次全量连跑均 **exit=0**。要彻底消除，需给目录名加 PID。

### 12.6 未做 / 未验证（如实登记）

1. **`miredit-gui` / `miredit-web` 未接**：`visible_layers` 是纯内核 API，界面侧还没有三层的勾选框
   （那两个 crate 本轮禁止改动）。
2. **碰撞层可见性没建模**：C# 有 `CollisionVisible`/`LowCollisionVisible`/`HighCollisionVisible` 三个开关，
   本 crate 只有三层（派单要求 `[bool; 3]`），且剪贴板不抓阻挡位 ⇒ 碰撞层对复制无影响（见 12.3 第 1 条）。
3. **`delete_selection` 仍不按可见性**（`[true; 3]`）：它的 C# 对照是 `MapViewport.cs:546`，那边也不判可见性；
   若产品上要求"删除键也遵守可见性"，需要另一条裁定（不是漏改）。