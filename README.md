# tui-token-monitor

终端里的三页监测台:宽窗口用左侧按钮切换,窄窗口自动让内容占满宽度;配额页用卡片和横向进度条,Token 用量页显示最近一小时活动、Agent/会话/项目三维度和可切换的堆叠趋势图,热力图页是 GitHub 风格贡献格 + 工作时段 Profile。中英双语(`l` 键),四套主题(`p` 键)。

## 范围(已确认,2026-09-26)

- **配额页**:OpenCode Go $10/mo(5h/周/月)+ Codex(ChatGPT Plus $20/mo,5h/7d)+ Command Code GOAT $10/mo(月 $70 / 5h $14 / 周 $35)+ **Antigravity(经 CPA 反代,unofficial)**.每条额度用文字和 █░ 横向进度条显示,行尾附窗口内匀速外推的预计耗尽时间(<1h 红色告警).
- **配额阈值通知**:任额度跨越 80% / 100% / 重置时弹 Windows toast(状态机去重,不重复轰炸).
- **采集链路**:OMP 优先 → 失败自动落 Codex 原生配额(`~/.codex/auth.json` OAuth → `chatgpt.com/backend-api/wham/usage`,口径照抄 CodexBar,unofficial);**Antigravity 卡**读 CPA(CLIProxyAPI)管理接口 `auth-files` 取凭证健康 + OMP stats.db 里 `provider='cpa'` 的今日 token 强度(今日 vs 近 7 日均值)——credits 百分比因官方端点对消费级账号 403 且当前 CPA 构建无该端点而不可用,以强度条替代;管理密钥放 `~/.tui-token-monitor/cpa-key`.
- **Token 用量页**:顶部显示近 60 分钟本机用量、用量最多的 Agent、最近记录和最紧张的真实订阅额度；下方保留今日/本周(ISO 周一起)/本月汇总与近 30 天按 Agent/会话/项目表。趋势图可切换滚动 24 小时或近 14 天、Token 或金额，按 Agent 堆叠并可点击时段看明细；`g` 切回每日表。六路数据源:

| Agent | 来源 | 成本 |
|---|---|---|
| Codex | `~/.codex/sessions/**/rollout-*.jsonl`(`last_token_usage` 增量) | LiteLLM 估价 |
| OMP | `~/.omp/stats.db` `messages`(官方预聚合) | **OMP 报告价** |
| OpenCode | `~/.local/share/opencode/opencode.db`(v1/v2 双 schema) | LiteLLM 估价 |
| ZCode | `~/.zcode/cli/db/db.sqlite` `model_usage`(exclusive 归一化) | LiteLLM 估价 |
| DeepSeek Harness | `~/.dsh/sessions/**/session*.jsonl.zstd`(多帧 zstd) | LiteLLM 估价 |
| Claude Code | `~/.claude/projects/**/*.jsonl`(`(message.id, requestId)` 去重) | LiteLLM 估价 |

- **热力图页**:近 26 周 × 7 天网格,每格 "██" 两字宽按当日 token 五级绿阶上色(GitHub light 色阶),月标签 + 图例,26 周合计;点击色块下钻当日按 agent 明细;网格下方为**工作时段 Profile**(清晨/白天/傍晚/深夜 + 星期分布 + 峰值小时,近 30 天).
- **会话/项目维度**:用量页顶部三标签(Agent|会话|项目,`s` 键循环);会话表显示短 id/项目/来源/总量/成本/最近活跃,项目表按项目聚合(会话数/总量/成本).
- **双语**:界面中/英文,`l` 键切换并持久化(`~/.tui-token-monitor/lang`);所有 UI 文案集中在 `ui/i18n.rs`.
- 美元估价:LiteLLM 开放价目数据集,jsdelivr 镜像优先拉取,缓存 7 天(`~/.tui-token-monitor/litellm-pricing.json`),查不到价的模型显示 `—`.
- 架构:`backend(omp + codex 原生回退)` + `goat(原生)` → `model(归一化)` → `ui(三页四主题双语)`;`ledger/` 六 collector → 估价 → 聚合,JSONL 源带进程级 mtime+size 文件缓存.
- 前置登录:`cmdc login` 必须在真实终端跑.未登录时 GOAT 卡显示 Missing,不弹错;Claude 无日志时在"源"行标黄提示.

## 运行

```sh
cargo run
```

或直接双击桌面上的 **Token 用量监测** 快捷方式(图标在 `assets/app.ico`):它调 `run_tui.bat`,脚本自己把会话交给 Windows Terminal 再跑 TUI(只有 WT 会用 Sarasa Term SC;`WT_SESSION` 已在则就地运行,`nowt` 参数防套娃).`run_tui.bat` 本身双击同样可用;正常 `q` 退出窗口即关,非 0 退出会暂停以显示报错.

工具链已全局固化(`~/.cargo/config.toml` + rustup default GNU + repo `rust-toolchain.toml`),新终端一遍过,无前缀.

交互:宽窗口可点击左侧按钮切换页面;`↑`/`k` 上一页,`↓`/`j`/`Tab` 下一页,`1·2·3` 直达页面 · 用量页 `s` 切维度、`g` 切每日表格/趋势图、`t` 切 24h/14d、`c` 切 Token/金额 · `l` 切中英文 · `p` 循环四套主题(均持久化)· `?` 显示快捷键 · 点击行与柱条看明细(`Esc` 收起)· `r` 刷新 · `q` 退出 · 每 60s 自动刷新.OMP 挂掉自动落 Codex 原生配额,Token 页不受影响.

字体:为中文对齐和进度条显示,终端字体推荐 **Sarasa Term SC**(已装,Windows Terminal `profiles.defaults.font.face`).

## 验证

```sh
cargo test                # fixture 回放 + 内存 SQLite 构造 + 多帧 zstd + 时间边界 + 图表点击 + 三页无头渲染
cargo run --example ledger_smoke   # 对本机真实数据源跑一遍 ledger,打印汇总
```

## 已知口径

- Token 页 token 计数(本地)与配额页百分比(服务商)不同源,对不上是正常的.
- 周一为每周起点;日界按本地时区;五路桶(input/output/cache_read/cache_write/reasoning)已归一化为互不重叠.
- chrono 钉 0.4.38:0.4.40+ 的 Windows Local 走 raw-dylib,在本机 WinLibs GCC 工具链上首次调用即崩(见 Cargo.toml 注释).
