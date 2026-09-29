# 安全策略

## 报告漏洞

请通过 GitHub [Private Security Advisory](https://github.com/jryang1997/tui-token-monitor/security/advisories/new) 私下报告,不要开公开 issue。会在 7 天内回应。

本项目处理凭据与本地会话数据,凡涉及以下内容的报告请格外标注复现步骤:

- 凭据泄漏(API key 出现在日志、导出文件、进程参数、core dump)
- 越权写入(修改任何 CLI 的自有状态文件)
- 注入面(toast 文案、CSV/JSON 导出、模型名渲染中的转义)

## 数据与凭据边界(设计承诺)

- 全部数据留在本机,无遥测、无上报。
- API key 只存 Windows 凭据管理器(`TuiTokenMonitor/*` 目标),不写 settings.json,不进前端存储。
- 对各 CLI 的登录态文件(`~/.claude/.credentials.json`、`~/.codex/auth.json`、OpenCode `auth.json`)**只读**;不代刷 OAuth token(refresh token 是旋转式的,代刷会使 CLI 自身登录态失效)。
- CPA 管理密钥存于 `~/.tui-token-monitor/cpa-key`(0600 语义由用户自行保证),只发往本机 `127.0.0.1:8317`。

## 已接受的风险(非漏洞)

- 部分配额端点为未文档化内部接口(Codex wham、GOAT `/alpha/billing`、Google `retrieveUserQuotaSummary`、Claude `oauth/usage`):无稳定性承诺,随时可变,失败降级为 Missing 卡。
- `CRED_PERSIST_LOCAL_MACHINE`:同一台机器的其他本地账户在 Windows 上可读取凭据管理器中该持久化级别的条目。如不接受,请改用环境变量传递 key。
- Windows 凭据管理器存储按当前用户会话解密;请勿在不受信任的机器上保存 key。
