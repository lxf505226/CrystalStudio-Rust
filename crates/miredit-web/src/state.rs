//! 进程内共享状态：配置、会话、图库、印章目录、WS 广播通道。
//!
//! # 一、并发模型（任务书 §四：`Arc<RwLock<…>>` + 写命令串行）
//!
//! ```text
//! Arc<AppState>
//!   ├── session: Arc<RwLock<Option<EditSession>>>   ← 唯一的可变状态
//!   ├── libs / source                                ← 只读（内部自带 Mutex 的帧缓存）
//!   ├── stamps:  Mutex<Option<Result<StampCatalog>>> ← 懒加载 + 解析缓存
//!   └── events:  broadcast::Sender<Event>            ← WS 推送（单向下发）
//! ```
//!
//! **为什么会话用 `RwLock` 而不是 `Mutex`**：出图（`/api/tiles`）只需要 `&Map`，
//! 多个读者可以同时渲染；而**编辑**（`/api/edit`）必须拿写锁 ⇒ 天然满足
//! 「同一时刻只允许一个写命令」。写命令内部还可能改撤销栈、脏标记，所以它不能是读锁。
//!
//! **锁的持有时间**：写锁只覆盖 `apply()` 那一次调用（不含 JSON 解析、不含网络 IO）；
//! 读锁覆盖一次渲染。**任何 `await` 都不在持锁期间发生**（渲染是同步代码，
//! 用 [`tokio::task::block_in_place`] 让出执行权，见 `routes/tiles.rs`）。
//!
//! # 二、配置全部来自环境变量（Docker 里只改 env，不改代码）
//!
//! | 变量 | 缺省 | 说明 |
//! |---|---|---|
//! | `MIREDIT_MAP_ROOT` | `GodotClient/maps` | `/api/map/open` 的**白名单根**（相对路径按进程 cwd 绝对化） |
//! | `MIREDIT_CLIENT_ROOT` | `mir2_assets::DEFAULT_CLIENT_ROOT` | 图库扫描根（`.wil/.wzl`） |
//! | `MIREDIT_PALETTE_JSON` | `<仓库根>/CrystalStudio/catalogue/palette.json` | 印章目录索引 |
//! | `MIREDIT_WEB_ROOT` | `<仓库根>/tools/miredit/web` | 静态前端目录（找不到就 404，**不伪造页面**） |
//! | `MIREDIT_BIND` | `0.0.0.0:8080` | 监听地址 |
//!
//! # 三、路径白名单（防目录穿越）
//!
//! [`AppState::resolve_map_path`] 是**唯一**允许把外部字符串变成磁盘路径的地方，四道闸：
//! ① 非空；② **任何 `..` 分量一律拒绝**（不区分 `\` 与 `/`）；③ 绝对化后必须
//! `starts_with(map_root_canon)`（`map_root_canon` 在启动时 canonicalize 过一次）；
//! ④ 必须是普通文件。任一条不过 ⇒ 4xx（400 / 403 / 404），**绝不 200**。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

use mir2_assets::{LibPaths, WilSource};
use mir2_stamp::StampCatalog;
use miredit_core::EditSession;
use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::error::{ApiError, ApiResult};

/// 缺省地图根（相对进程 cwd；任务书 §四指定）。
pub const DEFAULT_MAP_ROOT: &str = "GodotClient/maps";

/// 缺省监听地址（Docker 里要 0.0.0.0 才能被 `-p` 映射到）。
pub const DEFAULT_BIND: &str = "0.0.0.0:8080";

/// WS 广播通道容量（够 1 个编辑器客户端追上几十条编辑事件；追不上会收到 Lagged 并被跳过）。
const EVENT_CHANNEL_CAPACITY: usize = 64;

/// 一条服务端推送（`/api/events` 的报文）。
#[derive(Debug, Clone)]
pub enum Event {
    /// 脏标记变化。
    Dirty {
        /// 当前是否「有未保存的修改」。
        dirty: bool,
    },
    /// 地图内容变了 ⇒ 前端清瓦片缓存重绘。
    Rerender {
        /// 左上角世界格 X。
        x: u16,
        /// 左上角世界格 Y。
        y: u16,
        /// 宽（格）。`0` = 全屏（前端目前整屏重取）。
        w: u16,
        /// 高（格）。`0` = 全屏。
        h: u16,
    },
    /// 服务端侧的错误（保存失败等），前端弹轻提示。
    Error {
        /// 机器码（与 HTTP 错误体同一套）。
        code: String,
        /// 中文说明。
        message: String,
    },
}

impl Event {
    /// 装成 `{"type": …, "payload": {…}}`（前端 `web/README.md` §3.9 的报文格式）。
    pub fn to_json(&self) -> Value {
        match self {
            Event::Dirty { dirty } => json!({"type": "dirty", "payload": {"dirty": dirty}}),
            Event::Rerender { x, y, w, h } => json!({
                "type": "rerender",
                "payload": {"x": x, "y": y, "w": w, "h": h}
            }),
            Event::Error { code, message } => json!({
                "type": "error",
                "payload": {"code": code, "message": message}
            }),
        }
    }
}

/// 进程内共享状态（**只构造一次**，之后处处 `Arc` 克隆）。
pub struct AppState {
    /// 地图白名单根（启动时 canonicalize 过的绝对路径）。
    pub map_root: PathBuf,
    /// 静态前端目录（`None` = 目录不存在 ⇒ 所有静态请求 404，不伪造页面）。
    pub web_root: Option<PathBuf>,
    /// 编辑会话（`None` = 还没打开地图）。**唯一的可变状态**，见模块文档第一节。
    pub session: Arc<RwLock<Option<EditSession>>>,
    /// 图库登记表（启动时扫一次；扫不到就是 `None`，相关端点如实 503）。
    pub libs: Option<LibPaths>,
    /// 图源（帧解码 + LRU 缓存；`None` = 图库不可用）。
    pub source: Option<WilSource>,
    /// 印章目录（懒加载；`Some(Err)` = 加载失败过，错误文本原样保留）。
    pub stamps: Mutex<Option<Result<StampCatalog, String>>>,
    /// `tolerant_trailer_bytes` 的缓存：`(路径, 值)`。地图不变时只需读一次盘（见 `routes/map.rs`）。
    pub trailer_cache: Mutex<Option<(String, Option<usize>)>>,
    /// `palette.json` 的路径（懒加载时用；也是 503 文案里的定位信息）。
    pub palette_json: PathBuf,
    /// WS 推送通道。
    pub events: broadcast::Sender<Event>,
}

/// 共享句柄（handler 里 `State<Shared>` 拿到的就是它）。
pub type Shared = Arc<AppState>;

impl AppState {
    /// 按模块文档第二节的 env 表构造状态。
    ///
    /// **不 panic**：图库扫不到、印章目录不存在、静态目录缺失都只是 `None` / 空，
    /// 由各端点如实报 503 / 404 —— 启动失败与"功能不可用"是两件事，不混在一起。
    pub fn from_env() -> AppState {
        // ① 地图白名单根：相对路径按 cwd 绝对化（这样 `GodotClient/maps` 在仓库根启动就成立）
        let map_root_raw = env_string("MIREDIT_MAP_ROOT").unwrap_or_else(|| DEFAULT_MAP_ROOT.into());
        let map_root = absolutize(Path::new(&map_root_raw));
        let map_root = std::fs::canonicalize(&map_root).unwrap_or(map_root);

        // ② 静态前端目录
        let web_raw = env_string("MIREDIT_WEB_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| repo_relative(&["tools", "miredit", "web"]));
        let web_root = std::fs::canonicalize(&web_raw)
            .ok()
            .filter(|p| p.is_dir());

        // ③ 图库：扫不到就是 None（不造空表冒充成功）
        let client_root = env_string("MIREDIT_CLIENT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(mir2_assets::DEFAULT_CLIENT_ROOT));
        let libs = match LibPaths::discover(&client_root) {
            Ok(p) if !p.is_empty() => {
                println!(
                    "[图库] {} 已登记 {} 个（地面 {} / 中层 {} / 表层 {}）",
                    client_root.display(),
                    p.available(),
                    p.available_in(mir2_assets::Family::Ground),
                    p.available_in(mir2_assets::Family::Middle),
                    p.available_in(mir2_assets::Family::Surface)
                );
                Some(p)
            }
            Ok(p) => {
                eprintln!("[图库] {} 下一个库都没登记（{} 被跳过）⇒ 出图端点将返回 503",
                    client_root.display(), p.skipped().len());
                None
            }
            Err(e) => {
                eprintln!("[图库] 扫描失败：{e} ⇒ 出图端点将返回 503");
                None
            }
        };
        let source = match &libs {
            Some(p) => match WilSource::open(p) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!("[图库] 图源打开失败：{e}");
                    None
                }
            },
            None => None,
        };

        // ④ 印章目录索引（只记路径，首次 /api/stamps 时才读，避免拖慢启动）
        let palette_json = env_string("MIREDIT_PALETTE_JSON")
            .map(PathBuf::from)
            .unwrap_or_else(|| repo_relative(&["CrystalStudio", "catalogue", "palette.json"]));

        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        AppState {
            map_root,
            web_root,
            session: Arc::new(RwLock::new(None)),
            libs,
            source,
            stamps: Mutex::new(None),
            trailer_cache: Mutex::new(None),
            palette_json,
            events,
        }
    }

    // ───────────────────────── 会话锁 ─────────────────────────

    /// 取会话读锁（**渲染 / 元数据**用；可并发）。
    ///
    /// 锁中毒时取回内部值继续用：一个 handler panic 不该让整个服务从此读不到地图。
    pub fn read_session(&self) -> RwLockReadGuard<'_, Option<EditSession>> {
        self.session.read().unwrap_or_else(|e| e.into_inner())
    }

    /// 取会话写锁（**编辑 / 打开 / 保存**用；同一时刻只有一个写者）。
    pub fn write_session(&self) -> RwLockWriteGuard<'_, Option<EditSession>> {
        self.session.write().unwrap_or_else(|e| e.into_inner())
    }

    // ───────────────────────── 印章目录（懒加载）─────────────────────────

    /// 拿印章目录，交给闭包用（首次调用时才真的读 `palette.json`）。
    ///
    /// 加载失败 ⇒ 503 `assets_unavailable`，**错误文本原样带出**（不猜"大概是什么问题"）；
    /// 失败结果会被缓存，避免每个请求都重读 5 MB JSON。
    pub fn with_stamps<T>(&self, f: impl FnOnce(&mut StampCatalog) -> ApiResult<T>) -> ApiResult<T> {
        let mut guard = self.lock_stamps();
        if guard.is_none() {
            println!("[印章] 首次加载目录索引：{}", self.palette_json.display());
            let loaded = StampCatalog::load(&self.palette_json).map_err(|e| e.to_string());
            if let Ok(c) = &loaded {
                println!("[印章] 已索引 {} 条", c.len());
            } else if let Err(e) = &loaded {
                eprintln!("[印章] 加载失败：{e}");
            }
            *guard = Some(loaded);
        }
        match guard.as_mut() {
            Some(Ok(catalog)) => f(catalog),
            Some(Err(e)) => Err(ApiError::assets_unavailable(format!(
                "印章目录不可用：{e}"
            ))),
            // 上一行已保证 is_none 分支被填上，这里只是防御
            None => Err(ApiError::assets_unavailable("印章目录不可用")),
        }
    }

    /// 取印章目录锁（中毒时取回内部值）。
    fn lock_stamps(&self) -> MutexGuard<'_, Option<Result<StampCatalog, String>>> {
        self.stamps.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ───────────────────────── 路径白名单 ─────────────────────────

    /// 把请求里的 `path` 解析成**根目录内**的真实文件路径（四道闸见模块文档第三节）。
    ///
    /// 相对路径按 `map_root` 拼接；绝对路径也必须在 `map_root` 之内。
    pub fn resolve_map_path(&self, requested: &str) -> ApiResult<PathBuf> {
        let raw = requested.trim();
        if raw.is_empty() {
            return Err(ApiError::bad_request("path 不能为空"));
        }
        // 闸②：任何 `..` 分量一律拒绝（`\` 与 `/` 都算分隔符，Windows/Linux 一视同仁）
        if has_parent_component(raw) {
            return Err(ApiError::forbidden_path(format!(
                "路径不允许包含 `..`：{raw}"
            )));
        }
        let candidate = {
            let p = Path::new(raw);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.map_root.join(p)
            }
        };
        // 闸③：绝对化 + 必须在白名单根内（canonicalize 同时消掉符号链接绕行）
        let real = match std::fs::canonicalize(&candidate) {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ApiError::not_found(format!(
                    "文件不存在：{}",
                    display_path(&candidate)
                )))
            }
            Err(e) => {
                return Err(ApiError::io_error(format!(
                    "路径无法解析：{}（{e}）",
                    display_path(&candidate)
                )))
            }
        };
        if !real.starts_with(&self.map_root) {
            return Err(ApiError::forbidden_path(format!(
                "路径不在白名单根内：{}（根 = {}）",
                display_path(&real),
                display_path(&self.map_root)
            )));
        }
        // 闸④：必须是普通文件
        if !real.is_file() {
            return Err(ApiError::not_found(format!(
                "不是文件：{}",
                display_path(&real)
            )));
        }
        Ok(real)
    }

    /// 广播一条推送（没有订阅者时静默丢弃 —— WS 是可选功能，不能影响编辑）。
    pub fn emit(&self, ev: Event) {
        let _ = self.events.send(ev);
    }

    /// 图库不可用时统一报 503（出图 / 资源面板共用一句文案）。
    pub fn need_source(&self) -> ApiResult<&WilSource> {
        self.source
            .as_ref()
            .ok_or_else(|| ApiError::assets_unavailable(
                "图库未就绪：未扫描到任何 Tiles/SmTiles/Objects 库（设置 MIREDIT_CLIENT_ROOT）",
            ))
    }
}

// ───────────────────────── 小工具 ─────────────────────────

/// 读环境变量（空串按未设置处理 —— 容器里 `ENV X=` 很常见，不该被当成合法值）。
fn env_string(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => Some(v),
        _ => None,
    }
}

/// 相对路径按**进程 cwd** 绝对化（`std::path::absolute` 不要求路径存在）。
fn absolutize(p: &Path) -> PathBuf {
    if p.is_absolute() {
        return p.to_path_buf();
    }
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// 仓库根相对路径：`<crate 清单目录>/../../../..` = `E:\GodM2`（本 crate 在 `tools/miredit/crates/` 下）。
///
/// 编译期就把清单目录写进二进制，所以**换 cwd 也能找到**（Windows 本地跑的关键）；
/// Docker 里没有这些目录，会退化成 `None` / 503，由 env 覆盖。
fn repo_relative(parts: &[&str]) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..4 {
        p.push("..");
    }
    for part in parts {
        p.push(part);
    }
    p
}

/// 路径里是否有 `..` 分量（同时按 `\` 与 `/` 切，且忽略大小写无关——`..` 本身没有大小写）。
fn has_parent_component(raw: &str) -> bool {
    raw.split(['\\', '/']).any(|seg| seg == "..")
}

/// 给人看的路径：去掉 Windows canonicalize 加上的 `\\?\` 前缀。
pub fn display_path(p: &Path) -> String {
    let s = p.display().to_string();
    for prefix in ["\\\\?\\UNC\\", "\\\\?\\", "\\??\\"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            // UNC 路径要把前缀还原成 `\\`
            if prefix == "\\\\?\\UNC\\" {
                return format!("\\\\{rest}");
            }
            return rest.to_string();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 穿越分量识别() {
        assert!(has_parent_component("../etc/passwd"));
        assert!(has_parent_component("..\\..\\windows\\win.ini"));
        assert!(has_parent_component("a/../../b"));
        assert!(!has_parent_component("0.map"));
        assert!(!has_parent_component("GodotClient/maps/0.map"));
        // 只是名字里带两个点，不是分量
        assert!(!has_parent_component("a..b.map"));
    }

    #[test]
    fn 去掉了windows前缀() {
        assert_eq!(display_path(Path::new(r"\\?\E:\x\0.map")), r"E:\x\0.map");
        assert_eq!(display_path(Path::new(r"E:\x\0.map")), r"E:\x\0.map");
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\srv\share\0.map")),
            r"\\srv\share\0.map"
        );
    }

    #[test]
    fn 事件报文形状() {
        let v = Event::Dirty { dirty: true }.to_json();
        assert_eq!(v["type"], "dirty");
        assert_eq!(v["payload"]["dirty"], true);
        let v = Event::Rerender {
            x: 1,
            y: 2,
            w: 16,
            h: 16,
        }
        .to_json();
        assert_eq!(v["type"], "rerender");
        assert_eq!(v["payload"]["w"], 16);
        let v = Event::Error {
            code: "io_error".into(),
            message: "磁盘只读".into(),
        }
        .to_json();
        assert_eq!(v["payload"]["code"], "io_error");
    }
}
