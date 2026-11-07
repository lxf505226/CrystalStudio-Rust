//! 中列**标签条**（R12-F3 B 段，2026-11-07）：多标签的「文件名 + 脏标记」绘制与点击切换。
//!
//! # 上游真值（B1-FIX，2026-11-08 逐字核对 `_upstream/CrystalStudio/`）
//!
//! **上一版把这里写成"上游无多标签标签条、层次是本仓自定"——那是错的**：上游有完整实现，
//! 真值逐条如下（每个数值都在下面绘制代码的注释里重抄一次，方便就地核对）：
//!
//! | 项 | 上游出处 | 真值 |
//! |---|---|---|
//! | 标签栏容器 | `MainWindow.MapTabs.cs:32-37` | `StackPanel Horizontal` + `ScrollViewer`（横向 `Auto`、纵向 `Disabled`）；底色 `Theme1C2028` = `#1C2028`（`App.axaml:8`） |
//! | 标签项样式 | `EditorControls.axaml:58-66` `PaletteTabItem` | `Padding="8,7"`、`FontSize=11`、未选中前景 `Muted` `#929CAB`；`Border#Chrome` 背景 `Transparent`、`BorderThickness="0,0,0,2"` |
//! | 悬停 | `EditorControls.axaml:63` | `:pointerover Border#Chrome` 背景 `Theme29313E` = `#29313E`（`App.axaml:14`） |
//! | 选中 | `EditorControls.axaml:64-65` | 底边 `Theme8CA1FF` = `#8CA1FF`（`App.axaml:47`，厚 2）；前景 `ThemeE5E9F0` = `#E5E9F0`（`App.axaml:67`）+ `FontWeight=SemiBold` |
//! | 标签内容 | `MainWindow.MapTabs.cs:53-62` | 标题 `Label.Margin="4,2,8,2"`、居中；关闭钮 `×`(U+00D7) `Padding="4,0,4,0"`、透明底、无边框 |
//! | 标题文本 | `MainWindow.MapTabs.cs:120` | `string.Concat(MapDisplayName(map), dirty ? " *" : "")` —— 脏标记是**直接拼接 `" *"`**，不是模板 |
//! | 未命名地图 | `MainWindow.MapTabs.cs:125-131` | 有 `Path` ⇒ `Path.GetFileName(path)`；无 `Path` ⇒ `Untitled {n}`（见 [`crate::ui_text::TAB_TITLE_UNTITLED_TMPL`]，本仓**暂不可达**） |
//!
//! **本段只对齐"绘制层"**：横向滚动 / 关闭动作 / 右键菜单 4 项 / 拖拽重排 / 0 标签自动新建
//! 都不在本单（见回执「明确不做」）。
//!
//! # 本段范围（**只做两件事**）
//!
//! 1. 在页签条（[`Layout::tab_strip`]，标题栏/菜单栏之下、画布之上那条 38px 的带）里
//!    画出**每一张**标签的「文件名 + 脏标记」；
//! 2. 点某张**非活跃**标签 ⇒ [`Tabs::activate`] 切过去。
//!
//! 因此这里**没有**任何 `×` 的点击处理 —— 那个字形是既有装饰
//! （`canvas::labels` 的 `canvas.tab_close`），本段只把它按标签复制过去，语义不变。
//!
//! # 为什么不动 `Layout`
//!
//! 页签条**本来就存在**（`metrics::TABSTRIP_H = 38`，上游为 `GridLength.Auto`、
//! 本仓固定 38 是既有截图基线，见 `metrics::TABSTRIP_H` 的说明），
//! `Layout::viewport` 的上边界就是 `tab_strip.bottom()`。多标签只是把**同一条带**横向切成 N 份
//! ⇒ **`l.viewport` 一个数都没变**，`--shot` 的画布几何基线与上一版逐像素相同
//! （见本文件 `单标签矩形与旧版逐值相同` 用例）。
//!
//! # ★ 稳态契约：活跃标签的信息必须从宿主读
//!
//! [`crate::tabs`] 的契约是「`items[active]` 是占位槽、真值常驻 `app.*`」。
//! 因此**活跃**标签的文件名/脏标记走 [`Tabs::显示信息`]（内部读 `app.doc` / `app.edit`），
//! **非活跃**标签才读 `tabs.items[i]` 的快照。本模块**不直接碰** `tabs.items`
//! —— 全部经那一个只读入口，避免"读占位槽画出假名字/假脏状态"。
//!
//! # 与 `canvas::draw` 的分工（为什么不重复画第 0 张）
//!
//! `canvas::draw` 早就画好了**一张**标签（条底 + 标题 + 关闭符 + 选中下划线），位置是
//! `Layout::tab_rect()` = 标签条第 0 格。本段与它的分工是：
//!
//! | 情形 | 谁画第 0 张 |
//! |---|---|
//! | `active == 0`（**唯一可达的单标签态**，也是 CLI/`--shot` 的常态）| `canvas::draw`，本段**跳过** ⇒ 像素与上一版完全相同 |
//! | `active != 0` | `canvas::draw` 画的是**活跃**标签的标题（错位），本段先铺底色盖掉再按第 0 张的真实内容重画 |
//!
//! 这样单标签路径**零像素变化**（`--shot` 哈希不变），多标签路径也不会出现
//! "第 0 格显示的是活跃图名字"这种错位。


use egui::{pos2, vec2, Align2, Painter, Rect, Sense};

use crate::app::MireditApp;
use crate::layout::Layout;
use crate::metrics as m;
use crate::paint as pt;
use crate::tabs::Tabs;
use crate::text_fit::{DynLabel, LabelStyle};
use crate::theme as t;
use crate::ui_text as tx;

/// 第 `i` 张标签的矩形（`i` 从 0 起，顺序 = `Tabs::items` 顺序）。
///
/// # 几何口径（B1-FIX 按上游订正，见模块文档的真值表）
///
/// - 高度恒 `metrics::TAB_H`（30），贴页签条顶部（与 `Layout::tab_rect` 同口径）；
/// - **相邻标签边界相接、没有 gap**：上游相邻 `Border#Chrome` 背景 `Transparent`、
///   `BorderThickness="0,0,0,2"`（`EditorControls.axaml:60`），`StackPanel Orientation="Horizontal"`
///   里没有 Margin（`MainWindow.MapTabs.cs:32`）⇒ 视觉间隔全部来自标签**内部**的
///   `Padding="8,7"`（`:59`）与 `Label.Margin="4,2,8,2"`（`MapTabs.cs:54`）。
///   （旧版这里有个自定的 `TAB_GAP = 2`，已删，见 `metrics` 的删除说明。）
/// - 宽度 `min(TAB_W, 可用宽/n)`：`TAB_W` 是参考标题 `Untitled 1 ×` 的**实测自然宽** 98
///   （截图 x164..262，见 `metrics::TAB_W`）；放不下时等分收窄、**不设最小宽下限**
///   （旧版自定的 `TAB_MIN_W = 40` 已删）。上游此时是横向滚动
///   （`MapTabs.cs:34` `HorizontalScrollBarVisibility="Auto"`），本单不做滚动。
///
/// `n == 1` 时返回的唯一矩形与 [`Layout::tab_rect`] **逐值相同**（有断言钉住）。
pub fn rects(l: &Layout, n: usize) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    let strip = l.tab_strip;
    // 没有 gap ⇒ 每个标签占满 1/n；上限是参考标题的自然宽 `TAB_W`（放得下就按自然宽排）。
    let 宽 = (strip.width() / n as f32).min(m::TAB_W);
    (0..n)
        .map(|i| {
            Rect::from_min_size(
                pos2(strip.left() + i as f32 * 宽, strip.top()),
                vec2(宽, m::TAB_H),
            )
        })
        .collect()
}

/// 本帧**需要本模块亲自画**的标签下标（`canvas::draw` 已经画好的那张不算）。
///
/// 判据只有一条：第 0 张且它**就是活跃标签** ⇒ `canvas::draw` 画的正是它
/// （标题、关闭符、选中下划线都在那张矩形里）⇒ 跳过。其余全部由本模块画：
/// - 第 0 张不是活跃标签 ⇒ `canvas` 画的是**活跃**标签的内容，必须盖掉重画；
/// - 第 1 张起 ⇒ `canvas` 根本没画（它只有一张的位置）。
///
/// 单标签态（`len == 1` 且 `active == 0`，也是 CLI / `--shot` 的常态）⇒ 返回空表
/// ⇒ **本模块一个像素都不动**，`--shot` 哈希与 A 段逐字节相同。
///
/// ⚠ **B2 单已修掉的两处旧值**（`canvas.rs`，同一写域）：单标签态那张由 `canvas.rs` 画，
/// 原先 ① 关闭符前景取 `text_muted`、② 选中下划线取 `accent_dark`；现在按上游换成
/// `ui.text`（`QuietButton` ⇒ `Button` 前景 `ThemeE5E9F0`，`EditorControls.axaml:4`）与
/// `ui.tab_selected_underline`（`Theme8CA1FF`，`:64`）。
/// ⇒ 同一张标签在「只有 1 张」与「有 2 张」时这两笔**同色**了。
/// ⚠ 其中**关闭符那一笔在浅色下会变像素**（`text` `#434D5A` ≠ `text_muted` `#9EA4AB`），
/// 所以 `--shot` 基线随之更新（见回执「--shot」段）；下划线那一笔浅色同值、像素不变。
pub fn 要画的标签(tabs: &Tabs) -> Vec<usize> {
    (0..tabs.len())
        .filter(|&i| !(i == 0 && i == tabs.active()))
        .collect()
}

/// 标签矩形 `r` 上**关闭符的命中盒**（`r` 取自 [`rects`] 的某一项）。
///
/// # 横向：上游 `MainWindow.MapTabs.cs:57` 的按钮盒
///
/// ```csharp
/// var close = new Button { Content = ("\u00d7"), Padding = new Thickness(4, 0, 4, 0), … };
/// ```
/// ⇒ 按钮盒 = 字形盒左右各加 **4**（上下 0）。字形在本仓是 `Align2::RIGHT_CENTER`
/// 锚在 `r.right() - TAB_TITLE_INSET`（[`draw`] 与 `canvas::labels` 同一处几何），
/// 所以按钮盒的**左边界** = `r.right() - TAB_TITLE_INSET - TAB_CLOSE_GLYPH_W - TAB_CLOSE_PAD`。
///
/// # 纵向：**整格高**（本仓放大的命中盒，见下）
///
/// 上游上下 padding 是 0 ⇒ 按钮高 = 字形行高 11（`TAB_CLOSE_GLYPH_H`）。
/// 但命中盒取**整格高** `r.top()..r.bottom()`，理由是上游**点标签右上角会选中该标签**
/// （那一块属于 `TabItem` 的 header），若命中盒只有 11 高，右上/右下两个角会变成
/// **既不开也不关的死区** —— 那是本仓几何带来的新行为，不是上游行为。
/// 放大命中盒只影响可点性，**不改变任何绘制像素**（悬停底另见 [`close_button_rect`]）。
///
/// 右边界取到标签右沿 ⇒ 与 [`body_rect`] **恰好拼满整张标签、互不重叠**。
///
/// ⚠ **待拍板**：命中盒比上游按钮高（30 vs 11）是本仓的**产品取舍**，已登记在回执
/// 「待用户拍板」；若要严格照抄上游 11 高，把本函数改成返回 [`close_button_rect`] 即可。
pub fn close_rect(r: Rect) -> Rect {
    let 左 = r.right() - m::TAB_TITLE_INSET - m::TAB_CLOSE_GLYPH_W - m::TAB_CLOSE_PAD;
    Rect::from_min_max(pos2(左.max(r.left()), r.top()), r.max)
}

/// **上游那个关闭按钮的绘制盒**（悬停底就画这么大）：15.34375 × 11，垂直居中。
///
/// = `TAB_CLOSE_PAD*2 + TAB_CLOSE_GLYPH_W` 宽、`TAB_CLOSE_GLYPH_H` 高，右边界落在
/// `r.right() - TAB_TITLE_INSET + TAB_CLOSE_PAD`（= 字形右边界 + 4，与 [`close_rect`] 同一处横向基准）。
pub fn close_button_rect(r: Rect) -> Rect {
    let 右 = r.right() - m::TAB_TITLE_INSET + m::TAB_CLOSE_PAD;
    let 左 = 右 - (m::TAB_CLOSE_PAD * 2.0 + m::TAB_CLOSE_GLYPH_W);
    let 半高 = m::TAB_CLOSE_GLYPH_H / 2.0;
    let cy = r.center().y;
    Rect::from_min_max(pos2(左, cy - 半高), pos2(右, cy + 半高))
}

/// 标签**主体**的命中盒 = 整张标签减去 [`close_rect`] 那一竖条（两者恰好拼满、不重叠）。
pub fn body_rect(r: Rect) -> Rect {
    Rect::from_min_max(r.min, pos2(close_rect(r).left(), r.bottom()))
}

/// **关闭符的悬停底**（上游 `QuietButton` 继承的 `:pointerover` 底）。
///
/// 上游真值：`EditorControls.axaml:17` 的 `QuietButton` 是
/// `BasedOn="{StaticResource {x:Type Button}}"`、只把 `Background`/`BorderBrush`
/// 覆盖成 `Transparent` ⇒ 基模板 `:pointerover /template/ Border#Chrome` 的
/// `Background = Theme394354`（`:12`，`App.axaml:26` = `#394354`）**照样生效**，
/// 圆角取同一个模板的 `CornerRadius="5"`（`:9`）。
///
/// 只在指针**压住这张标签的关闭符命中盒**时画，画在 [`close_button_rect`] 里
/// （上游按钮原尺寸，不是放大的命中盒）。
/// 返回 `true` = 画了（调用方据此可以在别处做提示，但**前景色不变**：
/// `:pointerover` 只设 `Background`/`BorderBrush`，`Foreground` 仍是 `ThemeE5E9F0`）。
pub fn paint_close_hover(p: &Painter, r: Rect) -> bool {
    let 层 = p.layer_id();
    let 区 = p.clip_rect();
    if !p
        .ctx()
        .rect_contains_pointer(层, 区.intersect(close_rect(r)))
    {
        return false;
    }
    pt::fill_round(
        p,
        close_button_rect(r),
        m::TAB_CLOSE_RADIUS,
        t::ui().close_hover_bg,
    );
    true
}

/// 画整条标签栏（顺序 = [`rects`] 顺序）。
///
/// `app` 只用于**活跃标签**的真值（见模块文档的稳态契约）。
///
/// # 每个色值的上游出处（逐条照抄，改动前先回源）
///
/// | 画什么 | 取色 | 上游出处 |
/// |---|---|---|
/// | 标签常态底 | `ui.tab_strip_bg` `#1C2028` | `Border#Chrome Background="Transparent"`（`EditorControls.axaml:60`）透出容器底 `Theme1C2028`（`MainWindow.MapTabs.cs:36` / `App.axaml:8`）|
/// | 标签悬停底 | `ui.tab_hover_bg` `#29313E` | `:pointerover /template/ Border#Chrome`（`EditorControls.axaml:63` / `App.axaml:14`）|
/// | 选中底边 2px | `ui.tab_selected_underline` `#8CA1FF` | `:selected` 的 `BorderBrush`（`EditorControls.axaml:64` / `App.axaml:47`）+ `BorderThickness="0,0,0,2"`（`:60`）|
/// | 选中标题 | `ui.text` `#E5E9F0` | `:selected Foreground`（`EditorControls.axaml:65` / `App.axaml:67`）|
/// | 未选中标题 | `ui.text_muted` `#929CAB` | `Foreground={DynamicResource Muted}`（`EditorControls.axaml:59` / `App.axaml:70`）|
/// | 关闭符 `×` | `ui.text` `#E5E9F0` | `close.Theme = QuietButton`（`MainWindow.MapTabs.cs:58`）⇒ `Button` 默认前景 `ThemeE5E9F0`（`EditorControls.axaml:4`）|
/// | 关闭符悬停底 | `ui.close_hover_bg` `#394354` | `QuietButton` 继承基模板 `:pointerover /template/ Border#Chrome` 的 `Background` = `Theme394354`（`EditorControls.axaml:17` + `:12` / `App.axaml:26`）；圆角 5 取同一模板的 `CornerRadius`（`:9`）|
pub fn draw(p: &Painter, l: &Layout, tabs: &Tabs, app: &MireditApp) {
    let rects = rects(l, tabs.len());
    let ps = p.with_clip_rect(l.tab_strip);
    let ui = t::ui();
    // 悬停判据：与 `Ui::rect_contains_pointer` **同一条实现** ——
    // `egui-0.33.3/src/ui.rs:1236-1239` 就是
    // `ctx().rect_contains_pointer(layer_id, clip_rect().intersect(rect))`，
    // 本函数只拿到 `Painter`（没有 `Ui`），所以按同一式子直接调 `Painter::layer_id()`
    // / `Painter::clip_rect()`（`painter.rs:162-171`）把这两项补上，语义完全一致
    // （含"指针所在图层必须是本图层"这一条，弹层压住时不会误亮）。
    let 悬停层 = ps.layer_id();
    let 悬停区 = ps.clip_rect();
    // ⓪ **关闭符悬停底的分工**（本函数只负责它亲自画的那几张）：
    // 「第 0 张且它就是活跃标签」那一张由 `canvas::draw` 画，而 `canvas::draw` 在本函数
    // **之前**跑（`app.rs` 的 ⑥ 画布 / ⑦a3 标签条）⇒ 若在这里再盖一块底色，会把
    // `canvas` 已经画好的 `×` 字形盖掉。所以那一张的悬停底由 `canvas.rs` 在**字形之前**
    // 调同一个 [`paint_close_hover`]；本函数只在本循环里给**自己画的那几张**画悬停底
    // （见循环内的 `paint_close_hover(&ps, *r)`）。实现只有一份，绘制顺序各随其主。
    for i in 要画的标签(tabs) {
        let Some(r) = rects.get(i) else { continue };
        let 活跃 = i == tabs.active();
        let Some(信息) = tabs.显示信息(app, i) else {
            continue;
        };
        // 第 0 格被 `canvas::draw` 按**活跃**标签画过（标题 + 关闭符 + 选中下划线），
        // 而 `active != 0` 时那些内容属于活跃标签 ⇒ 必须先按条底色抹掉再重画。
        //
        // 抹的范围是 `canvas` 真画过的那两块矩形（`Layout::tab_rect` / `Layout::tab_underline`），
        // **不是** `rects[0]` —— 标签多到收窄时 `rects[0]` 比 `tab_rect` 窄，
        // 只抹 `rects[0]` 会把活跃标签的标题尾巴留在右边。
        if i == 0 {
            for r0 in [l.tab_rect(), l.tab_underline()] {
                pt::fill(&ps, r0, ui.tab_strip_bg);
            }
        }

        // 底色：常态 = 容器底（上游 `Border#Chrome` 背景是 `Transparent`，透出的就是它）；
        // `:pointerover` = `Theme29313E`。**没有"活跃/非活跃"两档底色** ——
        // 上游选中态只改底边 `BorderBrush` 与前景，不改 Background（`EditorControls.axaml:60-65`）。
        let 悬停 = p.ctx().rect_contains_pointer(悬停层, 悬停区.intersect(*r));
        pt::fill(
            &ps,
            *r,
            if 悬停 {
                ui.tab_hover_bg
            } else {
                ui.tab_strip_bg
            },
        );

        // 标题：左内边距与 `canvas::dyn_labels` 的 `canvas.tab_title` 同口径；
        // 右边界按上游 `Label.Margin` 的右边距 8 内缩（`MainWindow.MapTabs.cs:54`）。
        let 左 = r.left() + m::TAB_TITLE_INSET;
        let 右 = (r.right() - m::TAB_TITLE_INSET - m::TAB_LABEL_MARGIN_R).max(左 + 1.0);
        let clip = Rect::from_min_max(pos2(左, r.top()), pos2(右, r.bottom()));
        let mut lab = DynLabel::new(
            "tab_strip.title",
            信息.tmpl,
            信息.text.clone(),
            LabelStyle::new(
                m::TAB_FONT,
                pos2(左, r.center().y),
                Align2::LEFT_CENTER,
                clip,
                // 上游选中 `ThemeE5E9F0`（`EditorControls.axaml:65`）= 本板 `text`；
                // 未选中 `Muted` `#929CAB`（`:59` / `App.axaml:70`）= 本板 `text_muted`（深色逐字节相同）。
                if 活跃 { ui.text } else { ui.text_muted },
            ),
        );
        lab.truncate_to_fit(&ps);
        lab.draw(&ps);

        // 关闭符（与 `canvas.tab_close` 同位置同字号；**命中盒与关闭动作在 [`interact`]**）。
        // 上游 `close.Theme = QuietButton` ⇒ 前景 `ThemeE5E9F0` = `ui.text`（`EditorControls.axaml:4`）。
        // 悬停底先画、字形后画（底在字下面）：上游 `:pointerover` 只换 `Border#Chrome` 的
        // `Background`，前景不变（`EditorControls.axaml:12` 只设 Background/BorderBrush）。
        paint_close_hover(&ps, *r);
        pt::text(
            &ps,
            pos2(r.right() - m::TAB_TITLE_INSET, r.center().y),
            Align2::RIGHT_CENTER,
            tx::TAB_CLOSE,
            m::TAB_FONT,
            ui.text,
        );

        // 选中底边：只画在**活跃**那一张的底部（`canvas::draw` 只在第 0 张画，
        // `active != 0` 时它那条已经被上面的底色铺盖掉了）。
        // 厚 2 = 上游 `BorderThickness="0,0,0,2"`（`EditorControls.axaml:60`）。
        if 活跃 {
            pt::fill(
                &ps,
                Rect::from_min_max(pos2(r.left(), r.bottom() - m::TAB_UNDERLINE_H), r.max),
                ui.tab_selected_underline,
            );
        }
    }
}

/// 标签条的一次点击**落在哪**（三态：主体 / 关闭符 / 不响应）。
///
/// 上游只有两个控件：`TabItem` 的 header（点它由 `SelectionChanged` 切图，
/// `MainWindow.MapTabs.cs:38-42`）与它里面的关闭 `Button`（`:57-61`）。
/// 本仓是**每帧派生式**、结构上没有 `SelectionChanged` 事件 ⇒ 这里把
/// "点哪儿"如实回报给宿主，由宿主按 [`Tabs::dirty_of`] 决定要不要进确认态
/// （上游靠"先选中再判脏"，本仓**不许照抄那个时序**，见 `app::request_close_tab`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// 点**标签主体** ⇒ 请求切换到这张（活跃那张由宿主忽略，见 [`interact`]）。
    Switch(usize),
    /// 点**关闭符** ⇒ 请求关闭这张（是否真关由宿主按脏判据决定）。
    Close(usize),
}

/// 标签条的点击命中（三态，见 [`Click`]）。
///
/// 命中盒：主体 = [`body_rect`]、关闭符 = [`close_rect`]，两者**互不重叠**且拼满整张标签。
/// 返回 `None` 的两种情况：① 没点任何标签；② 点的是**当前活跃标签的主体**
/// （[`Tabs::activate`] 本身也是 no-op，这里提前拦下是为了"点活跃标签不产生任何副作用"
/// 这条能被单测直接读到）。
///
/// ⚠ 点**活跃标签的关闭符**照样返回 [`Click::Close`] —— 关掉活跃那张是最常见的关闭路径。
pub fn interact(ui: &mut egui::Ui, l: &Layout, tabs: &Tabs) -> Option<Click> {
    let rects = rects(l, tabs.len());
    for (i, r) in rects.iter().enumerate() {
        // ① 关闭符（先判，与上游"按钮压在 header 之上"的层次一致；两个盒子不重叠，
        //    所以先后只影响"谁能先吃到"，不影响语义）。
        if ui
            .interact(
                close_rect(*r),
                ui.id().with(("tab_close", i)),
                Sense::click(),
            )
            .clicked()
        {
            return Some(Click::Close(i));
        }
        // ② 主体：点活跃标签不产生副作用。
        if ui
            .interact(body_rect(*r), ui.id().with(("tab_body", i)), Sense::click())
            .clicked()
            && i != tabs.active()
        {
            return Some(Click::Switch(i));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::cli::Cli;
    use crate::edit::EditCmd;
    use crate::tabs::{TabState, Tabs};

    /// 窗口客户区（与运行时同尺寸）。
    fn 窗() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(m::WINDOW_W, m::WINDOW_H))
    }

    /// 最小合法 1×1 Old12 地图（52 字节头 + 12 字节格），`(0,0)` 的 `bk_img = 7`。
    ///
    /// 给 `--edit-erase` 一个真能改掉的值，从而产生**真脏态**（全 0 的图落笔不改任何字节，
    /// 造不出脏标记）。
    ///
    /// 文件名取**短名**（`a.map` / `b.map`）：标签宽 `TAB_W = 98` 减去左右内边距与关闭符
    /// 只剩约 79px，长文件名会被 `truncate_to_fit` 截成 `…`，那样断言就变成在测截断
    /// 而不是在测"哪张标签读了谁"。
    ///
    /// ⚠ 目录必须**每次调用唯一**（`NEXT_ID` 自增）：`cargo test` 的用例是**并行线程**，
    /// 只按 `tag + 进程 id` 命名会让两个用例共用同一目录，先跑完的那个在收尾时删目录，
    /// 后跑的那个就 `os error 3`（本用例第一版实测踩到，全量跑才红、单跑绿）。
    fn 临时图(tag: &str) -> PathBuf {
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "miredit_tabstrip_{tag}_{}_{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("测试目录必须能建");
        let mut bytes = vec![0u8; mir2_map::HEADER_LEN + 12];
        bytes[0..2].copy_from_slice(&1u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&1u16.to_le_bytes());
        let base = mir2_map::HEADER_LEN;
        bytes[base..base + 2].copy_from_slice(&7u16.to_le_bytes());
        let p = dir.join(format!("{tag}.map"));
        std::fs::write(&p, &bytes).expect("测试合成图必须能写");
        p
    }

    /// [`临时图`] 的目录唯一化计数器（并行用例各拿各的目录）。
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    /// 造宿主（`编辑 = true` 时走 `--edit-erase` 生产路径 ⇒ 有撤销步 + 脏标记）。
    fn 建应用(图: &Path, 编辑: bool, tag: &str) -> MireditApp {
        let mut cli = Cli {
            map: Some(图.to_path_buf()),
            settings: Some(std::env::temp_dir().join(format!(
                "miredit_tabstrip_settings_{tag}_{}.json",
                std::process::id()
            ))),
            ..Default::default()
        };
        if 编辑 {
            cli.edits.push(EditCmd::Erase(0, 0));
        }
        MireditApp::new(&cli)
    }

    /// 从宿主 `capture` 出一份**真快照**（非活跃槽用）。
    fn 造槽(图: &Path, 编辑: bool, tag: &str) -> TabState {
        let mut app = 建应用(图, 编辑, tag);
        let mut 槽 = TabState::占位();
        crate::app::capture(&mut app, &mut 槽);
        槽
    }

    /// ① **单标签 ⇒ 一个矩形，且与旧版 `Layout::tab_rect()` 逐值相同**。
    ///
    /// 这条同时钉住"标签条没有改画布几何"：`viewport` 的上边界仍是 `tab_strip.bottom()`。
    #[test]
    fn 单标签矩形与旧版逐值相同() {
        let l = Layout::compute(窗());
        let rs = rects(&l, 1);
        assert_eq!(rs.len(), 1, "单标签只该有一个矩形");
        let 旧 = l.tab_rect();
        assert!((rs[0].left() - 旧.left()).abs() < 0.01);
        assert!((rs[0].top() - 旧.top()).abs() < 0.01);
        assert!((rs[0].width() - 旧.width()).abs() < 0.01);
        assert!((rs[0].height() - 旧.height()).abs() < 0.01);
        assert!(
            l.tab_strip.contains_rect(rs[0]),
            "标签 {:?} 越出页签条 {:?}",
            rs[0],
            l.tab_strip
        );
        // 画布几何未被本段改动
        assert!((l.viewport.top() - l.tab_strip.bottom()).abs() < 0.01);
    }

    /// ② **双标签 ⇒ 两个矩形，互不重叠、都在栏内**。
    ///
    /// ⚠ B1-FIX 调整（唯一一处改断言的用例，硬约束允许「改常量来源不改断言意图」）：
    /// 旧版靠自定 `TAB_GAP = 2` 让两张矩形**分离**，于是 `!a.intersects(b)` 成立。
    /// 现在按上游**没有 gap**、两张**边界相接**（`EditorControls.axaml:60` 无 Margin），
    /// 而 `emath` 的 `Rect::intersects` 是**闭区间**判据（`rect.rs:250-255` 用 `<=`），
    /// 相接的矩形会被判成"相交" ⇒ 改判**内部**不重叠（各内缩 0.5px），
    /// 断言意图（两张标签不重叠、按顺序右排）一字未改，强度只增不减
    /// （旧版 `intersects` 连 1px 的分离都算不重叠，新版还要求不侵入对方内部）。
    #[test]
    fn 双标签矩形互不重叠且在栏内() {
        let l = Layout::compute(窗());
        let rs = rects(&l, 2);
        assert_eq!(rs.len(), 2);
        for (i, r) in rs.iter().enumerate() {
            assert!(
                l.tab_strip.contains_rect(*r),
                "第 {i} 张 {:?} 越出页签条 {:?}",
                r,
                l.tab_strip
            );
            assert!((r.height() - m::TAB_H).abs() < 0.01, "第 {i} 张高度不对");
        }
        assert!(
            !rs[0].shrink(0.5).intersects(rs[1].shrink(0.5)),
            "两张标签内部重叠：{:?} / {:?}",
            rs[0],
            rs[1]
        );
        assert!(rs[0].right() <= rs[1].left() + 0.01, "第 1 张必须在第 0 张右边");
    }

    /// ②b **相邻标签边界相接、没有自定间隙**（B1-FIX：删掉 `metrics::TAB_GAP` 的可执行证据）。
    ///
    /// 上游没有 gap：相邻 `Border#Chrome` 背景 `Transparent`、`BorderThickness="0,0,0,2"`
    /// （`EditorControls.axaml:60`），`StackPanel Orientation="Horizontal"` 里没有 Margin
    /// （`MainWindow.MapTabs.cs:32`）⇒ 视觉间隔全部在**标签内部**由 `Padding="8,7"`（`:59`）
    /// 与 `Label.Margin="4,2,8,2"`（`MapTabs.cs:54`）撑出。
    #[test]
    fn 相邻标签边界相接无自定间隙() {
        let l = Layout::compute(窗());
        let rs = rects(&l, 3);
        assert_eq!(rs.len(), 3);
        // 放得下 ⇒ 每张都是参考标题的实测自然宽 `TAB_W`（98）
        for (i, r) in rs.iter().enumerate() {
            assert!(
                (r.width() - m::TAB_W).abs() < 0.01,
                "第 {i} 张宽度应为自然宽 {}，实得 {}",
                m::TAB_W,
                r.width()
            );
        }
        // 关键判据：右边界 == 下一张左边界（不是"差一个 gap"）
        for i in 1..rs.len() {
            assert!(
                (rs[i].left() - rs[i - 1].right()).abs() < 0.01,
                "第 {i} 张左边界 {} 与第 {} 张右边界 {} 之间有间隙（上游无 gap）",
                rs[i].left(),
                i - 1,
                rs[i - 1].right()
            );
        }
    }

    /// ②c **标签多到放不下时宽度不设下限**（B1-FIX：删掉 `metrics::TAB_MIN_W` 的可执行证据）。
    ///
    /// 旧版自定下限 40（注释自承"推导…留 8px"）。上游没有最小宽：标签宽就是内容宽，
    /// 放不下由 `ScrollViewer HorizontalScrollBarVisibility="Auto"`（`MapTabs.cs:34`）横向滚动
    /// —— 滚动不在本单范围，本仓退化为等分收窄且**不收口**。
    #[test]
    fn 标签多时宽度不设下限() {
        let l = Layout::compute(窗());
        // 取一个让 `可用宽 / n` 明显小于旧下限 40 的标签数
        let n = (l.tab_strip.width() / 40.0).ceil() as usize + 1;
        let rs = rects(&l, n);
        assert_eq!(rs.len(), n);
        let 应得 = l.tab_strip.width() / n as f32;
        assert!(
            (rs[0].width() - 应得).abs() < 0.01,
            "溢出时应等分收窄：期望 {应得}，实得 {}",
            rs[0].width()
        );
        assert!(
            rs[0].width() < 40.0,
            "旧下限 40 必须已删（n = {n} 时宽度 {} 仍 ≥ 40）",
            rs[0].width()
        );
        // 等分之后整排仍不越出页签条（右侧不被裁掉）
        assert!(
            rs[n - 1].right() <= l.tab_strip.right() + 0.01,
            "第 {} 张右边界 {} 越出页签条 {}",
            n - 1,
            rs[n - 1].right(),
            l.tab_strip.right()
        );
    }

    /// ②d **底色 / 悬停底 / 选中底边三色逐字节等于上游真值**（B1-FIX）。
    ///
    /// 判据读的是 `FullOutput::shapes` 里 `Shape::Rect` 的 `fill`（egui 真要栅格化的填充色），
    /// 不是本模块的中间变量 ⇒ "算了没画"或"画了但色不对"都会红。
    ///
    /// 三色上游出处（见 [`draw`] 的表格）：
    /// - 容器底 `#1C2028`：`MainWindow.MapTabs.cs:36` `Theme1C2028`（`App.axaml:8`）；
    /// - 悬停底 `#29313E`：`EditorControls.axaml:63` `Theme29313E`（`App.axaml:14`）；
    /// - 选中底边 `#8CA1FF`：`EditorControls.axaml:64` `Theme8CA1FF`（`App.axaml:47`）。
    ///
    /// 断言取**当前生效**的色板（默认浅色）⇒ 深色侧靠 `theme` 自己的用例守（那里逐字节钉死两套）。
    #[test]
    fn 多标签三色取上游真值() {
        let l = Layout::compute(窗());
        let f = 双标签夹具(1); // active = 1 ⇒ 两张都由本模块画
        let (app, tabs) = (&f.app, &f.app.tabs);
        let rs = rects(&l, 2);
        let ui = t::ui();

        // ① 无悬停：两张的底都是容器底；活跃那张底部 2px 是选中底边；不许出现悬停底。
        let 图元 = 画一帧(&l, tabs, app, None);
        assert!(
            有填充(&图元, rs[0], ui.tab_strip_bg),
            "第 0 张底色必须是容器底（上游 Border#Chrome 背景 Transparent 透出 Theme1C2028）"
        );
        assert!(
            有填充(&图元, rs[1], ui.tab_strip_bg),
            "活跃标签底色同样是容器底（上游选中态只改底边与前景，不改 Background）"
        );
        let 下划 = Rect::from_min_max(
            pos2(rs[1].left(), rs[1].bottom() - m::TAB_UNDERLINE_H),
            rs[1].max,
        );
        assert!(
            有填充(&图元, 下划, ui.tab_selected_underline),
            "活跃标签底部 2px 必须是 Theme8CA1FF（BorderThickness=0,0,0,2）"
        );
        assert!(
            !图元.iter().any(|(_, c)| *c == ui.tab_hover_bg),
            "没有指针悬停时不许画悬停底 Theme29313E"
        );
        // 反向：不许再把非活跃标签压成 panel_header（旧版自定层次）
        assert!(
            !图元.iter().any(|(_, c)| *c == ui.panel_header),
            "旧版自定的 panel_header 底色必须已删"
        );

        // ② 指针停在第 0 张上：那一张换成悬停底（`:pointerover`）
        let 图元 = 画一帧(&l, tabs, app, Some(rs[0].center()));
        assert!(
            有填充(&图元, rs[0], ui.tab_hover_bg),
            "指针在标签上时必须画 Theme29313E 悬停底"
        );
        assert!(
            有填充(&图元, rs[1], ui.tab_strip_bg),
            "指针不在活跃标签上时它不该被画成悬停底"
        );
    }

    /// 跑一帧 [`draw`]，收集 `(矩形, 填充色)`（`Shape::Rect` 的 `rect` / `fill`）。
    fn 画一帧(
        l: &Layout,
        tabs: &Tabs,
        app: &MireditApp,
        ptr: Option<egui::Pos2>,
    ) -> Vec<(Rect, egui::Color32)> {
        let ctx = egui::Context::default();
        let mut events = Vec::new();
        if let Some(q) = ptr {
            events.push(egui::Event::PointerMoved(q));
        }
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    pos2(0.0, 0.0),
                    vec2(m::WINDOW_W, m::WINDOW_H),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| draw(ui.painter(), l, tabs, app));
            },
        );
        let mut 图元 = Vec::new();
        for cs in &out.shapes {
            if let egui::epaint::Shape::Rect(r) = &cs.shape {
                图元.push((r.rect, r.fill));
            }
        }
        图元
    }

    /// 图元里有没有"恰好这块矩形、恰好这个填充色"的一条。
    fn 有填充(图元: &[(Rect, egui::Color32)], r: Rect, c: egui::Color32) -> bool {
        图元.iter()
            .any(|(rr, cc)| *cc == c && (rr.left() - r.left()).abs() < 0.01
                && (rr.top() - r.top()).abs() < 0.01
                && (rr.right() - r.right()).abs() < 0.01
                && (rr.bottom() - r.bottom()).abs() < 0.01)
    }

    /// ③ **活跃标签的标题取自宿主 `app.*`，不读占位槽**（本架构最容易踩的坑）。
    ///
    /// 夹具：A = `a.map`（**脏**）、B = `b.map`（净）。
    /// 两档 `active` 各验一次，覆盖**两个数据来源的两个方向**：
    ///
    /// | `active` | 活跃（读 `app.*`）| 非活跃（读 `items[i]`）|
    /// |---|---|---|
    /// | 0 | A 脏 | B 净 |
    /// | 1 | B 净 | A 脏 |
    ///
    /// 同时断言**占位槽确实是占位**（`items[active].path_of_snapshot() == None`）：
    /// 若实现改成读 `items[active]`，那里没有文档 ⇒ 拿到 `None` / `—` ⇒ 断言当场红。
    #[test]
    fn 活跃标签标题取自宿主而非占位槽() {
        for active in [0usize, 1] {
            let f = 双标签夹具(active);
            let (app, tabs) = (&f.app, &f.app.tabs);
            let (甲名, 乙名) = (&f.甲名, &f.乙名);
            let (活名, 活脏, 非名, 非脏) = if active == 0 {
                (甲名, true, 乙名, false)
            } else {
                (乙名, false, 甲名, true)
            };
            // ① 活跃标签：文件名与脏标记都来自 app.*
            let a = tabs.显示信息(app, active).expect("活跃标签必须给出信息");
            assert_eq!(
                a.text,
                tx::fill(
                    if 活脏 {
                        tx::TAB_TITLE_DIRTY_TMPL
                    } else {
                        tx::TAB_TITLE_TMPL
                    },
                    &[("name", 活名)]
                ),
                "active={active}：活跃标签文案错"
            );
            assert_eq!(a.dirty, 活脏, "active={active}：活跃标签脏标记错");
            assert_eq!(app.edit.dirty(), 活脏, "active={active}：脏标记真源在 app.edit");
            // ② 占位槽确实是占位：它给不出业务路径 ⇒ 读它就是读假数据
            assert_eq!(
                tabs.items[active].path_of_snapshot(),
                None,
                "active={active}：items[active] 是占位槽，它不该有业务路径"
            );
            // ③ 非活跃标签：从真快照读
            let 非 = 1 - active;
            let b = tabs.显示信息(app, 非).expect("非活跃标签必须给出信息");
            assert_eq!(
                b.text,
                tx::fill(
                    if 非脏 {
                        tx::TAB_TITLE_DIRTY_TMPL
                    } else {
                        tx::TAB_TITLE_TMPL
                    },
                    &[("name", 非名)]
                ),
                "active={active}：非活跃标签文案错"
            );
            assert_eq!(b.dirty, 非脏, "active={active}：非活跃标签脏标记错");
            // ④ 越界 ⇒ None（不 panic、不编值）
            assert!(tabs.显示信息(app, 2).is_none());
        }
    }

    /// 双标签夹具：A = `a.map`（**脏**）、B = `b.map`（净）。
    ///
    /// `active` 决定谁的真值在 `app.*`、谁的真快照在 `items[i]`（**严格照稳态契约摆位**）：
    /// - `active == 0` ⇒ `app` 装 A、`items = [占位, B 快照]`；
    /// - `active == 1` ⇒ `app` 装 B、`items = [A 快照, 占位]`。
    ///
    /// ⚠ 摆位必须**只 capture 非活跃那一份**：`capture` 是 `mem::swap`，
    /// 它会把宿主里的真值换进槽、同时把槽里的占位换回宿主 —— 对活跃那份做 capture
    /// 就等于把"活跃标签的文档"清成占位（本用例第一版就踩了这个坑，`显示信息` 拿到 `None`）。
    ///
    /// 返回值实现 `Drop`：**即使断言 panic 也会删掉两张临时图**（不留垃圾）。
    ///
    /// 标签集合**装进 `app.tabs`**（不单独返回）：宿主层 [`MireditApp::activate_tab`]
    /// 操作的就是 `self.tabs`，独立返回一份会让"点击切换"用例在另一份容器上做无用功
    /// （第一版实测：`activate` 报「当前共 1 张标签」，因为 `app.tabs` 还是初始的那一张）。
    fn 双标签夹具(active: usize) -> 夹具 {
        assert!(active < 2, "本夹具只有两张标签");
        let 图甲 = 临时图("a");
        let 图乙 = 临时图("b");
        let mut 甲app = 建应用(&图甲, true, "host_a");
        assert!(甲app.edit.dirty(), "夹具：A 必须先有未保存修改");
        let 甲名 = 甲app.doc.doc.as_ref().expect("A 必须有文档").file_name.clone();
        let mut 乙app = 建应用(&图乙, false, "host_b");
        assert!(!乙app.edit.dirty(), "夹具：B 必须干净");
        let 乙名 = 乙app.doc.doc.as_ref().expect("B 必须有文档").file_name.clone();
        assert_ne!(甲名, 乙名, "夹具：两张标签的文件名必须不同，否则测不出串台");

        let app = if active == 0 {
            // A 留在宿主里当真值；只把 B capture 成真快照。
            let 乙槽 = 真快照(&mut 乙app);
            let mut t = Tabs::new(TabState::占位());
            t.push(乙槽);
            甲app.tabs = t;
            甲app
        } else {
            // B 留在宿主里当真值；只把 A capture 成真快照。
            let 甲槽 = 真快照(&mut 甲app);
            let mut t = Tabs::new(甲槽);
            t.push(TabState::占位());
            t.active = 1;
            乙app.tabs = t;
            乙app
        };
        assert_eq!(app.tabs.active(), active);
        夹具 {
            app,
            甲名,
            乙名,
            图甲,
            图乙,
        }
    }

    /// [`双标签夹具`] 的产物：宿主（标签集合在 `app.tabs` 里）+ 两个文件名 + 两个临时图路径。
    ///
    /// `Drop` 里删临时图 ⇒ 用例断言失败也**不把临时文件留在盘上**。
    struct 夹具 {
        app: MireditApp,
        甲名: String,
        乙名: String,
        图甲: PathBuf,
        图乙: PathBuf,
    }

    impl Drop for 夹具 {
        fn drop(&mut self) {
            清临时图(&self.图甲);
            清临时图(&self.图乙);
        }
    }

    /// 从宿主 `capture` 一份真快照（`双标签夹具` 用）。
    ///
    /// ⚠ 副作用：`capture` 会把宿主换空（占位）—— 调用方必须确保这是"非活跃那一份"。
    fn 真快照(app: &mut MireditApp) -> TabState {
        let mut 槽 = TabState::占位();
        crate::app::capture(app, &mut 槽);
        槽
    }

    /// 删掉 [`临时图`] 造的那张图**和它的目录**（临时文件用完即删）。
    fn 清临时图(p: &Path) {
        let _ = std::fs::remove_file(p);
        if let Some(dir) = p.parent() {
            let _ = std::fs::remove_dir(dir);
        }
    }

    /// ③b **单标签态本模块一个像素都不画**（`--shot` 哈希与 A 段逐字节相同的前提）。
    ///
    /// 这条是"不动画布几何"之外的第二个零变化保证：几何没变但多画一层底色，
    /// 像素照样会变。判据收在 [`要画的标签`] 一处，本用例直接对它断言。
    #[test]
    fn 单标签态不重复画() {
        let mut tabs = Tabs::new(TabState::占位());
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs.active(), 0);
        assert!(
            要画的标签(&tabs).is_empty(),
            "唯一一张且是活跃标签 ⇒ canvas::draw 已画好，本模块不许再画"
        );
        // 双标签：第 0 张 + 第 1 张都要本模块画（canvas 画的是活跃那张的标题，
        // active == 0 时它画的正是第 0 张 ⇒ 第 0 张跳过、第 1 张要画）
        tabs.push(TabState::占位());
        assert_eq!(要画的标签(&tabs), vec![1], "active == 0 时只需补画第 1 张");
        // active == 1 时第 0 格被 canvas 画成了活跃标签的内容 ⇒ 两张都要重画
        tabs.active = 1;
        assert_eq!(要画的标签(&tabs), vec![0, 1], "active != 0 时第 0 格必须盖掉重画");
    }

    /// ⑤ **真的画上屏了**：`active == 1` 时本模块亲自画两张，提交的文本图元里
    /// 同时有「A 的文件名 *」（非活跃，取自快照）与「B 的文件名」（活跃，取自 `app.*`）。
    ///
    /// 为什么必须验到"提交的图元"而不止"算出的矩形"：几何对了但没画（或画错位置被裁掉）
    /// 在单元测试里看不出来。这里读的是 `FullOutput::shapes` 里 `Shape::Text` 的
    /// `galley.job.text` —— 那就是 egui 真正要栅格化的字符串，不是我们自己的中间变量。
    ///
    /// 为什么用 `active == 1`：`active == 0` 时第 0 张由 `canvas::draw` 负责
    /// （见 [`要画的标签`]），单测里没调它 ⇒ 那张本来就不该出现在本模块的图元里。
    /// `active == 1` 时两张都归本模块画，正好一次覆盖两个数据来源。
    #[test]
    fn 双标签文案都提交上屏() {
        let l = Layout::compute(窗());
        let f = 双标签夹具(1);
        let (app, tabs, 甲名, 乙名) = (&f.app, &f.app.tabs, &f.甲名, &f.乙名);
        assert_eq!(tabs.active(), 1, "本用例要 active == 1");
        let ctx = egui::Context::default();
        let mut 上屏: Vec<String> = Vec::new();
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    pos2(0.0, 0.0),
                    vec2(m::WINDOW_W, m::WINDOW_H),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| draw(ui.painter(), &l, tabs, app));
            },
        );
        for cs in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &cs.shape {
                上屏.push(t.galley.job.text.clone());
            }
        }
        let 甲 = tx::fill(tx::TAB_TITLE_DIRTY_TMPL, &[("name", 甲名)]);
        let 乙 = tx::fill(tx::TAB_TITLE_TMPL, &[("name", 乙名)]);
        assert!(
            上屏.iter().any(|s| s == &甲),
            "非活跃标签（快照里的脏 A）没上屏。实际提交的文本：{上屏:?}"
        );
        assert!(
            上屏.iter().any(|s| s == &乙),
            "活跃标签（app.* 里的净 B）没上屏。实际提交的文本：{上屏:?}"
        );
        // 反向：不许出现占位槽那种「—」假名字
        assert!(
            !上屏.iter().any(|s| s == tx::VAL_UNKNOWN),
            "画出了占位槽的假名字（读 items[active] 的典型症状）：{上屏:?}"
        );
    }

    /// ⑥ **点击真的切过去了**：点非活跃标签 ⇒ `active` 变化 + `app.*` 换成那张图的真值；
    /// 点活跃标签 ⇒ `active` 不变、`app.*` 逐值不变（`activate` 自身 no-op）。
    ///
    /// 走的是宿主层的 [`MireditApp::activate_tab`]（标签条点击在 `interact` 里的唯一落点），
    /// 不是直接调 `Tabs::activate` —— 这样"接线漏了"这类缺陷也能被这条用例挡住。
    #[test]
    fn 点击切换改变活跃标签且点活跃不变() {
        let mut f = 双标签夹具(0);
        let 甲名 = f.甲名.clone();
        let 乙名 = f.乙名.clone();
        assert_eq!(f.app.tabs.active(), 0, "夹具从 A 开始");
        assert_eq!(
            f.app.tabs.显示信息(&f.app, 0).unwrap().text,
            tx::fill(tx::TAB_TITLE_DIRTY_TMPL, &[("name", &甲名)])
        );
        // A 在宿主里是脏的；B 在快照里是净的
        assert!(f.app.edit.dirty(), "A 脏");
        let a_pan = f.app.view.pan_x;
        let a_scale = f.app.view.scale;

        // ① 切到 B（下标 1）：app.* 必须换成 B 的真值
        let r = f.app.activate_tab(1);
        assert!(r.is_ok(), "切到 B 应成功：{r:?}");
        assert_eq!(f.app.tabs.active(), 1, "active 必须变成 1");
        assert!(
            !f.app.edit.dirty(),
            "切过去后 app.* 是 B（净），不许留着 A 的脏标记"
        );
        let b = f.app.tabs.显示信息(&f.app, 1).expect("B 必须有信息");
        assert_eq!(b.text, tx::fill(tx::TAB_TITLE_TMPL, &[("name", &乙名)]));
        // 非活跃的 A 现在在快照里，脏标记要还在
        let a = f.app.tabs.显示信息(&f.app, 0).expect("A 必须有信息");
        assert_eq!(
            a.text,
            tx::fill(tx::TAB_TITLE_DIRTY_TMPL, &[("name", &甲名)])
        );
        assert!(a.dirty, "A 的脏标记要留在快照里");

        // ② 再点一次**活跃**标签（下标 1）⇒ 一切不变（no-op）
        let b_pan = f.app.view.pan_x;
        let b_scale = f.app.view.scale;
        let r = f.app.activate_tab(1);
        assert!(r.is_ok());
        assert_eq!(f.app.tabs.active(), 1, "点活跃标签 active 不许变");
        assert_eq!(f.app.view.pan_x, b_pan, "no-op 不许改视口");
        assert_eq!(f.app.view.scale, b_scale);

        // ③ 切回 A：A 的脏标记与视口都要回来
        let r = f.app.activate_tab(0);
        assert!(r.is_ok());
        assert_eq!(f.app.tabs.active(), 0);
        assert!(f.app.edit.dirty(), "切回 A 后脏标记要回来");
        assert_eq!(f.app.view.pan_x, a_pan, "切回 A 后 pan 要回来");
        assert_eq!(f.app.view.scale, a_scale);
    }

    /// ④ 点击命中：`interact` 只对**非活跃**标签返回下标，点活跃标签返回 `None`。
    ///
    /// egui 的 `clicked()` 要求"按下前已经 hover" ⇒ 点击必须**两帧**，
    /// 且**必须复用同一个 `Context`**（命中判定读的是上一帧登记的 widget 表）。
    #[test]
    fn 点击非活跃标签才返回下标() {
        let l = Layout::compute(窗());
        let 图乙 = 临时图("click_b");
        let 乙 = 造槽(&图乙, false, "click_b");
        let mut tabs = Tabs::new(TabState::占位());
        tabs.push(乙);
        let rs = rects(&l, 2);

        let ctx = egui::Context::default();
        let mut 点中 = None;
        // 第 1 帧：只 hover（让 egui 登记 widget 与 hover 态）
        一帧(&ctx, &l, &tabs, vec![egui::Event::PointerMoved(rs[1].center())], &mut 点中);
        assert_eq!(点中, None, "只悬停不该算点击");
        // 第 2 帧：按下 + 抬起
        let p = rs[1].center();
        一帧(
            &ctx,
            &l,
            &tabs,
            vec![
                egui::Event::PointerMoved(p),
                按钮(p, true),
                按钮(p, false),
            ],
            &mut 点中,
        );
        assert_eq!(点中, Some(1), "点非活跃的第 1 张要返回 1");

        // 点**活跃**标签（下标 0）⇒ 不返回（activate 自身也是 no-op，这里提前拦下）
        let mut 点中0 = None;
        一帧(&ctx, &l, &tabs, vec![egui::Event::PointerMoved(rs[0].center())], &mut 点中0);
        let p0 = rs[0].center();
        一帧(
            &ctx,
            &l,
            &tabs,
            vec![egui::Event::PointerMoved(p0), 按钮(p0, true), 按钮(p0, false)],
            &mut 点中0,
        );
        assert_eq!(点中0, None, "点活跃标签不该触发切换");

        清临时图(&图乙);
    }

    /// ③c **关闭符命中盒 = 上游按钮盒（字形 advance + 上下左右 padding）**，且与主体不重叠。
    ///
    /// 上游: `MainWindow.MapTabs.cs:57`
    /// `new Button { Content = "\u00d7", Padding = new Thickness(4, 0, 4, 0), … }`
    /// ⇒ 按钮盒宽 = 4 + 字形宽 + 4、高 = 字形行高（上下 padding 0）。
    /// 字形宽/行高**上游不可得**（Avalonia + Segoe UI 的度量），本仓取**实际上屏字体**的实测值，
    /// 这条用例把它钉在 0.01 容差内 ⇒ 换字体或改 `TAB_FONT` 立刻红。
    #[test]
    fn 关闭符命中盒按字形advance加上游内边距() {
        // ① 两个字形常量必须等于本仓字体在 TAB_FONT 下的实测值
        let ctx = crate::fonts::measure_ctx();
        let mut 实测 = egui::Vec2::ZERO;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    实测 = ui
                        .painter()
                        .layout_no_wrap(
                            tx::TAB_CLOSE.to_owned(),
                            egui::FontId::proportional(m::TAB_FONT),
                            egui::Color32::WHITE,
                        )
                        .size();
                });
        });
        assert!(
            (实测.x - m::TAB_CLOSE_GLYPH_W).abs() < 0.01,
            "TAB_CLOSE_GLYPH_W 与实测 advance 不符：常量 {}，实测 {}",
            m::TAB_CLOSE_GLYPH_W,
            实测.x
        );
        assert!(
            (实测.y - m::TAB_CLOSE_GLYPH_H).abs() < 0.01,
            "TAB_CLOSE_GLYPH_H 与实测行高不符：常量 {}，实测 {}",
            m::TAB_CLOSE_GLYPH_H,
            实测.y
        );

        let l = Layout::compute(窗());
        let r = rects(&l, 1)[0];

        // ② 上游按钮盒：宽 = 2×padding + 字形宽，高 = 字形行高，垂直居中
        let 按 = close_button_rect(r);
        assert!(
            (按.width() - (m::TAB_CLOSE_PAD * 2.0 + m::TAB_CLOSE_GLYPH_W)).abs() < 0.01,
            "按钮盒宽必须是 4+字形+4，实得 {}",
            按.width()
        );
        assert!(
            (按.height() - m::TAB_CLOSE_GLYPH_H).abs() < 0.01,
            "按钮盒高必须是字形行高（上下 padding = 0），实得 {}",
            按.height()
        );
        assert!(
            (按.center().y - r.center().y).abs() < 0.01,
            "上游 VerticalContentAlignment=Center ⇒ 按钮必须垂直居中"
        );
        assert!(
            (按.right() - (r.right() - m::TAB_TITLE_INSET + m::TAB_CLOSE_PAD)).abs() < 0.01,
            "按钮右边界 = 字形右边界 + padding 右 4，实得 {}",
            按.right()
        );

        // ③ 命中盒（close_rect）与主体（body_rect）：两个不同的 rect、互不重叠、拼满整张标签
        let 关 = close_rect(r);
        let 体 = body_rect(r);
        assert_ne!(关, 体, "关闭符与主体必须是两个不同的命中盒");
        assert!(
            (体.right() - 关.left()).abs() < 0.01,
            "主体右边界 {} 必须等于关闭符左边界 {}（拼满、无死区）",
            体.right(),
            关.left()
        );
        assert!(
            !体.shrink(0.5).intersects(关.shrink(0.5)),
            "主体与关闭符内部重叠：{:?} / {:?}",
            体,
            关
        );
        for (名, x) in [("主体", 体), ("关闭符", 关)] {
            assert!(
                x.left() >= r.left() - 0.01 && x.right() <= r.right() + 0.01,
                "{名} {x:?} 越出标签 {r:?}"
            );
            assert!(
                (x.top() - r.top()).abs() < 0.01 && (x.bottom() - r.bottom()).abs() < 0.01,
                "{名} {x:?} 必须占满标签高度 {r:?}"
            );
        }
        // 命中盒必须盖住**看得见的**按钮盒（点得到它）
        assert!(
            关.left() <= 按.left() + 0.01 && 关.right() >= 按.right() - 0.01,
            "关闭符命中盒 {关:?} 没横向盖住按钮盒 {按:?}"
        );
    }

    /// ④b **点关闭符 ⇒ 关闭请求；点主体 ⇒ 切换**（三态互不串）。
    ///
    /// egui 的 `clicked()` 要求"按下前已经 hover" ⇒ 每次点击都要**两帧**且复用同一个 `Context`。
    #[test]
    fn 点关闭符返回关闭请求且不切换标签() {
        let l = Layout::compute(窗());
        let 图乙 = 临时图("closehit_b");
        let 乙 = 造槽(&图乙, false, "closehit_b");
        let mut tabs = Tabs::new(TabState::占位());
        tabs.push(乙);
        let rs = rects(&l, 2);

        // ① 点第 1 张（非活跃）的**关闭符** ⇒ Close(1)，不是 Switch(1)
        let ctx = egui::Context::default();
        let p = close_rect(rs[1]).center();
        assert_eq!(
            一帧三态(&ctx, &l, &tabs, vec![egui::Event::PointerMoved(p)]),
            None,
            "只悬停不该算点击"
        );
        assert_eq!(
            一帧三态(
                &ctx,
                &l,
                &tabs,
                vec![egui::Event::PointerMoved(p), 按钮(p, true), 按钮(p, false)],
            ),
            Some(Click::Close(1)),
            "点关闭符必须返回关闭请求（不是切换）"
        );

        // ② 点同一张的**主体** ⇒ Switch(1)，不许是 Close
        let ctx2 = egui::Context::default();
        let q = body_rect(rs[1]).center();
        assert_eq!(
            一帧三态(&ctx2, &l, &tabs, vec![egui::Event::PointerMoved(q)]),
            None
        );
        assert_eq!(
            一帧三态(
                &ctx2,
                &l,
                &tabs,
                vec![egui::Event::PointerMoved(q), 按钮(q, true), 按钮(q, false)],
            ),
            Some(Click::Switch(1)),
            "点主体必须返回切换"
        );

        // ③ 点**活跃**标签的关闭符 ⇒ 照样是关闭请求（关活跃那张是最常见的关闭路径）
        let ctx3 = egui::Context::default();
        let w = close_rect(rs[0]).center();
        assert_eq!(
            一帧三态(&ctx3, &l, &tabs, vec![egui::Event::PointerMoved(w)]),
            None
        );
        assert_eq!(
            一帧三态(
                &ctx3,
                &l,
                &tabs,
                vec![egui::Event::PointerMoved(w), 按钮(w, true), 按钮(w, false)],
            ),
            Some(Click::Close(0)),
            "点活跃标签的关闭符必须能关（不许因为是活跃就吞掉）"
        );

        // ④ 点**活跃**标签的主体 ⇒ 不响应（no-op，`activate` 自身也是 no-op）
        let ctx4 = egui::Context::default();
        let e = body_rect(rs[0]).center();
        assert_eq!(
            一帧三态(&ctx4, &l, &tabs, vec![egui::Event::PointerMoved(e)]),
            None
        );
        assert_eq!(
            一帧三态(
                &ctx4,
                &l,
                &tabs,
                vec![egui::Event::PointerMoved(e), 按钮(e, true), 按钮(e, false)],
            ),
            None,
            "点活跃标签主体不该有任何响应"
        );

        清临时图(&图乙);
    }

    fn 按钮(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    /// 跑一帧 [`interact`]，把命中的**主体切换**下标写回 `out`。
    ///
    /// ⚠ B2 单：`interact` 的返回值从 `Option<usize>` 变成三态 [`Click`]（多了"关闭符"这一态）。
    /// 本辅助函数只服务"点标签主体"的用例 ⇒ 只映射 [`Click::Switch`]；万一命中的是关闭符，
    /// 写一个**不可能的下标** `usize::MAX` 进去，让调用方的 `assert_eq!(点中, Some(1))` 当场红
    /// （而不是被静默忽略）。既有断言的文字与判据一字未改。
    fn 一帧(
        ctx: &egui::Context,
        l: &Layout,
        tabs: &Tabs,
        events: Vec<egui::Event>,
        out: &mut Option<usize>,
    ) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                vec2(m::WINDOW_W, m::WINDOW_H),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| match interact(ui, l, tabs) {
                    Some(Click::Switch(i)) => *out = Some(i),
                    Some(Click::Close(_)) => *out = Some(usize::MAX),
                    None => {}
                });
        });
    }

    /// 跑一帧 [`interact`]，返回**三态原值**（关闭符用例用，要区分 `Switch` / `Close`）。
    fn 一帧三态(
        ctx: &egui::Context,
        l: &Layout,
        tabs: &Tabs,
        events: Vec<egui::Event>,
    ) -> Option<Click> {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                vec2(m::WINDOW_W, m::WINDOW_H),
            )),
            events,
            ..Default::default()
        };
        let mut out = None;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| out = interact(ui, l, tabs));
        });
        out
    }
}
