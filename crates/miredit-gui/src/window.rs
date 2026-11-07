//! 窗口创建参数与运行入口。
//!
//! **为什么用无边框窗口（`with_decorations(false)`）**：
//! 1. 参考截图里 CrystalStudio 用的是 Avalonia 自绘标题栏（深金色标题栏 + 右侧 `− □ ×`），
//!    OS 原生标题栏会被画成系统样式，与截图不符；
//! 2. 任务书 §七-2 要求自截图**恰好 1500×950**。带原生边框时 `GetWindowRect` 会比客户区大
//!    一圈（约 16×39），截出来就不是 1500×950 了。
//!
//! 代价：窗口不可通过边框缩放；已用「标题栏拖拽 = `ViewportCommand::StartDrag`」保住移动能力。
//! 最小尺寸仍按任务书设置（1100×720），由 `with_min_inner_size` 交给 winit。
//!
//! # 本单变化：**先装载、再开窗**
//! [`MireditApp::new`] 会先把地图与图库装好（失败也照样开窗，只是状态栏/画布显示原因）。
//! 这样窗口标题、标题栏、状态栏、画布**第一帧就是真实数据**，不会先闪一帧"未命名 256×256"。
//!
//! # 窗口图标（2026-11-06 新增）
//!
//! [`native_options`] 里挂了 [`app_icon`]：任务栏 / `Alt+Tab` / 窗口左上角那个小图标。
//! 三件事分开说清楚，免得又混成"图标做了"：
//!
//! | 位置 | 来源 | 谁负责 |
//! |---|---|---|
//! | 资源管理器 / 桌面 / 文件属性里的 exe 图标 | **PE 资源段**（`1 ICON`） | `build.rs` + `embed-resource`（`assets/crystal-studio.ico`）|
//! | 窗口/任务栏/Alt+Tab 图标 | `ViewportBuilder::with_icon`（winit `set_window_icon`） | 本模块 [`app_icon`]（`assets/crystal-studio.png`）|
//! | 「关于」对话框里显示的产品名/版本 | 编译期常量 | `menu::about` |
//!
//! PNG 用 `include_bytes!` **嵌进 exe**，不是运行时读文件：exe 会被复制到别的目录
//! （交付归档、桌面）单独运行，靠相对路径找 `assets/` 必然找不到。

use crate::app::MireditApp;
use crate::cli::Cli;
use crate::metrics as m;

/// 应用图标 PNG 的字节（**构建期嵌入**，运行时零文件依赖）。
///
/// 尺寸/来源见 `assets/crystal-studio.png`（256×256，与 `.ico` 的最大帧同源同像素）。
const ICON_PNG: &[u8] = include_bytes!("../assets/crystal-studio.png");

/// 把嵌入的 PNG 解成 egui 的窗口图标（失败 ⇒ `None` + 一行诊断，**不 panic**）。
///
/// 为什么可以"失败也不 panic"：图标是观感问题，不该让编辑器打不开。
/// 但**也不能静默**（那会让人以为图标嵌进去了）：失败时打
/// [`crate::ui_text::DIAG_WINDOW_ICON_FAIL`]，并在
/// [`tests::嵌入的图标能解码`] 里把"正常构建下必然成功"钉住。
pub fn app_icon() -> Option<egui::viewport::IconData> {
    match eframe::icon_data::from_png_bytes(ICON_PNG) {
        Ok(icon) => Some(icon),
        Err(e) => {
            eprintln!(
                "{}",
                crate::ui_text::fill(crate::ui_text::DIAG_WINDOW_ICON_FAIL, &[("err", &e.to_string())])
            );
            None
        }
    }
}

/// **自动化截图（`--shot`）的固定窗口位置**（屏幕左上角为原点，单位 = 逻辑像素）。
///
/// # 为什么要钉位置
///
/// 截图内容必须与"窗口落在屏幕哪"无关，否则同一命令在不同次运行里可能拍到不一样的东西：
///
/// 1. **缩放比**：窗口落在哪块显示器上决定 `pixels_per_point`（混合 DPI 多屏机器上
///    100% 与 150% 会截出 1500×950 与 2250×1425 两张**尺寸都不同**的图，逐字节比对必失败）；
/// 2. **窗口被系统挪位 / 承接**：主显示器左上角是唯一"每台机器都指向同一处"的位置。
///
/// 指针那一维的不可复现性**不在这里解决**：它由
/// `app::MireditApp::update_hover` 的输入锁判据单独挡掉（`--shot` 下不读指针）。
/// 两条各管一维，缺哪条都会剩一类"同一命令两张图"。
///
/// 普通运行**不钉**：用户自己的窗口位置不该被程序抢走。
pub const SHOT_WINDOW_POS: [f32; 2] = [0.0, 0.0];

/// 构造 eframe 的原生窗口选项（标题用真实文档标题）。
///
/// `cli` 只用来判两件**取证相关**的事：`--shot` 时钉窗口位置（[`SHOT_WINDOW_POS`]）。
/// 其余窗口设置（尺寸/装饰/图标）与命令行无关 —— 谁跑都一样。
pub fn native_options(title: &str, cli: &Cli) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size([m::WINDOW_W, m::WINDOW_H])
        .with_min_inner_size([m::WINDOW_MIN_W, m::WINDOW_MIN_H])
        // 自绘标题栏：关掉 OS 装饰，客户区即 1500×950
        .with_decorations(false)
        .with_resizable(true);
    // `--shot`：窗口位置钉死（判据见 [`SHOT_WINDOW_POS`] 的文档）
    if cli.shot.is_some() {
        viewport = viewport.with_position(SHOT_WINDOW_POS);
    }
    // 窗口图标：解不出来就保持系统默认（`app_icon` 里已经打过诊断行）
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    eframe::NativeOptions {
        viewport,
        ..Default::default()
    }
}

/// 启动 GUI（阻塞直到窗口关闭）。
///
/// 中文字体在**创建 App 的那一帧之前**装好（`CreationContext` 里就有 `egui_ctx`），
/// 否则首帧会先按 egui 内置字体排一遍，屏幕上会闪一帧方框。
pub fn run(cli: &Cli) -> eframe::Result {
    let app = MireditApp::new(cli);
    let title = app.window_title().to_string();
    eframe::run_native(
        &title,
        native_options(&title, cli),
        Box::new(move |cc| {
            crate::fonts::install(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **嵌入的图标必须能解码**（正常构建下这条不可能红；红了说明资源坏了/被替换了）。
    #[test]
    fn 嵌入的图标能解码() {
        let icon = app_icon().expect("嵌入的 PNG 必须能解码（include_bytes 的构建期资源）");
        assert_eq!((icon.width, icon.height), (256, 256), "窗口图标是 256×256");
        assert_eq!(
            icon.rgba.len(),
            256 * 256 * 4,
            "RGBA 字节数必须是 w*h*4"
        );
        // 不透明像素必须存在（全透明的图标等于没图标）
        assert!(
            icon.rgba.chunks(4).any(|p| p[3] == 255),
            "图标全透明 —— 那和没有图标一样"
        );
    }

    /// **窗口选项真的挂上了图标**：改了尺寸/丢了 `with_icon`，这条用例会红。
    ///
    /// 为什么单列一条：`app_icon()` 能解出图 ≠ `native_options` 把它交上去了
    /// （两者之间只差一行的 `.with_icon(...)`，正是最容易在重构里被顺手删掉的那种）。
    #[test]
    fn 窗口选项挂了图标() {
        let opts = native_options("Crystal Studio", &Cli::default());
        let icon = opts.viewport.icon.as_ref().expect("NativeOptions 没挂窗口图标");
        assert_eq!((icon.width, icon.height), (256, 256));
        // 标题/尺寸这些既有契约不受影响（同一行改动不许把别的设置带跑）
        assert_eq!(opts.viewport.title.as_deref(), Some("Crystal Studio"));
        assert_eq!(opts.viewport.inner_size, Some(egui::vec2(m::WINDOW_W, m::WINDOW_H)));
        assert_eq!(opts.viewport.decorations, Some(false), "自绘标题栏：不要 OS 装饰");
    }

    /// **`--shot` 钉住窗口位置，普通运行不钉**。
    ///
    /// 这条挡两件事：① 取证截图别随"窗口落在哪块显示器"变（混合 DPI 下连像素尺寸都会变）；
    /// ② 别把普通运行也钉在 (0,0) —— 那会抢走用户自己摆好的窗口。
    #[test]
    fn 截图钉窗口位置普通运行不钉() {
        let 普通 = native_options("Crystal Studio", &Cli::default());
        assert_eq!(普通.viewport.position, None, "普通运行不许改用户窗口位置");

        let 截图 = native_options(
            "Crystal Studio",
            &Cli {
                shot: Some(std::path::PathBuf::from(r"E:\__shot__.png")),
                ..Default::default()
            },
        );
        assert_eq!(
            截图.viewport.position,
            Some(egui::pos2(SHOT_WINDOW_POS[0], SHOT_WINDOW_POS[1])),
            "--shot 必须把窗口钉在固定点"
        );
        // 钉位置不许连带改别的窗口设置
        assert_eq!(
            截图.viewport.inner_size,
            Some(egui::vec2(m::WINDOW_W, m::WINDOW_H))
        );
        assert_eq!(截图.viewport.decorations, Some(false));
    }
}
