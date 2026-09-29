# CodexBar（steipete/CodexBar）各 Provider 数据获取机制调研

核对日期：2026-09-29。核对对象：[steipete/CodexBar](https://github.com/steipete/CodexBar) `main` 分支，commit `bc36a048686a1bd700a3f5adf6dcf2923a10ddd1`（2026-09-28），MIT 协议。所有结论均直接读取该仓库源码（Swift）与 `docs/` 内的 provider 文档，引用格式为 `[文件名](https://github.com/steipete/CodexBar/blob/main/<path>)`。CodexBar 本身只支持 macOS / Linux（无 Windows 代码路径，源码中无 `AppData` 等 Windows 处理），但其"端点 + 凭据文件 + 解析字段"三层机制可以整体移植到本仓库（Rust, Windows-first）。

## 结论先行：可移植性排序

| Provider | 数据端点 | Token 来源 | Windows 可行性 | 移植工作量 | 优先级建议 |
| --- | --- | --- | --- | --- | --- |
| **Claude Pro/Max** | `GET https://api.anthropic.com/api/oauth/usage`（需 `anthropic-beta: oauth-2025-04-20` 头） | `%USERPROFILE%\.claude\.credentials.json`（macOS 上另有 Keychain `Claude Code-credentials`，Windows 无此层） | **高**：Windows 上 Claude Code 明文落盘 credentials.json，直接读 | 低-中（端点+解析 1 天；refresh 可选再 +1 天） | **P0，首选** |
| Codex/ChatGPT | `GET https://chatgpt.com/backend-api/wham/usage`（需 `ChatGPT-Account-Id` 头） | `%USERPROFILE%\.codex\auth.json` 的 `tokens.access_token` | **高**（本仓库已走官方 `codex app-server`，此为平行直连方案） | 低（已有 app-server，直连可作回退） | P1（备援） |
| z.ai / GLM | `GET https://api.z.ai/api/monitor/usage/quota/limit`（CN：`https://open.bigmodel.cn/...` 同路径） | 用户配置 API key 或 `Z_AI_API_KEY`/`ZHIPU_API_KEY` 等环境变量 | **高**：纯 API key，无本地文件依赖 | 低（1 个 GET + limits[] 解析） | **P0** |
| Kimi Code | `GET https://api.kimi.com/coding/v1/usages`（国际站 `api.kimi.ai`） | API key（`KIMI_CODE_API_KEY`），或只读 `~/.kimi-code/credentials/kimi-code.json` 的短期 access token | **高**（API key 路径零依赖；CLI 凭据文件路径是 home 相对的） | 低 | P1 |
| MiniMax | `GET https://platform.minimax.io/v1/api/openplatform/coding_plan/remains`（CN：`platform.minimaxi.com`） | Coding Plan token（`sk-cp-*`，`MINIMAX_CODING_API_KEY`）或手动 Cookie 头 | **高**（token 路径）；浏览器 Cookie 自动导入是 macOS 专属，跳过 | 低 | P1 |
| DeepSeek | `GET https://api.deepseek.com/user/balance` | `DEEPSEEK_API_KEY` | **高**：纯 API key；注意只有余额、无 5h/周窗口 | 低 | P2 |
| Moonshot/Kimi 开放平台 | `GET https://api.moonshot.ai/v1/users/me/balance`（CN：`api.moonshot.cn`） | `MOONSHOT_API_KEY` | **高**：只有余额（`available_balance`/`voucher_balance`/`cash_balance`），无窗口配额 | 低 | P2 |
| Copilot | `GET https://api.github.com/copilot_internal/user` | GitHub OAuth Device Flow（`POST github.com/login/device/code`）自取 token | **高**：Device Flow 是纯 HTTP，跨平台 | 中（需实现 device flow 轮询） | P2 |
| Cursor | `GET https://cursor.com/api/usage-summary` + `/api/auth/me` | SQLite 读 `%APPDATA%\Cursor\User\globalStorage\state.vscdb` 的 `ItemTable` → `cursorAuth/accessToken`（JWT，只读不刷新） | **高**：`state.vscdb` 是 VS Code 式 SQLite，Windows 路径已知 | 中（SQLite 只读 + UTF-16LE BLOB 解码坑） | P2 |
| Gemini（CLI OAuth） | `POST https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota` | `%USERPROFILE%\.gemini\oauth_creds.json`；refresh 需从本机 Gemini CLI 安装里正则提取 OAuth client_id/secret | **中**：凭据文件可读，但 client secret 提取要扫 npm 全局目录，内部端点无稳定性承诺 | 中-高 | P3 |

> 与本仓库的关系：[native-quota-sources.md](native-quota-sources.md) 已落地 Codex app-server / OpenCode Go / GOAT / Antigravity 四家。本报告补齐 Claude 订阅、z.ai、Kimi、MiniMax、DeepSeek、Cursor、Copilot、Gemini 八家的官方/半官方入口，全部来自 CodexBar 已在生产的实现，可信度高。

---

## Claude Pro/Max 订阅配额（重点）

### 端点与请求

CodexBar 的 OAuth 路径调用两个端点（[ClaudeOAuthUsageFetcher.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthUsageFetcher.swift)，L61-93）：

```
GET https://api.anthropic.com/api/oauth/usage     # 配额窗口
GET https://api.anthropic.com/api/oauth/profile   # 账号身份（account.uuid / email / organization.uuid）
```

请求头（L84-93）：

```
Authorization: Bearer <access_token>
anthropic-beta: oauth-2025-04-20        # 必须，注释写明 "OAuth usage endpoint currently requires the beta header"
User-Agent: claude-code/<version>       # 伪装 Claude Code CLI；版本号从环境探测，缺省 2.1.0（L65, L189-192）
Accept: application/json
Content-Type: application/json
```

`User-Agent` 必须长得像 Claude Code（`claude-code/<version>`），`ProviderVersionDetector.claudeVersion` 会从本机安装探测真实版本。429 时读取 `Retry-After` 并在客户端做 per-token 限流门（`ClaudeOAuthUsageRateLimitGate`，L73, L105-112）。

### 响应字段（5h / 周窗口 / plan）

`OAuthUsageResponse` 解码（[同文件 L261-356](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthUsageFetcher.swift)）：

```json
{
  "five_hour":  { "utilization": 0.42, "resets_at": "..." },
  "seven_day":  { "utilization": 0.18, "resets_at": "..." },
  "seven_day_opus":    { ... }, "seven_day_sonnet": { ... },
  "seven_day_oauth_apps": { ... }, "seven_day_routines": { ... },
  "iguana_necktie": { ... },
  "extra_usage": { "is_enabled": true, "monthly_limit": 100, "used_credits": 12.5, "utilization": 0.125, "currency": "USD" },
  "limits": [ { "kind": "weekly_scoped", "group": "weekly", "percent": 30,
                "resets_at": "...", "is_active": true,
                "scope": { "model": { "id": "...", "display_name": "..." } } } ]
}
```

映射关系（[docs/claude.md](https://github.com/steipete/CodexBar/blob/main/docs/claude.md) "OAuth API (preferred)" 节）：`five_hour` → 会话窗口；`seven_day` → 周窗口（`five_hour` 缺失时的回退）；`seven_day_opus`/`seven_day_sonnet` → 模型级周窗口；新版 `limits[].weekly_scoped` → 模型级周窗口（`scope.model.display_name` 命名）；`extra_usage` → 月度额外花费。

**Plan 层级**：凭据 JSON 里的 `subscriptionType` 优先，`rateLimitTier` 回退映射 Max/Pro/Team/Enterprise；Max 的 `default_claude_max_5x` / `default_claude_max_20x` 显示为 "Max 5x" / "Max 20x"（[docs/claude.md](https://github.com/steipete/CodexBar/blob/main/docs/claude.md) "Plan inference"；[ClaudePlan.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudePlan.swift)）。

### OAuth token 获取（对 Windows 最关键）

凭据解析顺序（[docs/claude.md](https://github.com/steipete/CodexBar/blob/main/docs/claude.md) "Credentials" 列表；实现见 [ClaudeOAuthCredentials.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthCredentials.swift)）：

1. 显式环境变量覆盖；
2. CodexBar 自己的 OAuth 缓存 Keychain（CodexBar 私有，与我们无关）；
3. **文件回退：`~/.claude/.credentials.json`**（`CLAUDE_CONFIG_DIR` / `CLAUDE_SECURESTORAGE_CONFIG_DIR` 可改根，见 [ClaudeConfigPaths.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudeConfigPaths.swift) L82-96）；
4. macOS Keychain 服务名 **`Claude Code-credentials`**（[ClaudeOAuthCredentials.swift L15](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthCredentials.swift)；实验性外部读取走 `/usr/bin/security find-generic-password -s "Claude Code-credentials" [-a account] -w`，见 [ClaudeOAuthCredentials+SecurityCLIReader.swift L276-300](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthCredentials+SecurityCLIReader.swift)）。

**Windows 上没有第 4 层**（macOS Keychain 是 macOS 专属机制），但第 3 层是通用 POSIX home 路径——Claude Code 在 Windows 上把同一份 JSON 写到 `%USERPROFILE%\.claude\.credentials.json`，直接读即可。JSON 结构（[ClaudeOAuthCredentialModels.swift L116-144](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthCredentialModels.swift)）：

```json
{ "claudeAiOauth": {
    "accessToken": "...", "refreshToken": "...",
    "expiresAt": 1735401600000,          // epoch 毫秒
    "scopes": ["user:profile", "user:inference"],
    "rateLimitTier": "default_claude_max_5x",
    "subscriptionType": "...", "emailAddress": null } }
```

注意 [docs/claude.md](https://github.com/steipete/CodexBar/blob/main/docs/claude.md)：usage 端点**要求 token 带 `user:profile` scope**；`claude setup-token` 产出的长周期 token 只有 `user:inference`，调不了 usage。另外 Claude Code 2.1.x 的 Keychain 条目可能只剩 MCP OAuth（无 `claudeAiOauth`），文件路径同理需要校验 `claudeAiOauth` 键存在（[ClaudeOAuthCredentialModels.swift L94](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthCredentialModels.swift)）。

### Refresh token 处理

CodexBar 会自行刷新（这点与 Codex 不同）：

- 端点：`POST https://platform.claude.com/v1/oauth/token`，`Content-Type: application/x-www-form-urlencoded`，body 为 `grant_type=refresh_token` + `refresh_token` + `client_id`（[ClaudeOAuthCredentials.swift L28, L1404-1420](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthCredentials.swift)）。
- `client_id = 9d1c250a-e61b-44d9-88ed-5944d1962f5e`——Claude Code 的公开 OAuth client ID，写死在源码并可用 `CODEXBAR_CLAUDE_OAUTH_CLIENT_ID` 覆盖（L25-33）。
- 响应 `TokenRefreshResponse{ access_token, refresh_token?, expires_in }`：**refresh token 可轮换**，响应里带新的就替换（L1460-1468）。
- 失败分类：`invalid_grant` 类（401/400）视为终态并要求重新 `claude login`；其他失败走指数退避（`ClaudeOAuthRefreshFailureGate`）。
- 表单编码必须保留字面 `+` 号（[docs/claude.md](https://github.com/steipete/CodexBar/blob/main/docs/claude.md)："OAuth refresh form-encodes credential values, preserving literal plus signs"）——Rust 侧用标准 form 编码即可，但别对 refresh_token 做 `+ → 空格` 的天真替换。

**对我们最省事的路线**：只读 `expiresAt`，过期不自行 refresh，而是提示用户跑一次 `claude` / `claude login`（Claude Code 会自己续期并重写 credentials.json）；或者完整抄上面的 refresh（client_id 是公开常量，无秘密）。

---

## Codex / ChatGPT 配额

凭据：`~/.codex/auth.json`（或 `$CODEX_HOME/auth.json`），JSON 形如 `{ "OPENAI_API_KEY": null, "tokens": { "id_token", "access_token", "refresh_token", "account_id" }, "last_refresh": "..." }`（[docs/codex-oauth.md](https://github.com/steipete/CodexBar/blob/main/docs/codex-oauth.md)，源自 `codex-rs/core/src/auth/storage.rs`）。

端点（[docs/codex.md](https://github.com/steipete/CodexBar/blob/main/docs/codex.md)；[docs/codex-oauth.md](https://github.com/steipete/CodexBar/blob/main/docs/codex-oauth.md)；实现 [CodexOAuthUsageFetcher.swift L380-415](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Codex/CodexOAuth/CodexOAuthUsageFetcher.swift)）：

```
GET https://chatgpt.com/backend-api/wham/usage
Headers:
  Authorization: Bearer <access_token>
  ChatGPT-Account-Id: <account_id>
  User-Agent: CodexBar
GET https://chatgpt.com/backend-api/wham/rate-limit-reset-credits   # 可选，重置积分
```

响应（docs/codex-oauth.md，源自 `codex-rs/backend-client/src/client.rs:161-170`）：`plan_type`（pro/plus/team/business/...）、`rate_limit.primary_window` / `secondary_window`（各含 `used_percent`、`reset_at`（epoch 秒）、`limit_window_seconds`：5h=18000、周=604800）、`credits{has_credits, unlimited, balance}`、模型级 `additional_rate_limits[]`（如 GPT-5.3-Codex-Spark）。

刷新策略（重要设计决策）：**CodexBar 不刷新 Codex token、不写 auth.json**。Codex CLI 拥有该文件的刷新生命周期；CodexBar 只从 JWT `exp`（5 分钟提前量）或 `last_refresh`（8 天规则）判断新鲜度，过期则报 `nativeRefreshRequired` 并把恢复动作委托给 `codex login`（[docs/codex-oauth.md](https://github.com/steipete/CodexBar/blob/main/docs/codex-oauth.md) "Token Freshness and Ownership"）。Windows 上 `auth.json` 同样在 `%USERPROFILE%\.codex\auth.json`。本仓库已优先走官方 `codex app-server`（见 [native-quota-sources.md](native-quota-sources.md)），wham/usage 直连可作为无 Codex CLI 时的受限回退。

---

## z.ai / GLM（BigModel）

纯 API token，无 Cookie（[docs/zai.md](https://github.com/steipete/CodexBar/blob/main/docs/zai.md)）。

- 端点：`GET https://api.z.ai/api/monitor/usage/quota/limit`（Global）；中国大陆为 `GET https://open.bigmodel.cn/api/monitor/usage/quota/limit`。可用 `Z_AI_API_HOST` / `Z_AI_QUOTA_URL` 覆盖（必须是显式 HTTPS，http 一律拒绝——fail-closed 设计）。
- 请求头：`authorization: Bearer <token>`、`accept: application/json`。**团队模式**追加 `type=2`（团队配额）/ `type=3`（团队按小时模型用量）查询参数和 `Bigmodel-Organization: <org>`、`Bigmodel-Project: <proj>` 头；两个头缺一个 API 会返回 200 + 空 `data`（docs 专门警告这个坑）。
- Token 来源回退链：config `providers[].apiKey` → `Z_AI_API_KEY` → （仅 CN 区）`BIGMODEL_API_KEY`/`ZHIPU_API_KEY`/`ZHIPUAI_API_KEY`/`GLM_API_KEY` → （仅 CN 区）单行文件 `~/.coding-relay/glm-api-key`、`~/.config/bigmodel/api_key`、`~/.config/zhipu/api_key`。
- 响应解析：`data.limits[]`（`TOKENS_LIMIT` 与 `CREDIT_LIMIT` 都算 Coding Plan 窗口，`TIME_LIMIT` 单独做 MCP 泳道）、`data.planName`（或 `plan`/`plan_type`/`packageName`/`level`）→ plan 标签；百分比取整数 `percentage`，或有 `usage` + `currentValue`/`remaining` 时计算并 clamp 0-100；`nextResetTime`（epoch 毫秒）→ 重置时间；`usageDetails[]` → 按模型明细。解析逻辑在插件 [zai.js](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Resources/Plugins/zai.js)。
- Windows：完全可移植（环境变量/用户配置 key），是工作量最低的一档。

---

## Kimi（Kimi Code / Kimi for Coding）

Kimi Code 订阅与 Moonshot 开放平台是**两个 provider**（[docs/kimi.md](https://github.com/steipete/CodexBar/blob/main/docs/kimi.md)）。

- **API key 路径（推荐）**：`GET https://api.kimi.com/coding/v1/usages`（中国区）/ `https://api.kimi.ai/coding/v1/usages`（国际区），Bearer 认证。响应兼容两种形状：旧计数型 `usage{limit, used, remaining, resetTime}` + `limits[]{window{duration, timeUnit}, detail{...}}`，和新 ratio 池（5h/周/月 `Total usage` 窗口直接给百分比）。会员档位：Andante 1024 请求/周、Moderato 2048、Allegretto 7168；全部 200 请求/5h。
- **CLI 凭据路径**：只读 `~/.kimi-code/credentials/kimi-code.json` 的短期 access token，并附带与官方 CLI 相同的设备指纹头（hostname、OS 信息、`~/.kimi-code/device_id`）。**铁律：绝不使用其 refresh token、绝不回写该文件**——Kimi 轮换 refresh token，自己刷新会让 CLI 保存的 token 失效（docs/kimi.md "Method 2"）。token 只有 15 分钟寿命，过期提示用户跑一次 `kimi`。
- **Web Cookie 路径**：`POST https://www.kimi.com/apiv2/kimi.gateway.billing.v1.BillingService/GetUsages`，`Authorization: Bearer <kimi-auth JWT cookie>`，响应 `usages[]{scope:"FEATURE_CODING", detail, limits[]}`。Cookie 自动导入依赖 macOS 浏览器解密（Windows 不做），但**手动粘贴 token** 跨平台可用；另有 `KIMI_AUTH_TOKEN` 环境变量。
- 认证优先级：API key → 新鲜 CLI token → 手动 token → 环境变量 → Desktop cookie → 浏览器 cookie → Chromium localStorage（docs/kimi.md "Authentication Priority"）。
- Windows：API key 路径完全可移植；CLI 凭据文件读取可移植（注意 15 分钟短寿命的设计假设）。

---

## Moonshot / Kimi 开放平台

与 Kimi Code 分离的 API 计费面（[docs/moonshot.md](https://github.com/steipete/CodexBar/blob/main/docs/moonshot.md)）：

- 端点：`GET https://api.moonshot.ai/v1/users/me/balance`（国际）/ `https://api.moonshot.cn/v1/users/me/balance`（中国），`Authorization: Bearer <key>`。
- 响应：`available_balance`、`voucher_balance`、`cash_balance`（国际站 USD、国内站 CNY，原样展示不换算；`cash_balance` 为负显示欠费）。
- **无任何 5h/周窗口**——开放平台不通过 API 暴露窗口配额。Token 来源：config 或 `MOONSHOT_API_KEY`/`MOONSHOT_KEY`，key 绑定区域，换区不发送。
- Windows：完全可移植，工作量最低，但信息价值只有余额。

---

## MiniMax

（[docs/minimax.md](https://github.com/steipete/CodexBar/blob/main/docs/minimax.md)；实现 [MiniMaxUsageFetcher.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/MiniMax/MiniMaxUsageFetcher.swift)、[MiniMaxAPIRegion.swift L9](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/MiniMax/MiniMaxAPIRegion.swift)）

- **Token 路径**：Coding Plan API token（`sk-cp-*` 前缀，`MINIMAX_CODING_API_KEY` 优先于 `MINIMAX_API_KEY`，防止普通 `sk-api-*` 掩盖 coding-plan key）。端点：`GET https://platform.minimax.io/v1/api/openplatform/coding_plan/remains`（Global）/ `https://platform.minimaxi.com/v1/api/openplatform/coding_plan/remains`（中国大陆）；Global 404 或凭证被拒时自动重试中国区主机。订阅元数据另有 `v1/api/openplatform/charge/combo/cycle_audio_resource_package`。
- **Web 路径**：浏览器 Cookie（Chromium localStorage/sessionStorage/IndexedDB 补充，均为只读）或手动粘贴的 `Cookie:` 头（也接受整条 "Copy as cURL"）；`MINIMAX_COOKIE`/`MINIMAX_COOKIE_HEADER` 环境变量兜底。可选账单历史端点映射 30 天 token 趋势与 top 模型，失败不拖垮配额卡。
- 解析：主要用量/重置/plan 取自 Coding Plan 响应字段或页面文本；快照容忍部分字段缺失，不因图表数据坏而判整卡失败。
- Windows：token 路径完全可移植；Cookie 自动导入（浏览器解密）跳过，手动 Cookie 头可用。

---

## DeepSeek

（[docs/deepseek.md](https://github.com/steipete/CodexBar/blob/main/docs/deepseek.md)）

- **API key 余额**：`GET https://api.deepseek.com/user/balance`，Bearer。响应 `is_available` + `balance_infos[]`（每币种 `total_balance`/`granted_balance`/`topped_up_balance`；多币种优先显示 USD）。
- **Platform 会话（可选增强）**：私有仪表盘端点，`GET https://platform.deepseek.com/api/v0/users/get_user_summary`；明细用量 `GET /api/v0/usage/by_api_key/amount|cost?start=<unix>&end=<unix>&tz=<offset>`（回退 `/api/v0/usage/amount?month=&year=`），请求头 `Authorization: Bearer <userToken>` + `x-client-platform: web`。docs 明确警告这些是**私有端点，随时可能变更**。
- `userToken` 来源：`DEEPSEEK_PLATFORM_TOKEN`/`DEEPSEEK_USER_TOKEN` 环境变量，或从 Chrome 的 `https://platform.deepseek.com` localStorage 读取（macOS 实现；Windows 上需自读 Chrome 的 LevelDB + DPAPI，成本高，建议只做环境变量/手动输入）。
- 认证失败码 `40002`/`40003` 视为会话过期。
- **无 5h/周窗口**——DeepSeek 不通过 API 暴露窗口配额，卡面只有余额 + 可选花销明细。
- Windows：API key 路径完全可移植。

---

## Cursor

（[docs/cursor.md](https://github.com/steipete/CodexBar/blob/main/docs/cursor.md)）

- **Cursor.app 本地 token（首选）**：只读 SQLite 打开 Cursor 的 VS Code 式全局状态库，查 `ItemTable` 键 `cursorAuth/accessToken`。路径：macOS `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`，Linux `$XDG_CONFIG_HOME|~/.config/Cursor/...`；**Windows 对应 `%APPDATA%\Cursor\User\globalStorage\state.vscdb`**（CodexBar 未实现 Windows，但机制同源——VS Code 系 SQLite 库路径规则固定）。关键坑：BLOB 解码要先试 **BOM-less UTF-16LE** 再试 UTF-8，否则 token 会带 NUL 字节；仅当 JWT 剩余有效期 > 60 秒时使用；**永不刷新**。读 WAL 库时活跃 WAL 正常读、无 sidecar 的闲置 WAL 用 immutable 模式打开，避免在 Cursor 目录里新建文件。
- **端点**：
  - `GET https://cursor.com/api/usage-summary` → 计划内用量、按需用量、账单周期；
  - `GET https://cursor.com/api/auth/me` → 稳定用户 ID / 邮箱 / 名称；
  - `GET https://cursor.com/api/usage?user=ID` → 旧版按请求计数的用量；
  - `POST https://cursor.com/api/dashboard/get-sand-usage-status` → Grok Bot 周用量（`usagePercent`、`nextResetTimestampUtc`），需同会话 Cookie 且带 `Origin: https://cursor.com`，best-effort。
- **Cookie 回退**（macOS 自动导入，Windows 手动）：域名 `cursor.com`/`cursor.sh`，任一 Cookie 即可：`WorkosCursorSessionToken`、`__Secure-next-auth.session-token`、`next-auth.session-token`；手动模式直接粘 `Cookie:` 头。
- Windows：SQLite 读 + 两个 GET 完全可移植；浏览器 Cookie 导入跳过。

---

## Copilot

（[docs/copilot.md](https://github.com/steipete/CodexBar/blob/main/docs/copilot.md)）

- **登录**：GitHub OAuth Device Flow——`POST https://github.com/login/device/code` 取设备码，`POST https://github.com/login/oauth/access_token` 轮询，scope 仅 `read:user`；企业版主机可配（`https://<enterprise>.ghe.com`）。token 存 CodexBar config。纯 HTTP，无平台依赖，Windows 完全可移植。
- **用量**：`GET https://api.github.com/copilot_internal/user`（企业为 `https://api.<enterpriseHost>/copilot_internal/user`），请求头：

```
Authorization: token <github_oauth_token>
Accept: application/json
Editor-Version: vscode/1.96.2
Editor-Plugin-Version: copilot-chat/0.26.7
User-Agent: GitHubCopilotChat/0.26.7
X-Github-Api-Version: 2025-04-01
```

  这套 vscode/Copilot Chat 伪装头是能否拿到数据的关键（实现 [CopilotUsageFetcher.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Copilot/CopilotUsageFetcher.swift)、[CopilotDeviceFlow.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Copilot/CopilotDeviceFlow.swift)）。
- 响应映射：主窗口 `quotaSnapshots.premiumInteractions.percentRemaining`，副窗口 `quotaSnapshots.chat.percentRemaining`；`copilotPlan` → plan 标签；`quota_snapshots.premium_interactions.credits_used` → 已用 AI 积分。**API 不提供重置日期；也不提供包含额度分母**——CodexBar 探测了全部 8 个计费端点都没有 ceiling，分母只能让用户手填。
- **预算附加（默认关）**：`GET https://github.com/settings/billing/budgets?page=&page_size=10&scope=customer`，需要 github.com 浏览器 Cookie + `X-Requested-With: XMLHttpRequest` + `GitHub-Verified-Fetch: true` + `X-Fetch-Nonce`（先访问 HTML 页取 nonce）。这是刻意绕开 REST API 的 web 端点，脆弱。

---

## Gemini（Gemini CLI OAuth）

（[docs/gemini.md](https://github.com/steipete/CodexBar/blob/main/docs/gemini.md)）

- **凭据**：读 `~/.gemini/settings.json` 判断 auth 类型（只支持 `oauth-personal`；`api-key`/`vertex-ai` 直接报错），再读 `~/.gemini/oauth_creds.json`（`access_token`、`refresh_token`（可选）、`id_token`、`expiry_date`）。
- **配额**：`POST https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota`，body `{"project": "<projectId>"}`（未知则 `{}`），头 `Authorization: Bearer <access_token>`。响应按模型给桶：`remainingFraction`、`resetTime`（ISO-8601）、`modelId`；每个模型取最低 `remainingFraction` × 100 作为 percentLeft；Pro 系做主窗口、Flash 系做副窗口。
- **Plan 探测**：`POST https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist`，body `{"metadata": {"ideType": "GEMINI_CLI", "pluginType": "GEMINI"}}`；`paidTier.name` 优先，回退 `standard-tier` → "Paid"。quota 用的 project ID 取 `loadCodeAssist` 的 `cloudaicompanionProject`，回退 `GET https://cloudresourcemanager.googleapis.com/v1/projects` 挑 `gen-lang-client*`。
- **Token 刷新**：`POST https://oauth2.googleapis.com/token`，form body：`client_id`、`client_secret`、`refresh_token`、`grant_type=refresh_token`。**client_id/secret 的获取是移植难点**：从本机安装的 Gemini CLI 的 `oauth2.js`（`@google/gemini-cli-core/dist/src/code_assist/oauth2.js`）里正则提取 `OAUTH_CLIENT_ID`/`OAUTH_CLIENT_SECRET`，或用 `GEMINI_OAUTH_CLIENT_ID`/`GEMINI_OAUTH_CLIENT_SECRET` 环境变量覆盖。Windows 上要扫 npm 全局前缀（`%APPDATA%\npm\node_modules\...`）。
- Windows：凭据文件可读（`%USERPROFILE%\.gemini\`）；两个 `v1internal` 端点是 Google 内部接口，无稳定性承诺；整体评级中-低优先。

---

## 架构：Provider 注册与凭据抽象（值得模仿的部分）

- **Descriptor 注册表**：`ProviderManifest.allDescriptors` 是脚本生成的第一方引导列表（[ProviderManifest.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/ProviderManifest.swift)），运行时再经 `ProviderDescriptorRegistry.register(_:)` 补充插件注册；`docs/architecture.md` 说明 Core 的公开 descriptor registry 维护一个有序集合加索引。每个 `ProviderDescriptor` 携带元数据、凭据解析器（`credentials.resolveToken(kind, environment, authFileURL)`）和 fetch 策略链（`ProviderFetchPlan`），如 [ProviderTokenResolver.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/ProviderTokenResolver.swift) 所示——统一入口 `ProviderTokenResolver.token(for:)` 把"找 token"从 fetch 逻辑里剥离。
- **策略泛型**：[APITokenFetchStrategy.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/APITokenFetchStrategy.swift) 是"API-key 类 provider"的通用骨架：`resolveToken(env)` → `loadUsage(token, ctx)` → `UsageSnapshot`，`isAvailable` 就是 token 是否存在。纯 API-key provider（z.ai、Moonshot、DeepSeek、MiniMax token 路径等）几乎零定制。
- **JS 插件解析层**：响应解析下沉为 TypeScript 插件（`Sources/CodexBarCore/Resources/Plugins/*.ts` 编译为 `.js`），在 **QuickJS**（Linux）/ JavaScriptCore（macOS）沙箱里运行（[ProviderPluginRuntime.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Plugins/ProviderPluginRuntime.swift)、[QuickJSProviderPluginEngine.swift](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Plugins/QuickJSProviderPluginEngine.swift)）。插件契约（[codexbar-plugin.d.ts](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Resources/Plugins/codexbar-plugin.d.ts)）：`defineProvider({ id, auth: {type:"bearer"|"x-api-key"|"header"|"authorization-scheme", secret}, endpoints, async fetchUsage(ctx) { ctx.http.getJSON(url) → {primary:{usedPercent, windowMinutes, resetsAt}, details} } })`；宿主负责 HTTP 与凭据，插件只做解析，禁 Node/文件/子进程，源码上限 1 MiB（[docs/plugins.md](https://github.com/steipete/CodexBar/blob/main/docs/plugins.md)）。对我们可借鉴为"Rust 端点+凭据层 / 数据形状归一化层"分离，或直接用 `deno_core`/`rquickjs` 做热更新解析。
- **多账号**：`providers[].tokenAccounts`（`~/.config/codexbar/config.json`，legacy `~/.codexbar/`，`CODEXBAR_CONFIG` 可覆盖）统一存多账号 token + label + 区域 + usageScope，所有 provider 共用一套账号编辑器与 CLI `--account` 机制。

## macOS 专属 vs 可移植到 Windows

| 机制 | 平台 | Windows 对策 |
| --- | --- | --- |
| Keychain 读凭据（`Claude Code-credentials`、CodexBar 缓存项、Chromium Safe Storage） | macOS 专属 | 不需要：Claude 在 Windows 走 `~/.claude/.credentials.json` 明文文件；Chromium Cookie 解密（DPAPI）不建议做 |
| Safari `Cookies.binarycookies` / Chromium Cookie 解密 / Firefox `cookies.sqlite` 自动导入 | macOS 专属（Linux 版也没有，见 [MAC_COMPARISON.md](https://github.com/steipete/CodexBar/blob/main/Integrations/Linux/MAC_COMPARISON.md)："browser credential imports ... remain outside"） | 全部跳过；各 provider 均有"手动粘贴 token/Cookie 头"或 API key 等价物 |
| Chrome localStorage 读 `userToken`（DeepSeek、Kimi） | macOS 实现 | 可选自研（Chrome LevelDB + DPAPI），成本高，建议环境变量替代 |
| `~/.claude/.credentials.json`、`~/.codex/auth.json`、`~/.gemini/oauth_creds.json`、`~/.kimi-code/credentials/kimi-code.json` | 跨平台 home 相对路径 | 直接读 `%USERPROFILE%\...` 等价路径 |
| Cursor `state.vscdb` SQLite 只读 | macOS/Linux 实现；Windows 路径可推 | `%APPDATA%\Cursor\User\globalStorage\state.vscdb`，rusqlite immutable/WAL 规则照抄 |
| Copilot Device Flow、Claude OAuth refresh（公开 client_id）、Gemini OAuth refresh | 纯 HTTP | 直接移植 |
| Claude/Codex CLI PTY 自动化（`/usage`、`/status` 屏幕解析） | 跨平台但复杂 | 不建议移植：有官方 OAuth/wham 端点就不必解析终端画面 |

## 协议与合规

- [LICENSE](https://github.com/steipete/CodexBar/blob/main/LICENSE)：**MIT License, Copyright (c) 2026 Peter Steinberger**。本项目（MIT）把 CodexBar 当参考实现阅读、按其端点/字段重新用 Rust 实现完全合法；端点 URL、JSON 字段名、请求头组合属于事实接口，不受版权保护。若直接复制大段代码（如 Swift 解析逻辑的逐行翻译），需在副本中保留其 MIT 版权与许可声明（`docs/THIRD_PARTY_LICENSES.md` 亦是其自身的第三方声明清单）。注意：调用这些**非公开文档化端点**（Anthropic OAuth usage、chatgpt.com wham、Cursor、Copilot internal、Google v1internal）本身是各工具厂商已默许的灰色地带，CodexBar 已公开运行多年，但我们仍应按其做法做 fail-closed、限流与降级。

## 可直接抄的要点 / 需要绕开的坑

### 可直接抄的要点

1. **Claude 一条龙**：`GET api.anthropic.com/api/oauth/usage` + `anthropic-beta: oauth-2025-04-20` + `User-Agent: claude-code/x` + `%USERPROFILE%\.claude\.credentials.json` 的 `claudeAiOauth` 块 → `five_hour`/`seven_day` 百分比 + `subscriptionType`/`rateLimitTier`（含 5x/20x 乘数）。刷新用公开 client_id `9d1c250a-e61b-44d9-88ed-5944d1962f5e` 打 `platform.claude.com/v1/oauth/token`。
2. **Codex 直连**：`GET chatgpt.com/backend-api/wham/usage` + `ChatGPT-Account-Id` 头 → `primary_window`/`secondary_window`{used_percent, reset_at, limit_window_seconds} + `plan_type`；**刷新交给 Codex CLI，自己只读不写**。
3. **Copilot 伪装头**：`Editor-Version`/`Editor-Plugin-Version`/`User-Agent: GitHubCopilotChat/x`/`X-Github-Api-Version` 四件套缺一不可；Device Flow scope 只需 `read:user`。
4. **Gemini 双内部端点**：`retrieveUserQuota`（配额桶）+ `loadCodeAssist`（tier 与 project ID），每模型取最低 `remainingFraction`。
5. **通用策略骨架**：descriptor/registry + `resolveToken` + 策略链 + `tokenAccounts` 多账号配置的分层，适合作为我们 provider 模块的组织模板；解析层可以做成与 HTTP 层解耦的独立 trait。
6. **429 纪律**：读 `Retry-After`，按 token 记忆冷却窗口（CodexBar 有 per-token gate），且 429 时保留旧快照标"stale"而不是清空。
7. **fail-closed 的端点覆盖**：所有 provider 的 URL 覆盖只收显式 HTTPS（z.ai/MiniMax docs 明确 http 拒绝，防止 Bearer 泄明文）——值得照抄进我们的配置校验。

### 需要绕开的坑

1. **Claude token scope**：usage 端点要求 `user:profile` scope；`claude setup-token` 的长周期 token 调不了 usage，别把它当凭据来源。2.1.x 凭据可能只有 `mcpOAuth` 没有 `claudeAiOauth`，需校验并提示重新登录。
2. **Claude 429 很敏感**：OAuth usage 端点被 Anthropic 主动限流，轮询间隔要保守（CodexBar 默认长间隔 + 手动刷新文化），否则会持续 429。
3. **Codex auth.json 是共享文件**：绝不能在自己的进程里刷新并回写（会和 Codex CLI 竞争），只读 + JWT `exp` 判断 + 引导用户跑 `codex login`。
4. **Kimi refresh token 轮换**：绝不能在内存里刷新 CLI 凭据，会让官方 CLI 的已存 token 失效；CLI access token 只有 15 分钟寿命，设计上只能当"有 CLI 就顺手用"。
5. **z.ai 团队模式**：`Bigmodel-Organization`/`Bigmodel-Project` 头缺一个时 API 返回 200 + 空 `data`，会被误判为"无用量"；还有 5h 窗口重置时间超过 5h+1min 的"假窗口"要过滤（时区不得猜测修正）。
6. **MiniMax**：`MINIMAX_CODING_API_KEY`（sk-cp-）优先于 `MINIMAX_API_KEY`（sk-api-），顺序反了会拿错凭据；Global 端点 404 要回退中国区主机。
7. **DeepSeek 平台端点私有**：`platform.deepseek.com/api/v0/*` 随时会变，且必须带 `x-client-platform: web`；认证码 40002/40003 = 会话过期。基础卡只依赖 `api.deepseek.com/user/balance` 这个相对稳定的端点。
8. **Copilot 分母未知**：API 只给 percent remaining 和 credits_used，没有重置日期、没有包含额度天花板——进度条分母要么用户手填，要么不画条，避免编造。
9. **Cursor BLOB 编码**：`cursorAuth/accessToken` 可能是 UTF-16LE 编码的 BLOB，先按 UTF-16LE 试再 UTF-8，否则 token 带 NUL；JWT 有效期不足 60 秒直接当过期，且永不刷新。
10. **Gemini client secret**：从本机 Gemini CLI 安装文件正则提取 client secret 的做法随安装方式（npm/bun/Homebrew）漂移，Windows 下维护成本高；抓不到就要求用户手动提供或降级不刷新。
11. **浏览器 Cookie 自动导入是 macOS 专属能力**（Keychain/沙箱/Full Disk Access），Windows 上不要试图复刻 Chromium Cookie DPAPI 解密；所有 provider 都留了手动 token/API key 后门，走这些即可。
