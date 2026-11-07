//! 多地图标签 —— **capture/restore 快照换入换出的纯数据层**（R12-F3 A 段，2026-11-07）。
//!
//! # 架构（Lead 裁定，台账 §8.55，不许另选）
//!
//! 上游 `MapViewport.cs:147-161` 的 `CaptureEditorState/RestoreEditorState` 同款：
//! 切标签时把 [`crate::app::MireditApp`] 里**随标签走**的字段整体换进/换出
//! [`TabState`] 槽，而不是把 240+ 个 `self.doc.doc` / `self.view` / `self.edit`
//! 访问点硬改成 Vec 索引。现有访问点**一行都不动**，单标签行为逐值不变。
//!
//! # ★ 稳态契约（本架构最容易踩的坑，B 段必读）
//!
//! - `items[i != active]` 存的是该标签的**真快照**；
//! - `items[active]` 是**占位槽**（内容无业务含义，只是 `mem::swap` 的缓冲）；
//! - **活跃标签的真值常驻 `app.*`**。
//!
//! 因此 **B 段标签条渲染活跃标签的标题 / 脏标记 / 路径时，必须从 `app.*` 读**
//! （例如 [`Tabs::active_path`]，`app.edit.dirty()`），**绝不许读 `items[active]`**
//! —— 那是占位槽，读出来的是空值/错值。非活跃标签的信息才从 `items[i]` 读。
//!
//! 同理，[`Tabs::active_mut`] 返回的是活跃槽本身（占位），仅供结构性维护使用，
//! **不是**"取活跃标签状态"的入口。
//!
//! # 随标签字段（只读审计总表 2026-11-07 20:21，15 项）
//!
//! `doc`（整份 DocState，含 `assets` 壳；底层 `Arc<WilSource>` 共享、壳随标签）
//! / `view`（含 GPU 纹理缓存）/ `minimap`（MinimapJob：state + rx + Ready{tex}）
//! / `minimap_drag` / `edit` / `selected_tool` / `hover` / `hover_anchor` / `lines`
//! / `lines_key` / `preview_tex` / `armed` / `sel_drag` / `op` / `overlay`。
//! 容器 `tabs` / `active` 放在 [`Tabs`] 里（`active` 由 Tabs 自己管）。
//!
//! **两处审计订正**（原判错，实为）：`text` 每帧 `screen_text::build` 全量重算
//! （`app.rs` 的 ⑥ 装配文案段）⇒ 纯本帧暂存、留全局；`minimap` / `overlay`
//! 必须随标签（前者 `map_gen` 每标签从 1 起，全局 job 会把 A 的总览贴到 B，
//! `minimap_job.rs:296-344` 上传不复核归属；后者 `CheckMap{gen}` 存检查那刻
//! `map_gen`，`menu/mod.rs:170-171`，全局化会让 B 拿 A 的代次判陈旧）。
//!
//! # 全局共享（**不**随标签走，留在 `MireditApp`，29 项）
//!
//! `palette` / `stamps` / `forced_hover` / `cell_hover` / `text` / `pending_center` /
//! `centered` / `frames` / `frame_gap_ms` / `update_ms` / `last_frame` / `last_canvas` /
//! `last_overlay` / `canvas_size` / `preview_alt` / `confirm_close` / `shown_title` /
//! `auto` / `pending_stamp` / `menu` / `panels` / `dark` / `grid` / `doors_open` /
//! `lighting` / `settings_path` / `recent` / `pending_menu_clicks` /
//! `pending_open_map` / `maps` / `maps_root_pinned`。
//!
//! 依据：台账 §8.55「全局共享」清单 + 产品口径「CLI / `--shot` 保持单图」。
//!
//! # ★★ 给 B 段的三条接线红线（只读审计总表 ⑥）
//!
//! 1. **「重复打开切到已有」必须在 `menu::action::Ctx` 构造之前 `activate()+return`**，
//!    **绝不能走 `open_path`**：`menu/action.rs:793` 的 `*cx.edit = EditSession::default()`
//!    会清空该标签未保存改动，与「脏标签逐个确认」产品口径直接冲突。
//! 2. 查重时**活跃标签路径从 `self.doc.doc` 读、非活跃从 `tabs[i].doc.doc` 读**
//!    （`tabs[active]` 是占位槽）；[`Tabs::find_by_path`] 已按此实现。
//! 3. 全局 `Assets` 底层 `Arc<WilSource>` **只共享一份**：新标签走
//!    `DocState::load_map`（`mapdoc.rs:384-419`，不扫图库）+ 浅拷贝 `assets`
//!    （B 段加 Clone/浅拷贝入口），**禁止**改成全局 `self.doc.assets`
//!    或为每标签重跑 `load_assets()`。
//!
//! # 三条硬约束（上游 `MapViewport.cs:149-152` CancelInteraction 同款）
//!
//! - `activate` 前收尾 `minimap_drag = None; sel_drag = None; armed = None`，
//!   防止把「正在拖一半」的交互状态带进另一张图；
//! - `activate` 后强制 `lines_key = None`（键第三元 = `doc.map_gen`，
//!   跨标签从 1 起会撞号 ⇒ 不失效就会显示上一张图的单元格信息）；
//! - `activate(active)` 为 **no-op**（防占位槽被当真值读）。
//!
//! # 为什么用占位槽而不是给状态加 `Default`
//!
//! [`crate::mapdoc::DocState`] / [`crate::screen_text::DocText`] /
//! [`crate::cellinfo::CellLines`] 都不实现 `Default`，硬加会往状态里编"假初值"。
//! 逐字段 `mem::swap` 让两侧永远是合法值，不需要任何占位语义的 `Default`。
//! [`TabState::占位`] 只用于**构造第一个槽**与测试夹具，纯内存、零 I/O、零日志，
//! 其内容按稳态契约永不参与业务判据。
//!
//! # 本段边界
//!
//! A 段**只交付数据层**：不接 UI、不接菜单、不改 CLI。`--map` / `--shot` 行为零变化
//! （`MireditApp::new` 只在 `Self { .. }` 里多放一个占位槽，`app.*` 的真值一个字节没动）。

use std::path::Path;

use egui::{Pos2, TextureId};

use crate::app::MireditApp;
use crate::canvas_view::CanvasView;
use crate::cellinfo::CellLines;
use crate::edit::EditSession;
use crate::mapdoc::{Assets, DocState};
use crate::menu::Overlay;
use crate::minimap_job::MinimapJob;
use crate::selection_ops::Drag;
use crate::stamp_place::Armed;

/// 一张标签**随标签走**的全部状态（只读审计总表 15 项）。
///
/// 字段逐一判定的依据见模块文档「全局共享」清单；`capture/restore` 在
/// [`crate::app`] 同模块内逐字段 `mem::swap`（那里能访问 `MireditApp` 的私有字段），
/// 因此这里只需 `pub(crate)`，不需要动任何现有访问点。
pub struct TabState {
    /// 文档 + 图库（**整份**，含 `assets`；底层 `Arc<WilSource>` 共享、壳随标签）。
    pub(crate) doc: DocState,
    /// 画布视口（平移 / 缩放 / 图层 / 阻挡通道 / GPU 纹理缓存）。
    pub(crate) view: CanvasView,
    /// 小地图总览纹理的后台任务 + 缓存（**必须随标签**：代次判据见模块文档）。
    pub(crate) minimap: MinimapJob,
    /// 小地图拖拽中的抓取偏移（按下瞬间算一次，捕获语义）。
    pub(crate) minimap_drag: Option<(f32, f32)>,
    /// 编辑会话（工具 / 笔刷 / 撤销重做 / 脏标记 / 选区 / 剪贴板）。
    pub(crate) edit: EditSession,
    /// 当前选中的工具下标（工具条高亮镜像；真源在 `EditSession::slot`）。
    pub(crate) selected_tool: usize,
    /// 鼠标所在世界格。
    pub(crate) hover: Option<(u16, u16)>,
    /// 悬停浮层锚点（上一帧指针位置；随标签，避免切标签后浮层锚到旧图的格）。
    pub(crate) hover_anchor: Option<Pos2>,
    /// 当前单元格信息缓存。
    pub(crate) lines: CellLines,
    /// `lines` 对应的 `(x, y, 地图代号)` 缓存键（切换后强制失效）。
    pub(crate) lines_key: Option<(u16, u16, u64)>,
    /// Alt 落位预览的内容纹理缓存（键是印章 id，与 `armed` 配对 ⇒ 随标签）。
    pub(crate) preview_tex: Option<(String, TextureId, u32, u32)>,
    /// 印章待放置态。
    pub(crate) armed: Option<Armed>,
    /// 矩形选区拖动起点。
    pub(crate) sel_drag: Option<Drag>,
    /// 最近一次选区 / 印章操作的回执 `(文案, 是否拒绝)`。
    pub(crate) op: Option<(String, bool)>,
    /// 当前浮层（`CheckMap{gen}` 存检查那刻代号 ⇒ 必须随标签）。
    pub(crate) overlay: Overlay,
}

impl TabState {
    /// 构造一个**占位槽**：纯内存、零 I/O、零日志、不编任何业务数据。
    ///
    /// 用途只有两个：① [`MireditApp::new`](crate::app::MireditApp::new) 里给
    /// [`Tabs::new`] 的第一个活跃槽（按稳态契约，活跃槽本就该是占位）；
    /// ② 测试夹具 / B 段新建标签时拿它当 `mem::swap` 的缓冲。
    /// `screen_text::build` / `CellLines::unknown` 都是真实函数，不是写死的假文案
    /// （构造出的 `DocText` 只用来填 `MireditApp::new` 的全局 `text` 字段，不进 TabState）。
    pub(crate) fn 占位() -> Self {
        let doc = DocState {
            doc: None,
            assets: Assets::Failed(String::new()),
            map_error: None,
            map_gen: 0,
        };
        Self {
            doc,
            view: CanvasView::default(),
            minimap: MinimapJob::default(),
            minimap_drag: None,
            edit: EditSession::default(),
            selected_tool: 0,
            hover: None,
            hover_anchor: None,
            lines: CellLines::unknown(),
            lines_key: None,
            preview_tex: None,
            armed: None,
            sel_drag: None,
            op: None,
            overlay: Overlay::None,
        }
    }

    /// 这张标签的地图路径（**仅非活跃槽可读**；活跃槽是占位，请用 [`Tabs::active_path`]）。
    pub(crate) fn path_of_snapshot(&self) -> Option<&Path> {
        self.doc.doc.as_ref().map(|d| d.path.as_path())
    }
}

/// 一张标签**要显示的信息**（标签条用）：文案 + 生成它的模板 + 脏标记。
///
/// 为什么把 `tmpl` 也带出来：[`crate::text_fit::DynLabel`] 要求模板必须是
/// [`crate::ui_text::ALL`] 里登记过的一行（排版自检按它核对），而"脏 / 净"两条模板
/// 是**二选一**的 —— 只有产生这段文字的地方才知道选了哪条。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabInfo {
    /// 上屏文案（`0.map` / `0.map *`）。
    pub text: String,
    /// 生成 [`Self::text`] 的那条模板（`TAB_TITLE_TMPL` 或 `TAB_TITLE_DIRTY_TMPL`）。
    pub tmpl: &'static str,
    /// 这张标签有没有未保存修改。
    pub dirty: bool,
}

/// 标签集合：`items[active]` 是占位，其余是真快照（见模块文档稳态契约）。
pub struct Tabs {
    pub(crate) items: Vec<TabState>,
    pub(crate) active: usize,
}

impl Tabs {
    /// 用第一个标签的槽构造（`active = 0`）。
    ///
    /// `first` 按稳态契约应是**占位槽**（`TabState::占位()`）：真值在 `app.*`，
    /// 不在 `items[0]`。B 段若要新建标签，请 `push` 一份真快照再 `activate` 过去。
    pub fn new(first: TabState) -> Self {
        Self {
            items: vec![first],
            active: 0,
        }
    }

    /// 标签数（恒 ≥ 1，见 [`Self::close`]）。
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 是否为空（恒为 `false`；留着给容器类调用方）。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 当前活跃下标（`active` 由 Tabs 自己管，`MireditApp` 不另存一份）。
    pub fn active(&self) -> usize {
        self.active
    }

    /// 活跃槽的**可变引用**。
    ///
    /// ⚠ **它是占位槽，不是活跃标签的状态**。返回它只为结构性维护（B 段若要改
    /// 活跃标签，请改 `app.*`）。读活跃标签信息一律从 `app.*` / [`Self::active_path`] 走。
    pub fn active_mut(&mut self) -> &mut TabState {
        &mut self.items[self.active]
    }

    /// **活跃标签的地图路径** —— 从 `app.*` 读，绝不碰 `items[active]` 占位槽。
    ///
    /// 这是给 B 段标签条的示范入口：活跃标签的一切信息都必须这样从宿主读。
    pub fn active_path(app: &MireditApp) -> Option<&Path> {
        app.doc.doc.as_ref().map(|d| d.path.as_path())
    }

    /// 追加一张标签（追加的是**真快照**，不改变当前 `active`）。
    pub fn push(&mut self, tab: TabState) {
        self.items.push(tab);
    }

    /// 切换到第 `idx` 张标签。
    ///
    /// 步骤（用 `Vec::remove/insert` 取出槽位，规避两个 `&mut items[..]` 的借用冲突）：
    /// 0. **CancelInteraction**（上游 `MapViewport.cs:149-152`）：收尾
    ///    `minimap_drag` / `sel_drag` / `armed`，不带半成品交互进另一张图；
    /// 1. 把 `app.*` 的当前真值 `capture` 进旧 `items[active]`（该槽变成真快照）；
    /// 2. 把 `items[idx]` 的真快照 `restore` 进 `app.*`（该槽变成占位）；
    /// 3. `lines_key = None`（键第三元 `map_gen` 跨标签会撞号）；
    /// 4. `active = idx`。
    ///
    /// `idx == active` ⇒ **no-op**，`app.*` 逐值不变（防占位槽被误当真值读出来），
    /// 且第 0/3 步都不执行（否则 no-op 就不成立）。
    /// 越界 ⇒ 返回中文错误，**不 panic、不改任何状态**。
    pub fn activate(&mut self, app: &mut MireditApp, idx: usize) -> Result<(), String> {
        if idx >= self.items.len() {
            return Err(format!(
                "标签下标越界：{idx}（当前共 {} 张标签）",
                self.items.len()
            ));
        }
        if idx == self.active {
            return Ok(());
        }

        // ⓪ CancelInteraction：交互状态留在**当前标签**的真值里，切过去那张必须是干净的。
        app.minimap_drag = None;
        app.sel_drag = None;
        app.armed = None;

        // ① 旧活跃槽（原是占位）拿出来，装进 app.* 的真值 ⇒ 变成真快照。
        let mut 旧槽 = self.items.remove(self.active);
        crate::app::capture(app, &mut 旧槽);
        self.items.insert(self.active, 旧槽);
        // 每次 remove 后立刻 insert 回同一位置 ⇒ 两步之间索引完全不变。

        // ② 新活跃槽（原是真快照）拿出来，换进 app.* ⇒ 变成占位。
        let mut 新槽 = self.items.remove(idx);
        crate::app::restore(app, &mut 新槽);
        self.items.insert(idx, 新槽);

        // ③ 单元格信息缓存键失效（`map_gen` 每标签从 1 起，跨标签会撞号）。
        app.lines_key = None;
        self.active = idx;
        Ok(())
    }

    /// [`Self::activate`] 的别名（上游 `Capture/RestoreEditorState` 语义里的「switch」）。
    pub fn switch(&mut self, app: &mut MireditApp, idx: usize) -> Result<(), String> {
        self.activate(app, idx)
    }

    /// 关闭第 `idx` 张标签，返回**被关标签的真快照**供 B 段做脏检查。
    ///
    /// - 关闭**非活跃**标签：直接取出它的真快照，`app.*` 不动；
    /// - 关闭**活跃**标签：先 CancelInteraction，再把 `app.*` 的真值 `capture`
    ///   成返回值，最后把新活跃标签的真快照 `restore` 进 `app.*`，
    ///   保证稳态契约不破（不会出现「app.* 还是被关图、items[active] 却是真快照」的错位）。
    ///
    /// `active` 收敛规则：关掉活跃标签时指向原位置（后面的标签上移）或新的末位；
    /// 关掉前面的标签时 `active -= 1`；关掉后面的标签时 `active` 不变。
    ///
    /// 只剩一张 ⇒ 返回中文错误（B 段再决定要不要允许全关），**不删、不改状态**。
    pub fn close(&mut self, app: &mut MireditApp, idx: usize) -> Result<TabState, String> {
        let 原长 = self.items.len();
        if idx >= 原长 {
            return Err(format!("标签下标越界：{idx}（当前共 {原长} 张标签）"));
        }
        if 原长 <= 1 {
            return Err("不能关闭唯一标签：至少保留一个标签（B 段再决定是否允许全关）".to_owned());
        }
        let 新长 = 原长 - 1;
        let 关活跃 = idx == self.active;
        let 新active = if 关活跃 {
            idx.min(新长 - 1)
        } else if idx < self.active {
            self.active - 1
        } else {
            self.active
        };

        if !关活跃 {
            // 非活跃槽本来就是真快照，直接返回；app.* 不变。
            let 被关 = self.items.remove(idx);
            self.active = 新active;
            return Ok(被关);
        }

        // CancelInteraction（与 activate 同款；被关标签不该带走"正在拖一半"的状态）。
        app.minimap_drag = None;
        app.sel_drag = None;
        app.armed = None;

        // 活跃槽是占位 ⇒ 先把 app.* 的真值换出来，作为"被关标签"的真快照返回。
        let mut 被关 = self.items.remove(idx);
        crate::app::capture(app, &mut 被关);
        // 此刻 app.* 是占位；把新活跃标签的真快照换进去，恢复稳态。
        let mut 新槽 = self.items.remove(新active);
        crate::app::restore(app, &mut 新槽);
        self.items.insert(新active, 新槽);
        app.lines_key = None;
        self.active = 新active;
        Ok(被关)
    }

    /// 按地图路径找标签（重复打开时切到已有，台账 §8.55 产品口径）。
    ///
    /// 活跃标签的真值在 `app.*`，所以从 `app` 读它的路径；非活跃标签从真快照读。
    /// **不读 `items[active]` 占位槽**。未命中返回 `None`。
    pub fn find_by_path(&self, app: &MireditApp, path: &Path) -> Option<usize> {
        for (i, tab) in self.items.iter().enumerate() {
            let p = if i == self.active {
                Self::active_path(app)
            } else {
                tab.path_of_snapshot()
            };
            if p == Some(path) {
                return Some(i);
            }
        }
        None
    }

    /// **第 `i` 张标签要显示的信息**（文件名 + 脏标记），供标签条绘制。
    ///
    /// # 稳态契约（本方法存在的唯一理由）
    ///
    /// - `i == active` ⇒ **从宿主 `app.*` 读**（`app.doc.doc.file_name` / `app.edit.dirty()`），
    ///   **绝不读 `items[active]`** —— 那是占位槽，读出来是空值/错值；
    /// - `i != active` ⇒ 从 `items[i]` 的**真快照**读。
    ///
    /// `i` 越界、或那张标签没有打开地图 ⇒ `None`（标签条就**不画**它，
    /// 而不是画一个 `—` 假名字；一张"没有文档的标签"在 B 段不存在，
    /// 唯一可能出现的场合是占位槽被误传进来 —— 那时 `None` 正好把它挡住）。
    ///
    /// # 文案口径 = 上游 `MainWindow.MapTabs.cs:120` / `:125-131`
    ///
    /// - 名字：有 `Path` ⇒ `Path.GetFileName(path)`；本仓 `DocState.file_name` 就是它
    ///   （`mapdoc.rs:456` 的 `file_name_of`，与内核 `Document::file_name` 同口径）。
    /// - 脏标记：上游 `(dirty ? " *" : "")` —— 直接拼接，见
    ///   [`crate::ui_text::TAB_TITLE_DIRTY_TMPL`]（`{name} *`，逐字节等价）。
    /// - ⚠ 上游的**无 Path 分支**（`Untitled {n}` / `Untitled stamp {n}`，
    ///   `MapTabs.cs:128-130`）在本仓**暂不可达**：没有"新建空白地图"入口，
    ///   `DocState.doc` 为 `None` 时本函数走 `?` 返回 `None`（标签条整张不画），
    ///   所有文档都从 `.map` 文件装载 ⇒ 恒有 `Path`。文案真值已备在
    ///   [`crate::ui_text::TAB_TITLE_UNTITLED_TMPL`]，等入口落地时直接用，别另写格式。
    pub fn 显示信息(&self, app: &MireditApp, i: usize) -> Option<TabInfo> {
        let name = self.name_of(app, i)?;
        let dirty = self.dirty_of(app, i)?;
        let tmpl = if dirty {
            crate::ui_text::TAB_TITLE_DIRTY_TMPL
        } else {
            crate::ui_text::TAB_TITLE_TMPL
        };
        Some(TabInfo {
            text: crate::ui_text::fill(tmpl, &[("name", &name)]),
            tmpl,
            dirty,
        })
    }

    /// **第 `i` 张标签的文件名**（**不带**脏标记）。
    ///
    /// 取值口径与 [`Self::显示信息`] **同一处**（活跃读 `app.doc`、非活跃读快照，
    /// 越界/无文档 ⇒ `None`），只是不拼 `" *"` —— 关闭确认框里已经写了
    /// 「有未保存的修改」（[`crate::ui_text::TAB_CLOSE_CONFIRM_TMPL`]），再带星号是重复。
    pub fn name_of(&self, app: &MireditApp, i: usize) -> Option<String> {
        if i >= self.items.len() {
            return None;
        }
        if i == self.active {
            Some(app.doc.doc.as_ref()?.file_name.clone())
        } else {
            Some(self.items[i].doc.doc.as_ref()?.file_name.clone())
        }
    }

    /// **第 `i` 张标签有没有未保存修改** —— 只读，不写任何业务逻辑。
    ///
    /// # 上游真值（逐条，2026-11-08 实读 `_upstream/CrystalStudio/`）
    ///
    /// `MainWindow.MapTabs.cs:119`
    /// ```csharp
    /// bool dirty = map.CategoryDirty || (ReferenceEquals(map, activeMap) ? Viewport.History.Dirty : map.State.History.Dirty);
    /// ```
    /// ⇒ 判据是「**活跃那张**读 `Viewport.History.Dirty`（= 本仓 `app.edit`），
    /// **非活跃那张**读它自己快照里的 `State.History.Dirty`（= 本仓 `items[i].edit`）」。
    /// 这与本模块的**稳态契约**逐字对应（活跃槽是占位、真值常驻 `app.*`）。
    ///
    /// ⚠ **上游有本仓无**：`map.CategoryDirty`（印章分类被改脏）在本仓**没有对应物**
    /// （没有"印章分类编辑"这条链）⇒ 本函数**只算 `History.Dirty` 那一支**，
    /// 不造空字段冒充（见回执「上游有本仓无」）。
    ///
    /// `i` 越界 ⇒ `None`（**不 panic、不编一个 `false` 出来**；调用方必须自己决定
    /// 越界时怎么办，不许被一个假 `false` 骗成"这张干净"）。
    pub fn dirty_of(&self, app: &MireditApp, i: usize) -> Option<bool> {
        if i >= self.items.len() {
            return None;
        }
        Some(if i == self.active {
            // 活跃标签的真值常驻宿主（稳态契约）——绝不读 `items[active]` 占位槽。
            app.edit.dirty()
        } else {
            self.items[i].edit.dirty()
        })
    }

    /// **全部标签里有没有任何一张脏**（含活跃那张）。
    ///
    /// # 上游真值：整窗关闭必须遍历**所有**标签
    ///
    /// `MainWindow.axaml.cs:424-430`（`OnClosing`）
    /// ```csharp
    /// foreach (var tab in mapTabs.Items.Cast<TabItem>().ToArray())
    /// {
    ///     var map = (OpenMapTab)tab.Tag;
    ///     if (!map.CategoryDirty && !(ReferenceEquals(map, activeMap) ? Viewport.History.Dirty : map.State.History.Dirty)) continue;
    ///     mapTabs.SelectedItem = tab;
    ///     if (!ConfirmDiscard()) { e.Cancel = true; return; }
    /// }
    /// ```
    /// ⇒ 上游**逐张**判脏、任一脏就拦；本仓的判据就是本函数（P1 数据丢失的修复点：
    /// 旧实现只看 `app.edit.dirty()` 一份，切到净标签 A 再改脏 B、直接关窗 ⇒ B 静默丢改动）。
    ///
    /// ⚠ 上游那段"逐张选中 + 逐张确认"的**逐个排队**不在本单：本仓先做"任一张脏就拦一次"，
    /// 见回执「待用户拍板」。
    pub fn any_dirty(&self, app: &MireditApp) -> bool {
        (0..self.items.len()).any(|i| self.dirty_of(app, i) == Some(true))
    }

    /// **脏标签的下标（升序）** —— 供后续单做「逐个确认」排队。
    ///
    /// 与 [`Self::any_dirty`] **同一条判据**（都走 [`Self::dirty_of`]），不另立一套；
    /// 越界项不可能出现（下标来自 `0..len`）。
    pub fn dirty_indices(&self, app: &MireditApp) -> Vec<usize> {
        (0..self.items.len())
            .filter(|&i| self.dirty_of(app, i) == Some(true))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use miredit_core::Selection;
    use miredit_render::{CollisionOverlay, LayerMask, RenderStats};

    use super::*;
    use crate::cli::Cli;
    use crate::edit::EditCmd;
    use crate::menu::Overlay;

    /// 一张标签的**逐值指纹**：覆盖 `TabState` 全部 15 个字段的可比部分。
    ///
    /// `DocState.assets` / `CanvasView.cache`（GPU 纹理）/ `MinimapJob.state`（含
    /// `TextureHandle` / `Receiver`）没有 `PartialEq`，用它们的公开派生量代替；
    /// 这几处限制在回执「未覆盖」里单列。
    #[derive(Debug, PartialEq)]
    struct 指纹 {
        // doc
        doc_path: Option<PathBuf>,
        doc_map_bytes: Vec<u8>,
        doc_map_gen: u64,
        doc_map_error: Option<String>,
        assets_lib_count: usize,
        assets_error: Option<String>,
        // view
        pan_x: f32,
        pan_y: f32,
        scale: f32,
        mask: LayerMask,
        collision: CollisionOverlay,
        render_calls: u64,
        last_render_ms: f32,
        budget_hit: bool,
        stats: RenderStats,
        // minimap
        mini_renders: u64,
        mini_uploads: u64,
        mini_drops: u64,
        mini_failures: u64,
        mini_last_ms: f32,
        mini_last_level: Option<u8>,
        // edit
        undo_len: usize,
        redo_len: usize,
        pos: u64,
        saved_pos: u64,
        dirty: bool,
        slot: usize,
        stroke_active: bool,
        last_cell: Option<(u16, u16)>,
        selection: Option<Selection>,
        // 其余随标签字段
        minimap_drag: Option<(f32, f32)>,
        selected_tool: usize,
        hover: Option<(u16, u16)>,
        hover_anchor: Option<Pos2>,
        lines: CellLines,
        lines_key: Option<(u16, u16, u64)>,
        preview_tex: Option<(String, TextureId, u32, u32)>,
        armed: Option<Armed>,
        sel_drag: Option<Drag>,
        op: Option<(String, bool)>,
        overlay_kind: &'static str,
    }

    impl 指纹 {
        fn of(s: &TabState) -> Self {
            let (doc_path, doc_map_bytes) = match &s.doc.doc {
                Some(d) => (Some(d.path.clone()), d.map.to_bytes()),
                None => (None, Vec::new()),
            };
            Self {
                doc_path,
                doc_map_bytes,
                doc_map_gen: s.doc.map_gen,
                doc_map_error: s.doc.map_error.clone(),
                assets_lib_count: s.doc.assets.lib_count(),
                assets_error: s.doc.assets.error().map(str::to_owned),
                pan_x: s.view.pan_x,
                pan_y: s.view.pan_y,
                scale: s.view.scale,
                mask: s.view.mask,
                collision: s.view.overlay,
                render_calls: s.view.render_calls,
                last_render_ms: s.view.last_render_ms,
                budget_hit: s.view.budget_hit,
                stats: s.view.stats,
                mini_renders: s.minimap.renders,
                mini_uploads: s.minimap.uploads,
                mini_drops: s.minimap.drops,
                mini_failures: s.minimap.failures,
                mini_last_ms: s.minimap.last_ms,
                mini_last_level: s.minimap.last_level,
                undo_len: s.edit.undo_len(),
                redo_len: s.edit.redo_len(),
                pos: s.edit.pos(),
                saved_pos: s.edit.saved_pos(),
                dirty: s.edit.dirty(),
                slot: s.edit.slot(),
                stroke_active: s.edit.stroke_active(),
                last_cell: s.edit.last_cell(),
                selection: s.edit.selection(),
                minimap_drag: s.minimap_drag,
                selected_tool: s.selected_tool,
                hover: s.hover,
                hover_anchor: s.hover_anchor,
                lines: s.lines.clone(),
                lines_key: s.lines_key,
                preview_tex: s.preview_tex.clone(),
                armed: s.armed.clone(),
                sel_drag: s.sel_drag,
                op: s.op.clone(),
                overlay_kind: match &s.overlay {
                    Overlay::None => "none",
                    Overlay::Shortcuts => "shortcuts",
                    Overlay::About => "about",
                    _ => "other",
                },
            }
        }
    }

    /// 最小合法 1×1 Old12 地图（52 字节头 + 12 字节格），纯合成、不依赖真实素材。
    ///
    /// 给 `(0,0)` 的 `bk_img` 写 7：这样 `--edit-erase` 能真的把它清成 0，
    /// 从而产生**一步撤销 + 脏标记**（全 0 的图落任何笔都是 0 改动，造不出脏态）。
    fn 临时图(tag: &str) -> PathBuf {
        let mut bytes = vec![0u8; mir2_map::HEADER_LEN + 12];
        bytes[0..2].copy_from_slice(&1u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&1u16.to_le_bytes());
        let base = mir2_map::HEADER_LEN;
        bytes[base..base + 2].copy_from_slice(&7u16.to_le_bytes());
        let p =
            std::env::temp_dir().join(format!("miredit_tabs_a_{tag}_{}.map", std::process::id()));
        std::fs::write(&p, &bytes).expect("测试合成图必须能写");
        p
    }

    /// 造一个真实宿主（`--edit-erase` 走的是生产路径，故 `edit` 有撤销步 / 脏标记）。
    fn 建应用(图: &Path, 编辑: bool, tag: &str) -> MireditApp {
        let mut cli = Cli {
            map: Some(图.to_path_buf()),
            settings: Some(std::env::temp_dir().join(format!(
                "miredit_tabs_a_settings_{tag}_{}.json",
                std::process::id()
            ))),
            ..Default::default()
        };
        if 编辑 {
            cli.edits.push(EditCmd::Erase(0, 0));
        }
        MireditApp::new(&cli)
    }

    /// 从宿主 `capture` 出一份**真快照**（含真实文档 / 编辑会话 / minimap / overlay）。
    fn 造槽(图: &Path, 编辑: bool, tag: &str) -> TabState {
        let mut app = 建应用(图, 编辑, tag);
        let mut 槽 = TabState::占位();
        crate::app::capture(&mut app, &mut 槽);
        槽
    }

    /// A/B 两个标签的判别值：pan 100/900、scale 0.5/2、工具 3/0、脏 1/0。
    fn 设定(槽: &mut TabState, pan: f32, scale: f32, tool: usize, 代号: u64) {
        槽.view.pan_x = pan;
        槽.view.pan_y = pan / 2.0;
        槽.view.scale = scale;
        槽.selected_tool = tool;
        槽.hover = Some((7 + 代号 as u16, 9 + 代号 as u16));
        槽.op = Some((format!("回执{代号}"), 代号 == 1));
        槽.minimap.renders = 代号 + 10;
        槽.minimap.uploads = 代号 + 20;
        // 不预置交互态：`activate` 会 CancelInteraction 收尾，预置值会被合法丢弃。
        槽.minimap_drag = None;
        槽.doc.map_gen = 代号 + 1;
        槽.overlay = if 代号 == 0 {
            Overlay::Shortcuts
        } else {
            Overlay::About
        };
    }

    /// ① 单标签 capture→restore 后所有字段逐值相等。
    #[test]
    fn 单标签_capture_restore_逐值相等() {
        let 图 = 临时图("roundtrip");
        let mut 槽 = 造槽(&图, true, "roundtrip");
        assert_eq!(槽.edit.undo_len(), 1, "夹具要先有一撤销步");
        assert!(槽.edit.dirty(), "夹具要先是脏的");
        设定(&mut 槽, 137.5, 0.5, 3, 0);
        槽.lines_key = Some((7, 9, 40));
        槽.hover_anchor = Some(Pos2::new(11.0, 22.0));
        槽.armed = Some(Armed {
            id: "stamp-a".to_owned(),
            name: "印章甲".to_owned(),
            bounds: Some((1, 2, 3, 4)),
            cells: None,
        });
        槽.sel_drag = Some(Drag { anchor: (2, 3) });
        槽.minimap.last_level = Some(7);
        let 前 = 指纹::of(&槽);

        // 把"目标真值"装进 app：restore 一次（app = 目标，槽 = 原 app 值）。
        let mut app = 建应用(&图, false, "roundtrip_host");
        crate::app::restore(&mut app, &mut 槽);

        // 标准 capture → restore 往返。
        let mut 快照 = TabState::占位();
        crate::app::capture(&mut app, &mut 快照);
        assert_eq!(指纹::of(&快照), 前, "capture 出来的必须是原真值");
        crate::app::restore(&mut app, &mut 快照);
        let mut 复 = TabState::占位();
        crate::app::capture(&mut app, &mut 复);
        assert_eq!(指纹::of(&复), 前, "restore 后所有字段逐值相等");

        // 逐项点名核对派单书要求的关键字段。
        assert_eq!(复.view.pan_x, 137.5);
        assert_eq!(复.view.scale, 0.5);
        assert_eq!(复.edit.undo_len(), 1);
        assert!(复.edit.dirty());
        assert_eq!(复.selected_tool, 3);
        assert_eq!(复.hover, Some((7, 9)));
        assert_eq!(复.lines_key, Some((7, 9, 40)));
        assert_eq!(复.minimap.renders, 10, "MinimapJob 要随快照换出换入");
        assert_eq!(
            复.overlay_kind_of(),
            "shortcuts",
            "Overlay 要随快照换出换入"
        );

        let _ = std::fs::remove_file(&图);
    }

    impl TabState {
        fn overlay_kind_of(&self) -> &'static str {
            match &self.overlay {
                Overlay::None => "none",
                Overlay::Shortcuts => "shortcuts",
                Overlay::About => "about",
                _ => "other",
            }
        }
    }

    /// ② 双标签来回 `activate` 不串台（来回切两次逐值核对）。
    #[test]
    fn 双标签_来回_activate_不串台() {
        let 图a = 临时图("swap_a");
        let 图b = 临时图("swap_b");
        // A：pan=100 / scale=0.5 / 工具=3 / 脏=1（真值在 app.*）
        let mut app = 建应用(&图a, true, "swap_host");
        app.view.pan_x = 100.0;
        app.view.pan_y = 50.0;
        app.view.scale = 0.5;
        app.selected_tool = 3;
        app.lines_key = Some((1, 2, 1));
        assert!(app.edit.dirty(), "A 必须脏");

        // B：pan=900 / scale=2 / 工具=0 / 脏=0
        let mut b = 造槽(&图b, false, "swap_b");
        设定(&mut b, 900.0, 2.0, 0, 1);
        assert!(!b.edit.dirty(), "B 必须不脏");
        let b指纹 = 指纹::of(&b);

        let mut tabs = Tabs::new(TabState::占位());
        tabs.push(b);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs.active(), 0);

        // 第 1 次：A → B
        assert!(tabs.activate(&mut app, 1).is_ok());
        assert_eq!(tabs.active(), 1);
        assert_eq!(app.view.pan_x, 900.0, "切到 B 后 pan 应是 900");
        assert_eq!(app.view.scale, 2.0);
        assert_eq!(app.selected_tool, 0);
        assert!(!app.edit.dirty(), "B 不该被 A 的脏标记串台");
        assert_eq!(app.lines_key, None, "硬约束①：切换后 lines_key 必须失效");
        assert_eq!(app.minimap.renders, 11, "B 的 MinimapJob 必须跟着换进来");
        assert!(
            matches!(app.overlay, Overlay::About),
            "B 的 Overlay 必须跟着换进来"
        );
        // 旧活跃槽（items[0]）现在必须是 A 的真快照。
        let mut a快照 = 指纹::of(&tabs.items[0]);
        assert_eq!(a快照.pan_x, 100.0);
        assert_eq!(a快照.scale, 0.5);
        assert_eq!(a快照.selected_tool, 3);
        assert!(a快照.dirty, "A 的脏标记要留在快照里");
        // `lines_key` 是硬约束①的易失派生缓存：切换后恒为 None，不参与往返相等判据。
        a快照.lines_key = None;

        // 第 2 次：B → A
        assert!(tabs.activate(&mut app, 0).is_ok());
        assert_eq!(app.view.pan_x, 100.0, "切回 A 后 pan 应是 100");
        assert_eq!(app.view.scale, 0.5);
        assert_eq!(app.selected_tool, 3);
        assert!(app.edit.dirty(), "A 的脏标记要回来");
        assert_eq!(app.lines_key, None, "硬约束①：再切回来仍要失效");
        assert_eq!(指纹::of(&tabs.items[1]), b指纹, "B 快照要完整留在槽里");
        // A 的真值现在常驻 app.*（items[0] 是占位）。
        let mut 现a = TabState::占位();
        crate::app::capture(&mut app, &mut 现a);
        assert_eq!(指纹::of(&现a), a快照, "切回 A 后 app.* 逐值等于 A 真值");
        crate::app::restore(&mut app, &mut 现a);

        // 第 3、4 次：再来回一轮，逐值核对不漂移。
        assert!(tabs.switch(&mut app, 1).is_ok());
        assert_eq!(app.view.pan_x, 900.0);
        assert_eq!(app.view.scale, 2.0);
        assert_eq!(app.selected_tool, 0);
        assert!(!app.edit.dirty());
        assert!(tabs.activate(&mut app, 0).is_ok());
        assert_eq!(app.view.pan_x, 100.0);
        assert_eq!(app.view.scale, 0.5);
        assert_eq!(app.selected_tool, 3);
        assert!(app.edit.dirty());
        // 终局 active=0 ⇒ items[0] 是占位、items[1] 是 B 真快照。
        let mut 终a = TabState::占位();
        crate::app::capture(&mut app, &mut 终a);
        assert_eq!(指纹::of(&终a), a快照, "A 两轮后仍逐值不变");
        crate::app::restore(&mut app, &mut 终a);
        assert_eq!(指纹::of(&tabs.items[1]), b指纹, "B 两轮后仍逐值不变");

        let _ = std::fs::remove_file(&图a);
        let _ = std::fs::remove_file(&图b);
    }

    /// ③ `activate` 到自身是 no-op（防占位槽被误当真值读出来）。
    #[test]
    fn activate_到自身_是_noop() {
        let 图 = 临时图("noop");
        let mut app = 建应用(&图, true, "noop");
        app.view.pan_x = 321.0;
        app.selected_tool = 6;
        app.lines_key = Some((3, 4, 5));
        app.armed = Some(Armed {
            id: "keep".to_owned(),
            name: "保留".to_owned(),
            bounds: None,
            cells: None,
        });
        app.sel_drag = Some(Drag { anchor: (1, 1) });
        app.minimap_drag = Some((1.0, 2.0));
        let mut tabs = Tabs::new(TabState::占位());

        let mut 前 = TabState::占位();
        crate::app::capture(&mut app, &mut 前);
        let 指纹前 = 指纹::of(&前);
        crate::app::restore(&mut app, &mut 前);

        assert!(tabs.activate(&mut app, tabs.active()).is_ok());
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs.active(), 0);
        assert_eq!(app.view.pan_x, 321.0, "no-op 不许把占位槽换进 app");
        assert_eq!(app.lines_key, Some((3, 4, 5)), "no-op 不许触发缓存失效");
        assert!(app.armed.is_some(), "no-op 不许触发 CancelInteraction");

        let mut 后 = TabState::占位();
        crate::app::capture(&mut app, &mut 后);
        assert_eq!(指纹::of(&后), 指纹前, "no-op 前后所有字段逐值不变");
        crate::app::restore(&mut app, &mut 后);
        let _ = std::fs::remove_file(&图);
    }

    /// ③b 硬约束② CancelInteraction：切换前收尾三个交互态（**丢弃**，不带进任何标签）。
    #[test]
    #[allow(non_snake_case)]
    fn activate_前_CancelInteraction_收尾交互态() {
        let 图a = 临时图("cancel_a");
        let 图b = 临时图("cancel_b");
        let mut app = 建应用(&图a, false, "cancel_host");
        app.minimap_drag = Some((3.0, 4.0));
        app.sel_drag = Some(Drag { anchor: (5, 6) });
        app.armed = Some(Armed {
            id: "old".to_owned(),
            name: "旧印章".to_owned(),
            bounds: None,
            cells: None,
        });
        let b = 造槽(&图b, false, "cancel_b");
        let mut tabs = Tabs::new(TabState::占位());
        tabs.push(b);

        assert!(tabs.activate(&mut app, 1).is_ok());
        assert_eq!(app.minimap_drag, None, "新标签不该继承小地图拖拽");
        assert_eq!(app.sel_drag, None, "新标签不该继承选区拖动");
        assert!(app.armed.is_none(), "新标签不该继承待放置印章");
        // A 的真快照里也是收尾后的干净值（CancelInteraction 在 capture 之前执行）。
        assert_eq!(tabs.items[0].minimap_drag, None);
        assert_eq!(tabs.items[0].sel_drag, None);
        assert!(tabs.items[0].armed.is_none());
        let _ = std::fs::remove_file(&图a);
        let _ = std::fs::remove_file(&图b);
    }

    /// ④ 活跃标签真值在 `app.*`，`items[active]` 是占位（B 段红线可执行化）。
    #[test]
    fn 活跃槽是占位_真值常驻宿主() {
        let 图a = 临时图("contract_a");
        let 图b = 临时图("contract_b");
        let mut app = 建应用(&图a, true, "contract_host");
        app.view.pan_x = 456.0;
        let mut b = 造槽(&图b, false, "contract_b");
        设定(&mut b, 789.0, 1.5, 2, 1);

        let mut tabs = Tabs::new(TabState::占位());
        tabs.push(b);
        assert!(tabs.activate(&mut app, 1).is_ok());

        // 活跃槽（items[1]）是占位：它的 pan_x 不是 789，真值 789 在 app.*。
        assert_ne!(tabs.items[1].view.pan_x, 789.0, "活跃槽不许存真值");
        assert_eq!(app.view.pan_x, 789.0, "真值必须常驻 app.*");
        // B 段要读活跃标签路径，走 active_path(app)，不许读 items[active]。
        assert_eq!(Tabs::active_path(&app), Some(图b.as_path()));
        assert_eq!(
            tabs.items[1].path_of_snapshot(),
            None,
            "占位槽不该给出业务路径"
        );
        let _ = std::fs::remove_file(&图a);
        let _ = std::fs::remove_file(&图b);
    }

    /// ⑤ 越界 `activate` 不 panic、返回中文错误、状态不变。
    #[test]
    fn 越界_activate_不panic_返回中文错误() {
        let 图 = 临时图("oob");
        let mut app = 建应用(&图, true, "oob");
        app.view.pan_x = 88.0;
        let mut tabs = Tabs::new(TabState::占位());

        for idx in [1, 2, usize::MAX] {
            let e = tabs.activate(&mut app, idx).expect_err("越界必须返回错误");
            assert!(e.contains("越界"), "错误要带中文原因，实得：{e}");
            assert!(e.contains("标签"), "错误要带中文原因，实得：{e}");
        }
        assert_eq!(tabs.active(), 0);
        assert_eq!(app.view.pan_x, 88.0, "越界失败不许改动 app.*");
        let _ = std::fs::remove_file(&图);
    }

    /// ⑥ `close(0)` 后剩余标签上移、`active` 收敛不越界。
    #[test]
    fn close_非活跃_剩余上移_active_收敛() {
        let 图a = 临时图("close_a");
        let 图b = 临时图("close_b");
        let 图c = 临时图("close_c");
        let mut app = 建应用(&图b, false, "close_host");
        let a = 造槽(&图a, true, "close_a");
        let c = 造槽(&图c, false, "close_c");
        let a指纹 = 指纹::of(&a);
        let c指纹 = 指纹::of(&c);

        // items = [真A, 占位B(active=1), 真C]
        let mut tabs = Tabs::new(a);
        tabs.push(TabState::占位());
        tabs.push(c);
        tabs.active = 1;

        let 被关 = tabs.close(&mut app, 0).expect("关闭非活跃标签应成功");
        assert_eq!(指纹::of(&被关), a指纹, "返回的必须是被关标签真快照");
        assert_eq!(tabs.len(), 2, "剩余两张");
        assert_eq!(tabs.active(), 0, "active 从 1 收敛到 0，不越界");
        assert_eq!(指纹::of(&tabs.items[1]), c指纹, "C 上移到下标 1");
        let _ = std::fs::remove_file(&图a);
        let _ = std::fs::remove_file(&图b);
        let _ = std::fs::remove_file(&图c);
    }

    /// ⑦ 关闭活跃标签：返回真快照 + 新活跃标签换入 `app.*`（稳态契约不破）。
    #[test]
    fn close_活跃_换入新标签且不破坏稳态() {
        let 图a = 临时图("closeact_a");
        let 图b = 临时图("closeact_b");
        let mut app = 建应用(&图a, true, "closeact_host");
        app.view.pan_x = 111.0;
        let mut b = 造槽(&图b, false, "closeact_b");
        设定(&mut b, 222.0, 1.25, 4, 1);
        let b指纹 = 指纹::of(&b);

        let mut tabs = Tabs::new(TabState::占位());
        tabs.push(b);

        let 被关 = tabs.close(&mut app, 0).expect("关闭活跃标签应成功");
        assert_eq!(被关.view.pan_x, 111.0, "返回值要是被关 A 的真快照");
        assert!(被关.edit.dirty(), "B 段脏检查要能读到 A 的脏标记");
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs.active(), 0);
        assert_eq!(app.view.pan_x, 222.0, "B 必须已换入 app.*");
        assert_eq!(app.view.scale, 1.25);
        assert_eq!(app.selected_tool, 4);
        assert_eq!(app.lines_key, None, "关闭活跃标签同样要失效缓存键");
        // 新活跃槽应是占位，真快照已换进 app.*。
        assert_ne!(tabs.items[0].view.pan_x, 222.0);
        let mut 现 = TabState::占位();
        crate::app::capture(&mut app, &mut 现);
        assert_eq!(指纹::of(&现), b指纹, "换入后的 app.* 逐值等于 B 真快照");
        crate::app::restore(&mut app, &mut 现);
        let _ = std::fs::remove_file(&图a);
        let _ = std::fs::remove_file(&图b);
    }

    /// ⑧ 关闭唯一标签 ⇒ 中文错误；越界 close ⇒ 中文错误。
    #[test]
    fn close_唯一标签与越界_返回中文错误() {
        let 图 = 临时图("closelast");
        let mut app = 建应用(&图, false, "closelast");
        let mut tabs = Tabs::new(TabState::占位());
        let e = match tabs.close(&mut app, 0) {
            Err(msg) => msg,
            Ok(_) => panic!("唯一标签不许关，却返回了 Ok"),
        };
        assert!(e.contains("唯一标签"), "实得：{e}");
        assert_eq!(tabs.len(), 1);
        let e2 = match tabs.close(&mut app, 7) {
            Err(msg) => msg,
            Ok(_) => panic!("越界不许关，却返回了 Ok"),
        };
        assert!(e2.contains("越界"), "实得：{e2}");
        let _ = std::fs::remove_file(&图);
    }

    /// ⑩ `dirty_of` 三态：活跃读 `app.edit`、非活跃读快照、越界 `None`；
    /// `any_dirty` / `dirty_indices` 与它**同一条判据**（整窗关闭的 P1 修复就靠这一处）。
    #[test]
    fn dirty_of_活跃非活跃越界三态() {
        let 图a = 临时图("dirty_a");
        let 图b = 临时图("dirty_b");
        // 摆位故意"两个来源相反"：活跃 A **净**、快照 B **脏**。
        let mut app = 建应用(&图a, false, "dirty_host");
        assert!(!app.edit.dirty(), "夹具：活跃标签 A 必须净");
        let b = 造槽(&图b, true, "dirty_b");
        assert!(b.edit.dirty(), "夹具：快照 B 必须脏");
        let mut tabs = Tabs::new(TabState::占位());
        tabs.push(b);

        assert_eq!(tabs.dirty_of(&app, 0), Some(false), "活跃标签读 app.edit");
        assert_eq!(tabs.dirty_of(&app, 1), Some(true), "非活跃标签读 items[i].edit");
        assert_eq!(
            tabs.dirty_of(&app, 2),
            None,
            "越界必须 None（不许编一个假 false 出来骗调用方）"
        );
        assert_eq!(tabs.dirty_of(&app, usize::MAX), None);
        assert!(tabs.any_dirty(&app), "有一张脏 ⇒ any_dirty 必须为真");
        assert_eq!(tabs.dirty_indices(&app), vec![1], "脏下标必须是 [1]");

        // 切到 B 之后两个来源对调：判据必须跟着换（活跃读 app.*，快照读 items[i]）
        assert!(tabs.activate(&mut app, 1).is_ok());
        assert_eq!(tabs.dirty_of(&app, 1), Some(true), "切换后 B 是活跃 ⇒ 读 app.edit");
        assert_eq!(tabs.dirty_of(&app, 0), Some(false), "切换后 A 是快照 ⇒ 读 items[0].edit");
        assert!(tabs.any_dirty(&app), "切换不改变「有一张脏」这个事实");
        assert_eq!(tabs.dirty_indices(&app), vec![1], "切换不改变脏下标");

        // 全净（只有活跃那一张、且它干净）⇒ any_dirty 假、dirty_indices 空
        let 图c = 临时图("dirty_c");
        let app2 = 建应用(&图c, false, "dirty_host2");
        let tabs2 = Tabs::new(TabState::占位());
        assert_eq!(tabs2.dirty_of(&app2, 0), Some(false));
        assert!(!tabs2.any_dirty(&app2), "全净 ⇒ any_dirty 必须为假");
        assert!(tabs2.dirty_indices(&app2).is_empty());

        let _ = std::fs::remove_file(&图a);
        let _ = std::fs::remove_file(&图b);
        let _ = std::fs::remove_file(&图c);
    }

    /// ⑨ `find_by_path` 命中（含活跃标签）/ 未命中 None。
    #[test]
    fn find_by_path_命中与未命中() {
        let 图a = 临时图("find_a");
        let 图b = 临时图("find_b");
        let mut app = 建应用(&图a, false, "find_host");
        let b = 造槽(&图b, false, "find_b");
        let mut tabs = Tabs::new(TabState::占位());
        tabs.push(b);

        assert_eq!(
            tabs.find_by_path(&app, &图a),
            Some(0),
            "活跃标签从 app.* 读"
        );
        assert_eq!(tabs.find_by_path(&app, &图b), Some(1), "非活跃标签从快照读");
        assert_eq!(tabs.find_by_path(&app, Path::new("不存在的路径.map")), None);
        // 切到 B 后，命中下标不变但真值来源对调，仍不许读占位槽。
        assert!(tabs.activate(&mut app, 1).is_ok());
        assert_eq!(tabs.find_by_path(&app, &图b), Some(1));
        assert_eq!(tabs.find_by_path(&app, &图a), Some(0));
        let _ = std::fs::remove_file(&图a);
        let _ = std::fs::remove_file(&图b);
    }
}
