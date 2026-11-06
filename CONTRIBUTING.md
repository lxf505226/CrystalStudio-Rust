# 贡献指南

本仓是 **Crystal Studio** 地图编辑器的 Rust 移植。改动请遵守下面的门禁与边界。

## 一、环境要求

- Rust stable（`edition = "2021"`，workspace 8 个 crate）。
- `crates/mir2-assets` 通过 **path 依赖**引用 `wil2atlas`（WIL/WZL 解码，本仓不做二进制解析复制）。
  当前清单里写的是开发机绝对路径，克隆后请先把它指向你自己的 `wil2atlas` 副本（或用发布版依赖替换）。
- 游戏素材（`.map`、`.wil/.wix`、`.wzl/.wzx`、`.lib`、`Objects/*.X`）**不入库**，见第四节。

## 二、构建与门禁

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

提交前至少跑一次 `cargo clippy --all-targets -- -D warnings`（要求 0 warning），并在 PR 里贴实际输出。
全量测试耗时较长时可先跑受影响的 crate（`cargo test -p <crate>`），但合入前要跑一次全量。

## 三、代码风格

- **不跑 `cargo fmt`**：本项目明确禁用自动格式化，仓库现有排版即风格基准（缩进、换行、注释位置都按现状延续）。
- 注释与文档**全中文**，标识符保持英文。
- 不引入新依赖除非有明确理由；`mir2-format` 的零第三方依赖是**刻意约束**，不要往里加 serde 之类的库。
- 生产代码不用 `unwrap()` / `expect()` 处理可恢复错误；错误信息要带上下文，不要吞掉。
- 数值（字节布局、常量、阈值、alpha、尺寸）一律回源上游；上游确实不可得时显式标注「原版不可得 ⇒ 自定值」并写明依据，**禁止静默自造数值**。

## 四、素材与本地测试

- **不要提交任何游戏素材**：地图、图库、印章、客户端资源、截图与渲染产物都不进仓库。
- 网页版相关根目录用环境变量指定：`MIREDIT_MAP_ROOT`、`MIREDIT_CLIENT_ROOT`、`MIREDIT_PALETTE_JSON`、`MIREDIT_WEB_ROOT`（见 [README.md](README.md) 第四节）。
- 依赖素材的测试在目录缺失时**多数**会 `eprintln!("跳过：…")` 后直接返回（例：`crates/mir2-assets/tests/acceptance.rs`），
  因此没有客户端素材也能跑通大部分套件。但**不保证全部**测试都这么处理：缺素材导致的失败请按 README 配好根目录，
  **不要**为了变绿去放宽断言或删除测试。

## 五、提交与 PR

1. Fork 本仓，从 `main` 拉分支（`feat/…`、`fix/…`、`docs/…`）。
2. 一个 PR 只做一件事；涉及多个 crate 时说明依赖顺序。
3. 提交信息写清「改了什么 / 为什么 / 验证方式」，涉及上游对齐时带 `文件:行号@提交号`。
4. 提 PR 前自查：
   - `cargo clippy --all-targets -- -D warnings` 通过；
   - 没有提交 token / 密钥 / 素材 / `target/` 等构建产物；
   - 文档与代码同步（接口契约、格式规格、README）。
5. 行为变更请同时更新 `docs/` 下对应文档；被推翻的结论就地标注「作废 + 原因」，不要删历史结论。

## 六、许可

贡献即视为同意以 [MIT](LICENSE) 许可发布。
