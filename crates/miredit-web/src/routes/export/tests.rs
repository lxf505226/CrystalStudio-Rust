//! `routes::export` 的测试（拆出来是因为「主体 + 测试」会超 600 行）。
//!
//! 与 `routes/tiles/tests.rs` 同一套办法：`export.rs` 末尾只留 `#[cfg(test)] mod tests;`，
//! 测试用 `use super::*` 拿到全部私有项（`parse_size` / `target_size` / `render_minimap` /
//! `数值头` / `content_disposition`），**断言的就是 handler 真调的那些函数**。
//!
//! 三条纪律：
//! 1. 合成地图就地造（`52 + 12*w*h` 的 legacy 9 格式），**不依赖本机真实素材**；
//! 2. 需要真图库的端到端用例按「有素材真断言、无素材 `[跳过]` 返回」门控；
//! 3. 临时文件只落 `std::env::temp_dir()`（本机已重定向到 `I:\cargo\tmp`），**一个都不写进仓库**。

use super::*;
use axum::extract::Query;
use miredit_render::MapLighting;
use mir2_assets::{Frame, LibRef, SpriteSource};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use crate::state::AppState;

/// PNG 文件头（8 字节魔数）——「出的是不是 PNG」的唯一硬判据。
const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// 把 `(&str, &str)` 列表包成 handler 用的查询参数（与 `tiles/tests.rs` 同款）。
fn mk(pairs: &[(&str, &str)]) -> Params {
    let m: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    Params::from_query(Ok(Query(m))).expect("查询参数应可解析")
}

/// 合成一张 `cw × ch` 格的 legacy 9 格式地图（`52 + 12*w*h` 字节，全 0 格）。
///
/// 全 0 格 = 三层图号都为 0 ⇒ 渲染结果是一张**全透明**的图，
/// 但尺寸与档位口径完全按真实地图走（本文件的断言全是尺寸/头/错误码，不看像素内容）。
fn 合成图(cw: u16, ch: u16) -> Map {
    let mut bytes = vec![0u8; 52 + 12 * cw as usize * ch as usize];
    bytes[0..2].copy_from_slice(&cw.to_le_bytes());
    bytes[2..4].copy_from_slice(&ch.to_le_bytes());
    Map::parse(&bytes).expect("合成 legacy 9 格式地图应可解析")
}

/// 合成一张 `w×h` 格的地图，并在 `(x,y)` 设一个**前帧图号 1**（用于验扩边/渲染）。
///
/// 与 [`合成图`] 的区别只有这一处：全 0 的图「没有帧可画」⇒ 测不出外溢与拼图内容。
fn 合成图带前帧(w: u16, h: u16, x: u16, y: u16) -> Map {
    let mut bytes = vec![0u8; 52 + 12 * w as usize * h as usize];
    bytes[0..2].copy_from_slice(&w.to_le_bytes());
    bytes[2..4].copy_from_slice(&h.to_le_bytes());
    // 单格 12 字节：bk(0..2) mid(2..4) fr(4..6) …
    let base = 52 + (y as usize * w as usize + x as usize) * 12 + 4;
    bytes[base..base + 2].copy_from_slice(&1u16.to_le_bytes());
    Map::parse(&bytes).expect("合成 legacy 9 格式地图应可解析")
}

/// 什么都不画的图源：缺帧是数据事实（不画），不是错误 ⇒ 尺寸仍严格等于目标。
struct 空源;

impl SpriteSource for 空源 {
    fn frame(&self, _lib: LibRef, _index: u32) -> Option<Frame> {
        None
    }
}

/// 每个测试一个独立临时目录里的合成地图文件（pid + 自增序号，互不撞名）。
fn 临时地图(cw: u16, ch: u16, 标签: &str) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!(
        "miredit-web-export-{}-{n}-{标签}.map",
        std::process::id()
    ));
    let mut bytes = vec![0u8; 52 + 12 * cw as usize * ch as usize];
    bytes[0..2].copy_from_slice(&cw.to_le_bytes());
    bytes[2..4].copy_from_slice(&ch.to_le_bytes());
    std::fs::write(&p, bytes).expect("写临时地图");
    p
}

/// 不读 env 的空状态（没有地图、没有图库、没有地图白名单根）。
fn 空状态() -> Shared {
    let (events, _) = tokio::sync::broadcast::channel(4);
    crate::server::share(AppState {
        map_root: PathBuf::new(),
        web_root: None,
        session: Arc::new(RwLock::new(None)),
        libs: None,
        source: None,
        // A-024：测试夹具只装 WIL ⇒ Default 号段
        号段: miredit_core::库号号段::Wemade,
        stamps: Mutex::new(None),
        trailer_cache: Mutex::new(None),
        palette_json: PathBuf::new(),
        events,
    })
}

/// 起一个真 TCP 服务（端口 0 由内核分配，不与其它测试抢）。
async fn 起服务(st: Shared) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("回环端口应可绑定");
    let addr = listener.local_addr().expect("应能取到本地地址");
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, crate::server::app(st)).await;
    });
    (addr, server)
}

/// 发一条 HTTP/1.1 GET（`Connection: close`），返回 `(状态行, 头文本小写, 响应体字节)`。
///
/// 头体按 `\r\n\r\n` 切开 —— PNG 是二进制，**不能**当字符串读（UTF-8 有损替换会让
/// 「PNG 魔数」断言失真）。
fn http_get(addr: std::net::SocketAddr, path: &str) -> (String, String, Vec<u8>) {
    http_get_timeout(addr, path, 60)
}

/// 同 [`http_get`]，但读超时可调（700×700 全图在 debug 下要几分钟）。
fn http_get_timeout(
    addr: std::net::SocketAddr,
    path: &str,
    secs: u64,
) -> (String, String, Vec<u8>) {
    use std::io::{Read as _, Write as _};
    let mut s = std::net::TcpStream::connect(addr).expect("应能连上测试服务");
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(secs)));
    let req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).expect("应能发出请求");
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).expect("应能读到完整响应");
    let pos = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("响应里没有头体分隔：{path}"));
    let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
    let status = head.lines().next().unwrap_or_default().to_string();
    (status, head, buf[pos + 4..].to_vec())
}

/// 出图 + 解回尺寸：断言 PNG 魔数与**实际宽高**（不是「差不多」）。
fn 出图并验(图: &Map, w: u32, h: u32) -> Vec<u8> {
    let (ow, oh, png) =
        render_minimap(图, &空源, (w, h), RenderOptions::closed()).expect("出图应成功");
    assert_eq!((ow, oh), (w, h), "回执宽高必须等于目标宽高");
    assert_eq!(&png[..8], &PNG_MAGIC, "必须是 PNG 魔数");
    let img = miredit_render::png_io::read_png_from_memory(&png).expect("PNG 应可解码");
    assert_eq!((img.width, img.height), (w, h), "PNG 实际尺寸");
    png
}

/* ===================== 一、尺寸口径（缺省 / 三档 / 自定义） ===================== */

/// 缺省必须是 `small`，且 `W*12 × H*8` —— **与 GUI 的 `MinimapSize::Default` 逐像素相同**。
///
/// 3×2 格 ⇒ `36×16`（不是 24×16 那种对称值，写反了立刻能看出来）。
#[test]
fn 缺省是small且尺寸为十二乘八() {
    let p = mk(&[]);
    let size = parse_size(&p).expect("缺省应合法");
    assert_eq!(
        size,
        MinimapSize::Preset(MinimapPreset::Small),
        "缺省预设必须是 small"
    );
    // 与官方缺省的等价性：两条口径对同一张图必须算出同一个尺寸
    assert_eq!(
        size.resolve(3, 2).expect("可算"),
        MinimapSize::Default.resolve(3, 2).expect("可算"),
        "Preset(Small) 与 Default 必须逐像素相同（W*48/4 == W*12）"
    );

    let map = 合成图(3, 2);
    let (w, h) = target_size(size, 3, 2).expect("尺寸合法");
    assert_eq!((w, h), (3 * 12, 2 * 8), "缺省尺寸必须是 W*12 × H*8");
    let (ow, oh, png) =
        render_minimap(&map, &空源, (w, h), RenderOptions::closed()).expect("出图");
    assert_eq!(&png[..8], &PNG_MAGIC);
    let img = miredit_render::png_io::read_png_from_memory(&png).expect("解码");
    assert_eq!((img.width, img.height), (36, 16));

    // 响应头取自**产物**（`render_minimap` 的返回值），与 PNG 实际尺寸同源
    assert_eq!(
        数值头("x-export-width", ow).expect("头").1.to_str().expect("ASCII"),
        "36"
    );
    assert_eq!(
        数值头("x-export-height", oh).expect("头").1.to_str().expect("ASCII"),
        "16"
    );
}

/// 三档预设：除数 `{4,2,1}` 逐档落在 `W*48/d × H*32/d`，PNG 与宽高头都跟着走。
#[test]
fn 三档预设的png与宽高头() {
    let map = 合成图(3, 2);
    for (key, want) in [
        ("small", (36u32, 16u32)),
        ("medium", (72, 32)),
        ("large", (144, 64)),
    ] {
        let p = mk(&[("preset", key)]);
        let size = parse_size(&p).expect(key);
        assert_eq!(
            size,
            MinimapSize::Preset(MinimapPreset::parse(key).expect(key)),
            "{key}"
        );
        let (w, h) = target_size(size, 3, 2).expect("尺寸合法");
        assert_eq!((w, h), want, "{key} 的尺寸必须是 W*48/d × H*32/d");
        let (ow, oh, png) =
            render_minimap(&map, &空源, (w, h), RenderOptions::closed()).expect("出图");
        assert_eq!((ow, oh), want, "{key} 的产物宽高");
        assert_eq!(&png[..8], &PNG_MAGIC, "{key}");
        assert_eq!(
            数值头("x-export-width", ow)
                .expect("头")
                .1
                .to_str()
                .expect("ASCII")
                .to_string(),
            want.0.to_string(),
            "{key} 的 X-Export-Width"
        );
        assert_eq!(
            数值头("x-export-height", oh)
                .expect("头")
                .1
                .to_str()
                .expect("ASCII")
                .to_string(),
            want.1.to_string(),
            "{key} 的 X-Export-Height"
        );
    }
}

/// 自定义宽高：**原样落到目标像素**（缩、放两个方向都要精确命中，不留 24×16 那种中间尺寸）。
#[test]
fn 自定义宽高原样落位() {
    let map = 合成图(3, 2);
    // 缩小：Quarter 原生 36×16 ⇒ 面积平均缩到 10×7
    let p = mk(&[("width", "10"), ("height", "7")]);
    let size = parse_size(&p).expect("合法");
    assert_eq!(size, MinimapSize::Explicit { width: 10, height: 7 });
    let (w, h) = target_size(size, 3, 2).expect("合法");
    assert_eq!((w, h), (10, 7));
    出图并验(&map, w, h);

    // 放大：Quarter 原生 36×16 比目标小 ⇒ 仍要精确落到 40×30（官方 redraw 语义）
    let p = mk(&[("width", "40"), ("height", "30")]);
    let (w, h) = target_size(parse_size(&p).expect("合法"), 3, 2).expect("合法");
    let (ow, oh, png) =
        render_minimap(&map, &空源, (w, h), RenderOptions::closed()).expect("出图");
    assert_eq!((ow, oh), (40, 30), "目标比原生图大时也必须精确落到目标宽高");
    assert_eq!(&png[..8], &PNG_MAGIC);
    assert_eq!(
        数值头("x-export-width", ow).expect("头").1.to_str().expect("ASCII"),
        "40"
    );
}

/* ===================== 二、参数错误（全部 400 + 中文原因） ===================== */

/// `preset` 与 `width/height` **互斥**：同时给 ⇒ 400，不静默只认一个。
#[test]
fn preset与宽高同时给必400() {
    for pairs in [
        vec![("preset", "medium"), ("width", "100"), ("height", "100")],
        vec![("preset", "small"), ("width", "100")],
        vec![("preset", "large"), ("height", "100")],
    ] {
        let e = parse_size(&mk(&pairs)).expect_err("互斥写法必须拒绝");
        assert_eq!(e.status(), axum::http::StatusCode::BAD_REQUEST, "{pairs:?}");
        assert_eq!(e.code(), "bad_request", "{pairs:?}");
        assert!(e.message().contains("互斥"), "{pairs:?}：{}", e.message());
    }
}

/// `width` 缺失 / 只给一半 / 非整数 / `0` / 超单边 / 超字节 ⇒ 一律 400 + 中文原因。
#[test]
fn 宽高缺失或越界一律400() {
    // 只给一个（缺另一个）⇒ 400
    for pairs in [vec![("width", "100")], vec![("height", "100")]] {
        let e = parse_size(&mk(&pairs)).expect_err("只给一半必须拒绝");
        assert_eq!(e.status(), axum::http::StatusCode::BAD_REQUEST, "{pairs:?}");
        assert!(e.message().contains("必须同时给"), "{}", e.message());
    }
    // 非整数 / 负数 ⇒ 400（不猜、不四舍五入）
    for pairs in [
        vec![("width", "abc"), ("height", "10")],
        vec![("width", "-1"), ("height", "10")],
        vec![("width", "10.5"), ("height", "10")],
    ] {
        let e = parse_size(&mk(&pairs)).expect_err("非法整数必须拒绝");
        assert!(e.message().contains("非负整数"), "{pairs:?}：{}", e.message());
    }
    // `0` ⇒ 解析通过，但尺寸校验那一步拒（官方 ValidSize 的第一条）
    let size = parse_size(&mk(&[("width", "0"), ("height", "10")])).expect("0 收进来");
    let e = target_size(size, 3, 2).expect_err("宽 0 必须拒绝");
    assert_eq!(e.status(), axum::http::StatusCode::BAD_REQUEST);
    assert!(e.message().contains("正整数"), "{}", e.message());
    // 超单边上限（32768）
    let size = parse_size(&mk(&[("width", "40000"), ("height", "10")])).expect("可解析");
    let e = target_size(size, 3, 2).expect_err("超单边必须拒绝");
    assert!(e.message().contains("单边上限"), "{}", e.message());
    // 两边都在单边内，但 RGBA 字节超 512 MiB
    let size = parse_size(&mk(&[("width", "20000"), ("height", "20000")])).expect("可解析");
    let e = target_size(size, 3, 2).expect_err("超字节必须拒绝");
    assert!(e.message().contains("MB"), "{}", e.message());
    // 未知 / 空 preset
    for raw in ["huge", ""] {
        let e = parse_size(&mk(&[("preset", raw)])).expect_err("非法 preset 必须拒绝");
        assert_eq!(e.status(), axum::http::StatusCode::BAD_REQUEST, "{raw:?}");
        assert!(
            e.message().contains("preset 只支持") && e.message().contains("small"),
            "{raw:?}：{}",
            e.message()
        );
    }
}

/// 大图 + 超大预设：按 `MAX_EXPORT_SIDE` / `MAX_EXPORT_BYTES` **拒绝**，而**不是**静默降档。
///
/// 700×700 格（真实 `0.map` 的量级）：
/// - `small` = `8400×5600×4 = 188 MB` ✅ 通过尺寸校验；
/// - `medium` = `16800×11200×4 = 752 MB` ❌ 超 512 MiB；
/// - `large` = `33600×22400` ❌ 单边 33600 > 32768。
///
/// 两条都是 **400 + 中文原因**，绝不允许「那给你出 medium 吧」这种悄悄降档。
#[test]
fn 超大预设拒绝不降档() {
    assert!(target_size(MinimapSize::Preset(MinimapPreset::Small), 700, 700).is_ok());
    let e = target_size(MinimapSize::Preset(MinimapPreset::Medium), 700, 700)
        .expect_err("medium 超字节必须拒绝");
    assert_eq!(e.status(), axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(e.code(), "bad_request");
    assert!(e.message().contains("MB"), "{}", e.message());
    let e = target_size(MinimapSize::Preset(MinimapPreset::Large), 700, 700)
        .expect_err("large 超单边必须拒绝");
    assert!(e.message().contains("单边上限"), "{}", e.message());
}

/// 尺寸合法但**单次渲染装不下**（1× 中间图 184 MB > 128 MiB）⇒ 分块渲染照常出图。
///
/// `200×150` 格 `@Quarter`：整幅一次渲要先合成 `9600×4800×4 = 184,320,000 B`
/// （175.8 MiB）的 1× 中间图 ⇒ 被 [`miredit_render::INTERMEDIATE_MAX_BYTES`] 拒；
/// 分块（64 格一块）后每块中间图 `3072×2048×4 = 24 MiB` ⇒ 通过。
///
/// 这条钉住的是「网页导出不再是整幅一次渲」这件事：换成整幅单次，本用例当场 413。
#[test]
fn 分块渲染跨过单次上限仍成功() {
    // 先自证「整幅一次渲确实装不下」（判据不靠注释）
    let vp = miredit_render::Viewport::new(0, 0, 200, 150);
    let 整幅 = miredit_render::try_render_region_scaled(
        &合成图(200, 150),
        &空源,
        &vp,
        &LayerMask::all(),
        CollisionOverlay::none(),
        RenderScale::Quarter,
    );
    assert!(
        整幅.is_err(),
        "前提：200×150 @Quarter 的整幅单次渲染必须被拒（否则本用例失去意义）"
    );

    // 分块渲染：同样的窗口与档位，必须成功且尺寸精确
    let map = 合成图(200, 150);
    let (w, h) = target_size(MinimapSize::Preset(MinimapPreset::Small), 200, 150).expect("尺寸合法");
    assert_eq!((w, h), (2400, 1200), "200×150 的 small = W*12 × H*8");
    let (ow, oh, png) =
        render_minimap(&map, &空源, (w, h), RenderOptions::closed()).expect("分块渲染应成功");
    assert_eq!((ow, oh), (2400, 1200));
    assert_eq!(&png[..8], &PNG_MAGIC);
    let img = miredit_render::png_io::read_png_from_memory(&png).expect("解码");
    assert_eq!((img.width, img.height), (2400, 1200));
}

/// 帧比整块还大 ⇒ 扩边超本档位上限 ⇒ **413 `too_large` 中文**（不拼缺像素的图）。
///
/// `70×4` 格 `@One`（`3360×128`），块网格 `2×1`：块 1 从第 64 列开始，
/// 而 `(0,0)` 那格的表层帧宽 3300 px（≈69 格）⇒ 要把它画进块 1 得扩 64 格，
/// 超 `MAX_MARGIN_CELLS = 48` 的上限 ⇒ 只能报错。这条同时证明 web 的错误映射
/// **按 `TiledError` 语义分类**（不去匹配中文子串）。
#[test]
fn 帧比块还大明确413() {
    /// 一帧宽 3300 px 的假源（块边长 64 格 = 3072 px，装不下）。
    struct 巨帧源;
    impl SpriteSource for 巨帧源 {
        fn frame(&self, _lib: LibRef, _index: u32) -> Option<Frame> {
            let (w, h) = (3300u32, 32u32);
            Some(Frame {
                width: w,
                height: h,
                rgba: vec![255u8; (w * h) as usize * 4],
                ..Frame::default()
            })
        }
    }

    // ⚠ 必须真有图号：全 0 的图「没有帧可画」⇒ 也就没有外溢，测不出扩边上限
    let map = 合成图带前帧(70, 4, 0, 0);
    let (w, h) = target_size(
        MinimapSize::Explicit {
            width: 3360,
            height: 128,
        },
        70,
        4,
    )
    .expect("尺寸合法");
    let e = render_minimap(&map, &巨帧源, (w, h), RenderOptions::closed())
        .expect_err("帧比块还大必须明确报错");
    assert_eq!(e.status(), axum::http::StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(e.code(), "too_large");
    for must in ["帧尺寸过大", "外扩 64 格", "不静默降档"] {
        assert!(e.message().contains(must), "缺 {must}：{}", e.message());
    }
}

/// **700×700 默认 small 走分块出整幅**（合成图版，不做 PNG 编码）。
///
/// Q2 之前这一发是 413（1× 中间图 `33600×22400×4 = 3 GB` 超单次护栏）；
/// 改成共享分块渲染后必须真的拼出 `8400×5600`，且**块数 > 1**（否则等于没分块）。
/// 端到端（真图库 + 真 `.map` + PNG 落盘）由 `真地图700x700默认small导出成功` 覆盖。
///
/// ⚠ **默认 `#[ignore]`**：121 块 × 每块 `3072×2048` 的 1× 中间图，debug 下约 65 s
/// （`200×150` 的 [`分块渲染跨过单次上限仍成功`] 是几秒级的常跑替代）。验收时手工跑：
/// `cargo test -p miredit-web --bin miredit-web 合成700x700 -- --ignored --nocapture`
#[ignore = "debug 下约 65 s；常跑用 200×150 那条，700×700 交付验收手工跑"]
#[test]
fn 合成700x700默认small走分块出整幅() {
    let map = 合成图(700, 700);
    let (w, h) = target_size(MinimapSize::Preset(MinimapPreset::Small), 700, 700).expect("尺寸合法");
    assert_eq!((w, h), (8400, 5600), "700×700 的 small = W*12 × H*8");
    let img = render_tiled(
        &map,
        &空源,
        &LayerMask::all(),
        CollisionOverlay::none(),
        RenderScale::Quarter,
        (0, 0),
        (700, 700),
        RenderOptions::closed(),
    )
    .expect("700×700 @Quarter 必须能分块渲出来");
    assert_eq!((img.width, img.height), (8400, 5600));
    assert_eq!(img.rgba.len(), 8400 * 5600 * 4);
    assert!(
        img.tiles > 100,
        "700×700 必须切成多块（64 格一块 ⇒ 11×11 = 121 块），实得 {} 块",
        img.tiles
    );
}

/* ===================== 三、响应头 / 文件名 ===================== */

/// 下载文件名：ASCII 原名直接落 `filename="…"`，非 ASCII 追加 RFC 5987 的 `filename*`。
#[test]
fn 下载文件名与宽高头格式() {
    assert_eq!(
        content_disposition("0", 24, 16)
            .expect("头应合法")
            .to_str()
            .expect("ASCII"),
        r#"attachment; filename="0-minimap-24x16.png""#
    );
    // 非 ASCII 原名：ASCII 回落 + 百分号编码的第二参数（两者都要有）
    let v = content_disposition("地图", 8400, 5600)
        .expect("头应合法（中文不直接进 filename=）")
        .to_str()
        .expect("ASCII")
        .to_string();
    assert!(v.starts_with(r#"attachment; filename="__-minimap-8400x5600.png""#), "{v}");
    assert!(
        v.contains("filename*=UTF-8''%E5%9C%B0%E5%9B%BE-minimap-8400x5600.png"),
        "{v}"
    );
    // 空名 / 路径分隔符 / 空格都不能让头非法
    for stem in ["", "a b", "a/b\\c"] {
        let v = content_disposition(stem, 1, 1)
            .expect("头应合法")
            .to_str()
            .expect("ASCII")
            .to_string();
        assert!(v.contains("-minimap-1x1.png"), "{stem:?} → {v}");
        assert!(!v.contains('/'), "文件名里不许留分隔符：{v}");
    }
    // 宽高头就是十进制 ASCII
    assert_eq!(
        数值头("x-export-width", 8400).expect("头").1.to_str().expect("ASCII"),
        "8400"
    );
    assert_eq!(
        数值头("x-export-height", 5600).expect("头").1.to_str().expect("ASCII"),
        "5600"
    );
    // 地图名取不到时退回 `map`（不猜、不用空串）
    assert_eq!(download_stem(None), "map");
    assert_eq!(download_stem(Some(std::ffi::OsStr::new("  "))), "map");
    assert_eq!(download_stem(Some(std::ffi::OsStr::new("EM001"))), "EM001");
}

/// `doors` / `lighting` 与 `/api/tiles` **同口径**（复用同一对解析函数，不是抄一份）。
#[test]
fn 门与光照与tiles同口径() {
    assert!(!parse_doors_open(None).expect("缺省"));
    assert!(parse_doors_open(Some("1")).expect("1"));
    assert!(parse_doors_open(Some("2")).is_err(), "只认 0/1");
    assert!(parse_doors_open(Some("")).is_err(), "空值也是非法值");
    assert_eq!(parse_lighting(None).expect("缺省"), MapLighting::Day);
    assert_eq!(
        parse_lighting(Some("night")).expect("night"),
        MapLighting::Night
    );
    assert!(parse_lighting(Some("dusk")).is_err(), "只认三档");
    // 两个开关一次组合（分两步构造会互相覆盖）
    let o = RenderOptions::with_doors_and_lighting(true, MapLighting::Night);
    assert!(o.doors_open);
    assert_eq!(o.lighting, MapLighting::Night);
}

/* ===================== 四、HTTP 层：409 / 400 判序 ===================== */

/// 没有地图 ⇒ **409 `no_map` + 中文**（且与图库无关：本用例连图库都没有）。
///
/// 判序是本端点**有意**与 `/api/tiles` 不同的一处（地图先于图库），理由见模块文档 §三：
/// 小地图的尺寸由地图格数推导 ⇒ 没有地图时连「该出多大」都不知道。这条测试同时钉住判序。
#[tokio::test(flavor = "multi_thread")]
async fn 无地图返回409中文() {
    let (addr, server) = 起服务(空状态()).await;
    let (status, head, body) = std::thread::spawn(move || http_get(addr, "/api/export/minimap"))
        .join()
        .expect("客户端线程不应 panic");
    server.abort();

    assert!(status.starts_with("http/1.1 409"), "应 409：{status}");
    assert!(
        head.contains("content-type: application/json; charset=utf-8"),
        "错误体必须是 JSON：{head}"
    );
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("\"code\":\"no_map\""), "{body}");
    assert!(body.contains("尚未打开地图"), "必须是中文原因：{body}");
}

/// 参数错**先于**环境错：没地图也该先拿到 400（与 `/api/tiles` 的 `doors` 同一条纪律）。
#[tokio::test(flavor = "multi_thread")]
async fn 参数错先于无地图() {
    let (addr, server) = 起服务(空状态()).await;
    let got = std::thread::spawn(move || {
        (
            http_get(addr, "/api/export/minimap?preset=medium&width=100&height=100"),
            http_get(addr, "/api/export/minimap?preset=huge"),
            http_get(addr, "/api/export/minimap?width=10"),
        )
    })
    .join()
    .expect("客户端线程不应 panic");
    server.abort();

    for (label, (status, head, body)) in [
        ("互斥", got.0),
        ("未知 preset", got.1),
        ("缺 height", got.2),
    ] {
        assert!(status.starts_with("http/1.1 400"), "{label} 应 400：{status}");
        assert!(head.contains("content-type: application/json"), "{label}：{head}");
        let body = String::from_utf8_lossy(&body);
        assert!(body.contains("\"code\":\"bad_request\""), "{label}：{body}");
        // 错误体里必须真的有一段中文原因（不是空壳、不是英文原文）
        assert!(
            body.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
            "{label} 的错误体缺中文原因：{body}"
        );
    }
}

/// 有地图但**图库未就绪** ⇒ **503 `assets_unavailable` + 中文**（不输出全透明假图）。
#[tokio::test(flavor = "multi_thread")]
async fn 无图库返回503中文() {
    let 地图 = 临时地图(3, 2, "无图库");
    let (events, _) = tokio::sync::broadcast::channel(4);
    let session = miredit_core::EditSession::load(&地图).expect("打开合成地图");
    let st = crate::server::share(AppState {
        map_root: PathBuf::new(),
        web_root: None,
        session: Arc::new(RwLock::new(Some(session))),
        libs: None,
        source: None,
        // A-024：测试夹具只装 WIL ⇒ Default 号段
        号段: miredit_core::库号号段::Wemade,
        stamps: Mutex::new(None),
        trailer_cache: Mutex::new(None),
        palette_json: PathBuf::new(),
        events,
    });

    let (addr, server) = 起服务(st).await;
    let (status, head, body) = std::thread::spawn(move || http_get(addr, "/api/export/minimap"))
        .join()
        .expect("客户端线程不应 panic");
    server.abort();
    let _ = std::fs::remove_file(&地图);

    assert!(status.starts_with("http/1.1 503"), "应 503：{status}");
    assert!(head.contains("content-type: application/json"), "{head}");
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("\"code\":\"assets_unavailable\""), "{body}");
    assert!(body.contains("图库未就绪"), "必须是中文原因：{body}");
}

/* ===================== 五、真图库端到端（有素材真断言，无素材跳过） ===================== */

/// **HTTP 层端到端**：真图库 + 合成小地图（2×2 格）⇒ 200 + `image/png` + 宽高头 = `24×16`
/// + `Content-Disposition` 里的 `-minimap-24x16.png` + PNG 魔数。
///
/// 为什么用合成地图：这条验的是「成功路径的每一环」（头、体、文件名），
/// 用最小窗口才跑得快。真实 `0.map`（700×700）的默认 small 由
/// [`真地图700x700默认small导出成功`] 单独覆盖。
#[tokio::test(flavor = "multi_thread")]
async fn 真图库端到端出png与宽高头() {
    let st = crate::server::share(AppState::from_env());
    if st.source.is_none() {
        eprintln!("[跳过] 图库不可用（AppState::from_env 扫不到 .wil/.wzl）");
        return;
    }
    let 地图 = 临时地图(2, 2, "e2e");
    {
        let mut g = st.write_session();
        *g = Some(miredit_core::EditSession::load(&地图).expect("打开合成地图"));
    }
    let (addr, server) = 起服务(st).await;
    let (status, head, body) = std::thread::spawn(move || http_get(addr, "/api/export/minimap"))
        .join()
        .expect("客户端线程不应 panic");
    server.abort();
    let _ = std::fs::remove_file(&地图);

    assert!(status.starts_with("http/1.1 200"), "应 200：{status}\n{head}");
    assert!(head.contains("content-type: image/png"), "{head}");
    assert!(head.contains("x-export-width: 24"), "缺 X-Export-Width: 24：{head}");
    assert!(
        head.contains("x-export-height: 16"),
        "缺 X-Export-Height: 16：{head}"
    );
    assert!(
        head.contains("content-disposition: attachment; filename=\"") && head.contains("-minimap-24x16.png"),
        "下载文件名不对：{head}"
    );
    assert_eq!(&body[..8], &PNG_MAGIC, "响应体必须是 PNG");
    let img = miredit_render::png_io::read_png_from_memory(&body).expect("PNG 应可解码");
    assert_eq!((img.width, img.height), (24, 16));
}

/// 同一张图换三档预设，**HTTP 层的宽高头**跟着变（medium `48×32` / large `96×64`）。
#[tokio::test(flavor = "multi_thread")]
async fn 真图库三档宽高头() {
    let st = crate::server::share(AppState::from_env());
    if st.source.is_none() {
        eprintln!("[跳过] 图库不可用（AppState::from_env 扫不到 .wil/.wzl）");
        return;
    }
    let 地图 = 临时地图(2, 2, "e2e-三档");
    {
        let mut g = st.write_session();
        *g = Some(miredit_core::EditSession::load(&地图).expect("打开合成地图"));
    }
    let (addr, server) = 起服务(st).await;
    let got = std::thread::spawn(move || {
        (
            http_get(addr, "/api/export/minimap?preset=medium"),
            http_get(addr, "/api/export/minimap?preset=large"),
            http_get(addr, "/api/export/minimap?width=10&height=7"),
        )
    })
    .join()
    .expect("客户端线程不应 panic");
    server.abort();
    let _ = std::fs::remove_file(&地图);

    for (label, r, want) in [
        ("medium", got.0, (48u32, 32u32)),
        ("large", got.1, (96, 64)),
        ("自定义 10×7", got.2, (10, 7)),
    ] {
        let (status, head, body) = r;
        assert!(status.starts_with("http/1.1 200"), "{label} 应 200：{status}\n{head}");
        assert!(
            head.contains(&format!("x-export-width: {}", want.0)),
            "{label} 的头里没有 x-export-width: {}\n{head}",
            want.0
        );
        assert!(
            head.contains(&format!("x-export-height: {}", want.1)),
            "{label} 的头里没有 x-export-height: {}\n{head}",
            want.1
        );
        assert_eq!(&body[..8], &PNG_MAGIC, "{label} 必须是 PNG");
        let img = miredit_render::png_io::read_png_from_memory(&body).expect("PNG 应可解码");
        assert_eq!((img.width, img.height), want, "{label} 的 PNG 实际尺寸");
        assert!(
            head.contains(&format!("-minimap-{}x{}.png", want.0, want.1)),
            "{label} 的下载文件名不对：{head}"
        );
    }
}


/* ===================== 六、真实 700×700 地图（Q2 的硬要求） ===================== */

/// 直接从 PNG 字节读 IHDR 里的宽高（188 MB 的图不必再解一份像素）。
fn png_ihdr_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], &PNG_MAGIC, "PNG 魔数");
    assert_eq!(&bytes[12..16], b"IHDR", "第一个块必须是 IHDR");
    let w = u32::from_be_bytes(bytes[16..20].try_into().expect("4 字节"));
    let h = u32::from_be_bytes(bytes[20..24].try_into().expect("4 字节"));
    (w, h)
}

/// **真实 700×700 地图 + 真图库：默认 small 必须 200 出 `8400×5600` PNG**。
///
/// 这条是 Q2 的验收核心：上移分块渲染之前，同一发请求是 413
/// （`33600×22400` 的 1× 中间图超单次护栏）；上移之后必须真的出图。
///
/// 素材门控：`MIREDIT_ASSET_ROOT`（缺省 `E:\GodM2`）+ `MIREDIT_CLIENT_ROOT`
/// 两者缺一就打印原因跳过（公开仓/CI 上没有游戏素材属正常）。
///
/// ⚠ **默认 `#[ignore]`**：debug 构建下 700×700 全图 = 121 块 × 490k 格 × 3 层，
/// 本机实测约 4~5 分钟，放进日常门禁会把 `cargo test -p miredit-web` 拖垮。
/// 交付验收时手工跑一次：
///
/// ```text
/// cargo test -p miredit-web --bin miredit-web 真地图700x700 -- --ignored --nocapture
/// ```
#[ignore = "debug 下 700×700 全图约 4~5 分钟；交付验收手工跑 --ignored"]
#[tokio::test(flavor = "multi_thread")]
async fn 真地图700x700默认small导出成功() {
    let st = crate::server::share(AppState::from_env());
    if st.source.is_none() {
        eprintln!("[跳过] 图库不可用（AppState::from_env 扫不到 .wil/.wzl）");
        return;
    }
    let root = std::env::var_os("MIREDIT_ASSET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"E:\GodM2"));
    let map_path = root.join("GodotClient").join("maps").join("0.map");
    if !map_path.is_file() {
        eprintln!("[跳过] 真实地图不存在：{}", map_path.display());
        return;
    }
    // 尺寸自证：本用例依赖「0.map 是 700×700」这个事实，变了要显式红（不静默放宽）
    let 地图 = mir2_map::Map::parse(&std::fs::read(&map_path).expect("读真实地图"))
        .expect("解析真实地图");
    assert_eq!(
        (u32::from(地图.width), u32::from(地图.height)),
        (700, 700),
        "0.map 的尺寸变了，本用例的期望像素（8400×5600）要跟着改"
    );
    {
        let mut g = st.write_session();
        *g = Some(miredit_core::EditSession::load(&map_path).expect("打开真实地图"));
    }
    let (addr, server) = 起服务(st).await;
    // debug 下这张图要跑 4~5 分钟 ⇒ 客户端读超时放宽到 15 分钟（只此一处）
    let (status, head, body) = std::thread::spawn(move || {
        http_get_timeout(addr, "/api/export/minimap", 900)
    })
    .join()
    .expect("客户端线程不应 panic");
    server.abort();

    assert!(status.starts_with("http/1.1 200"), "应 200：{status}\n{head}");
    assert!(head.contains("content-type: image/png"), "{head}");
    assert!(
        head.contains("x-export-width: 8400"),
        "缺 X-Export-Width: 8400：{head}"
    );
    assert!(
        head.contains("x-export-height: 5600"),
        "缺 X-Export-Height: 5600：{head}"
    );
    assert!(
        head.contains("content-disposition: attachment; filename=\"")
            && head.contains("-minimap-8400x5600.png"),
        "下载文件名不对：{head}"
    );
    assert_eq!(png_ihdr_size(&body), (8400, 5600), "PNG 实际宽高");
    // 空白图会被压成几百字节；真实地图必须远大于此（防止「成功但出白图」）
    assert!(
        body.len() > 1_000_000,
        "PNG 只有 {} B —— 真实 700×700 地图不该是空白图",
        body.len()
    );
    println!(
        "[实测] 0.map 700×700 默认 small：PNG {} B（{}×{}）",
        body.len(),
        8400,
        5600
    );
}
