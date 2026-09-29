# 贡献指南

## 开发环境

- Rust 1.85+(edition 2024;`rust-toolchain.toml` 会自动装 stable 与 windows-gnu target)。
- GUI 需要 Node.js(gui/frontend,Vite + React 19)。

```sh
cargo test                                   # 核心 crate,113 项测试
cargo test --manifest-path gui/Cargo.toml    # GUI 壳测试,11 项
cd gui/frontend && npm ci && npm run build   # GUI 前端构建
```

## 提交规矩

- 提交信息:英文 type + 中文正文,如 `feat(quota): Claude Pro/Max subscription card` + 正文说明动机与边界。
- 一轮改动一个主题;不夹带无关格式化。
- type:`feat` / `fix` / `docs` / `chore` / `ci` / `refactor`。

## 代码约定

- 新数据源(账本)放 `src/ledger/<name>.rs`,实现 `scan_paths` + `parse_file`(或 SQLite `db_path` + `collect`),在 `ledger/mod.rs` 注册;解析必须带 schema 校验,坏数据宁可整源报错也不显示 0% 误报。
- 新配额卡放 `src/<name>_quota.rs`,产出 `model::Report`;在 `model::ProviderId::all()`、`backend::fetch_snapshot`、`gui/src/ipc.rs` 三处接线,Missing 卡必须给出准确的双语修复提示(`missing_hint`)。
- 所有出站 HTTP 走 `crate::http::get/post`(统一系统代理),必须带超时。
- UI 文案集中在 `src/ui/i18n.rs`,中英双语缺一不可。
- 不修改用户 CLI 的自有状态文件;一切登录态只读。

## 文档

- 涉及第三方接口的改动,先在 `docs/research/` 落一手调研记录(带引用 URL),再动代码;命名用 kebab-case。
- 面向用户的口径变化(定价、窗口、导出格式)同步更新 README 与「已知口径」。

## 报告问题

Bug 请附 `cargo run --example ledger_smoke` 的输出(会脱敏,仅计数与汇总);安全问题走 [SECURITY.md](SECURITY.md) 的私密通道。
