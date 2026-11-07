//! 帮助 → **关于 Crystal Studio**：只读模态（产品名 / 版本 / 实现 / 上游 / 本仓 / 许可证 / 构建档位）。
//!
//! # 为什么要它
//!
//! 2026-11-06 用户报"GUI 版没有版本入口"，实查是**旧 exe**（源码里 `--version` 早就有）。
//! 但 `--version` 只对命令行用户有用：双击 exe 的人没有任何地方能看到"我手上这版是几"。
//! ⇒ 这个对话框是**GUI 里的版本出口**，与 `--version`、网页 `/api/health` 三处同源。
//!
//! # 三条口径
//!
//! 1. **版本号现算**：`env!("CARGO_PKG_VERSION")`（= 根 `[workspace.package].version`，
//!    唯一真源）。本模块**不写任何版本数字** —— 写死一个就必然在 bump 之后与 `--version`
//!    各说一套，那正是本轮要修的病。
//! 2. **构建档位现算**：`cfg!(debug_assertions)`。用户拿到的是 debug 还是 release，
//!    直接决定了"卡不卡""能不能崩给我看"，这一项必须是真的，不许瞎猜。
//! 3. **只读 + 可关**：Esc / 点击任意处关闭（与 [`crate::menu::shortcuts`] 同一套交互）。
//!    打开期间 `app::handle_keys` 整段早退（`overlay.is_some()`），所以**编辑快捷键不会被吃进地图**
//!    —— 模态不吃键，但也不放行，避免"看着对话框却把 `Ctrl+S` 按进文档里"。
//!
//! # 上游 / 本仓地址的来源
//!
//! * 上游：[`tx::ABOUT_UPSTREAM`] —— `github.com/fluttergo/CrystalStudio`（C# + Avalonia 原版）；
//! * 本仓：[`tx::ABOUT_REPO`] —— `github.com/lxf505226/CrystalStudio-Rust`（Rust 移植）。
//!
//! 两个地址都写全 `github.com/...`（不带 `https://` 前缀）：本仓**不做可点击链接**
//! （自绘界面里没有浏览器集成），显示成 URL 是为了让人能肉眼抄走。

use egui::{pos2, vec2, Align2, Painter, Rect};

use crate::layout::Layout;
use crate::metrics as m;
use crate::paint as pt;
use crate::theme as t;
use crate::ui_text as tx;

/// 本程序声称的版本号（**唯一来源**：`CARGO_PKG_VERSION`）。
///
/// 与 [`crate::cli::Cli::version_line`]（`--version` 打的那一行）、网页 `/api/health`
/// 是同一个值：三者都来自根 `[workspace.package].version`（见 `VERSIONING.md` §3.1）。
/// 单测[`tests::版本与cli同源`]钉住这条关系。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 构建档位文案（`debug` / `release`，取自编译期断言开关）。
pub fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        tx::ABOUT_PROFILE_DEBUG
    } else {
        tx::ABOUT_PROFILE_RELEASE
    }
}

/// 对话框里的一行：`标签` + `值`。
///
/// 值用 `String`（版本与构建档位是现算的），标签用 `&'static str`（全部来自 `ui_text`，
/// 这样字形自测能扫到）。**不派生 `Debug`/`Clone`**：它只在这一帧里活一次。
pub struct Row {
    /// 左侧标签（中文，`ui_text` 常量）。
    pub label: &'static str,
    /// 右侧值（产品名/版本/地址/许可证/档位）。
    pub value: String,
}

/// 对话框的 7 行内容（顺序即显示顺序）。
///
/// 逐行都对应任务书点名的字段：产品名、版本、`Rust 全量移植`、上游、本仓、许可证、构建档位。
pub fn rows() -> Vec<Row> {
    vec![
        Row {
            label: tx::ABOUT_LABEL_PRODUCT,
            value: tx::BRAND.to_owned(),
        },
        Row {
            label: tx::ABOUT_LABEL_VERSION,
            value: VERSION.to_owned(),
        },
        Row {
            label: tx::ABOUT_LABEL_IMPL,
            value: tx::ABOUT_IMPL.to_owned(),
        },
        Row {
            label: tx::ABOUT_LABEL_UPSTREAM,
            value: tx::ABOUT_UPSTREAM.to_owned(),
        },
        Row {
            label: tx::ABOUT_LABEL_REPO,
            value: tx::ABOUT_REPO.to_owned(),
        },
        Row {
            label: tx::ABOUT_LABEL_LICENSE,
            value: tx::ABOUT_LICENSE.to_owned(),
        },
        Row {
            label: tx::ABOUT_LABEL_PROFILE,
            value: build_profile().to_owned(),
        },
    ]
}

/// 对话框矩形（居中）。
pub fn rect(l: &Layout) -> Rect {
    Rect::from_center_size(l.content.center(), vec2(m::ABOUT_W, m::ABOUT_H))
}

/// 内容高（标题 + 分隔线 + 各行 + 底部提示 + 上下内边距）。
///
/// 抽成函数是为了让[`tests::面板放得下内容`]与绘制**用同一个算式**：
/// 改了行数/行高却忘了改 `ABOUT_H`，那条用例会当场红（而不是"上线后提示被裁掉"）。
pub fn content_height() -> f32 {
    m::ABOUT_PAD * 2.0
        + m::ABOUT_TITLE_H
        + 1.0
        + m::ABOUT_TITLE_GAP
        + rows().len() as f32 * m::ABOUT_ROW_H
        + m::ABOUT_HINT_H
}

/// 画对话框（遮罩 + 面板 + 标题 + 七行 + 底部提示）。
pub fn draw(p: &Painter, l: &Layout) {
    let pal = t::ui();
    // 遮罩：压住整个窗口（只读模态，底下的东西不该被误点）
    pt::fill(p, l.content, pal.modal_scrim);
    let r = rect(l);
    pt::fill(p, r, pal.panel_bg);
    pt::border(p, r, pal.menu_popup_border);

    let inner_l = r.left() + m::ABOUT_PAD;
    let inner_w = r.width() - m::ABOUT_PAD * 2.0;
    let mut y = r.top() + m::ABOUT_PAD;

    // 标题
    pt::text(
        p,
        pos2(inner_l, y + m::ABOUT_TITLE_H * 0.5),
        Align2::LEFT_CENTER,
        tx::ABOUT_TITLE,
        m::PANEL_TITLE_FONT,
        pal.text,
    );
    y += m::ABOUT_TITLE_H;
    // 标题下 1px 分隔线
    pt::fill(
        p,
        Rect::from_min_size(pos2(inner_l, y), vec2(inner_w, 1.0)),
        pal.separator,
    );
    y += 1.0 + m::ABOUT_TITLE_GAP;

    // 七行：标签列（闷色）+ 值列（正文色）
    for row in rows() {
        pt::text(
            p,
            pos2(inner_l, y + m::ABOUT_ROW_H * 0.5),
            Align2::LEFT_CENTER,
            row.label,
            m::BODY_FONT,
            pal.text_muted,
        );
        pt::text(
            p,
            pos2(inner_l + m::ABOUT_LABEL_COL_W, y + m::ABOUT_ROW_H * 0.5),
            Align2::LEFT_CENTER,
            &row.value,
            m::BODY_FONT,
            pal.text,
        );
        y += m::ABOUT_ROW_H;
    }

    // 底部关闭提示（贴着面板下沿，与内容留 4px 呼吸）
    pt::text(
        p,
        pos2(inner_l, r.bottom() - m::ABOUT_PAD - m::ABOUT_HINT_H * 0.5),
        Align2::LEFT_CENTER,
        tx::ABOUT_CLOSE_HINT,
        m::BODY_FONT,
        pal.text_muted,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn 布局() -> Layout {
        Layout::compute(Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(m::WINDOW_W, m::WINDOW_H),
        ))
    }

    /// **七行内容齐全，且版本/档位是现算的**（不是写死的字面量）。
    #[test]
    fn 七行内容齐全() {
        let rows = rows();
        assert_eq!(rows.len(), 7, "产品/版本/实现/上游/本仓/许可证/构建 共 7 行");
        assert_eq!(rows[0].label, tx::ABOUT_LABEL_PRODUCT);
        assert_eq!(rows[0].value, tx::BRAND, "产品名必须取统一品牌常量");
        assert_eq!(rows[1].label, tx::ABOUT_LABEL_VERSION);
        assert_eq!(
            rows[1].value,
            env!("CARGO_PKG_VERSION"),
            "版本必须取 CARGO_PKG_VERSION（版本号唯一真源），不许写死"
        );
        assert_eq!(rows[2].value, tx::ABOUT_IMPL);
        assert!(rows[2].value.contains("Rust"), "实现形态要点明 Rust");
        assert_eq!(rows[3].value, "github.com/fluttergo/CrystalStudio");
        assert_eq!(rows[4].value, "github.com/lxf505226/CrystalStudio-Rust");
        assert_eq!(rows[5].value, "MIT");
        assert_eq!(rows[6].value, build_profile());
        // 标签/值都不许为空（空行就是"没填"，等于假信息）
        for r in &rows {
            assert!(!r.label.is_empty(), "有空标签");
            assert!(!r.value.is_empty(), "{} 的值是空的", r.label);
        }
    }

    /// **构建档位跟着编译期开关走**：debug 构建不许自称 release（反之亦然）。
    #[test]
    fn 构建档位与编译期开关一致() {
        if cfg!(debug_assertions) {
            assert_eq!(build_profile(), tx::ABOUT_PROFILE_DEBUG);
            assert!(build_profile().starts_with("debug"));
        } else {
            assert_eq!(build_profile(), tx::ABOUT_PROFILE_RELEASE);
            assert!(build_profile().starts_with("release"));
        }
    }

    /// **版本与 `--version` 同源**：这里是 GUI 的版本出口，命令行是另一个出口，
    /// 两者必须指向同一个版本号（否则「关于」与 `--version` 会各说一套 —— 本轮修的就是这个）。
    #[test]
    fn 版本与cli同源() {
        assert_eq!(
            crate::cli::Cli::version_line(),
            format!("{} {}", tx::BRAND, VERSION),
            "`--version` 那一行必须由同一个版本常量拼出来"
        );
        assert!(rows().iter().any(|r| r.value == VERSION));
    }

    /// **面板放得下内容**（改行数/行高却忘了改 `ABOUT_H` 会在这里红，而不是上线后被裁掉）。
    #[test]
    fn 面板放得下内容() {
        let need = content_height();
        assert!(
            need <= m::ABOUT_H,
            "关于对话框要 {need}px，面板只有 {}px（底部提示会被裁掉）",
            m::ABOUT_H
        );
        // 反过来也别留太多空白（超过 40px 说明尺寸没跟着内容算）
        assert!(
            m::ABOUT_H - need <= 40.0,
            "面板高 {}px 与内容 {need}px 差得太多（留白过大）",
            m::ABOUT_H
        );
    }

    /// 对话框在窗口内，且标签列不会压到值列上。
    #[test]
    fn 几何自洽() {
        let l = 布局();
        let r = rect(&l);
        assert!(l.content.contains_rect(r), "对话框越出窗口：{r:?}");
        assert!(r.width() > m::ABOUT_LABEL_COL_W + m::ABOUT_PAD * 2.0);
        // 值列起点必须落在面板内部（否则地址会画到面板外）
        let value_x = r.left() + m::ABOUT_PAD + m::ABOUT_LABEL_COL_W;
        assert!(value_x < r.right() - m::ABOUT_PAD);
        // 最长的值是本仓地址：按 12px 字号估算宽度，必须塞得进值列
        let longest = rows()
            .into_iter()
            .map(|r| r.value.chars().count())
            .max()
            .unwrap_or(0);
        let avail = r.right() - m::ABOUT_PAD - value_x;
        let est = longest as f32 * m::BODY_FONT * 0.62; // ASCII 均宽 ≈ 0.62em（保守上限）
        assert!(
            est <= avail,
            "最长的值约 {est}px，值列只有 {avail}px（会被裁掉）"
        );
    }

    /// **D-008：菜单项与模态标题是同一份文案**（2026-11-07）。
    ///
    /// 原来是两个独立字面量，注释却写「同一份」⇒ 改一处忘另一处就会让菜单项与标题说法不一致。
    /// 现在 [`tx::MENU_ABOUT`] 直接引用 [`tx::ABOUT_TITLE`]；`ui_text::ALL` 仍按名字各登记一条
    /// （守卫视按名字找，不受"引用同一份"影响）。
    #[test]
    fn 菜单项与模态标题同一份() {
        assert_eq!(tx::MENU_ABOUT, tx::ABOUT_TITLE, "D-008：必须同一份");
        for 名 in ["MENU_ABOUT", "ABOUT_TITLE"] {
            assert!(
                tx::ALL.iter().any(|(n, _)| *n == 名),
                "{名} 必须仍登记在 ui_text::ALL 里（字形守卫按名字扫）"
            );
        }
    }

    /// **字形齐全**：标题/标签/值/提示里每个字符在 `ui_text` 的字体里都有字形。
    ///
    /// 缺字在界面上就是豆腐块（历史事故：`DIAG_GRID` 那次）。文案虽然已进 `ui_text::ALL`
    /// （由 `fonts::tests` 统一扫），这里再钉一遍本模块**画出来的那一串**：
    /// 版本号与构建档位是现算的字符串，只有在这里才拼得出来。
    #[test]
    fn 文案字形齐全() {
        use ab_glyph::Font as _;
        let f = crate::fonts::load().unwrap_or_else(|e| panic!("加载中文字体失败：{e}"));
        let mut texts: Vec<String> = vec![tx::ABOUT_TITLE.to_owned(), tx::ABOUT_CLOSE_HINT.to_owned()];
        for r in rows() {
            texts.push(r.label.to_owned());
            texts.push(r.value);
        }
        for text in texts {
            for c in text.chars() {
                assert!(
                    f.font.glyph_id(c).0 != 0,
                    "缺字形 {c:?} U+{:04X}：{text}",
                    c as u32
                );
            }
        }
    }
}
