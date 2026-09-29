# CodeBurn / TokenScale 名称核实与竞品全景(一手来源)

调研日期:2026-09-29。stars 等为当日 GitHub API 读数,会随时间漂移。所有竞品事实只取对方 GitHub 仓库(README / GitHub API),不采信第三方博客。此文与 [Windows 桌面版竞品功能审计](competitor-feature-audit.md)互补:那篇聚焦 GUI 交互细节,这篇核实项目所有者点名的两个产品名,并给出全景成熟度与对标结论。本仓库基线见 [README](../../README.md)。

## 先看结论

1. **CodeBurn 实有其物且已是大体量竞品**:[getagentseal/codeburn](https://github.com/getagentseal/codeburn)(11,268 stars,MIT,TypeScript,2026-04-13 建仓),`npx codeburn` 即用,本地解析 40 个 AI 编码工具的会话文件,LiteLLM 估价,`codeburn quota` 读已登录工具的实时限额,还带订阅套餐 pacing、浪费诊断和 Windows 桌面版。它是与本项目定位重叠度最高、增速最快的产品(约 5.5 个月到 11k stars)。[仓库 README](https://github.com/getagentseal/codeburn) · [GitHub API](https://api.github.com/repos/getagentseal/codeburn)
2. **TokenScale 查无此产品;最接近且判断为同一所指的是 tokscale**([junhoyeo/tokscale](https://github.com/junhoyeo/tokscale),5,567 stars,MIT,官网 [tokscale.ai](https://tokscale.ai)):Rust/Ratatui TUI,50+ 客户端本地解析 + 多路官方/半官方接口,名字与本项目的描述(“AI coding 用量追踪”)完全吻合,应为口误或记忆偏差。检索到的唯一其他 "Tokenscale" 是一篇 LLM serving 弹性伸缩论文(CoRR abs/2512.03416),与用量监测无关。[仓库](https://github.com/junhoyeo/tokscale)
3. **头部格局已固化,打法分四类**:本地账本解析的事实标准 ccusage(18.8k)、单工具深挖的 Claude-Code-Usage-Monitor(8.7k,Python)、官方端点配额最全的 CodexBar(22k,macOS)、两端通吃的 CodeBurn/tokscale。共同点:全 MIT、全有一行命令安装、全把 LiteLLM 当价目源。本项目的差异化(OpenCode Go / GOAT / Antigravity 经 CPA 的官方端点配额、Windows TUI+GUI 双形态、来源口径透明)在 README 层面无人重复;最大短板不是功能而是开源基建:无 LICENSE、无 CI、无 Releases、无一键安装。[对标结论](#对标结论)见文末。

## 0. 名称核实(先对齐)

- **CodeBurn**:[getagentseal/codeburn](https://github.com/getagentseal/codeburn) 确认存在,官方主页 https://codeburn.app,GitHub API 描述 "Free, local tool to track AI coding token usage and cost across 37 tools and agents (Claude Code, Cursor, Codex, Gemini and more)"。[GitHub API](https://api.github.com/repos/getagentseal/codeburn)
- **TokenScale**:多轮检索(GitHub / npm / 通用 web)未找到任何名为 TokenScale 的用量监测产品;最接近的是 **tokscale**([junhoyeo/tokscale](https://github.com/junhoyeo/tokscale)),本仓库 [RESEARCH.md](../../RESEARCH.md) 立项时已收录(当时记 ~5.5k stars)。本文按 tokscale 建档。若所有者所指另有其物,需要补充线索再查。
- **易混名提醒**:另有 **TokenBurn**([ZephrFish/TokenBurn](https://github.com/ZephrFish/TokenBurn)),与 CodeBurn 同名易混,是个人自托管小项目(41 stars),见第 6 节。

## 1. CodeBurn(getagentseal/codeburn)— 定位重叠度最高的正面竞品

| 维度 | 一手来源中的事实 |
|---|---|
| 监测对象 | README "All 40 tools":Antigravity · Claude Code · Cline · Codebuff · Codex · CodeWhale · Copilot · Crush · Cursor · DeepSeek Harness · Devin · Droid · Goose · Grok · KiloCode · Kimi · Kiro · OMP · OpenCode · Pi · Qwen · Warp · ZCode · Zed · Zerostack 等;另读 Vercel AI Gateway 上报 API(默认不计入总量)。**注意:本项目六路账本中的 ZCode 与 DeepSeek Harness 它都已覆盖。** [README](https://github.com/getagentseal/codeburn) |
| 数据获取 | 读各工具已写在磁盘上的本地会话文件,"no account, no API key and no proxy";估价用 LiteLLM 公开价目(每日刷新,缓存于 `~/.cache/codeburn/`,常见 Claude/GPT 模型内置兜底);`codeburn quota` "reads the live limits out of the tools you are already signed in to"(5 小时/周窗口);WSL 下同时读发行版与 Windows 两个 home。 [README](https://github.com/getagentseal/codeburn) |
| 功能 | 终端 dashboard + 桌面应用 + macOS 菜单栏/Windows 托盘/GNOME 扩展;按工具/模型/项目/git 分支/任务(13 类)拆分;`codeburn plan set claude-max` 等订阅预设(claude-max/pro、cursor-pro、copilot-pro、custom `--monthly-usd`)+ "on pace to exceed" 预测;`codeburn guard` 消费上限(Claude $5 警告/$15 停止,可配);`codeburn optimize` 浪费诊断(重复读文件、闲置 MCP、臃肿 CLAUDE.md,可应用/回滚);`codeburn compare` 模型对比;per-PR 成本;本地 MCP server 让 agent 查自己的用量;局域网多机配对;10 年日粒度历史。README 无 heatmap、无模型 leaderboard 字样。 [README](https://github.com/getagentseal/codeburn) |
| 平台与安装 | `npx codeburn` / `npm i -g codeburn` / `brew install codeburn`(需 Node.js 22.13+);桌面版 macOS 签名公证 DMG、Windows Microsoft Store 或 `.exe`、Linux `.deb`/`.rpm`/AppImage(当前 v0.9.25)。 [README](https://github.com/getagentseal/codeburn) |
| 许可与成熟度 | MIT;11,268 stars / 865 forks / 21 open issues;2026-04-13 建仓,2026-09-28 仍在推送;TypeScript;2,399 commits。 [GitHub API](https://api.github.com/repos/getagentseal/codeburn) |
| 隐私 | "The CLI sends nothing";桌面应用遥测需首次启动选择加入,EU/EEA/UK/瑞士默认关;不收集 prompt/代码/文件名/项目名。 [README](https://github.com/getagentseal/codeburn) |

## 2. tokscale(junhoyeo/tokscale)— "TokenScale" 的实际所指,覆盖面最广的 TUI

| 维度 | 一手来源中的事实 |
|---|---|
| 监测对象 | 50+ 客户端:Claude Code、Codex CLI、OpenCode、Copilot CLI、Gemini CLI、Cursor、Amp、Droid、Roo Code、Cline、Crush、Goose、Zed、Kiro、Trae、Warp、Kimi、Qwen、Junie、Devin、Antigravity、Pi、Oh My Pi、**DeepSeek Harness**、MiniMax Code、MiMo Code 等。 [README](https://github.com/junhoyeo/tokscale) |
| 数据获取 | 四路并存:①本地文件解析(JSONL/JSON/SQLite,如 `~/.claude/projects/`、`~/.codex/sessions/`、`~/.local/share/opencode/opencode.db`),Rust 原生模块加速;②厂商接口(Cursor 用 cookie 走 usage-export、Trae 官方 usage API、Warp GraphQL、Antigravity 语言服务器 RPC);③OAuth/API key 订阅额度(Claude、Codex、Copilot、Kimi、**Z.ai**、MiniMax 等);④包裹 `codex exec` 捕获。估价 LiteLLM 为主、OpenRouter 兜底 + 自定义覆盖,约 1 小时缓存。 [README](https://github.com/junhoyeo/tokscale) |
| 功能 | Ratatui 交互 TUI,10 个视图(Overview/Usage/Models/Daily/Hourly/Monthly/Sessions/Projects/Stats/Agents);按模型/客户端/会话/工作区分组,worktree 合并,日期与客户端过滤;订阅额度展示(session/weekly 重置);线上社区:tokscale.ai 全球 leaderboard、GitHub 验证登录、Wrapped 年度回顾、embed 徽章。 [README](https://github.com/junhoyeo/tokscale) |
| 平台与安装 | macOS(x86_64/arm64)、Linux(glibc/musl)、Windows(x86_64/arm64);`npx tokscale@latest` 或 `bunx`(需 Bun/Node 生态)。 [README](https://github.com/junhoyeo/tokscale) |
| 许可与成熟度 | MIT;5,567 stars / 458 forks / 69 open issues;2025-12-01 建仓,2026-09-25 仍在推送;GitHub API 语言标 Rust。 [GitHub API](https://api.github.com/repos/junhoyeo/tokscale) |

## 3. ccusage — 本地账本解析的事实标准,无配额能力

| 维度 | 一手来源中的事实 |
|---|---|
| 监测对象 | 本地日志覆盖约 18 个 CLI:Claude Code、Codex、OpenCode、Amp、Droid、Codebuff、Goose、Kimi、Qwen、Copilot CLI、Gemini CLI、Antigravity、Grok Build CLI、ZCode 等。 [README](https://github.com/ccusage/ccusage) |
| 数据获取 | 只读本地各 CLI 的用量日志(支持自定义路径),无 OAuth/官方端点/云同步;Nix 构建把锁定版本的 LiteLLM 价目文件直接嵌入二进制,非 Nix Cargo 构建也从同一锁定版本拉取。 [README](https://github.com/ccusage/ccusage) |
| 功能 | 日/周/月/会话/5 小时 billing block 报表;statusline 集成(Beta);`--breakdown` 按模型拆分;`--no-cost`、时区、日期过滤、JSON 输出、离线模式、`ccusage.json` 自定义价格。README **未提** burn rate、订阅额度、套餐、heatmap、leaderboard、TUI/GUI。 [README](https://github.com/ccusage/ccusage) |
| 平台与安装 | `npx ccusage@latest` / `bunx` / `pnpm dlx`;`nix run github:ccusage/ccusage`;文档站 [ccusage.com](https://ccusage.com)。 [README](https://github.com/ccusage/ccusage) |
| 许可与成熟度 | README 页脚 "MIT © @ryoppippi",但 GitHub API license 字段为 NOASSERTION(自定义文本,[RESEARCH.md](../../RESEARCH.md) 标注"需复核"仍未解决);18,778 stars / 852 forks;2025-05-29 建仓,2026-09-28 仍在推送;GitHub API 语言标 Rust(仓库含 `rust/` 目录,即 TS 起家后重写为 Rust)。 [GitHub API](https://api.github.com/repos/ccusage/ccusage) |

## 4. Claude-Code-Usage-Monitor — burn rate 与预测范式的定义者,但只做 Claude

| 维度 | 一手来源中的事实 |
|---|---|
| 监测对象 | 仅 Claude Code 订阅用量;Cursor 与 Claude Desktop 明确 out of scope;OpenCode/z.ai/LiteLLM 等以"独立 adapter"叙事带过。 [README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor) |
| 数据获取 | 三路:①扫描本地 JSONL(`--data-paths`、`CLAUDE_CONFIG_DIR`、WSL 探测);②`--statusline` 以 Claude Code statusline hook 身份运行,**捕获官方 rate_limits,官方数据优先于本地估计**;③`--api` 实验性 opt-in Anthropic OAuth usage API。 [README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor) |
| 功能 | burn rate(tokens/min)与投影;P90 自适应限额检测(默认 custom 档自动探测);套餐档位 pro(~19k)/max5(~88k)/max20(~220k)/team/custom,Pro 超阈自动切更高档;Rich 实时 TUI(realtime/daily/monthly/session/entries/burn-rate 视图);接近限额警告;`--once` 退出码 0/10/11 可接自动化;`--write-state`/`--output json` 供菜单栏等外部 GUI 消费(本身无 web/桌面 GUI)。 [README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor) |
| 平台与安装 | Linux/macOS/Windows;`uv tool install claude-monitor`(推荐)/ pip / pipx;Python 3.9+。 [README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor) |
| 许可与成熟度 | MIT;8,728 stars / 464 forks;2025-06-19 建仓;**最后推送 2026-07-05,近三个月无提交**,是五家里唯一明显放缓的。 [GitHub API](https://api.github.com/repos/Maciek-roboblog/Claude-Code-Usage-Monitor) |

## 5. CodexBar(steipete)— 官方端点配额采集的最全实现,但无 Windows 官方版

| 维度 | 一手来源中的事实 |
|---|---|
| 监测对象 | 约 90 个 provider(Codex、Claude、Gemini、Copilot、Cursor、Windsurf、Zed、Grok、DeepSeek、OpenRouter、AWS Bedrock 等),含 provider 接入指南;[RESEARCH.md](../../RESEARCH.md) 当时记录 `docs/provider-ids.md` 为 84。 [README](https://github.com/steipete/CodexBar) |
| 数据获取 | 路径最多:官方 API(OpenAI Admin API 花费、Copilot usage API、z.ai/MiniMax 等配额端点)、OAuth/设备流(Codex/Claude/Gemini/Copilot/Vertex)、浏览器 cookie 与 localStorage(Cursor/OpenCode/Windsurf 等,opt-in)、本地文件(JSONL、SQLite、JetBrains 配置)、API key(`codexbar config set-api-key`)。 [README](https://github.com/steipete/CodexBar) |
| 功能 | 菜单栏用量表 + 重置倒计时 + 动态条形图标;花费图表(Admin API/OpenRouter/LiteLLM/Bedrock);`codexbar cost` 本地成本扫描(SQLite,上限 25,000 会话);`codexbar serve` 快照接口;provider 故障状态徽章;通知;23 种语言本地化;无 TUI。pace(相对匀速超前/余量)见 [docs/ui.md](https://github.com/steipete/CodexBar/blob/main/docs/ui.md#pace-tracking)。 [README](https://github.com/steipete/CodexBar) |
| 平台与安装 | macOS 14+ 菜单栏应用(`brew install --cask codexbar`);Linux 有 Qt6 桌面 + CLI(brew tap/AUR/包);**Windows 无官方版,只有两个第三方移植**(与 [competitor-feature-audit.md](competitor-feature-audit.md) 记录一致)。 [README](https://github.com/steipete/CodexBar) |
| 许可与成熟度 | MIT;22,033 stars / 2,016 forks / 149 open issues;2025-11-16 建仓,2026-09-28 仍在推送;Swift。 [GitHub API](https://api.github.com/repos/steipete/CodexBar) |

## 6. TokenBurn(ZephrFish)— 与 CodeBurn 同名易混的自托管小项目

Python 自托管仪表盘:"Self-hosted dashboard for tracking LLM token usage and cost across multiple machines"。每台机器跑 `collect.py` 导出 JSON 经 git 同步,`build.py` 生成静态 `index.html`;核心卖点是 **API 等值成本 vs 订阅月费的回本对比**(可配月费与起算日),图表含每日 token、按模型/项目成本、小时活跃;支持 macOS/Linux/Windows,需 Python 3.6+ 与既有会话数据。成熟度很弱:41 stars、main 分支仅 3 个 commit、无 release、许可未标注。[仓库 README](https://github.com/ZephrFish/TokenBurn) 它的"回本对比"思路与本项目的月费基准(README:月度 API 等值相对月费基准的倍数)同源,但规模不可比。

## 7. 横向对照(2026-09-29 读数)

| | CodeBurn | tokscale | ccusage | CCUM | CodexBar | 本项目 |
|---|---|---|---|---|---|---|
| stars / 建仓 | 11,268 / 2026-04 | 5,567 / 2025-12 | 18,778 / 2025-05 | 8,728 / 2025-06 | 22,033 / 2025-11 | 新仓库 |
| 最近推送 | 2026-09-28 | 2026-09-25 | 2026-09-28 | **2026-07-05** | 2026-09-28 | 持续 |
| 语言 / 依赖 | TS(Node 22+) | Rust+TS(Bun) | Rust(嵌入价目) | Python 3.9+ | Swift | Rust |
| License | MIT | MIT | MIT(NOASSERTION) | MIT | MIT | **未设** |
| 形态 | CLI+桌面+托盘 | TUI+线上社区 | CLI 报表 | TUI | 菜单栏(Linux Qt) | **TUI+Windows GUI** |
| 工具覆盖 | 40 | 50+ | ~18 | 仅 Claude | ~90 provider | 6 账本 + 4 配额源 |
| 本地账本解析 | ✔ | ✔ | ✔ | ✔ | ✔ | ✔ |
| 官方端点/OAuth 配额 | quota 读已登录工具 | ✔(含 Z.ai) | ✘ | hook+实验 API | ✔ 最全 | ✔(Go/GOAT/Codex/CPA) |
| burn rate / pace | plan pacing | 重置展示 | ✘ | ✔ 最深 | pace | ✔(带可靠性门槛) |
| 订阅套餐卡/月费对比 | ✔ 预设+pacing | ✔ | ✘ | ✔ 档位 | 花费为主 | ✔ 月费基准 |
| 热力图 / 模型榜 | ✘ / ✘ | Stats 视图 / 线上榜单 | ✘ | ✘ | ✘ | ✔ / ✔(本地) |
| 一键安装 | npx/brew/Store | npx/bunx | npx/nix | uv/pip | brew/AUR | cargo build |
| 生态出口 | MCP | 线上 API/徽章 | statusline Beta | statusline/JSON/退出码 | serve 快照/CLI | CSV/JSON 导出 |

## 对标结论

以下引用除注明外均指向上文各竞品 [README](https://github.com/getagentseal/codeburn)/[GitHub API](https://api.github.com/repos/getagentseal/codeburn) 小节;本项目事实引自 [README](../../README.md)。

**我们有、README 层面无人有的**

1. **OpenCode Go usage API + GOAT billing API + Codex 官方 `app-server` + Antigravity 经 CPA 读 Google 上游配额**这一组合(README"采集链路"节)。四家头部里官方端点配额最强的是 CodexBar(约 90 provider,含 z.ai/MiniMax 配额端点,[README](https://github.com/steipete/CodexBar)),但未见 OpenCode Go、GOAT、Antigravity/CPA 任何一家;CodeBurn 的 `codeburn quota` 只描述"读你已登录工具的实时限额"(5 小时/周窗口,[README](https://github.com/getagentseal/codeburn))。订阅侧我们也是唯一把 GOAT 月度百分比由套餐上限与服务端余额推算的。
2. **Windows 一等公民 + 无运行时依赖**。官方支持 Windows 桌面的只有 CodeBurn(Store/`.exe`,但要 Node 22.13+);CodexBar 明确无 Windows 官方版([README](https://github.com/steipete/CodexBar));CCUM 是 Python;tokscale 走 npx/Bun。本项目 Rust TUI + WebView2 GUI + 托盘风险色 + toast 阈值通知 + 登录启动,全部零 Node/Python 依赖。
3. **每张卡可展开"来源、窗口、计算口径"并区分报告价与估价**(README:导出含来源、报告金额、估算金额、缺价标记)。CCUM 只在机器可读快照里标 official/local_estimate([README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor));我们把这做成了一级 UI,且 burn/耗尽预测在样本不足或数据过期时抑制显示,与 CodexBar 的 pace 门槛([docs/ui.md](https://github.com/steipete/CodexBar/blob/main/docs/ui.md#pace-tracking))同一哲学但覆盖四家配额源。
4. **本地热力图 + 工作时段 Profile + 模型榜**:CodeBurn README 无 heatmap/leaderboard 字样,ccusage/CCUM/CodexBar 也没有;tokscale 有 Stats 视图但其榜单是 [tokscale.ai](https://tokscale.ai) 线上服务。26 周 GitHub 风格热力图 + 下钻在本地工具里未见对标。

**别人有、我们没有的**

1. **一键安装与开源基建**——上线前最大硬伤。四家全部一行安装(npx/brew/uv/cask),且全为 MIT;本仓库尚无 LICENSE、无 CI、无 Releases,分发要 `npm ci` + `cargo build` + bat 脚本。这不是功能问题,是开源Launch的信任门槛。
2. **生态出口**:CodeBurn 有本地 MCP server(README:agent 可查自己的用量)、ccusage 有 statusline Beta、CCUM 的 statusline hook 同时是它的官方额度获取通道、CodexBar 有 `serve` 快照接口与 CLI 子命令([各自 README](https://github.com/steipete/CodexBar))。本项目只有 CSV/JSON 文件导出,无 `--json` 快照、无 statusline、无 MCP,无法被别的工具组合。
3. **覆盖广度**:40 / 50+ / ~90 / ~18 对我们 6+4;且 CodeBurn 已把 ZCode、DeepSeek Harness、OMP、OpenCode 都列进 40 工具清单([README](https://github.com/getagentseal/codeburn)),我们的"独有账本"窗口正在关闭。广度战打不赢也不必打,但要承认 CodeBurn 单品覆盖已大于我们。
4. **浪费诊断与组织维度**:CodeBurn 的 `optimize`(重复读文件、闲置 MCP)与 per-PR 成本、CodexBar 的 OpenAI Admin API 组织花费([README](https://github.com/steipete/CodexBar))都是我们没有的叙事;CCUM 的 P90 自动探测套餐档位与自动化退出码(0/10/11)也是可借鉴的工程细节([README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor))。
5. **社区与文档资产**:ccusage 有独立文档站与 Nix 打包、tokscale 有线上榜单、CodexBar 有 23 语言本地化与 provider 状态徽章;本项目 README 即全部文档。

**建议(按投入产出排序)**

1. **先补基建再谈功能**:加 LICENSE(MIT,与赛道一致)、GitHub Actions CI + 二进制 Release(cargo binstall / winget / Scoop 清单)、TUI 免构建单文件下载。四家对标里这项差距最刺眼也最便宜。
2. **输出一个机器可读快照**:对齐 CodeBurn MCP / CCUM `--output json` / CodexBar `serve` 三种已有先例,先做 `--json` + statusline,再做 MCP。没有出口,本项目只能当终端孤岛用。
3. **把官方端点纵深做成护城河**:OpenCode Go、GOAT、CPA→Google 这三路是 README 层面零竞品的组合,且 CodeBurn 增速证明"本地解析+套餐 pacing"市场已被吃掉;应继续加深官方端点的稳定性与降级路径(现有 native-quota-sources 方向),而不是扩账本数量。
4. **保留双口径优势**:CodeBurn 只有 LiteLLM 单一口径,我们"来源报告价 vs LiteLLM 估价"分列 + 缺价标记更诚实,值得在宣传里作为隐私/准确度卖点讲。
5. **盯住 CodeBurn 的 quota 演进**:它 5.5 个月到 11k stars 且已带 plan pacing 与 Windows 桌面;若其 `codeburn quota` 扩展到 OpenCode Go/GOAT,本项目差异化窗口收窄,届时再考虑错位(更深的订阅卡与 Windows 常驻体验)。
