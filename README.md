<div align="center">

# QuotaKeeper

终端与 Windows 桌面上的 AI coding 用量监测台:订阅配额、token 账本、燃烧率、热力图。本地优先,不经过任何中转服务器。

![GUI 配额页](assets/screenshots/gui-overview.png)

[![CI](https://github.com/jryang1997/QuotaKeeper/actions/workflows/ci.yml/badge.svg)](https://github.com/jryang1997/QuotaKeeper/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![TUI Rust 1.88+](https://img.shields.io/badge/TUI%20Rust-1.88%2B-orange)
![Platform: Windows 优先](https://img.shields.io/badge/platform-Windows%20first-lightgrey)

</div>

## 它做什么

面向重度 AI coding CLI 用户(Claude Code、Codex、OpenCode、z.ai GLM、Kimi、DeepSeek 等):配额从各服务的官方/登录态接口直采,token 账本读本机会话文件,美元估价用 LiteLLM 公开价目(后台每 24 小时刷新,离线复用旧缓存)。全部数据留在本机,无遥测、无上报。

Rust 单二进制 TUI(ratatui)+ Windows 桌面壳(WebView2):

- **配额页** — 每家订阅一张卡,5h/周/月窗口 + █░ 进度条 + 按窗口内匀速外推的预计耗尽时间;任一额度跨越 80% / 100% / 重置弹系统 toast(状态机去重,夜间静默可关)。
- **用量页** — 近 60 分钟活动、今日 / ISO 周 / 本月汇总、Agent / 会话 / 项目 / 模型四维拆分、24h / 14d 堆叠趋势图、月度 API 等值 vs 订阅月费基准、CSV / JSON 导出(桌面版)。
- **热力图页** — 近 26 周 GitHub 风格贡献格 + 工作时段 Profile(清晨/白天/傍晚/深夜 + 星期分布 + 峰值小时)。
- **桌面版(GUI)** — 四页:**配额**(全部服务商额度带 + 每张卡的窗口/重置/耗尽预测)、**用量**(时间段摘要 + 趋势/模型/热力图等 20 个可编辑模块 + **成就墙**)、**分享**(分享卡 PNG + 局域网看板)、**设置**(外观 / 数据源与凭据 / 桌面与通知 / 基准与货币 / 关于);外加置顶三环悬浮窗、托盘驻留与成就解锁提示。
- 中英双语(`l`)、四套主题(`p`)、60s 自动刷新;每家配额独立采集,一家失败不影响别家,账本纯本地、离线可用。

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
| Kimi Coding(开发中) | coding usage 接口 | `KIMI_CODE_API_KEY`(Kimi For Coding key,不是 Moonshot 开放平台 key) |

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

### TUI

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
- 主题 `p` 循环(跟随系统/浅色/深色/深海/暖纸),语言 `l` 切换,均持久化在 `~/.quotakeeper/`。
- 交互:`1·2·3·4` 切页(总览/配额/用量/热力图),总览页 `←→` 切周期(今日/7天/30天/90天/全部),`↑↓/kj/Tab` 翻页;用量页 `s` 切维度、`g` 切表格/趋势、`t` 切 24h/14d、`c` 切 Token/金额;`?` 全部快捷键;`r` 刷新;`q` 退出。
- 终端字体推荐等宽中文字体(如 Sarasa Term SC / 更纱黑体),进度条与中文对齐不跑版。

## 隐私

- 全部数据留本机,无遥测、无上报、无账号体系。
- API key 只进 Windows 凭据管理器,不落 settings.json、不进前端存储。
- 不修改任何 CLI 的自有状态(登录态文件只读;不代刷 OAuth token)。

## 验证

```sh
cargo test                                    # fixture 回放 / 内存 SQLite / 多帧 zstd / 时间边界 / 图表命中 / 无头渲染
cargo test --manifest-path gui/Cargo.toml     # GUI 壳测试
npm --prefix gui/frontend test                 # GUI 布局、偏好与通信契约
cargo run --example ledger_smoke              # 对本机真实数据源跑一遍 ledger,打印汇总
cargo run --example quota_smoke               # 对本机真实凭据跑一遍配额快照
```

## 已知口径

- Token 页 token 计数(本地)与配额页百分比(服务商)不同源,对不上是正常的。
- 周一为每周起点;日界按本地时区;input/output/cache_read/cache_write 四个计数桶互不重叠,reasoning 已包含在 output 中,仅单独展示。
- 美元估价为 LiteLLM 第三方公开价目,未逐项由厂商核验,不反映时段/上下文档位/订阅折扣;OMP 来源报告价与估价分开记录。
- 消耗的缓存桶缺少单价时,先用厂商官方条目补缺桶(渠道价完整的桶保留),仍缺才标记未估价、不按免费处理;裸名与后缀歧义的模型回落厂商官方条目(zai/deepseek/gemini/moonshot/minimax 五族),推广短名经反向别名取价(如 deepseek-flash ↔ deepseek-v4.1-flash)。后缀众数/家族兜底仍是近似值,未逐项由厂商核验。初次无缓存时先显示本地 token 与补充价,后台下载后下一次刷新补全估价。
- 所有账本视图与导出只统计采集时刻之前的记录。CSV 对公式前缀文本加单引号以供电子表格查看,JSON 保留原文;重新保存 CSV 后仍需遵循所用电子表格的导入规则。
- chrono 钉 0.4.38:0.4.40+ 的 Windows Local 走 raw-dylib,在 WinLibs GCC 的 windows-gnu 目标上首次调用即崩(见根 Cargo.toml 注释)。

## 致谢与来源

项目的配额采集与展示调研参考了 [CodexBar](https://github.com/steipete/CodexBar)、[CodeBurn](https://github.com/getagentseal/codeburn)、[tokscale](https://github.com/junhoyeo/tokscale) 和 [ccusage](https://github.com/ccusage/ccusage) 的公开实现与文档。具体数据来源及取舍见 [研究记录](docs/research/);品牌图标来源见 [图标清单](gui/frontend/public/logos/README.md)。

最近一次[代码审计](docs/research/codebase-audit-2026-10-01.md)与[竞品成熟度对标](docs/research/codebase-audit-landscape-2026-10-01.md)记录了已修复问题和后续补强顺序。

## License

[MIT](LICENSE)
