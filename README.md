<div align="center">

# QuotaKeeper

Windows 上的 AI 编程用量监测。打开就能看到订阅还剩多少额度、本机用了多少 Token。只读采集，不改 Agent 配置。

![GUI 配额页](assets/screenshots/gui-overview.png)

[![CI](https://github.com/jryang1997/QuotaKeeper/actions/workflows/ci.yml/badge.svg)](https://github.com/jryang1997/QuotaKeeper/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![TUI Rust 1.88+](https://img.shields.io/badge/TUI%20Rust-1.88%2B-orange)
![Platform: Windows 优先](https://img.shields.io/badge/platform-Windows%20first-lightgrey)

</div>

## 它做什么

面向重度 AI coding CLI 用户(Claude Code、Codex、OpenCode、z.ai GLM、Kimi、DeepSeek 等):配额从各服务的官方/登录态接口直采,token 账本读本机会话文件,美元估价用 LiteLLM 公开价目(后台每 24 小时刷新,离线复用旧缓存)。全部数据留在本机,无遥测、无上报。

桌面版使用 Rust 和 WebView2，主导航只有订阅额度、Token 用量和设置。

- **订阅额度**：每个服务商一张卡，显示全部额度窗口，默认看剩余，也可切换已用。重置时间与套餐到期分开显示。旧读数标明读取时间，过了预定重置时刻显示待刷新；未接入和已隐藏的服务集中展开。
- **Token 用量**：首屏有本机今日、本周、本月摘要；明细页按日期与采集源统一筛选总量、请求数、API 等值参考、趋势、输入输出和缓存构成。支持 CSV、JSON 导出。API 等值不是订阅账单，缺价记录单独标注。
- **桌面常驻**：托盘、置顶额度悬浮窗、80% 和耗尽及重置通知、套餐到期提醒。设置中可开启局域网只读额度页。
- 中英双语，主题仅保留跟随系统、浅色、深色和卡带未来。服务商配额每 60 秒刷新，本机账本每 5 秒刷新，两条采集互不阻塞。一家服务失败不影响其他服务，账本离线可用。

产品不再包含成就墙、分享图片、自由拖拽模块、工作习惯分析和 CLI 安装升级。取舍与 Magpie 一手资料见[产品收口研究](docs/research/product-focus-2026-10-15.md)。

## 数据源

### 配额(独立采集)

| 卡 | 路径 | 凭据来源 |
|---|---|---|
| OpenCode Go | 官方 usage API | Windows 凭据管理器 / `OPENCODE_GO_API_KEY` / OpenCode 登录态 |
| Codex | 官方 `app-server`(回退:wham 接口 → OMP 兜底) | Codex CLI 登录态(`CODEX_HOME` 可重定向) |
| Command Code GOAT | billing API | Windows 凭据管理器 / `COMMAND_CODE_API_KEY` / `cmdc login` 登录态 |
| Antigravity | 经 [CPA](https://github.com/luispater/CLIProxyAPI) 代查 Google 上游配额 | `~/.quotakeeper/cpa-key` |
| Claude Pro/Max | 官方 `oauth/usage` 接口 | Claude Code 登录态(`~/.claude/.credentials.json`) |
| Cursor | usage-summary(未文档化接口) | Cursor 登录态(state.vscdb / `cursor-agent login`) |
| GLM Coding | z.ai / bigmodel monitor 接口 | `GLM_API_KEY`;国际站另设 `GLM_API_BASE=https://api.z.ai` |
| Kimi Coding | coding usage 接口 | `KIMI_CODE_API_KEY`（Kimi For Coding key，不是 Moonshot 开放平台 key） |
| MiniMax Coding | 官方 coding_plan remains 接口 | Windows 凭据管理器 `TuiTokenMonitor/MiniMax` / `MINIMAX_CODING_API_KEY` |

GLM/Kimi 的设置页 key 输入尚未接入,设置环境变量后需重启应用。DeepSeek API 余额另作余额卡展示,与订阅配额不同。

有订阅周期的卡(GOAT / Codex / Cursor)会显示**订阅到期日**,到期前 7/3/1 天分级弹系统提醒(档位去重,续费后静默)。各家的订阅期可得性依据见[订阅周期数据来源调研](docs/research/subscription-period-sources.md)。

### Token 账本(六源,纯本地)

| Agent | 来源 | 成本口径 |
|---|---|---|
| Codex | `~/.codex/sessions/**/rollout-*.jsonl`(`last_token_usage` 增量) | LiteLLM 估价 |
| OMP | `~/.omp/stats.db` `messages`(官方预聚合) | 来源报告价 |
| OpenCode | `~/.local/share/opencode/opencode.db`(v1/v2 双 schema) | LiteLLM 估价 |
| ZCode | `~/.zcode/cli/db/db.sqlite` `model_usage` | LiteLLM 估价 |
| DeepSeek Harness | `~/.dsh/sessions/**/session*.jsonl.zstd`(多帧 zstd) | LiteLLM 估价 |
| Claude Code | `~/.claude/projects/**/*.jsonl`(`(message.id, requestId)` 去重) | LiteLLM 估价 |

> **未文档化接口声明**:Codex wham、GOAT `/alpha/billing`、Google `retrieveUserQuotaSummary` 为无稳定性承诺的内部接口,随时可变;失败时各自降级为 Missing 卡,不影响其他卡与本地账本。一手调研记录见 [docs/research/](docs/research/)。

外网请求遵循 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` 环境变量。环回地址(含本机 CPA)强制直连且不跟随重定向,管理密钥不会发给环境代理。

## 安装

### TUI（既有版本，功能冻结）

前置:Rust 1.88+(edition 2024)。

```sh
cargo install --path .        # 或 cargo build --release,产物在 target/release/
```

Windows 默认走 MSVC 工具链(需 Visual Studio Build Tools);本仓库 CI 同时验证 GNU 工具链路径,固化 GNU 的方法见 `.cargo/config.toml` 内注释。

### Windows 桌面版(GUI)

前置:Rust 1.90+、Node.js。

```sh
cd gui/frontend && npm ci && npm run build && cd ../..
cargo build --release --manifest-path gui/Cargo.toml
run_gui.bat                   # GNU 构建自动把 WebView2Loader.dll 复制到程序旁
```

需要 WebView2 Runtime(Win10/11 自带)。前端产物经 `include_dir!` 内嵌进二进制,分发只需 exe(+DLL)。双击 `run_tui.bat` / `run_gui.bat` 都可用;GUI 关窗最小化到托盘,可退出。GUI exe 内嵌应用图标,也可运行 `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/new-gui-shortcut.ps1` 生成桌面直启快捷方式(直指 exe,无控制台窗)。

## 平台支持

| | TUI | 桌面 GUI |
|---|---|---|
| Windows | ✅ 第一公民(MSVC / GNU 双验证) | ✅ WebView2 |
| macOS / Linux | ✅ 编译 + 账本全量可用;配额卡仅支持环境变量与 CLI 登录态凭据,无系统 toast | ❌ |

## 配置与键位

- 配额凭据按上表配置即可,缺哪张卡,卡片会显示准确的缺失原因与修复途径。
- 主题和语言在设置中调整；桌面 `p` 循环四种主题。已删除主题的本机偏好自动回退为跟随系统。
- 桌面快捷键：`1` 订阅额度、`2` Token 用量、`3` 设置，`Ctrl+K` 快速跳转，`r` 刷新，`p` 切主题。TUI 保留原键位，按 `?` 查看。
- 终端字体推荐等宽中文字体(如 Sarasa Term SC / 更纱黑体),进度条与中文对齐不跑版。

## 隐私

- 全部数据留本机,无遥测、无上报、无账号体系。
- API key 只进 Windows 凭据管理器,不落 settings.json、不进前端存储。
- 不修改任何 CLI 的自有状态(登录态文件只读;不代刷 OAuth token)。
- 局域网展示默认关闭；开启后，同一局域网的设备可以读取剩余额度，接口不提供密钥。

## 验证

```sh
cargo test                                    # fixture 回放 / 内存 SQLite / 多帧 zstd / 时间边界 / 图表命中 / 无头渲染
cargo test --manifest-path gui/Cargo.toml     # GUI 壳测试
npm --prefix gui/frontend test                 # 额度边界、范围筛选、偏好与金额格式
cargo run --example ledger_smoke              # 对本机真实数据源跑一遍 ledger,打印汇总
cargo run --example quota_smoke               # 对本机真实凭据跑一遍配额快照
```

## 已知口径

- Token 页 token 计数(本地)与配额页百分比(服务商)不同源,对不上是正常的。
- 周一为每周起点;日界按本地时区;input/output/cache_read/cache_write 四个计数桶互不重叠,reasoning 已包含在 output 中,仅单独展示。
- API 等值参考使用 LiteLLM 第三方公开价目和来源报告价，不反映实际订阅扣款、时段或上下文档位及折扣。缺价记录不按免费处理。
- 消耗的缓存桶缺少单价时,先用厂商官方条目补缺桶(渠道价完整的桶保留),仍缺才标记未估价、不按免费处理;裸名与后缀歧义的模型回落厂商官方条目(zai/deepseek/gemini/moonshot/minimax 五族),推广短名经反向别名取价(如 deepseek-flash ↔ deepseek-v4.1-flash)。后缀众数/家族兜底仍是近似值,未逐项由厂商核验。初次无缓存时先显示本地 token 与补充价,后台下载后下一次刷新补全估价。
- 所有账本视图与导出只统计采集时刻之前的记录。CSV 对公式前缀文本加单引号以供电子表格查看,JSON 保留原文;重新保存 CSV 后仍需遵循所用电子表格的导入规则。
- chrono 钉 0.4.38:0.4.40+ 的 Windows Local 走 raw-dylib,在 WinLibs GCC 的 windows-gnu 目标上首次调用即崩(见根 Cargo.toml 注释)。

## 致谢与来源

项目的配额采集与展示调研参考了 [CodexBar](https://github.com/steipete/CodexBar)、[CodeBurn](https://github.com/getagentseal/codeburn)、[tokscale](https://github.com/junhoyeo/tokscale) 和 [ccusage](https://github.com/ccusage/ccusage) 的公开实现与文档。具体数据来源及取舍见 [研究记录](docs/research/);品牌图标来源见 [图标清单](gui/frontend/public/logos/README.md)。

最近一次[代码审计](docs/research/codebase-audit-2026-10-01.md)与[竞品成熟度对标](docs/research/codebase-audit-landscape-2026-10-01.md)记录了已修复问题和后续补强顺序。

## License

[MIT](LICENSE)
