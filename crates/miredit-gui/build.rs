//! 构建脚本：把**应用图标 + 版本资源**嵌进 Windows exe（PE 资源段）。
//!
//! # 为什么需要它
//!
//! 窗口图标（`window.rs` 的 `with_icon`）只管**运行期那扇窗**；而
//!
//! * 资源管理器 / 桌面快捷方式 / `Alt+Tab` 缩略图上那个图标，
//! * 文件「属性 → 详细信息」里的产品名、版本号、说明，
//!
//! 全部来自 exe 自己的 **PE 资源段**。没有这一步，用户拿到的就是"界面里好看、
//! 文件夹里是个默认白纸图标"的 exe —— 那种差异一眼就能看见，但很容易被当成"没做"。
//!
//! # 三条口径
//!
//! 1. **版本号只有一个真源**：`CARGO_PKG_VERSION`（= 根 `[workspace.package].version`，
//!    见 `VERSIONING.md` §3.1）。本文件**不写任何版本数字**，`.rc` 里的
//!    `FILEVERSION` / `ProductVersion` 都是构建期现算的。
//! 2. **`.rc` 在 `OUT_DIR` 里现生成**，不往仓库里塞手写 `.rc`：
//!    手写文件里必然有第二份版本号，bump 时就会漂。
//! 3. **没有资源编译器 ⇒ 构建失败**（不是警告、不是静默跳过）。
//!    `embed-resource` 在"找不到 windres/rc.exe"时返回 `NotAttempted`，
//!    而它的 `manifest_optional()` 会把这一档当成功 —— 那正好产出"没图标的 exe"。
//!    所以这里**显式判 `Ok`**，其余一律 `panic!` 并把原因写清楚。
//!
//! # 本机与 CI 的差别（都走 embed-resource，不自己拼命令行）
//!
//! | 环境 | 资源编译器 | 说明 |
//! |---|---|---|
//! | 本机 `x86_64-pc-windows-gnu` | PATH 上的 `windres`（MinGW） | 用 `-fo/-C -no-preprocess/-c 65001` |
//! | CI `windows-latest`（MSVC） | `rc.exe`（embed-resource 去 VS/SDK 目录里找） | 用 `/fo` |
//!
//! 缺编译器时的替代方案（这里只记录，不自动装）：
//! * GNU 目标：装 MinGW-w64（`winget install BrechtSanders.WinLibs.POSIX.MSVCRT`）后
//!   `windres` 进 PATH；或设 `RC=<windres 绝对路径>` 覆盖；
//! * MSVC 目标：装 "Visual Studio Build Tools" 的 **C++ 生成工具**（带 `rc.exe`），
//!   或设 `RC=<rc.exe 绝对路径>`。
//!
//! # `.rc` 为什么全是 ASCII
//!
//! `windres` 的 `-c 65001` 与 `rc.exe` 对非 ASCII 字面量的处理不完全一致
//! （前者按指定代码页，后者默认走系统 ANSI 代码页 + `/nologo` 的隐含行为），
//! 中文写进资源段在不同机器上会出现乱码。**文件属性里的字符串就按 ASCII 写**，
//! 中文界面文案在 `ui_text` 里（`about` 对话框），不进 PE 资源。

use std::env;
use std::fs;
use std::path::PathBuf;

/// 应用图标（多尺寸 16/32/48/64/128/256，见 `assets/` 与生成脚本说明）。
const APP_ICON: &str = "assets/crystal-studio.ico";

fn main() {
    // 资源变了就重跑（否则换了图标不重新链接，`cargo build` 会"绿着"给你旧图标）
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={APP_ICON}");
    println!("cargo:rerun-if-changed=assets/crystal-studio.png");

    // 非 Windows 目标没有 PE 资源这回事（`miredit-web` 之类的 Linux 构建不受影响）
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("[构建] 读不到 CARGO_MANIFEST_DIR"),
    );
    let version =
        env::var("CARGO_PKG_VERSION").expect("[构建] 读不到 CARGO_PKG_VERSION（版本真源）");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("[构建] 读不到 OUT_DIR"));

    let icon = manifest_dir.join(APP_ICON);
    if !icon.is_file() {
        panic!(
            "[构建] 图标不存在：{} —— 没有它就只能产出默认图标的 exe，故直接失败",
            icon.display()
        );
    }

    // ① 现生成 .rc（版本号现算；路径统一用 `/`，windres/rc.exe 都认）
    let rc_path = out_dir.join("crystal-studio.rc");
    let rc_text = rc_text(&version, &slash_path(&icon));
    if let Err(e) = fs::write(&rc_path, &rc_text) {
        panic!("[构建] 写 {} 失败：{e}", rc_path.display());
    }

    // ② 编译并**显式校验结果**（不能用 `manifest_optional()` 把 NotAttempted 当成功）
    let result = embed_resource::compile(&rc_path, embed_resource::NONE);
    match result {
        embed_resource::CompilationResult::Ok => {}
        other => panic!(
            "[构建] 资源编译失败（{}）：{other}\n\
             exe 会因此没有图标与版本信息，按本项目纪律不许当成成功。\n\
             本机是 *-pc-windows-gnu ⇒ 需要 PATH 上有 windres（MinGW-w64）；\n\
             MSVC 目标 ⇒ 需要 VS Build Tools 的 rc.exe。\n\
             补救：装好后重跑，或用环境变量 RC=<资源编译器绝对路径> 指定。",
            rc_path.display()
        ),
    }
}

/// 把 Windows 路径里的 `\` 换成 `/`（`.rc` 里的字符串不做反斜杠转义时最省事）。
fn slash_path(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// 日期版本 → Windows `FILEVERSION` 的四个 `u16`。
///
/// ⚠ 为什么不能直接把三段 parse 成 u16（2026-11-07 用户改用日期版本后暴露）：
/// `20261107 > u16::MAX(65535)`，`parse::<u16>()` 会**静默失败再兜成 0** ⇒
/// exe 属性里数值版本显示 `0.0.0.0`，字符串版本却是 `20261107.0.0`，自相矛盾。
/// 而 Windows 资源只给四个 u16，装不下 8 位日期。
///
/// # 现行方案（2026-11-08 用户拍板：`年.月.日` + 同日字母）
///
/// 用户要的形态是 `2026.11.08a`，但 cargo 两种写法都拒：
/// * `2026.11.8a`  ⇒ `unexpected character 'a' after patch version number`
/// * `2026.11.08a` ⇒ `invalid leading zero in patch version number`（报的是**前导零**，不是字母）
///
/// 所以同日字母走 **build metadata**：`2026.11.8+a`。本函数把字母折算成**第四段**：
///   `2026.11.8`   ⇒ `[2026, 11, 8, 0]`（不带字母；本仓发布一律带字母，此档仅为兼容/测试）
///   `2026.11.8+a` ⇒ `[2026, 11, 8, 1]`（当天第 1 个发布）
///   `2026.11.8+b` ⇒ `[2026, 11, 8, 2]`（当天第 2 个）
///   … `+z` ⇒ 26；换天 ⇒ `<新日期>+a` ⇒ 第四段回到 1
/// 这样同一天多次发布在**数值版本上也能区分且单调**（a < b < c），
/// 字符串版本仍用完整的 `CARGO_PKG_VERSION`（`2026.11.8+a`），两者各司其职。
///
/// # 历史形态（原样保留，不清理）
///
/// * 8 位日期 `<YYYYMMDD>.<轮次>.<修正>`（2026-11-07 ~ 2026-11-08 用过）：把 8 位日期
///   拆成「年(4) / 月日(4)」两个 u16，后两段照放：
///   `20261107.1.2` ⇒ `[2026, 1107, 1, 2]`；`20261108.0.0` ⇒ `[2026, 1108, 0, 0]`。
///   数值版本既不溢出、又单调可比（同一年内日期递增 ⇒ 第二段递增）。
/// * 旧 `0.x.y`（历史 tag 仍可能从源码构建）：原样放 `[0, x, y, 0]`，不 panic、不造假数字。
fn version_quad(v: &str) -> [u16; 4] {
    let mut parts = v.split(['.', '-', '+']);
    let first = parts.next().unwrap_or("0");
    let second = parts.next().unwrap_or("0");
    let third = parts.next().unwrap_or("0");
    // 第三段之后的那一段：`2026.11.8+a` 的 `a`（构建元数据；`-rc1` 之类预发布也落这里，本仓不用）
    let fourth = parts.next().unwrap_or("");

    let to_u16 = |s: &str| -> u16 {
        // 超 u16 的段一律折成 0，并在构建日志里如实说明（不静默）
        match s.trim().parse::<u16>() {
            Ok(n) => n,
            Err(_) => {
                println!("cargo:warning=[构建] 版本段 '{s}' 超出 u16，数值版本对应位记 0；字符串版本仍为 {v}");
                0
            }
        }
    };

    if first.len() == 8 && first.bytes().all(|b| b.is_ascii_digit()) {
        // 历史 8 位日期：拆「年 / 月日」，第四段仍是原来的修正段
        let year = &first[..4];
        let month_day = &first[4..];
        return [
            to_u16(year),
            to_u16(month_day),
            to_u16(second),
            to_u16(third),
        ];
    }
    // 新方案 `年.月.日[+字母]`：第四段承载当天字母序号；旧 `0.x.y` 没有附加段 ⇒ 第四段 0
    [to_u16(first), to_u16(second), to_u16(third), letter_ordinal(fourth, v)]
}

/// 当天字母 ⇒ 序号：`a` = 1、`b` = 2 … `z` = 26；空串（没有附加段）= 0。
///
/// 其它形态（大写、多字符、`+g<sha>` 之类）一律记 0，并在构建日志里如实说明 ——
/// 与上面的 `to_u16` 同一纪律：**宁可显示 0 并告警，也不静默编一个数字**。
fn letter_ordinal(meta: &str, full_version: &str) -> u16 {
    if meta.is_empty() {
        return 0;
    }
    let bytes = meta.as_bytes();
    if bytes.len() == 1 && bytes[0].is_ascii_lowercase() {
        return u16::from(bytes[0] - b'a') + 1;
    }
    println!(
        "cargo:warning=[构建] 版本号 {full_version} 的附加段 '{meta}' 不是单个小写字母 a-z，\
         数值版本第四段记 0；字符串版本仍为 {full_version}"
    );
    0
}

/// 生成 `.rc` 文本（纯函数；不含任何写死的版本号）。
///
/// ⚠ `cargo test` **不会**编译 `build.rs` 的 `#[cfg(test)]` 段（构建脚本只在构建期以
/// `main()` 跑，不进测试 harness），所以本文件的测试要用 `rustc --test` 直接跑 ——
/// 命令写在文件末尾 `mod tests` 上方；只写测试不跑，等于假装测过。
/// `rc_text` 自身的真判据仍是**构建出来的 exe**：图标能被 `ExtractAssociatedIcon` 取到、
/// 文件属性里能读到 `ProductName` / `ProductVersion` 与数值 `FILEVERSION`。
fn rc_text(version: &str, icon_path: &str) -> String {
    let [a, b, c, d] = version_quad(version);
    format!(
        r#"// 构建期自动生成（crates/miredit-gui/build.rs）—— 不要手改，也不要提交进仓库。
// 版本号来源：CARGO_PKG_VERSION = {version}

1 ICON "{icon}"

1 VERSIONINFO
FILEVERSION {a},{b},{c},{d}
PRODUCTVERSION {a},{b},{c},{d}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "CrystalStudio-Rust"
            VALUE "FileDescription", "Crystal Studio map editor (Rust port)"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "miredit-gui"
            VALUE "LegalCopyright", "MIT License"
            VALUE "OriginalFilename", "miredit-gui.exe"
            VALUE "ProductName", "Crystal Studio"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#,
        version = version,
        icon = icon_path,
        a = a,
        b = b,
        c = c,
        d = d,
    )
}

// ───────────────────────────── 单元测试 ─────────────────────────────
//
// ⚠ **`cargo test` 不会跑这里的测试**：`build.rs` 是构建脚本，只在构建期以 `main()`
// 运行，`#[cfg(test)]` 段根本不参与编译。要真跑，得把本文件当成一个测试 crate 直接喂给
// `rustc --test`（`main()` 用到 build-dependency `embed_resource`，必须显式给 rlib 与
// 依赖搜索路径，否则报 `can't find crate for 'cc' which 'embed_resource' depends on`）：
//
//   $deps = "<CARGO_TARGET_DIR>\debug\deps"
//   $rlib = (Get-ChildItem $deps -Filter 'libembed_resource-*.rlib')[0].FullName
//   rustc --edition 2021 --test -L "dependency=$deps" --extern "embed_resource=$rlib" `
//         -o I:\cargo\tmp\buildrs_tests.exe crates/miredit-gui/build.rs
//   & I:\cargo\tmp\buildrs_tests.exe
//
// 2026-11-08 实测（CARGO_TARGET_DIR = I:\cargo\scratch\miredit-ver-20261108-192258）：
// `test result: ok. 7 passed; 0 failed`。
#[cfg(test)]
mod tests {
    use super::version_quad;

    /// 新方案、不带字母 ⇒ 第四段 0。
    #[test]
    fn new_scheme_without_letter() {
        assert_eq!(version_quad("2026.11.8"), [2026, 11, 8, 0]);
    }

    /// 新方案、当天第 1 个（`+a`）⇒ 第四段 1。
    #[test]
    fn new_scheme_first_of_day() {
        assert_eq!(version_quad("2026.11.8+a"), [2026, 11, 8, 1]);
    }

    /// 新方案、当天第 2 个（`+b`）⇒ 第四段 2（与 `+a` 在数值版本上可区分）。
    #[test]
    fn new_scheme_second_of_day() {
        assert_eq!(version_quad("2026.11.8+b"), [2026, 11, 8, 2]);
    }

    /// 历史 8 位日期方案必须原样保留：`20261108.0.0` ⇒ `[2026, 1108, 0, 0]`。
    #[test]
    fn legacy_eight_digit_date() {
        assert_eq!(version_quad("20261108.0.0"), [2026, 1108, 0, 0]);
    }

    /// 更早的 `0.x.y` 必须原样保留：`0.7.0` ⇒ `[0, 7, 0, 0]`。
    #[test]
    fn legacy_zero_xy() {
        assert_eq!(version_quad("0.7.0"), [0, 7, 0, 0]);
    }

    /// `+z` 是当天第 26 个（字母序号上限）。
    #[test]
    fn new_scheme_last_letter_of_day() {
        assert_eq!(version_quad("2026.11.8+z"), [2026, 11, 8, 26]);
    }

    /// 附加段不是单个小写字母（如 `+g4639dd7`）⇒ 第四段 0，并打 `cargo:warning`（不静默编数字）。
    #[test]
    fn unexpected_build_metadata_falls_back_to_zero() {
        assert_eq!(version_quad("2026.11.8+g4639dd7"), [2026, 11, 8, 0]);
    }
}
