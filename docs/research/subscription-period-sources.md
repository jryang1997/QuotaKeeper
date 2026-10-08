# 订阅周期(到期/续费时间)数据来源调研

核对日期:2026-09-29。问题:订阅制 AI coding 服务的"订阅到期时间"(时间维度)从哪里拿?用量维度(80%/100%/重置)提醒已有,本文补时间维度。结论先行,随后逐家给一手证据。

## 结论矩阵

| 服务 | 订阅期 | 端点 → 字段 | 凭据 | 本仓库动作 |
|---|---|---|---|---|
| Command Code GOAT | ✅ | `/alpha/billing/subscriptions` → `subscription.currentPeriodEnd` | 已有 API key | **已打通**(`goat.rs`,2026-09-28 已采集,本轮透传) |
| Codex / ChatGPT | ✅ | `GET chatgpt.com/backend-api/accounts/check/v4-2023-04-27` → `accounts.*.entitlement.renews_at`(另有 `has_active_subscription`/`subscription_plan`/`is_delinquent`) | `~/.codex/auth.json` 的 `tokens.access_token`(与 wham/usage **同一 token,零新凭据**) | **已打通**(2026-09-29 真机验证:plus 账号返回 renews_at) |
| Cursor | ✅ | `GET https://cursor.com/api/usage-summary` → `summary.billingCycleStart/billingCycleEnd` + `membershipType` + `individualUsage.plan.totalPercentUsed` | Cookie `WorkosCursorSessionToken=<uid>%3A%3A<accessToken>`;uid/exp 从 JWT payload 解;token 源①`%APPDATA%\Cursor\User\globalStorage\state.vscdb` ItemTable `cursorAuth/accessToken`(UTF-16LE BLOB)②cursor-agent CLI auth.json | **已落地第六卡**(`src/cursor_quota.rs`) |
| Kimi | ⚠️ | kimi.com billing gRPC `GetUsages` → 订阅池 `expireTime` | web JWT cookie | 不做卡;"手动粘贴 token"模式列 backlog |
| MiniMax | ✅ | 到期:`v1/api/openplatform/charge/combo/cycle_audio_resource_package` → `current_subscribe_end_time_ts` + `renewal_trigger_time_ts`(cookie,未做)。**卡片用的 `sk-cp-` 密钥只能读窗口**:`GET {host}/v1/api/openplatform/coding_plan/remains` → `model_remains[general]` 的 `current_interval_remaining_percent`/`end_time`(周窗口需 `current_weekly_status==1`) | 到期:cookie(www.minimax.io / minimaxi.com 双域);余量:`sk-cp-` 编程套餐密钥 | 余量卡已做(`minimax_quota.rs`,2026-10-08 按 cc-switch/opencodex 实现,待真机);到期未做 |
| z.ai GLM | ✅ | `GET {base}/api/biz/subscription/list` → `data[].nextRenewTime`(+`inCurrentPeriod`);账户余额 `GET {base}/api/biz/account/query-customer-account-report` → `data.availableBalance`(CNY,仅国内站) | coding plan key(Bearer 同样被接受) | **已打通**(`glm_quota.rs`,订阅期 2026-10-01 核对;余额 2026-10-08 按 CodexBar/TokenLedger 实现,待真机 key 联调) |
| OpenCode Go | ❌ | console 仓库 `zen/go/v1/` 仅 usage/chat/messages/models/responses/systemone 路由,**无 billing/subscription**;`monthly.resetAt` 是最接近的周期信号(已展示) | Go API key 只达 usage | — |
| Claude | ❌ | `api.anthropic.com/api/oauth/profile` 只有订阅档;`claude.ai/api/organizations/{org}/usage` 是用量桶;billing 端点无公开逆向先例 | — | 卡上如实不显示订阅期 |
| GitHub Copilot | ⚠️ | `GET api.github.com/copilot_internal/user`(内部端点)→ `copilot_plan` + `access_type_sku` + `quota_snapshots`,多来源称含订阅到期(schema 未一手确认) | GitHub OAuth token(gho_…,需 token 源如 gh CLI 登录态) | 后续 |
| Gemini / Antigravity | ❌ | Google 内部配额端点只有额度桶;订阅期在 Google One / Play | — | — |

## 逐家证据

### Codex / ChatGPT(零新凭据的关键先例)

[xx2689/codex-switcher](https://github.com/xx2689/codex-switcher)(MIT,单文件 Python)演示三调用:`wham/usage`、`accounts/check/v4-2023-04-27`、`auth.openai.com/oauth/token` 刷新;全部用 `~/.codex/auth.json` 的 `tokens.access_token` 做 Bearer,并从响应 `accounts`(map)遍历读 `entitlement.has_active_subscription` / `subscription_plan` / **`renews_at`** / `is_delinquent`。JWT claims 里另有 `chatgpt_subscription_active_until` 备用。本仓库 2026-09-29 真机验证命中。

### Cursor

[CodexBar docs/cursor.md](https://github.com/steipete/CodexBar/blob/main/docs/cursor.md) 与其 `CursorStatusProbe+UsageSummary.swift`(解析 `summary.billingCycleStart/billingCycleEnd`,ISO8601)确认字段;Rust 一手实现 [vct-core cursor.rs](https://docs.rs/vct-core/latest/src/vct_core/quota/cursor.rs.html) 给出完整请求形状:Cookie `WorkosCursorSessionToken=<uid>%3A%3A<accessToken>`(`::` URL 编码),uid 取 JWT `sub` 末段,`exp` 过期判定,token 约 60 天、**不代刷**(401 → NeedsLogin)。编辑器 token 在 state.vscdb 的 `cursorAuth/accessToken`(UTF-16LE BLOB)。

### Kimi

CodexBar `KimiUsageSnapshot.swift`:订阅池 `KimiSubscriptionBalance{feature,type,amountUsedRatio,expireTime}`,`expireTime` 是共享订阅池的到期;认证走 kimi.com web JWT cookie(其 macOS cookie 导入依赖浏览器解密,**Windows 不做**——与本文 cookie 决策一致),或手动粘贴 token / `KIMI_AUTH_TOKEN`。

### MiniMax

CodexBar `MiniMaxSubscriptionMetadata.swift`:combo cycle 端点返回 `current_subscribe_end_time_ts` / `renewal_trigger_time_ts`(epoch 秒或毫秒,>10^10 视为毫秒)与 plan 名(tokenplanplus/max/ultra);cookie 认证。

### z.ai

官方插件 [zai-org/zai-coding-plugins](https://github.com/zai-org/zai-coding-plugins)(Apache-2.0)`glm-plan-usage` 的 `query-usage.mjs`:三端点 `api.z.ai/api/monitor/usage/{quota/limit,model-usage,tool-usage}`,auth 为 `Authorization: <ANTHROPIC_AUTH_TOKEN 原值>`(**无 Bearer 前缀**),国际站 `api.z.ai` / 国内 `open.bigmodel.cn`;quota/limit 响应仅 `limits[]{type: TOKENS_LIMIT|TIME_LIMIT, percentage, currentValue, usage, usageDetails}` —— **无订阅期**。第三方 [guyinwonder168/opencode-glm-quota](https://github.com/guyinwonder168/opencode-glm-quota) 同样只有窗口百分比与 reset。订阅期唯一线索是 `api.z.ai/biz/api/subscription/list`(某多 provider 工具引用,auth 未验证)。

### 浏览器 cookie 自动提取:决策不做

- Chrome/Edge 在 Windows 自 2024.07 起默认 **App-Bound Encryption**(DPAPI 之上的系统级提升服务);2025-2026 的绕过手段(如 [Kaspersky 报告的 VoidStealer](https://www.kaspersky.com)、KREMLIN/REF9334,xaitax 的公开 PoC)全部依赖进程注入/COM 提升等**恶意软件同款技术**,开源监控工具采用会被杀软普遍标记,与 [SECURITY.md](../../SECURITY.md) 的凭据边界承诺冲突。
- Firefox 的 `cookies.sqlite` 明文可读,Rust 有现成 crate([cookie-scoop](https://crates.io/crates/cookie-scoop)、rookie 系列、decrypt-cookies),但为单个字段引入全站 cookie 读取面不成比例。
- **替代结论**:订阅期的可得路径几乎都在**服务商自己的登录态文件**(auth.json / state.vscdb / .credentials.json),只读、无浏览器依赖;真正需要 web 会话的(Kimi/MiniMax/z.ai)走"用户手动粘贴 token → Windows 凭据管理器"模式(CodexBar 验证过的跨平台做法),需要时按卡逐个加。

## 实现约定(本仓库)

- `model::Report.subscription_ends_at_ms: Option<i64>`:语义是**付费订阅周期结束**(续费/到期),与配额窗口 reset(月度窗口等)严格区分——GOAT 的 `currentPeriodEnd` 恰好两者同值是特例不是通则。
- 到期提醒档位(`ui::notify::expiry_level`):0 正常 / 1 ≤7 天 / 2 ≤3 天 / 3 ≤1 天或已过期;**只在档位上升时 toast**,续费回落静默;复用通知开关与夜间静默。
- accounts/check、usage-summary 均为未文档化接口:失败静默降级,卡面来源标注"未文档化接口"。
