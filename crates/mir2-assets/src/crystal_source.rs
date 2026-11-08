//! [`CrystalLibSource`]：把 [`CrystalLibPaths`] 变成可按帧取像素的图源。
//!
//! # 一、职责
//!
//! 与 [`crate::WilSource`] 同构，只做三件事：**懒打开 + 小容量 LRU + 错误归集**。
//! 真正的二进制解析在 [`crate::CrystalLibrary`]（对齐 `_upstream/CrystalStudio/Assets.cs:14-204`），
//! 路径登记在 [`crate::CrystalLibPaths`]（对齐 `_upstream/CrystalStudio/Assets.cs:342-405`）。
//!
//! # 二、内存与缓存（path-backed：**不再整文件读入**）
//!
//! [`crate::CrystalLibrary::open`] 只解析**元数据区**：v2/v3 的文件头 + 偏移表
//! （`8 或 12 + 4×count` 字节），或 Sanxiu 的 `9 + headerLength` 字节头部；
//! 帧数据在取帧时按偏移 `seek` 现读、读完即丢（对齐上游 `File.OpenRead` +
//! 按帧 `Position`，`_upstream/CrystalStudio/Assets.cs:85-106`）。
//!
//! 所以这里的 LRU 缓存的是**库元数据**（偏移表 / 每图元数据），不是几百 MB 的库字节：
//!
//! - 真实素材 `Map/WemadeMir2/Tiles.Lib` 595 MB、`Objects.Lib` 128.9 MB ——
//!   整文件读入 + 4 槽 LRU 会吃掉 GB 级内存，按帧 seek 后常驻的只有几 MB 偏移表；
//! - 每次 [`SpriteSource::frame`] 的代价 = 一两次 `open`/`seek` + gzip 解压那一帧
//!   （打开系统调用相对 gzip 可忽略）；
//! - 「已打开的库」仍然是**懒**的：第一次取帧才解析元数据，解析失败进失败记录；
//! - 4 个槽位留 1 个余量给「刚切换过去的那张」。
//!
//! # 三、失败不静默
//!
//! [`SpriteSource::frame`] 的契约只允许 `Some` / `None`，所以「库打不开 / 帧头非法 /
//! gzip 损坏 / 内嵌图片解不开」这类**真错误**也传不上去。为了不埋掉它们，
//! 本模块把每次失败记进内部环形列表，由 [`CrystalLibSource::failures`] /
//! [`CrystalLibSource::drain_failures`] 取走 —— 调用方据此在状态栏显示「该库损坏」，
//! 而不是误判成「这一帧不存在」。
//!
//! # 四、锚点快路径
//!
//! [`SpriteSource::anchor`] 被覆盖为「只读帧头」（[`crate::CrystalLibrary::read_header`]），
//! 不解码像素：Sanxiu `fileType == 1` 的 GIF 尺寸回退也只读 10 字节。
//! 光罩（地图光照）每帧要问几百次，只需要 `(px, py)` 两个 `i16`。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::crystal_lib::CrystalLibrary;
use crate::crystal_paths::CrystalLibPaths;
use crate::error::AssetError;
use crate::frame::{Frame, SpriteSource};
use crate::lib_paths::LibRef;

/// 默认同时驻留的 `.lib` 库数。
///
/// 三族各一张 + 1 个余量（与 [`crate::DEFAULT_CACHE_CAPACITY`] 同值，但语义独立：
/// `.lib` 是单文件容器，但缓存的是**元数据**（偏移表 / 每图元数据），
/// 不是整库字节 —— 见模块文档 §二）。
pub const DEFAULT_LIB_CACHE_CAPACITY: usize = 4;

/// 失败记录最多保留多少条（超出丢最早的，避免长时间运行的编辑器无限涨）。
const MAX_FAILURES: usize = 64;

/// 缓存内部状态（受 `Mutex` 保护，见 [`CrystalLibSource::lock`] 的毒化处理）。
struct Inner {
    /// 定位键 →（已打开的库, 最近一次使用的 tick）。
    slots: HashMap<LibRef, (Arc<CrystalLibrary>, u64)>,
    /// 单调递增的 LRU 时钟。
    tick: u64,
    /// 最近的失败记录（环形，最多 [`MAX_FAILURES`] 条）。
    failures: Vec<String>,
}

/// 原版 `.lib` 图源：按 [`LibRef`]（族 + 族内索引）懒打开 `.lib` 并解码单帧。
///
/// ```no_run
/// use mir2_assets::{CrystalAssetFormat, CrystalLibPaths, CrystalLibSource, LibRef, SpriteSource};
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let paths = CrystalLibPaths::discover(Path::new(r"D:\Data"), CrystalAssetFormat::Auto)?;
/// let src = CrystalLibSource::open(&paths)?;
/// let tiles = LibRef::ground(0); // 地面 0 号 = Tiles.lib
/// if let Some(f) = src.frame(tiles, 0) {
///     println!("{}×{} {} 字节", f.width, f.height, f.rgba.len());
/// }
/// # Ok(())
/// # }
/// ```
pub struct CrystalLibSource {
    /// 登记表（只读，克隆一份自用）。
    paths: CrystalLibPaths,
    /// 同时驻留的库数上限。
    capacity: usize,
    /// 缓存与失败记录。
    inner: Mutex<Inner>,
}

impl CrystalLibSource {
    /// 用默认缓存容量打开。
    ///
    /// 登记表为空（`available() == 0`）时返回 [`AssetError::NoLibraries`]：
    /// 空表属于配置错误，不该等到第一次取帧时才发现。
    pub fn open(paths: &CrystalLibPaths) -> Result<Self, AssetError> {
        Self::with_cache_capacity(paths, DEFAULT_LIB_CACHE_CAPACITY)
    }

    /// 指定同时驻留的库数（内部按 `max(1)` 收敛，0 视为 1）。
    pub fn with_cache_capacity(
        paths: &CrystalLibPaths,
        capacity: usize,
    ) -> Result<Self, AssetError> {
        if paths.available() == 0 {
            return Err(AssetError::NoLibraries {
                root: paths.root().to_path_buf(),
            });
        }
        Ok(Self {
            paths: paths.clone(),
            capacity: capacity.max(1),
            inner: Mutex::new(Inner {
                slots: HashMap::new(),
                tick: 0,
                failures: Vec::new(),
            }),
        })
    }

    /// 底层登记表。
    pub fn paths(&self) -> &CrystalLibPaths {
        &self.paths
    }

    /// 同时驻留库数上限。
    pub fn cache_capacity(&self) -> usize {
        self.capacity
    }

    /// 当前驻留的库数。
    pub fn cached(&self) -> usize {
        self.lock().slots.len()
    }

    /// 清空缓存（丢掉已解析的库元数据；磁盘上的库文件不受影响）。
    pub fn clear_cache(&self) {
        self.lock().slots.clear();
    }

    /// 最近的失败记录（按发生顺序，最多 [`MAX_FAILURES`] 条）。
    pub fn failures(&self) -> Vec<String> {
        self.lock().failures.clone()
    }

    /// 取走失败记录并清空。
    pub fn drain_failures(&self) -> Vec<String> {
        std::mem::take(&mut self.lock().failures)
    }

    /// 取帧 —— 与 [`SpriteSource::frame`] 等价，省掉调用点引入 trait。
    pub fn frame_of(&self, lib: LibRef, index: u32) -> Option<Frame> {
        self.lookup_frame(lib, index)
    }

    /// 该库的帧总数；库未登记 / 打不开 → `None`。
    ///
    /// 帧数在打开时就从头部解析好了（[`CrystalLibrary::count`] = 偏移表长度），
    /// 所以这里只走「懒打开 + LRU」路径，不重新数、不解码任何一帧。
    /// 返回的是**索引表长度**（含空帧/无效帧），与「能出像素的帧数」不是一回事。
    pub fn frame_count(&self, lib: LibRef) -> Option<u32> {
        self.slot(lib).map(|slot| slot.count())
    }

    /// 帧查找的实际实现（三个 `None` 出口：库不可得 / 帧号越界 / 该帧无效）。
    fn lookup_frame(&self, lib: LibRef, index: u32) -> Option<Frame> {
        let slot = self.slot(lib)?;
        match slot.decode_checked(index) {
            Ok(frame) => frame,
            Err(e) => {
                self.note_failure(format!("{lib} 第 {index} 帧解码失败：{e}"));
                None
            }
        }
    }

    /// 只取锚点偏移：**只读帧头，不解码像素**。
    ///
    /// 与 [`CrystalLibSource::lookup_frame`] 走同一条「懒打开 + LRU」路径，
    /// 也做同样的两道 `None` 判定，差别只在跳过像素解码。
    fn anchor_of(&self, lib: LibRef, index: u32) -> Option<(i16, i16)> {
        let slot = self.slot(lib)?;
        match slot.read_header_checked(index) {
            Ok(header) => header.map(|(_, _, x, y)| (x, y)),
            Err(e) => {
                self.note_failure(format!("{lib} 第 {index} 帧读帧头失败：{e}"));
                None
            }
        }
    }

    /// 取（必要时打开）某个库。
    fn slot(&self, lib: LibRef) -> Option<Arc<CrystalLibrary>> {
        if let Some(slot) = self.touch(lib) {
            return Some(slot);
        }
        let path = self.paths.resolve(lib)?.to_path_buf();
        match CrystalLibrary::open(&path) {
            Ok(lib_file) => {
                let arc = Arc::new(lib_file);
                self.store(lib, Arc::clone(&arc));
                Some(arc)
            }
            Err(e) => {
                self.note_failure(format!("{lib} → {}：{e}", path.display()));
                None
            }
        }
    }

    /// 命中缓存时刷新 LRU 时间戳并返回。
    fn touch(&self, lib: LibRef) -> Option<Arc<CrystalLibrary>> {
        let mut inner = self.lock();
        inner.tick += 1;
        let tick = inner.tick;
        let entry = inner.slots.get_mut(&lib)?;
        entry.1 = tick;
        Some(Arc::clone(&entry.0))
    }

    /// 放入缓存并按需淘汰最久未用的库。
    fn store(&self, lib: LibRef, slot: Arc<CrystalLibrary>) {
        let mut inner = self.lock();
        inner.tick += 1;
        let tick = inner.tick;
        inner.slots.insert(lib, (slot, tick));
        while inner.slots.len() > self.capacity {
            let victim = inner
                .slots
                .iter()
                .min_by_key(|(_, (_, t))| *t)
                .map(|(k, _)| *k);
            match victim {
                Some(k) => {
                    inner.slots.remove(&k);
                }
                None => break,
            }
        }
    }

    /// 记录一条失败（环形，最多 [`MAX_FAILURES`] 条）。
    fn note_failure(&self, msg: String) {
        let mut inner = self.lock();
        if inner.failures.len() >= MAX_FAILURES {
            inner.failures.remove(0);
        }
        inner.failures.push(msg);
    }

    /// 加锁；互斥量若被毒化（持锁线程 panic 过）也继续用内部状态，**不 panic**。
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl SpriteSource for CrystalLibSource {
    /// 覆盖缺省实现：**只读帧头**（`CrystalLibrary::read_header_checked`），不解码像素。
    ///
    /// 五态（2026-11-07 R33 补齐 `OutOfRange`）：
    /// 库不可得 ⇒ `MissingLibrary`；**图号 ≥ 帧数 ⇒ `OutOfRange`**；`Ok(None)`（帧头宽高
    /// 非正 / 超 8192 等「帧本身无效」）⇒ `Empty`；`Err`（帧头截断 / 数据损坏）⇒
    /// `Unreadable`；`Ok(Some)` ⇒ `Ok`。
    ///
    /// ⚠ **为什么必须先判 `count()`**：`read_header_checked` 对越界图号返回的是
    /// `Ok(None)`（「帧本身无效」那一支），不先比帧数就会把**越界**报成上游的
    /// `Empty image`（`MapCheck.cs:31-32` 两者是不同文案）。
    fn frame_state(&self, lib: LibRef, index: u32) -> crate::ImageState {
        use crate::ImageState;
        let Some(slot) = self.slot(lib) else {
            // 「没登记」与「登记了但打不开」都让 `slot()` 返回 `None`，但上游是两条不同文案
            // （`Missing library` / `Unreadable image`，`MapCheck.cs:29-32`）⇒ 用登记表再判一次。
            // 与 `WilSource::frame_state` 同一条口径（R34 复核项 3）。
            return if self.paths.resolve(lib).is_some() {
                ImageState::Unreadable
            } else {
                ImageState::MissingLibrary
            };
        };
        if index >= slot.count() {
            return ImageState::OutOfRange;
        }
        match slot.read_header_checked(index) {
            Ok(Some(_)) => ImageState::Ok,
            Ok(None) => ImageState::Empty,
            Err(e) => {
                self.note_failure(format!("{lib} 第 {index} 帧读帧头失败：{e}"));
                ImageState::Unreadable
            }
        }
    }
    fn frame(&self, lib: LibRef, index: u32) -> Option<Frame> {
        self.lookup_frame(lib, index)
    }

    /// 覆盖默认实现：走「只读帧头」的 [`CrystalLibSource::anchor_of`]，不解码像素。
    fn anchor(&self, lib: LibRef, index: u32) -> Option<(i16, i16)> {
        self.anchor_of(lib, index)
    }
}

/// 手写 `Debug`：库元数据可能很大，只报路径/格式/登记数/容量/已驻留。
impl std::fmt::Debug for CrystalLibSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrystalLibSource")
            .field("root", &self.paths.root())
            .field("format", &self.paths.resolved_format())
            .field("available", &self.paths.available())
            .field("capacity", &self.capacity)
            .field("cached", &self.lock().slots.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crystal_paths::CrystalAssetFormat;

    /// 空登记表打开即报错（不等到第一次取帧才发现配置错）。
    #[test]
    fn 空登记表打开即报错() {
        let dir = std::env::temp_dir().join(format!("mir2_assets_lib_empty_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let paths = CrystalLibPaths::discover(&dir, CrystalAssetFormat::Auto).expect("空目录应返回空表");
        assert!(paths.is_empty());
        let err = CrystalLibSource::open(&paths).unwrap_err();
        assert!(matches!(err, AssetError::NoLibraries { .. }), "{err:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 缓存容量为零时收敛为一（与 [`crate::WilSource`] 同口径）。
    #[test]
    fn 缓存容量为零时收敛为一() {
        let dir =
            std::env::temp_dir().join(format!("mir2_assets_lib_cap_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let sub = dir.join("WemadeMir2");
        std::fs::create_dir_all(&sub).expect("建临时目录");
        std::fs::write(sub.join("Tiles.lib"), vec![0u8; 8]).expect("造空壳 .lib");

        let paths = CrystalLibPaths::discover(&dir, CrystalAssetFormat::WemadeMir2)
            .expect("扫描应成功");
        let src = CrystalLibSource::with_cache_capacity(&paths, 0).expect("应能打开");
        assert_eq!(src.cache_capacity(), 1);
        assert_eq!(src.cached(), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **五态单测**（2026-11-07 R33）：缺库 / 越界 / 空帧 / 读不出 / 正常各一条。
    ///
    /// 全部用**合成 `.lib`**（不依赖 `MIREDIT_LIB_ROOT`，任何机器上都能跑）：
    /// v2 容器 = `version(i32) + count(i32) + count×offset(i32)`，帧头 17 字节
    /// （`w,h,x,y` 各 i16 + 5 未用 + `length(i32)`；依据 `crystal_lib.rs` 的
    /// `LEGACY_HEADER_LEN` 与 `legacy_frame` 的读取顺序）。
    #[test]
    fn frame_state五态() {
        let dir = std::env::temp_dir().join(format!(
            "mir2_assets_lib_state_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let count: i32 = 3;
        let 帧0 = 8 + 4 * count as usize; // 头 8 + 偏移表
        let 帧1 = 帧0 + 17 + 16; // 帧0 头 + 2×2×4 像素
        let 帧2 = 帧1 + 17; // 帧1 头（空帧，无像素）
        let mut b = Vec::new();
        b.extend_from_slice(&2i32.to_le_bytes()); // version = 2
        b.extend_from_slice(&count.to_le_bytes());
        b.extend_from_slice(&(帧0 as i32).to_le_bytes());
        b.extend_from_slice(&(帧1 as i32).to_le_bytes());
        // 帧 2 的偏移**必须落在文件内**（`parse_legacy` 会拒绝整库否则），
        // 但只留 4 字节 ⇒ 17 字节帧头被截断 ⇒ `read_header_checked` 给 `Err`。
        b.extend_from_slice(&(帧2 as i32).to_le_bytes());
        // 帧 0：2×2、锚点 (0, 0)、length = 16
        for v in [2i16, 2, 0, 0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[0u8; 5]);
        b.extend_from_slice(&16i32.to_le_bytes());
        b.extend_from_slice(&[0x11u8; 16]);
        // 帧 1：空帧（w = h = 0）
        for v in [0i16, 0, 0, 0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[0u8; 5]);
        b.extend_from_slice(&0i32.to_le_bytes());
        b.extend_from_slice(&[0u8; 4]); // 帧 2：只剩 4 字节 ⇒ 帧头截断
        std::fs::write(dir.join("Objects.lib"), &b).expect("写合成 .lib");

        let paths =
            CrystalLibPaths::discover(&dir, CrystalAssetFormat::Flat).expect("扫描应成功");
        let src = CrystalLibSource::open(&paths).expect("应能打开");
        let objs = LibRef::surface(0);
        assert_eq!(src.frame_state(objs, 0), crate::ImageState::Ok, "有效帧");
        assert_eq!(src.frame_state(objs, 1), crate::ImageState::Empty, "w=h=0 ⇒ 空帧");
        assert_eq!(
            src.frame_state(objs, 2),
            crate::ImageState::Unreadable,
            "帧头被截断 ⇒ 读不出"
        );
        assert_eq!(
            src.frame_state(objs, 3),
            crate::ImageState::OutOfRange,
            "越界必须先判越界（否则会被 read_header_checked 的 Ok(None) 误判成空帧）"
        );
        assert_eq!(
            src.frame_state(LibRef::surface(7), 0),
            crate::ImageState::MissingLibrary,
            "没登记的库"
        );
        assert!(
            !src.failures().is_empty(),
            "帧头截断必须留痕（不许静默当成空帧）"
        );

        // **登记了但打不开**（头部非法）⇒ `Unreadable`，不许冒充「缺库」（R34 复核项 3）
        std::fs::write(dir.join("Tiles.lib"), vec![0u8; 4]).expect("写坏库");
        let paths =
            CrystalLibPaths::discover(&dir, CrystalAssetFormat::Flat).expect("扫描应成功");
        let src = CrystalLibSource::open(&paths).expect("应能打开");
        assert_eq!(
            src.frame_state(LibRef::ground(0), 0),
            crate::ImageState::Unreadable,
            "登记了但 .lib 头非法 ⇒ 读不出（与 WilSource 同口径）"
        );
        assert_eq!(
            src.frame_state(LibRef::middle(5), 0),
            crate::ImageState::MissingLibrary,
            "压根没登记 ⇒ 缺库"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
