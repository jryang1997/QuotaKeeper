# TUI Coding Plan 用量监测 — 立项调研(防重复造轮子)

日期: 2026-09-24 | 快照 stars 为当日读数 | 结论先行:**不要从零写解析器和 quota 采集,自研只做 quota 归一化 + 多套餐汇总 TUI。**

## 0. 名称歧义(先对齐)

- **Open Code Go = OpenCode Go($10/mo 订阅)**,唯一官方对应。文档: https://dev.opencode.ai/docs/go/
- **Command Code Goat / Code Goat:无此官方 Plan**。最可能是 OpenCode Go 口误;若指模型则是 Go 内模型 `muse-spark-1.3-contributor`(月额度 $60),Zen 内对应 `muse-spark-1.3-contributor-free` 限时免费。来源同上 + https://dev.opencode.ai/docs/zen
- **$20 档**:官方无 $20 Go 档。三选一需用户确认:(1)OpenCode Zen PAYG 充值 $20(低于 $5 自动补 $20,是钱包不是订阅);(2)ChatGPT Plus $20/mo(https://chatgpt.com/pricing/)或 Claude Pro $20/mo(https://support.anthropic.com/en/articles/11049762-choosing-a-claude-ai-plan);(3)ChatGPT Pro $200 / Claude Max 20x $200 口误。Pro $200 于 2026-09-10 起停售新购,存量不受影响(https://help.openai.com/en/articles/9793128-what-is-chatgpt-pro/)。
- Claude 系官方只有 Pro $20 / Max 5x $100 / Max 20x $200,无 $10 档。

## 1. OMP `/usage`(纠正:是 `/` 不是 `\`)

- 仓库: [can1357/oh-my-pi](https://github.com/can1357/oh-my-pi)(~33k stars, MIT, 官网 https://omp.sh)。
- 入口三态同源:CLI `omp usage`(`packages/coding-agent/src/commands/usage.ts`,flags `--provider/-p --json/-j --redact/-r --history --days/-d` 默认 7);渲染 `packages/coding-agent/src/cli/usage-cli.ts`(28 宽彩色 bar,≥80% warning,≥100% exhausted);TUI `handleUsageCommand() → showUsageDashboard`(见 `modes/controllers/command-controller.ts:632`,`selector-controller.ts` 挂载 `UsageDashboardComponent`);文本回退 `buildUsageReportText()`。
- 归一化 schema:`packages/ai/src/usage.ts`(`UsageReport/UsageLimit/UsageWindow/UsageAmount/ResetCredits`);20 个内置 provider(`packages/ai/src/usage/registry.ts`):openai-codex、anthropic、opencode-go、github-copilot、cursor 等;全 catalog 约 80 provider(`packages/catalog/src/compat/provider-ids.ts`)。
- 数据源:在线配额 API(`packages/ai/src/usage/*.ts`)+ 本地 `~/.omp/agent/agent.db` 凭证池 + hourly snapshots + `~/.omp/stats.db` heatmap(`packages/stats/README.md`)。
- 显示形式:全屏 quota 卡 + reset 倒计时 + Saved rate-limit resets + Activity heatmap(截图见 https://github.com/can1357/oh-my-pi/issues/11206)。**可直接抄**:卡片布局、阈值染色、reset 倒计时、`UsageReport` 归一化结构。不可复用 UI 代码(Bun/TS 栈,且与 agent 耦合)。

## 2. CodexBar(两个同名项目,认准 steipete)

- 主目标:[steipete/CodexBar](https://github.com/steipete/CodexBar)(作者 Peter Steinberger,~21.8k stars,Swift,MIT,官网 https://codexbar.app)。84 个 provider(`docs/provider-ids.md`,矩阵见 `docs/providers.md`);每 provider 一菜单栏 meter + popover(bars + reset 倒计时 + credits/spend/cost);另有 `cost/cards/dashboard/serve/guard` CLI(`docs/cli.md`)与快照契约 `GET /usage|/cost|/dashboard/v1/snapshot`(`docs/dashboard-api.md`)。
- 平台限制:需 macOS 14+(MenuBarExtra/WidgetKit);Linux 有 Qt6 版;**Windows 仅社区版,无全屏 TUI**。Keychain 弹窗、Safari cookie 需 Full Disk Access(README + `docs/keychain-prompts.md`)。
- 数据源多路 fallback 值得抄:Codex 三路 OAuth `wham/usage` → CLI RPC → web extras(`docs/codex.md`);Pi/OMP 本地历史扫 `~/.pi|~/.omp` sessions(`docs/pi.md`)。
- 极简同名:[snakeye/CodexBar](https://github.com/snakeye/CodexBar)(1 star,仅 macOS 单 Codex,5min 轮询,无 CLI/TUI)。**无复用价值**。
- 复用建议:最高性价比是消费 `codexbar --format json` + serve 快照 schema 做适配层,自研补 TUI 渲染。

## 3. 终端 TUI 生态(第一梯队 + 最接近你需求的 3 候选)

成熟度断层:ccusage(18.7k)+ Claude-Code-Usage-Monitor(8.7k)+ tokscale(约 5.5k)第一梯队,其余 TUI 均 <200 stars。框架分治:Rust Ratatui 主流、Go BubbleTea、Python Rich live;**Python/Textual 无成熟 quota 底座,不推荐**。

| 名称 | URL | 语言 | stars | 支持 plan | TUI | License |
|---|---|---|---|---|---|---|
| ccusage | https://github.com/ccusage/ccusage | Rust | 18729 | 18+ providers 本地解析 | 否,CLI blocks/statusline/--json | Other(早期 MIT,需复核) |
| Claude-Code-Usage-Monitor | https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor | Python+Rich | 8720 | Claude Pro/Max(官方 rate_limits+本地估计) | 是,Rich 实时 | MIT |
| tokscale | https://github.com/junhoyeo/tokscale | TS/Bun+Rust/Ratatui | ~5500 | 40+ clients 本地+订阅 Usage | 是,8 视图 | MIT |
| token-stats(fork) | https://github.com/Annihilater/token-stats | Rust+TS/Ratatui | 8 | 同上 40+ | 是,Overview/Models/Daily/Hourly | MIT |
| agent-usage-monitor(aum) | https://github.com/youming-ai/agent-usage-monitor | Rust/Ratatui | 3 | Claude+Codex+Cursor(本地+live quota) | 是,堆叠分 platform+JSON+MCP | MIT |
| ccu | https://github.com/sammcj/ccu | Go/BubbleTea | 5 | Claude OAuth 5h+周+分模型周 | 是,+HTTP API :19840 | Apache-2.0 |
| ccdash | https://github.com/jedarden/ccdash | Go/BubbleTea | 15 | Claude+Codex 本地+hooks | 是,三面板 | Apache-2.0 |
| openusage | https://openusage.sh | Go | 未公示 | 34 providers | 是,dashboard/tmux | 未验证 |
| LimitDeck | https://github.com/rockythink/limitdeck | Rust TUI | 9 | 订阅剩余额度+reset | 是 | 需复核 |
| toktop | https://github.com/htin1/toktop | Rust TUI | 173 | Admin API 组织账单(非订阅 quota) | 是 | 需复核 |

最接近多套餐 TUI 汇总:

1. [aum](https://github.com/youming-ai/agent-usage-monitor)—最接近 MVP。已有 Claude+Codex 双 live quota + 堆叠 TUI + stats JSON + MCP。二次开发:加 OpenCode Zen/Go 采集 + 顶部汇总行,数天级;风险:stars 3 极新,无 Windows 官方支持。
2. [token-stats](https://github.com/Annihilater/token-stats)/[tokscale](https://github.com/junhoyeo/tokscale)—覆盖最广。40+ providers 已含 OpenCode,可当账本 + JSON 源;代价是构建重(631 文件)。
3. [ccdash](https://github.com/jedarden/ccdash)+ ccusage 引擎—Go 栈最易改。ccdash(`internal/metrics/+internal/ui/`,SQLite `~/.ccdash/tokens.db`)为最小可 hack 底座,ccusage `--json` 为最稳解析后端;约 1–2 周。

通用架构(抄这个):Collectors(Claude `~/.claude/projects/*.jsonl` + Codex `~/.codex/sessions/.../rollout-*.jsonl` + OpenCode `opencode.db` + 官方 quota API)→ 归一化(tokens/cost/5h·7d·30d/reset) → 前端 + 机器接口(JSON/MCP/SQLite/HTTP)。

## 4. 各 Plan 用量 API 可行性

### OpenAI 系(Codex / Plus)

- 官方 Admin 计费 API(`GET api.openai.com/v1/organization/usage/completions`,需 `sk-admin-...`)**与订阅配额无关,不适用**。文档: https://developers.openai.com/api/docs/guides/admin-apis
- 推荐首选本地 app-server RPC:`codex app-server --stdio` → `account/read → account/rateLimits/read`,字段 `rateLimits.{planType, primary/secondary:{usedPercent, windowDurationMins, resetsAt}}`,以 `windowDurationMins=300/10080` 识别 5h/周。源码: https://github.com/openai/codex/blob/main/codex-rs/app-server-protocol/src/protocol/v2/account.rs
- 备用直调 `GET chatgpt.com/backend-api/wham/usage`(header `Authorization: Bearer <access_token>` + `ChatGPT-Account-Id`),第一方源码 `codex-rs/backend-client/src/client.rs`。风险:未文档化,随时变;按 60–300s + 429/5xx 退避,401/403 停转重登。

### Claude 系(Pro/Max)

- 无官方订阅剩余额度 REST。首选 `GET api.anthropic.com/api/oauth/usage`(header `Authorization: Bearer <sk-ant-oat01-... 来自 ~/.claude/.credentials.json>` + `anthropic-beta: oauth-2025-04-20`),返回 `five_hour/seven_day/...{utilization, resets_at}`。实现参考: https://github.com/sirmalloc/ccstatusline/blob/main/src/utils/usage-fetch.ts
- 无 token 时回退本地 JSONL 估算(ccusage 路径)并标 estimated;cookie(`claude.ai/api/organizations/{uuid}/usage`)仅可选增强,不可送第三方代理。

### OpenCode / Zen 系(差异化点)

- **Go 有唯一稳定的官方订阅用量接口**:`GET opencode.ai/zen/go/v1/usage`(header `Authorization: Bearer <OPENCODE_API_KEY>`),返回 `usage.{rolling,weekly,monthly:{status, percent(百分制,1=1%), resetsAt}}`,rolling=月额度 20%/weekly=50%/monthly=100% 按美元等值。源码: https://github.com/anomalyco/opencode/blob/dev/packages/console/app/src/routes/zen/go/v1/usage.ts ;文档: https://dev.opencode.ai/docs/go/
- 历史对账:`GET opencode.ai/console/api/v1/usage/export?scope=organization&range=24h|7d|30d`(仅 service-account `oc_sk_...`,CSV)。文档: https://opencode.ai/v2/docs/console/usage/
- **Zen 余额无官方 API-key 接口**(open 需求 https://github.com/anomalyco/opencode/issues/44189)。社区做法全是本机 cookie 私有接口(`console/api/billing/status` 取 `balanceMicroCents/1e8=USD`),只能本机可选增强,标来源。

## 5. 立项建议

- 最小可用:Go=`zen/go/v1/usage`;Codex=app-server RPC(+wham 备用);Claude=`oauth/usage`(+JSONL);Zen 余额=可选本机 cookie。非官方值标 unofficial,60s+ 轮询 + Retry-After + 401 停转。
- 技术栈:Rust+Ratatui 选 aum/token-stats 底座;Go+BubbleTea 选 ccdash。Windows 常驻是空白点(现有方案 macOS 菜栏或 Linux tray 为主)。
- 下一步待确认:澄清 `$20` 到底是 Zen 充值 / Plus / Pro 中的哪一个(决定 Zen 余额链路做不做);确认技术栈偏好(Rust 还是 Go);首版三个 provider 是否就是 Go + Codex + Claude。
