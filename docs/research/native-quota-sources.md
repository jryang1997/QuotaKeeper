# Windows 配额与订阅信息：减少 OMP 依赖的一手来源调研

核对日期：2026-09-27。范围是**账号或订阅的服务端额度**，与本机日志中的 token/会话统计分开。调研阶段依据服务商文档、服务商公开源码和本仓库实现；落地后使用本机已登录的 Codex 官方程序对官方协议做了读取验证。其他服务的源码确认和解析单测不等于本机账号已验通。

## 本轮落地记录

下文“现状”记录的是改造前的基线。现已落地：四家独立采集；Go 直连 usage 接口；Codex 优先官方 app-server（已在本机登录态实测成功），失败才走旧直连或 OMP；GOAT 可由应用独立保存 API key，订阅接口失败时不猜测月额度；Antigravity 优先经 CPA 代查 Google 上游配额，失败才回退代理健康／本机活动，OMP stats 缺失时不制造零活动条。Go 与 GOAT 专用 key 存 Windows Credential Manager，界面只接收是否已保存。`README.md` 和卡片详情展示实际来源。

本机已实测 Codex app-server 和 CPA Antigravity 上游配额；CPA 7.3.15 上经 `api-call` 得到 2 组、4 个额度桶。当前机器没有 Go 专用 key，因此 Go 直连完成了响应结构单测，尚未做账号在线验证。GOAT 原有脱敏响应形状测试通过。服务商若调整未文档化的接口，相关卡会降级或显示缺失。

## 现状与建议顺序

改造前的 `src/backend.rs` 先运行 `omp usage --json`，只在找不到 OMP 程序时才回退到 Codex 直连；如果 OMP 存在但失败/超时，甚至可能阻断 Codex 回退。OpenCode Go 没有独立采集器。GOAT 已经由 `src/goat.rs` 直接请求 Command Code，但调用的是未文档化的 `/alpha/billing/*`。Antigravity 卡读取本机 CPA 管理接口，并依赖 `~/.omp/stats.db` 算当天活动强度；这不是 Google 的配额。[本仓库 backend.rs](../../src/backend.rs)、[GOAT 实现](../../src/goat.rs)

实施路径是**每家单独采集、单独超时、单独来源标签**：OpenCode Go 直接调服务商的 Go usage 接口；Codex 优先使用官方 `codex app-server` 的 `account/rateLimits/read`，在不具备 Codex 二进制时保留现有直连作为受限回退；GOAT 保持现有直连并标明 alpha 契约风险；Antigravity 用 CPA 已管理的账号代查 Google 内部额度汇总，失败时只展示本机活动/代理健康，不能把本机强度折算为上游剩余额度。OMP 仅为兼容兜底。[CPA 路径专项核验](cpa-antigravity-usage-paths.md)

| 服务 | 可核实的一手入口 | 可获取内容 | 凭据 | 支持程度与边界 |
| --- | --- | --- | --- | --- |
| OpenCode Go | `GET https://opencode.ai/zen/go/v1/usage`；[OpenCode 官方源码](https://github.com/anomalyco/opencode/blob/dev/packages/console/app/src/routes/zen/go/v1/usage.ts)、[合并记录](https://github.com/anomalyco/opencode/pull/16513) | 服务端按用户记录的 rolling、weekly、monthly 已用百分比、状态、ISO 重置时间 | Go API key 的 Bearer；[官方 Go 文档](https://opencode.ai/v2/docs/console/go)、[凭据存储说明](https://opencode.ai/docs/providers) | 源码公开、已合并；文档仅明确 Console 可看用量，尚未看到该具体接口的版本化 API 文档。不要误用另一项仅接受**服务账号 key**的 [CSV Usage API](https://opencode.ai/v2/docs/console/usage/) 作为实时 Go 配额。 |
| Codex / ChatGPT | `codex app-server` 的 `account/rateLimits/read`；[OpenAI 官方协议文档](https://developers.openai.com/codex/app-server) | 额度窗口、`usedPercent`、`windowDurationMins`、`resetsAt`、plan、credits；新版有 `rateLimitsByLimitId` 多桶 | Codex 官方登录态由 app-server 管理 | 官方文档化本地协议；需要用户已安装/登录 Codex，属于第一方程序依赖。独立 Rust GUI 无需 OMP，也无需自行复制 OAuth 刷新逻辑。 |
| GOAT | 当前直连 `https://api.commandcode.ai/alpha/billing/credits` 和 `/alpha/billing/subscriptions` | 5h/周 `used/cap/resetAt`；月余额及订阅周期 | Command Code API key；官方允许在 [Studio 创建 key](https://commandcode.ai/docs/plans/goat)，[CLI 登录](https://commandcode.ai/docs/quickstart)写 `~/.commandcode/auth.json` | 本仓库已有脱敏响应形状测试，但 `/alpha/billing/*` 未列在[官方 Provider API 端点表](https://commandcode.ai/docs/provider)中，不能保证稳定。官方支持的查看方式是 [CLI `/usage` 与 Studio](https://commandcode.ai/docs/resources/usage-limits)。 |
| Google Antigravity | [CPA 管理 WebUI 额度实现](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/features/quota/providers/antigravity/data.ts)、[Google 官方 CLI 更新记录](https://www.antigravity.google/changelog) | CPA 代查正在代理的账号：Gemini / Claude-GPT 额度桶与重置；官方 CLI 1.1.11 起可用 `agy -p "/usage" --output-format json` | CPA 管理 key 与 CPA 已管理的 Antigravity 凭据；或另一个 Google 官方 CLI 登录态 | 本机 CPA 7.3.15 代查已实测 200。Google `retrieveUserQuotaSummary` 是内部接口，不是公开稳定契约；CLI 账号可能与 CPA 账号不同。OMP stats 仅为本机活动。 |

## 分源细节

### OpenCode Go：可立即改为独立直连

[OpenCode 服务端源码](https://github.com/anomalyco/opencode/blob/dev/packages/console/app/src/routes/zen/go/v1/usage.ts)明确从 `Authorization: Bearer <key>` 读取 key，以该 key 对应的用户/工作区在服务端查用量。成功响应是 `usage.rolling`、`usage.weekly`、`usage.monthly`，每项有 `status`、`percent` 和 ISO `resetsAt`。未带/无效 key 返回 401；账号没有 Go 用量记录返回 403。实现不应再用本机 OpenCode SQLite 记录反推跨设备的配额，也不应把 `percent` 当成剩余比例。此前的[功能请求与合并记录](https://github.com/anomalyco/opencode/pull/16513)确认接口已进入上游；当前源码形状与早期 PR 草案不同，解析器应按**当前源码**并保留可诊断的形状错误。

[官方 Go 页面](https://opencode.ai/v2/docs/console/go)说明用户在 Console 订阅并复制 API key，且配额由 5 小时、每周、每月三个美元价值窗口组成；可在 Console 跟踪当前用量。[官方 Provider 文档](https://opencode.ai/docs/providers)说明 `/connect` 保存凭据到 `~/.local/share/opencode/auth.json`。GUI 可只读地查该文件中明确属于 `opencode-go` 的 key，或允许用户单独配置 key；须校验 provider ID，不能把普通 Zen/key 当 Go 权益，也不能回显 key。若 key 未配置，应展示来源缺失和官方获取入口。401 提示重新连接/更换 key，403 提示 Go 权益缺失；429/5xx 退避并保留明确标旧的缓存。

### Codex：优先官方本地协议

[OpenAI app-server 文档](https://developers.openai.com/codex/app-server)定义 stdio JSONL 协议：启动 `codex app-server`，先发 `initialize`（带客户端身份），再发 `initialized`，然后请求 `account/rateLimits/read`。文档给出返回结构：优先按 `rateLimitsByLimitId` 找 `codex`，没有时才读兼容字段 `rateLimits`；窗口缺失时应隐藏对应条目，而不是固定编造 5h/7d。`resetsAt` 是 Unix 秒。该协议由 Codex 管理登录和刷新，避免 GUI 接触长期 refresh token。官方帮助中心还说明使用情况可在设置/用量页或 CLI `/status` 查看，实际额度依套餐与账号而异，不能硬编码固定用量上限。[OpenAI 帮助中心](https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan)

改造前的 `src/backend.rs` 读取 `~/.codex/auth.json` 后调用 `https://chatgpt.com/backend-api/wham/usage`。该后端路径可从 [OpenAI Codex 源码/问题记录](https://github.com/openai/codex/issues/10869)追溯，但不是面向外部产品的公开 HTTP API 契约；GUI 自己读取 access token 也不能自动解决 token 更新、凭据改存系统密钥库、workspace 路由或返回形状变化。此直连仅宜作为标明“未公开接口”的兼容回退。OpenAI API 的[组织 RPM/TPM 限速文档](https://developers.openai.com/api/docs/guides/rate-limits)描述的是另一个计费/认证体系，不能用 API key 额度替代 ChatGPT/Codex 订阅额度。

### GOAT：已经脱离 OMP，但要收紧口径

[官方 GOAT 计划说明](https://commandcode.ai/docs/plans/goat)与[Usage Limits](https://commandcode.ai/docs/resources/usage-limits)确认 GOAT 的月池、5 小时、每周窗口及 reset 语义；`/usage` 可由官方 CLI 查看，Studio 也可查看。[官方 Quickstart](https://commandcode.ai/docs/quickstart)说明 Windows 命令为 `cmdc`，登录凭据写 `~/.commandcode/auth.json`；[官方设置文档](https://commandcode.ai/docs/settings)列出 `COMMAND_CODE_API_KEY`。本仓库目前支持的是 `CMD_API_KEY` 和本机 auth 文件，环境变量名称应与服务商官方约定再对齐。

改造前直接拉 `/alpha/billing/credits` 与 `/alpha/billing/subscriptions` 已经不经过 OMP，5h/周来自服务端。月百分比则由本仓库按 plan ID 静态映射月 cap 再减去 `monthlyCredits` 推出；它是**推算值**，若订阅请求失败或未知 plan，不应默认为 GOAT $70 并展示一个看似官方的百分比。额外购买 credits 与套餐内月池也要分开。[官方 Usage Limits](https://commandcode.ai/docs/resources/usage-limits)强调额外 credits 不受滚动窗口限制，GOAT/Pro 的 credits 是使用价值单位，并非所有模型都一律按 1 credit = $1 结算。应把月卡的来源/推算方法和已知缺口写清楚。

### Antigravity：CPA 代查上游，官方 CLI 可独立读取

[Google 官方 Plans](https://antigravity.google/docs/plans)说明基础额度按计划不同并可能调整；[官方 CLI `/usage` 文档](https://antigravity.google/docs/cli/commands/usage)说明模型额度；[AI credits 文档](https://antigravity.google/docs/cli/credits/)说明 credits 与基础配额是不同项目。官方 CLI 自 1.1.11 起支持 `agy -p "/usage" --output-format json`，但本机未安装 `agy`，因此尚未对其登录身份和输出结构做在线验证。[Google 更新记录](https://www.antigravity.google/changelog)

当前本仓库先从 CPA `auth-files` 得到 `auth_index` 与 `project_id`，再经 CPA `api-call` 带所选账号的刷新后令牌调用 Google `retrieveUserQuotaSummary`。本机实测 Google 上游 200、2 组 4 桶；例如 `remainingFraction=0.86` 对应已用 14%，重置时间来自响应。来源明确标为“CPA 代查 Google 上游配额 · 内部接口”，不能称作公开官方 API，也不应把它与 credits 或 CPA 请求 token 量混同。接口失败时才回退 CPA 凭证健康和 OMP stats 本机活动。[详细请求形状和边界](cpa-antigravity-usage-paths.md)

## 安全与验收边界

- 所有服务独立刷新，一家 401/403/429/超时不能让其它卡消失；失败后仅显示带“上次成功时间”的缓存，不能伪装为最新数据。
- 优先使用服务商自身 API key、官方本地协议或用户主动提供的凭据；只读扫描现有配置。令牌只进入请求内存与对应官方 HTTPS 主机，日志、IPC、导出、错误详情中均不输出密钥。若允许应用自行保存 key，应使用 Windows Credential Manager/DPAPI，而不是明文设置文件。
- OpenCode Go 服务端百分比与 GOAT 服务端窗口可标“服务端”；GOAT 月 cap 推导标“估算”；Codex app-server 标“官方本地协议”；Antigravity 代查额度标“CPA 代查 Google 上游 · 内部接口”，回退的代理健康/本机强度标“代理/本机”。价格、窗口及身份都不能由静态套餐名代替账号实测。
- 验收应包含无 OMP 安装、OMP 命令失败、各服务单独断网/401/403/429、凭据过期、多账号/工作区、窗口缺字段、跨设备使用、重置与订阅变更。没有真实账号在线探测时只能声明解析/降级经过模拟测试。
