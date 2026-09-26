# AI 编程工具用量监测项目对标调研

调研日期：2026-09-25  
范围：GitHub 上面向 AI 编程工具的本机 token/成本统计、订阅额度监测和终端/桌面面板。功能事实以各项目自己的 GitHub README、文档或源码为准；这是一轮产品对标，不代表对每个仓库逐行审计。

后续决定（2026-09-26）：用户选择先实现“最近在用什么”和更丰富的图表；数字可信性不作为 UI 展示主题，保留简短的来源错误提示即可。本轮图表对标见[图表表达研究](chart-expression.md)。以下原始建议保留作调研记录。

## 结论

这个项目已经把几类通常分开的需求放进一个 Windows 终端面板：看订阅额度、看各编程助手产生的 token 用量、回看趋势和收到额度提醒。当前最有辨识度的组合是 **Windows 优先的 TUI + 多家订阅额度卡 + 六种本机使用记录来源 + 热力图**。本轮抽样中，其他工具有的侧重额度、有的侧重历史报表、有的侧重实时会话；这个项目可以继续把“现在发生了什么、额度还能撑多久、这些数字靠不靠谱”合在同一块屏幕上。

下一阶段不建议只追“支持多少个工具”。最值得形成记忆点的是：把最近的本机活动、真实观测到的用量速率、服务商额度余量和数字可信程度放在一起。这样用户不只知道“用了多少”，还能看懂“刚才哪个工具用得多、额度大概还能撑多久、这条数据是不是新鲜”。

## 当前项目的基线

依据本仓库 [README](../../README.md)、`src/backend.rs`、`src/model.rs`、`src/ledger/` 和 `src/ui/`：

| 能力 | 当前实现 |
|---|---|
| 订阅额度 | OpenCode Go、Codex/ChatGPT Plus、Command Code GOAT、Antigravity；窗口百分比、重置倒计时、额度线性耗尽时间估算、卡片/同心环双视图。OMP 不可用时 Codex 有原生回退。 |
| 额度提醒 | 80%、100% 和重置时发 Windows 通知，并做重复提醒去重。 |
| 本机用量记录 | Codex、OMP、OpenCode、ZCode、DeepSeek Harness、Claude Code 六路采集；按 Agent、会话、项目汇总，可查看每日数据和明细。 |
| 花费口径 | OMP 来源报告的价格与 LiteLLM 模型单价估值分开累计；没有价格的模型不虚构金额。 |
| 活动分析 | 26 周热力图、按日下钻，以及最近 30 天的工作时段分析。 |
| 体验 | 中文/英文、四套主题、鼠标和键盘操作、每分钟刷新；Windows 桌面快捷方式和 `run_tui.bat`。 |
| 当前边界 | Antigravity 卡显示 CPA 凭证健康与 token 强度，不是官方 credits 百分比；各服务额度与本机 token 记录来自不同口径。项目 README 已解释这点。 |

这不是一张普通的 token 计数器：一部分数据来自服务商额度接口，一部分来自编程工具本机留下的会话记录。二者并排展示本身就是产品价值，前提是把来源和准确程度继续讲清楚。

## 值得参考的项目

| 项目 | 它实际提供什么 | 可以借鉴 | 适用边界 |
|---|---|---|---|
| [Token Use](https://github.com/russmckendrick/tokenuse) | 与本项目最接近：Rust + Ratatui TUI 为默认入口，另有跨平台 Tauri 桌面壳；记录本地会话到 SQLite 归档，支持按工具、项目、模型、会话和活动看成本。还提供用量页、`doctor` 自检命令、MCP 和 HTML/PDF/SVG/PNG/JSON/Excel/CSV 报告。当前 README 列出 Claude Code、Cursor、Codex、GitHub Copilot 四类来源；Claude/Codex 额度同步是用户主动触发的可选项。 | 多页信息架构、统一归档、来源自检、可导出报告；报告导出能让用户在终端外继续分析。 | 项目要求终端至少约 120×40，小终端会显示调整提示；配额覆盖方式与本项目的 OpenCode Go、GOAT、Antigravity 卡片不同。适合作为同栈参考，不必照搬整套分析产品。 |
| [Tokscale](https://github.com/junhoyeo/tokscale) | Ratatui 交互界面，README列出 10 个常用视图（Overview、Usage、Models、Daily、Hourly、Monthly、Sessions、Projects、Stats、Agents）；支持大量编程助手的本地日志/数据库，并使用 LiteLLM 价格数据。另有本地可视化前端、2D/3D 热力图和公开排行榜提交功能。 | 新来源适配、热力图筛选和下钻、视图按“模型/小时/会话”等分析问题拆分；本地记录保留也值得了解。 | 本地统计广度很强，但公开资料提交是明确的用户主动功能；本项目应维持本机数据默认留在本机。跨平台依赖与视图数量也会增加维护面，没必要为数字而追求相同规模。 |
| [ccusage](https://github.com/ccusage/ccusage) | 本机数据统计 CLI，覆盖 Claude、Codex、OpenCode、Amp、Droid、Hermes、OpenClaw、Gemini、Antigravity、ZCode 等来源；提供日、周、月、会话报告、Claude 5 小时 blocks、Claude statusline、JSON 输出、日期区间、时区和项目筛选。 | 使用明确的命令/筛选概念设计机器可读导出；聚合规则、日期和时区选择值得对照。可作为新来源的行为参考。 | 以一次性命令和状态栏输出为主，不是多服务额度与可视化历史合一的 TUI 控制台。不要把它的报表入口重复造进主屏，借鉴数据口径和导出更合适。 |
| [CodexBar](https://github.com/steipete/CodexBar) | 多服务额度面板：macOS 菜单栏应用、Linux 桌面应用及 CLI，覆盖 Codex、Claude、Cursor、Gemini、Copilot、OpenRouter 等多类提供方；强调会话/周/月窗口、重置倒计时、余额/花费、服务状态和多种凭据来源。CLI 有卡片、JSON 和本地网页面板等入口。 | 每张额度卡的重置时间和状态提示、提供方分层接入、额度接口失败时的多路回退；把额度查看压缩到随手可见的位置。 | 主产品形态是 macOS 菜单栏，Linux 桌面端需要单独运行时；它不是 Windows 原生 TUI。它主打额度面板，本项目则把订阅额度与本机会话账本放在一起。 |
| [ccdash](https://github.com/jedarden/ccdash) | Bubble Tea 终端面板，把 Claude Code/Codex 的 token、成本、模型拆分、每分钟用量和“正在工作/等待提问/空闲”等会话状态放在一起；会话状态可从 tmux 面板观察，或由 hooks 提供。 | 实时会话状态是很好的方向参考；可以先用本项目已有的最近活动时间和近一小时用量做保守的“最近在忙什么”，确认价值后再研究 hook 或进程状态。 | 它着重 Claude/Codex 的终端运行状态和系统资源；本项目强项仍是多家额度与六类本机账本的整合。 |
| [Claude Code Usage Monitor](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor) | 面向 Claude Code 的 Rich 实时终端面板。新 README 提供 statusline 捕获的官方 `rate_limits`、过期时回退并标注为本地估算、版本化状态导出、燃烧率与会话耗尽预测、机器可读模式和可选用量存储。数据标注 `official`、`local_estimate`、`experimental`、`unknown` 等来源可信度。 | **强烈建议借鉴来源可信度标记**；预测不只给一个时间，也可显示估算/实验口径；为脚本留机器可读快照。 | 聚焦 Claude Code，不负责多平台账本和跨服务额度总览。预测和计划识别不应直接搬算法，需基于本项目能实际取得的数据。 |
| [Claude Code Usage](https://github.com/ClaudeCodeUsage/ClaudeCodeUsage) | Claude Code 和 Codex 的 VS Code 扩展：状态栏显示今日 token 与额度，仪表板按会话/项目/时间查看用量和缓存；把订阅额度、实际 token 和估算成本分开说明，并提供本地导出及可选择的建议功能。 | 对“估算不等于账单、额度读数可能只是本机最后一次观察”的说明很清楚；可参考其按项目/会话下钻和在操作前先展示数据范围的做法。 | 依赖 VS Code，专注 Claude Code/Codex；本项目可以保持编辑器无关，以 Windows TUI 汇总更多本机 Agent 和额度为主。 |
| [Token Monitor](https://github.com/Javis603/token-monitor) | Electron 桌面组件，README列出 40 多种工具的数据来源、额度和会话支持，并有 Windows 安装包/便携版；可按工具、设备、模型、会话、项目或账户查看。支持多个账户、订阅支出记录、CSV/JSON 导出、热力图保留，以及选择性多设备同步。默认单机模式不需要同步服务；同步需用户配置本机 hub、局域网/自建节点或 Cloudflare Worker。 | Windows 安装包、多个账户及订阅价格登记、跨设备前保留明确的单机默认、规范化 CSV/JSON 导出。 | 广度和设置项很多，基于 Electron，配置面比当前 TUI 大。同步不是本项目首要差异点；如未来需要 NAS 同步，优先研究不依赖付费托管的自建方案。 |

## 产品方向与优先级

建议先围绕三个主方向打磨，保留 Windows 本地 TUI 的中心位置。

### 1. “最近在用什么”：Right Now 总览

让用户一眼看到最近活跃的 Agent/会话、近一小时观测到的用量、最紧张的订阅额度及预计耗尽时间。`SessionRow.last_ts` 可支持“最近活动”，`LedgerReport::burn_rate` 已计算所有来源合计的过去 60 分钟 token/min，但目前未显示；它不是每个 Agent 的速率，若要按 Agent/会话回答“谁用了多少”，还需要增加分组统计。配额页现有的耗尽时间是从当前额度百分比和窗口已过时间做线性外推，并非用量账本的滚动速率。界面应分别标注“近一小时观测”和“额度线性估算”，并说明估算依据。

目前主界面年龄信息来自最近一次成功的额度读取；本机用量报表有生成时间，但还没有各数据源单独的最近读取时间。初版可把“额度数据更新于”和“用量统计生成于”分开显示。没有进程/hooks 证据时只写“最近活动”，不要说 Agent 此刻正在运行。ccdash 证明终端里同时看用量与会话状态有实际产品形态；本项目的差异点可以是把“最近活动 + 多家额度余量 + 跨 Agent 账本”合成一个 Windows TUI。

### 2. “这些数字可信吗”：来源说明与诊断

在额度卡和用量摘要附近说明数字来自服务商还是本机记录、是官方值还是模型单价估算、额度何时最后更新、哪些本机数据源失败、Codex 额度是否使用了原生回退。Antigravity 的代理 token 强度要与真实额度百分比使用不同图例。费用继续区分“来源报告价”和“模型价格估算”。

当前 `SourceStatus` 只有来源名称、记录数和可选错误；主界面有最近额度读取时间；本机账本有生成时间。缺少逐来源最近读取时间、回退链路、价格表更新时间和面向新手的“缺什么/下一步做什么”检查。因此可以把来源说明与 `doctor` 式检查合成一个可信度/诊断页面。Token Use 的 `doctor` 和 Claude Code Usage Monitor 的来源标签是交互与文案参考；ClaudeCodeUsage/ClaudeCodeUsage 则明确标出 Codex 额度是本地最后一次观察、成本属于 API 等价估算，不是账单。

### 3. Windows“额度守护”

现有通知已覆盖额度 80%、100% 和重置，并会去重。后续可谨慎加入额度数据过期/读取失败、预计耗尽时间过短等提醒，且在通知里指出是哪张额度卡或数据源需要关注。保留本机运行和 TUI 控制；先评估轻量 watch 模式是否够用，不要马上重写成常驻托盘应用。

### 次要待办

- **安装与更新**：补齐 GitHub Release 的便携包/安装包、首次启动检查和版本说明；Token Use 与 Token Monitor 都把原生安装包作为正式入口。
- **本机报告导出**：提供 JSON/CSV，覆盖日期、Agent、项目、模型、会话和成本口径，默认写到本地。Token Use/Token Monitor 有 CSV+JSON 等报告，ccusage 提供结构化 JSON；这是方便性补充，不需要改变 TUI 核心。
- **新增来源/账户**：一次接入一个有明确 Windows 用户需求的来源，并配齐数据路径、schema、计数规则、价格依据和错误场景。不要把支持工具数量当作质量目标。

### 暂不做：为了追平竞品而变成通用平台

近期不追求一次性覆盖更多服务、云同步、排行榜、Web/IDE 界面和长期云端存储。Tokscale 的公开提交需要用户主动上传；Token Monitor 的多设备同步也需要用户选择并配置节点。这些能力会增加隐私、认证和维护面，却不加强当前核心。以后若确实需要 NAS/多电脑查看，再以用户主动开启、自托管、无需付费托管为前提单独设计。

## 推荐产品定位

**“Windows 上的本地 AI 编程控制台：看最近哪个 Agent 用得多、额度大概还能撑多久，并能查清每个数字从哪里来。”**

三条主线是“最近活动与额度 runway”“来源可信度与自检”“Windows 额度提醒”。安装体验和导出放在次要待办，继续用本机数据、中文友好和键盘操作巩固体验。

## 一手来源

- [Token Use README](https://github.com/russmckendrick/tokenuse)
- [Tokscale README](https://github.com/junhoyeo/tokscale)
- [ccusage README](https://github.com/ccusage/ccusage)
- [CodexBar README](https://github.com/steipete/CodexBar) · [CodexBar provider documentation](https://github.com/steipete/CodexBar/blob/main/docs/providers.md) · [CodexBar CLI documentation](https://github.com/steipete/CodexBar/blob/main/docs/cli.md)
- [ccdash README](https://github.com/jedarden/ccdash)
- [Claude Code Usage (VS Code) README](https://github.com/ClaudeCodeUsage/ClaudeCodeUsage)
- [Claude Code Usage Monitor README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor)
- [Token Monitor README](https://github.com/Javis603/token-monitor)
- 本项目：[README](../../README.md)、`src/backend.rs`、`src/model.rs`、`src/ledger/`、`src/ui/`。
