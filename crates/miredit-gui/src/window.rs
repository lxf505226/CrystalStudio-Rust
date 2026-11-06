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

/// 构造 eframe 的原生窗口选项（标题用真实文档标题）。
pub fn native_options(title: &str) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size([m::WINDOW_W, m::WINDOW_H])
        .with_min_inner_size([m::WINDOW_MIN_W, m::WINDOW_MIN_H])
        // 自绘标题栏：关掉 OS 装饰，客户区即 1500×950
        .with_decorations(false)
        .with_resizable(true);
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
        native_options(&title),
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
        let opts = native_options("Crystal Studio");
        let icon = opts.viewport.icon.as_ref().expect("NativeOptions 没挂窗口图标");
        assert_eq!((icon.width, icon.height), (256, 256));
        // 标题/尺寸这些既有契约不受影响（同一行改动不许把别的设置带跑）
        assert_eq!(opts.viewport.title.as_deref(), Some("Crystal Studio"));
        assert_eq!(opts.viewport.inner_size, Some(egui::vec2(m::WINDOW_W, m::WINDOW_H)));
        assert_eq!(opts.viewport.decorations, Some(false), "自绘标题栏：不要 OS 装饰");
    }
}
