//! 小地图**总览纹理**的后台渲染 + 缓存 + 代次校验（B 段，2026-11-07）。
//!
//! # 一、为什么必须后台
//!
//! [`miredit_render::render_overview`] 要对**整张地图**做一次三层合成
//! （`0.map` 是 `700×700` = 49 万格 × 3 层，落格函数与 1× 合成同一份）。
//! 放在 UI 线程里就是每次几百毫秒的**卡死**（`0.map` 实测 **404~542 ms**，见 `[小地图]`
//! 诊断行）；而它又是"打开图就要有"的常驻内容 ⇒ 只能扔到工作线程，
//! 主线程只负责"收结果 + 传纹理"。图越大、帧种类越多，这个数越往秒级走。
//!
//! # 二、线程边界（硬约束）
//!
//! | 允许跨线程 | 禁止跨线程 |
//! |---|---|
//! | `Map`（`Clone`）、`Arc<WilSource>`（内部 `Mutex` ⇒ `Send + Sync`）、`LayerMask` / `RenderOptions`（均 `Copy`） | `egui::Context`、任何 `TextureHandle` / `Painter` |
//!
//! worker **绝不持 `egui::Context`**：它只把 `Vec<u8>` + `w/h/level` + 发起时的
//! `map_gen` 装进 [`MiniReply`] 回传；`ctx.load_texture` 只在主线程
//! [`MinimapJob::poll`] 里调（与 `canvas_view::ensure` 的既有口径一致）。
//!
//! # 三、缓存键与"命中不重算"
//!
//! 键 = [`MiniKey`]：`(map_gen, 图尺寸, level, 目标像素尺寸, 图层开关, 渲染选项)`。
//! 前四项是任务书要求的最小集（同一张图只平移/缩放 ⇒ 键不动 ⇒ **0 次重算**）；
//! 后两项是**必须补**的：关掉一层会改像素、`doors_open` 会改表层帧号 ——
//! 少了它们就会出现「开关点了、小地图没变」的假交互（`canvas_view` 的 E3 / P0-9
//! 两条审计坑同源）。`options.lighting` 在总览里不生效（`overview` 模块文档 §四.2），
//! 但整份选项进键是"同一份真源、不拆两条路径"，宁可多一次重算也不让两条路径漂。
//!
//! # 四、代次校验（`map_gen`）
//!
//! 发起时把当时的 `map_gen` 钉进回传包；主线程收包时与**当前** `map_gen` 比对：
//! - 不符 ⇒ **丢弃**（计一次 [`MinimapJob::drops`]），状态回 `Idle` 让下一帧按新代号重发；
//!   绝不把旧图的像素贴到新图上（那是"画了一张不是当前的图"，比空白更坏）；
//! - 相符 ⇒ 上传纹理并缓存。
//!
//! # 五、失败态（不许画假像素、不许每帧重试刷屏）
//!
//! `OverviewError` ⇒ 缓存一条**失败态**（带 `map_gen` + 一句中文原因），并：
//! 1. **不产生任何纹理**（[`MinimapJob::texture`] 恒 `None`）⇒ 界面画不出假像素；
//! 2. **同一 `map_gen` 只试一次**：失败态只要 `map_gen` 没变就直接短路，
//!    换档/改窗口尺寸也不会重试（防止"每帧重试 + 每帧打一行日志"把界面与 stderr 一起刷爆）。
//!    换图（`map_gen + 1`）才会给这张图第二次机会 —— 那本来也是另一张图。
//!
//! # 六、档位怎么选（[`pick_level`]）
//!
//! 目标是把总览图做成"**大概就是屏幕上那块地图像素那么多**"：
//! `level = floor(log2(世界像素 / 目标像素))`。**向下取整**是有意的 —— 宁可多几个像素
//! （略微过采样）也不要少（少就是糊）；`overview` 自己的预算护栏还会在超 64 MiB 时抬档，
//! 抬过的实际档位由 [`miredit_render::Overview::level`] 如实回报，本模块把它存进
//! [`MinimapJob::last_level`]，**不假设**它就是发起时那个 `level`。

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

use egui::{Context, TextureHandle};
use mir2_assets::SpriteSource;
use mir2_map::Map;
use miredit_render::{render_overview, LayerMask, RenderOptions, OVERVIEW_MAX_LEVEL};

use crate::metrics as m;
use crate::ui_text as tx;

/// 上屏纹理的名字（`ctx.load_texture` 的名字，只为调试器里认得出）。
const TEX_NAME: &str = "miredit-minimap";

/// 一次后台渲染的**输入键**：这一组全部相等才复用上屏纹理（§三）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MiniKey {
    /// 地图代号：换图 / 改图 `+1`（与画布缓存同一个真源 `DocState::map_gen`）。
    pub map_gen: u64,
    /// 地图格数 `(宽, 高)`：裁剪 / 扩边会改它。
    pub cells: (u16, u16),
    /// 总览档位（`1 px = 2^level` 世界像素）：由目标像素尺寸与格数算出（[`pick_level`]）。
    pub level: u8,
    /// 目标像素尺寸（地图区矩形 × `pixels_per_point`，取整）。
    pub target: (u32, u32),
    /// 三层开关（关某层会改像素 ⇒ 必须在键里）。
    pub mask: LayerMask,
    /// 渲染选项（总览只消费 `doors_open`，但整份进键 —— 同一份真源不拆路径）。
    pub options: RenderOptions,
}

/// 后台线程回传的**纯数据**（§二：这里不许出现任何 `egui` 类型）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct MiniImage {
    width: u32,
    height: u32,
    /// **实际**档位（预算抬过档时可能大于发起时传的那个）。
    level: u8,
    /// 行优先 RGBA8，长度 = `width * height * 4`。
    rgba: Vec<u8>,
}

/// 一次后台渲染的回执：发起时的键 + 耗时 + 结果（纯数据）。
#[derive(Debug)]
struct MiniReply {
    map_gen: u64,
    key: MiniKey,
    /// worker 内实测的渲染耗时（毫秒，只进诊断，不上屏）。
    ms: f32,
    out: Result<MiniImage, String>,
}

/// 状态机（同一时刻只有一支）。
enum State {
    /// 什么都不做：没地图 / 没图库，或结果刚被丢弃等下一帧重发。
    Idle,
    /// 后台在跑（`rx` 非 `None`）。
    Loading { key: MiniKey },
    /// 有一张可上屏的纹理。
    Ready {
        key: MiniKey,
        tex: TextureHandle,
        /// 纹理像素尺寸（诊断/验收用；不是屏幕矩形）。
        size: (u32, u32),
    },
    /// 失败：`map_gen` 不变就不再试（§五）。
    Failed {
        map_gen: u64,
        key: MiniKey,
        msg: String,
    },
}

/// 小地图总览纹理的后台任务 + 缓存。
///
/// 计数器（[`Self::renders`] / [`Self::uploads`] / [`Self::drops`] / [`Self::failures`]）是
/// 本模块的**取证口**：缓存命中、代次丢弃、失败短路这三条纪律都只能靠它们证伪
/// （参考 `canvas_view::CanvasView::render_calls`）。
pub struct MinimapJob {
    state: State,
    rx: Option<Receiver<MiniReply>>,
    /// 发起过的后台渲染次数（含失败的、被丢弃的）。
    pub renders: u64,
    /// 成功上传纹理的次数。
    pub uploads: u64,
    /// 结果因 `map_gen` 不符被丢弃的次数。
    pub drops: u64,
    /// 进入失败态的次数。
    pub failures: u64,
    /// 最近一次**成功**渲染在 worker 内实测的耗时（毫秒）。
    pub last_ms: f32,
    /// 最近一次成功渲染的**实际**档位（预算抬档后的真值）。
    pub last_level: Option<u8>,
}

impl Default for MinimapJob {
    fn default() -> Self {
        Self {
            state: State::Idle,
            rx: None,
            renders: 0,
            uploads: 0,
            drops: 0,
            failures: 0,
            last_ms: 0.0,
            last_level: None,
        }
    }
}

impl MinimapJob {
    /// 当前可上屏的纹理（失败态 / 未落定 ⇒ `None`）。
    pub fn texture(&self) -> Option<&TextureHandle> {
        match &self.state {
            State::Ready { tex, .. } => Some(tex),
            _ => None,
        }
    }

    /// 上屏纹理的像素尺寸 `(宽, 高)`（还没成功过 ⇒ `None`）。
    pub fn texture_size(&self) -> Option<(u32, u32)> {
        match &self.state {
            State::Ready { size, .. } => Some(*size),
            _ => None,
        }
    }

    /// 后台还在跑（截图前的"有界等待"看这个，见 `app::SHOT_MINIMAP_WAIT_MAX_FRAMES`）。
    pub fn pending(&self) -> bool {
        matches!(self.state, State::Loading { .. })
    }

    /// 失败原因（中文一句话；就绪 / 未开始 ⇒ `None`）。
    pub fn failure(&self) -> Option<&str> {
        match &self.state {
            State::Failed { msg, .. } => Some(msg.as_str()),
            _ => None,
        }
    }

    /// 当前缓存键（加载中 / 就绪 / 失败都有键；`Idle` ⇒ `None`）。
    pub fn key(&self) -> Option<MiniKey> {
        match &self.state {
            State::Idle => None,
            State::Loading { key } | State::Ready { key, .. } | State::Failed { key, .. } => {
                Some(*key)
            }
        }
    }

    /// 清空（没地图 / 没图库时调用；不留上一张图的残影）。
    pub fn clear(&mut self) {
        self.state = State::Idle;
        self.rx = None;
    }

    /// 保证"当前输入"有一份总览图：命中缓存 ⇒ **不重算**；否则起一个后台线程。
    ///
    /// 返回 `true` = 这一帧真的发起了新任务（调用方据此 `request_repaint`，
    /// 否则后台跑完没人来收，界面会停在「缩略图生成中…」等用户动鼠标）。
    pub fn ensure(
        &mut self,
        map: Option<&Map>,
        src: Option<Arc<dyn SpriteSource + Send + Sync>>,
        map_gen: u64,
        mask: LayerMask,
        options: RenderOptions,
        target: (u32, u32),
    ) -> bool {
        let (Some(map), Some(src)) = (map, src) else {
            self.clear();
            return false;
        };
        let cells = (map.width, map.height);
        let level = pick_level(cells, target);
        let key = MiniKey {
            map_gen,
            cells,
            level,
            target,
            mask,
            options,
        };

        // ① 换图 ⇒ 旧纹理**立刻作废**（同一张图的换档/改尺寸则保留旧图直到新图落定，
        //    避免拖动窗口尺寸时小地图闪成空白）
        if let State::Ready { key: 旧, .. } = &self.state {
            if 旧.map_gen != map_gen {
                self.state = State::Idle;
            }
        }

        // ② 缓存命中 / 在跑 / 同一代次已失败 ⇒ 什么都不做
        match &self.state {
            State::Ready { key: 旧, .. } if *旧 == key => return false,
            State::Loading { key: 旧 } if *旧 == key => return false,
            State::Failed {
                map_gen: 旧代号, ..
            } if *旧代号 == map_gen => return false,
            _ => {}
        }

        // ③ 起线程：**只搬纯数据**（`Map` 克隆 + `Arc` 句柄 + 两个 `Copy` 选项 + level）
        let (txr, rx) = mpsc::channel();
        let 图: Map = (*map).clone();
        self.state = State::Loading { key };
        self.rx = Some(rx);
        self.renders += 1;
        std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            // 去掉 `Send + Sync` 两个 auto trait：渲染库只认 `&dyn SpriteSource`
            let 源: &dyn SpriteSource = &*src;
            let out = render_overview(&图, 源, level, &mask, &options)
                .map(|ov| MiniImage {
                    width: ov.width,
                    height: ov.height,
                    level: ov.level,
                    rgba: ov.rgba,
                })
                .map_err(|e| e.to_string());
            // 发送失败 = 主线程已经不要这个结果了（换图后丢弃接收端）⇒ 静默结束即可
            let _ = txr.send(MiniReply {
                map_gen,
                key,
                ms: t0.elapsed().as_secs_f32() * 1e3,
                out,
            });
        });
        true
    }

    /// 收一次后台结果（**主线程**：上传纹理只能在这里做）。
    ///
    /// 返回 `true` = 这一帧状态变了（调用方据此 `request_repaint`）。
    pub fn poll(&mut self, ctx: &Context, map_gen: u64) -> bool {
        // 借用收在一句里：下面各支都要改 `self.state` / `self.rx`
        let 结果 = {
            let Some(rx) = self.rx.as_ref() else {
                return false;
            };
            rx.try_recv()
        };
        match 结果 {
            Ok(reply) => {
                self.rx = None;
                if reply.map_gen != map_gen {
                    // §四：结果作废。回 `Idle` ⇒ 下一帧 `ensure` 按新代号重发。
                    self.drops += 1;
                    self.state = State::Idle;
                    eprintln!(
                        "{}",
                        tx::fill(
                            tx::DIAG_MINIMAP_DROP,
                            &[
                                ("old", &reply.map_gen.to_string()),
                                ("now", &map_gen.to_string()),
                            ]
                        )
                    );
                    return true;
                }
                match reply.out {
                    Ok(img) => {
                        let 图 = egui::ColorImage::from_rgba_unmultiplied(
                            [img.width as usize, img.height as usize],
                            &img.rgba,
                        );
                        // 总览本来就被画到比它略小的一块矩形上；LINEAR 让它别像素化
                        // （最近邻只在"整数倍"时才对，见 `canvas_view` 文档的过滤档一节）。
                        let tex = ctx.load_texture(TEX_NAME, 图, egui::TextureOptions::LINEAR);
                        self.last_ms = reply.ms;
                        self.last_level = Some(img.level);
                        self.uploads += 1;
                        eprintln!(
                            "{}",
                            tx::fill(
                                tx::DIAG_MINIMAP,
                                &[
                                    ("level", &img.level.to_string()),
                                    ("w", &img.width.to_string()),
                                    ("h", &img.height.to_string()),
                                    ("ms", &format!("{:.1}", reply.ms)),
                                    ("n", &self.renders.to_string()),
                                ]
                            )
                        );
                        self.state = State::Ready {
                            key: reply.key,
                            tex,
                            size: (img.width, img.height),
                        };
                        true
                    }
                    Err(msg) => {
                        self.failures += 1;
                        eprintln!("{}", tx::fill(tx::DIAG_MINIMAP_FAIL, &[("err", &msg)]));
                        self.state = State::Failed {
                            map_gen: reply.map_gen,
                            key: reply.key,
                            msg,
                        };
                        true
                    }
                }
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                // 线程没回传就退了（panic / 被 kill）：按失败态记账，**不重试**（同一代次只试一次）
                let key = match &self.state {
                    State::Loading { key } => *key,
                    _ => return false,
                };
                let msg = tx::MINIMAP_THREAD_LOST.to_owned();
                self.rx = None;
                self.failures += 1;
                eprintln!("{}", tx::fill(tx::DIAG_MINIMAP_FAIL, &[("err", &msg)]));
                self.state = State::Failed {
                    map_gen: key.map_gen,
                    key,
                    msg,
                };
                true
            }
        }
    }
}

/// 目标像素尺寸 ⇒ 总览档位（§六）：`floor(log2(世界像素 / 目标像素))`，夹到 `0..=16`。
///
/// `target` 的 `0` 按 `1` 处理（**不除零**）；返回值恒 `<= OVERVIEW_MAX_LEVEL`，
/// 因为超过 16 的档位在 `render_overview` 里是明确报错而不是"自动夹"。
fn pick_level(cells: (u16, u16), target: (u32, u32)) -> u8 {
    let 世界宽 = f64::from(cells.0) * f64::from(m::CELL_W);
    let 世界高 = f64::from(cells.1) * f64::from(m::CELL_H);
    let 比 = (世界宽 / f64::from(target.0.max(1)))
        .max(世界高 / f64::from(target.1.max(1)))
        .max(1.0);
    // `比 >= 1` 且输入都是有限数 ⇒ log2 不会是 NaN；仍做一次 `is_finite` 兜底，
    // 免得 `as i64` 把非有限值静默变成 0（静默出错比报错难查）。
    let 档 = if 比.is_finite() {
        比.log2().floor() as i64
    } else {
        0
    };
    档.clamp(0, i64::from(OVERVIEW_MAX_LEVEL)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_assets::{Frame, LibRef};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// `WilSource` 必须能跨线程：`Arc<WilSource>` 要 `Send` 就得 `WilSource: Send + Sync`。
    ///
    /// 这条断言是**编译期**的（函数体不运行也算数）：哪天有人在 `WilSource` 里塞进
    /// `Rc`/`RefCell`，`Arc` 改造会当场在这里红，而不是等到线上偶发数据竞争。
    #[test]
    fn 图源可跨线程() {
        fn 断言_send_sync<T: Send + Sync>() {}
        断言_send_sync::<mir2_assets::WilSource>();
    }

    /// 造一张 `w×h` 的 Old12 图，**每格三层都带图号 `1`**（好让图源真的被问到）。
    ///
    /// 12 B/格、列优先，与 [`mir2_map::Map::cells`] 同序：`cells[x * h + y]`。
    fn 造图(w: u16, h: u16) -> Map {
        let mut bytes = vec![0u8; 52 + 12 * (w as usize * h as usize)];
        bytes[0..2].copy_from_slice(&w.to_le_bytes());
        bytes[2..4].copy_from_slice(&h.to_le_bytes());
        for i in 0..(w as usize * h as usize) {
            let base = 52 + i * 12;
            bytes[base..base + 2].copy_from_slice(&1u16.to_le_bytes()); // bk_img
            bytes[base + 2..base + 4].copy_from_slice(&1u16.to_le_bytes()); // mid_img
            bytes[base + 4..base + 6].copy_from_slice(&1u16.to_le_bytes()); // fr_img
        }
        Map::parse(&bytes).expect("合成地图应能解析")
    }

    /// 取帧计数源：每次 `frame` 计数 +1（用来证明"命中缓存不再向图库要帧"）。
    struct 计数源 {
        calls: AtomicUsize,
    }

    impl 计数源 {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
            })
        }

        fn 次数(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl SpriteSource for 计数源 {
        fn frame(&self, _lib: LibRef, _index: u32) -> Option<Frame> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    fn 句柄(src: Arc<计数源>) -> Arc<dyn SpriteSource + Send + Sync> {
        // 显式返回类型的函数体是"目标类型明确的赋值位置" ⇒ unsize 强制转换在这里成立
        src
    }

    /// 等后台落定（最多 5 s；超时即夹具失效，**不**静默放过）。
    fn 等落定(job: &mut MinimapJob, ctx: &Context, gen: u64) {
        let t0 = std::time::Instant::now();
        while job.pending() && t0.elapsed() < std::time::Duration::from_secs(5) {
            job.poll(ctx, gen);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(!job.pending(), "后台渲染 5 s 内没落定（夹具失效）");
    }

    /// 用指定代号 `poll` 直到状态真的变了（最多 5 s）—— 代次丢弃用例要掐这一刻。
    fn 等到状态变化(job: &mut MinimapJob, ctx: &Context, gen: u64) {
        let t0 = std::time::Instant::now();
        while t0.elapsed() < std::time::Duration::from_secs(5) {
            if job.poll(ctx, gen) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("后台结果 5 s 内没到（夹具失效）");
    }

    /// **T1 缓存**：同一组输入（`map_gen` 不变，只有 pan/缩放这种"目标像素不变"的动作）
    /// ⇒ **重算 0 次**；`map_gen + 1` ⇒ **恰 1 次**。
    #[test]
    fn 缓存命中不重算_换代号恰重算一次() {
        let ctx = Context::default();
        let map = 造图(8, 8);
        let src = 计数源::new();
        let mask = LayerMask::all();
        let opts = RenderOptions::default();
        let target = (188, 126);
        let mut job = MinimapJob::default();

        // 第一次：必然要渲
        assert!(
            job.ensure(Some(&map), Some(句柄(src.clone())), 7, mask, opts, target),
            "第一次必须发起后台任务"
        );
        assert_eq!(job.renders, 1);
        等落定(&mut job, &ctx, 7);
        assert_eq!(job.uploads, 1, "落定后要上传一次纹理");
        // 8*48/2 = 192、8*32/2 = 128（level 1：比 = max(384/188, 256/126) = 2.04 ⇒ floor(log2) = 1）
        assert_eq!(job.texture_size(), Some((192, 128)));
        assert_eq!(
            job.last_level,
            Some(1),
            "档位要如实回报（不是发起时的猜测）"
        );
        let 取帧次数 = src.次数();
        assert!(取帧次数 >= 3, "三层各要问一次帧（实际 {取帧次数}）");

        // 再调 3 次（模拟"只平移/缩放、窗口尺寸不变"的连续帧）：一次都不许重算
        for _ in 0..3 {
            assert!(
                !job.ensure(Some(&map), Some(句柄(src.clone())), 7, mask, opts, target),
                "同键必须命中缓存"
            );
        }
        assert_eq!(job.renders, 1, "同键不许重算（缓存命中 0 次重算）");
        assert_eq!(src.次数(), 取帧次数, "命中缓存不许再向图库要帧");

        // `map_gen + 1`：换图 ⇒ 恰重算一次
        assert!(
            job.ensure(Some(&map), Some(句柄(src.clone())), 8, mask, opts, target),
            "换代号必须重新发起"
        );
        assert_eq!(job.renders, 2, "map_gen + 1 恰重算一次");
        等落定(&mut job, &ctx, 8);
        assert_eq!(job.uploads, 2);
        assert_eq!(job.key().map(|k| k.map_gen), Some(8), "缓存键要跟上新代号");

        // 图层开关进键：关掉中层 ⇒ 也是新键（否则「关了一层、小地图没变」）
        let 关中层 = LayerMask {
            ground: true,
            middle: false,
            front: true,
        };
        assert!(
            job.ensure(Some(&map), Some(句柄(src)), 8, 关中层, opts, target),
            "图层开关是缓存键的一部分"
        );
        assert_eq!(job.renders, 3);
    }

    /// **T2 代次**：发起后把 `map_gen` 改掉再 `poll` ⇒ 结果被丢弃、纹理不更新。
    #[test]
    fn 代次不符的结果被丢弃且不传纹理() {
        let ctx = Context::default();
        let map = 造图(8, 8);
        let src = 计数源::new();
        let mut job = MinimapJob::default();
        assert!(job.ensure(
            Some(&map),
            Some(句柄(src.clone())),
            5,
            LayerMask::all(),
            RenderOptions::default(),
            (188, 126)
        ));
        assert_eq!(job.renders, 1);

        // 发起的是 5 号图，收包时已经是 6 号图
        等到状态变化(&mut job, &ctx, 6);
        assert_eq!(job.drops, 1, "代号不符必须记一次丢弃");
        assert_eq!(job.uploads, 0, "被丢弃的结果绝不许上传");
        assert!(job.texture().is_none(), "被丢弃的结果绝不许上屏");
        assert!(!job.pending(), "丢弃后回 Idle ⇒ 下一帧可以按新代号重发");

        // 下一帧按新代号重发（同一张图的新代号才允许再试）
        assert!(
            job.ensure(
                Some(&map),
                Some(句柄(src)),
                6,
                LayerMask::all(),
                RenderOptions::default(),
                (188, 126)
            ),
            "丢弃后必须能按新代号重发"
        );
        assert_eq!(job.renders, 2);
        等落定(&mut job, &ctx, 6);
        assert!(job.texture().is_some(), "重发的这一次要正常上屏");
    }

    /// **T3 失败态**：每个 `map_gen` 只试一次、有中文原因、**无纹理**（不许画假像素）。
    ///
    /// 失败用 `render_overview` 自己的 `ZeroCells` 护栏制造（`0×0` 地图），
    /// 不是 mock 一个"假错误" —— 走的是与线上完全同一条失败链路。
    #[test]
    fn 失败态每代次只试一次且无纹理() {
        let ctx = Context::default();
        let mut 空图 = 造图(1, 1);
        空图.width = 0;
        空图.height = 0;
        空图.cells.clear();
        let src = 计数源::new();
        let mut job = MinimapJob::default();
        let mask = LayerMask::all();
        let opts = RenderOptions::default();

        assert!(job.ensure(
            Some(&空图),
            Some(句柄(src.clone())),
            3,
            mask,
            opts,
            (188, 126)
        ));
        等落定(&mut job, &ctx, 3);
        assert_eq!(job.failures, 1, "失败要记一次");
        assert!(job.texture().is_none(), "失败态绝不许有纹理（那是假像素）");
        let msg = job.failure().expect("失败必须带中文原因").to_owned();
        assert!(!msg.is_empty(), "原因不许是空串");

        // 同一代号：无论换档 / 改目标尺寸都不许再试（防每帧重试刷屏）
        assert!(!job.ensure(
            Some(&空图),
            Some(句柄(src.clone())),
            3,
            mask,
            opts,
            (188, 126)
        ));
        assert!(!job.ensure(
            Some(&空图),
            Some(句柄(src.clone())),
            3,
            LayerMask::none(),
            RenderOptions::default(),
            (99, 77)
        ));
        assert_eq!(job.renders, 1, "同一 map_gen 只许试一次");
        assert_eq!(job.failures, 1);
        assert!(job.texture().is_none());

        // 换图（新代号）：给这一次机会 ⇒ 重新发起
        assert!(job.ensure(Some(&空图), Some(句柄(src)), 4, mask, opts, (188, 126)));
        assert_eq!(job.renders, 2, "换代号才允许再试");
    }

    /// 档位选择：向下取整（宁可过采样）、`0` 目标不除零、夹到上游 16 级上限。
    #[test]
    fn 档位选择口径() {
        // 700×700 格 = 33600×22400 世界像素；目标 188×126 ⇒ 比 = 178.7 ⇒ floor(log2) = 7
        assert_eq!(pick_level((700, 700), (188, 126)), 7);
        // 目标比世界还大 ⇒ 0 档（不许负数）
        assert_eq!(pick_level((8, 8), (10_000, 10_000)), 0);
        // 目标 0 按 1 处理：8*48/1 = 384 ⇒ log2 = 8.58 ⇒ 8（不除零、也不夹到 0）
        assert_eq!(pick_level((8, 8), (0, 0)), 8);
        // 目标极小 ⇒ 夹到上游 16 级上限
        assert_eq!(pick_level((u16::MAX, u16::MAX), (1, 1)), OVERVIEW_MAX_LEVEL);
    }

    /// 没地图 / 没图库 ⇒ 清空（不留上一张图的残影）。
    #[test]
    fn 没有图源就清空() {
        let ctx = Context::default();
        let map = 造图(4, 4);
        let src = 计数源::new();
        let mut job = MinimapJob::default();
        assert!(job.ensure(
            Some(&map),
            Some(句柄(src)),
            1,
            LayerMask::all(),
            RenderOptions::default(),
            (188, 126)
        ));
        等落定(&mut job, &ctx, 1);
        assert!(job.texture().is_some());

        assert!(!job.ensure(
            None,
            None,
            1,
            LayerMask::all(),
            RenderOptions::default(),
            (1, 1)
        ));
        assert!(job.texture().is_none(), "没有图源必须清空纹理");
        assert!(job.key().is_none());
    }
}
