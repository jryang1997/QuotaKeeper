# CPA 反代的 Antigravity 用量读取路径

核对日期：2026-09-27。本文区分 **Google 账号配额**、**CPA 代理请求/token 遥测**、**Google AI Premium credits**；三者不可互相推算。之前 [native-quota-sources.md](native-quota-sources.md) 所述“Antigravity 没有可读数字路径”已过时：CPA 当前管理 WebUI 已实现上游配额读取；Google 官方 CLI 也新增了非交互 JSON 路径。

## 结论与优先级

| 路径 | 读到什么 | 依赖和定位 |
| --- | --- | --- |
| **CPA 管理 API `/auth-files` → `/api-call` → Google `retrieveUserQuotaSummary`** | 每个 CPA Antigravity 凭据的分组配额，包含 `remainingFraction`、窗口、重置时间；另用 `loadCodeAssist` 取 tier | 最贴合“CPA 反代正在用的哪个账号”，但 Google RPC 是内部接口，CPA 管理 key 有高权限；应标注“CPA 代查上游配额”。[CPA 官方 WebUI 额度实现](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/features/quota/providers/antigravity/data.ts) · [订阅实现](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/services/api/antigravitySubscription.ts) |
| **Google 官方 `agy -p "/usage" --output-format json`** | CLI 登录账号的真实额度；官方自 1.1.11 起将 `/usage` 提供为非交互结构化输出，且不启动模型回合 | 稳定性和授权边界更好，但 CLI 可能与 CPA 使用不同账号，不能不核对身份就标成“CPA 账号配额”。[Google 1.1.11 更新记录](https://www.antigravity.google/changelog) · [Headless 文档](https://antigravity.google/docs/cli/headless/) |
| **CPA `/usage-queue`** | 经代理处理的逐请求 token/延迟/失败等遥测 | 不是上游剩余额度；读取会把记录从队列弹出，不能作为监控 GUI 的无损轮询源，也可能影响已有消费者。[CPA 管理 API 文档](https://github.com/router-for-me/CLIProxyAPIDocs/blob/main/docs/en/management/api.md) |
| **现有 `auth-files` 健康 + OMP `stats.db`** | 代理凭据状态、本机记录活动 | 只能做辅助说明，不能显示为 Google 配额。[本项目当前实现](../../src/backend.rs) |

## CPA 当前官方管理 WebUI 的具体读法

1. 用管理 key 调 `GET http://127.0.0.1:8317/v0/management/auth-files`，筛选 `provider/type = antigravity` 且未禁用的凭据。响应的 `auth_index` 是稳定运行时索引，`status` / `unavailable` 表示代理健康。若没有可用 `auth_index`，不能代查。[CPA 管理 API 的 auth-files 契约](https://github.com/router-for-me/CLIProxyAPIDocs/blob/main/docs/en/management/api.md) · [管理 WebUI 实现](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/features/quota/providers/antigravity/data.ts)
2. 取得该凭据的 `project_id`：WebUI 依次检查 auth-file 条目的 `project_id`/`projectId`、`metadata`、`attributes.gemini_virtual_project`，最后才下载对应 auth JSON 读取。下载文件含敏感凭据，应尽量避免；找不到项目时明确显示不可用，不要传空 project 并接受可能虚假的 100%。[WebUI `resolveAntigravityProjectId`](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/features/quota/providers/antigravity/data.ts)
3. 用 `POST /v0/management/api-call` 代发 Google 上游 POST：`auth_index` 选中 CPA 正在管理的账号，header 中 `Authorization: Bearer $TOKEN$` 由 CPA 在服务端替换，当前源码还会为 Antigravity 刷新 access token；请求 `data` 是 JSON 字符串 `{"project":"<project_id>"}`。管理 API 要求管理 key，即使是 localhost；`/api-call` 的 HTTP 200 只表示代发成功，真正上游状态在 `status_code`，响应 JSON 字符串在 `body`。[CPA `/api-call` 文档](https://github.com/router-for-me/CLIProxyAPIDocs/blob/main/docs/en/management/api.md) · [CPA 代发与 token 刷新源码](https://github.com/router-for-me/CLIProxyAPI/blob/main/internal/api/handlers/management/api_tools.go)
4. 当前 WebUI 的首选 URL 是 `https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary`，之后尝试 `daily-cloudcode-pa.sandbox.googleapis.com` 与 `cloudcode-pa.googleapis.com`。返回 `groups[]`，每组 `displayName` 和 `buckets[]`；每个 bucket 可有 `bucketId`、`displayName`、`window`、`remainingFraction`、`resetTime`、`description`，字段也可能是 snake_case。`remainingFraction` 是**剩余比例**，显示已用比例需做 `100 × (1 - remainingFraction)`；原始窗口和重置时间应保存，不能凭套餐名生成固定数值。当前 WebUI 识别 `5h` 和 `weekly` 窗口。[URL/headers 源码](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/constants.ts) · [分组解析源码](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/builders.ts) · [响应类型](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/types/quota.ts)
5. 套餐单独调 `https://daily-cloudcode-pa.googleapis.com/v1internal:loadCodeAssist`，body `{"metadata":{"ideType":"ANTIGRAVITY"}}`，同样经 `/api-call` 和 `$TOKEN$`。WebUI 从 `paidTier` 优先、否则 `currentTier` 读 tier id/name；已知映射为 `free-tier`、`g1-pro-tier`、`g1-ultra-tier`、`g1-ultra-lite-tier`。套餐接口失败不应吞掉成功的额度；tier 不明时显示未知。[CPA 官方 WebUI 订阅源码](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/services/api/antigravitySubscription.ts)

可用于实现的**请求形状**（占位符不可原样使用；不要记录管理 key 或响应中的身份字段）：

```json
{
  "auth_index": "<auth-files 返回的 auth_index>",
  "method": "POST",
  "url": "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
  "header": {
    "Authorization": "Bearer $TOKEN$",
    "Content-Type": "application/json",
    "User-Agent": "antigravity/cli/<compatible-version>"
  },
  "data": "{\"project\":\"<该账号的 project_id>\"}"
}
```

注意：示例只描述 CPA 管理 WebUI 已采用的方式，**不等于 Google 对外承诺的公开 API**。WebUI 内的 User-Agent 拼装与多个 Cloud Code 主机均可能变化；在产品里应做响应校验、限频、明确的失败状态和版本兼容处理。[CPA WebUI URL 与 User-Agent](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/constants.ts) · [Google 官方公开 CLI 用量说明](https://antigravity.google/docs/cli/commands/usage/)

## 推荐落地约束

- **CPA 账号视图首选 CPA 代查上游**，按每个 `auth_index` 保留身份和配额；多个账号不得直接相加为一个“剩余额度”。如只显示一个摘要，必须说明取哪一个账号、按什么规则选取。CPA WebUI 自己也是按凭据缓存配额。[CPA 缓存源码](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/stores/useQuotaStore.ts)
- **先验证字段再绘图**：至少要求有非空 `groups[].buckets[]`、有限且在 `[0,1]` 的剩余比例、可识别的窗口；缺失的组/窗口不制造 100%，上游 403/404/429 明确显示不可用。`fetchAvailableModels` 的模型可用性响应不应冒充完整配额；Google/CPA 项目已有报告指出它可能显示 60–100% 但实际生成仍 429。[CPA 问题报告 #1015](https://github.com/router-for-me/CLIProxyAPI/issues/1015) · [Google CLI 403 报告 #785](https://github.com/google-antigravity/antigravity-cli/issues/785)
- **管理 key 必须只去环回地址**；`/api-call` 能任意代发带账号 token 的请求，是高权限接口。将 Google 目标主机写死，不接受前端任意 URL；限定响应大小、6–15 秒超时和约数分钟缓存。若用 `auth-files/download` 回退取 `project_id`，只在本机内存解析并丢弃全文，不写日志或遥测。[CPA 管理 API 安全说明](https://github.com/router-for-me/CLIProxyAPIDocs/blob/main/docs/en/management/api.md) · [CPA `/api-call` 实现](https://github.com/router-for-me/CLIProxyAPI/blob/main/internal/api/handlers/management/api_tools.go)
- **官方 CLI 可作为独立路径**：`agy -p "/usage" --output-format json`，先核对结构和登录身份，再显示官方 CLI 账号额度。不要与 CPA 账号混用；`-p` 命令须单独运行，不能塞入 `--input-format stream-json` 的持续会话。CLI 1.1.11 更新记录称该只读命令不启动 agent 回合、不消耗模型额度。CLI 仍是额外安装/登录依赖。[Google 更新记录](https://www.antigravity.google/changelog) · [Google Headless 说明](https://antigravity.google/docs/cli/headless/)
- **credits 与模型配额分开**：`/usage` 是模型配额；`/credits` 才是 AI Premium credit 余额/历史。Google 也将二者分别写成独立命令。[Google `/usage`](https://antigravity.google/docs/cli/commands/usage/) · [Google `/credits`](https://antigravity.google/docs/cli/commands/credits/)

## 本机验证结果

已确认本机 CPA 是 **7.3.15**。`/auth-files` 有 1 个启用的 Antigravity 凭据，含 `auth_index` 和 `project_id`；其 `quota.signals` 为空。用本机管理 key 经 `/api-call` 对首选 Google URL 发只读配额查询，**上游 `status_code=200`**，得到 2 组、4 个桶：Gemini 的 weekly/5h、Claude/GPT 的 weekly/5h。Gemini weekly 的 `remainingFraction=0.86`，即已用 14%；其余桶当时为 1.0。每桶均有 `resetTime`。仅记录了脱敏响应结构与数值，没有输出管理 key、账号邮箱、access token 或 auth 索引。应用中的端到端忽略测试 `local_cpa_returns_real_upstream_antigravity_quota` 也已通过。

这确认了**本机 CPA 账号可读取上游配额**，但不等于 Google 为外部程序发布了稳定 API。官方 CLI 路径尚未在本机验证：`agy` 不在当前 PATH。管理 WebUI README 声明最小兼容 CPA `≥ 7.2.147`；更旧的版本需按端点能力判断。[管理 WebUI README](https://github.com/router-for-me/Cli-Proxy-API-Management-Center)
