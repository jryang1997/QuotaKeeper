# tui-token-monitor

终端里的三页监测台:左侧圆角按钮切换(鼠标可点),配额页支持卡片/圆环双视图(右上 ⟳ 切换,圆环为同心三环+点阵大数字环心),Token 用量页支持 Agent/会话/项目三维度,热力图页是 GitHub 风格贡献格 + 工作时段 Profile。中英双语(`l` 键),四套主题(`p` 键)。

## 范围(已确认,2026-09-25)

- **配额页**:OpenCode Go $10/mo(5h/周/月)+ Codex(ChatGPT Plus $20/mo,5h/7d)+ Command Code GOAT $10/mo(月 $70 / 5h $14 / 周 $35)+ **Antigravity(经 CPA 反代,unofficial)**.卡片视图每条额度 = 文字 + █░ 粗块条,行尾附**燃烧率预测**("预计 X 耗尽",窗口内匀速外推,<1h 红色告警);圆环视图用盲文点阵(U+2800,2×4 子像素/格)画**同心环**(外=月/中=周/内=5h),环心为点阵大数字百分比,点击环体换聚焦窗口;右上 ⟳ 按钮(或 `v` 键)切换两视图.
- **配额阈值通知**:任额度跨越 80% / 100% / 重置时弹 Windows toast(状态机去重,不重复轰炸).
- **采集链路**:OMP 优先 → 失败自动落 Codex 原生配额(`~/.codex/auth.json` OAuth → `chatgpt.com/backend-api/wham/usage`,口径照抄 CodexBar,unofficial);**Antigravity 卡**读 CPA(CLIProxyAPI)管理接口 `auth-files` 取凭证健康 + OMP stats.db 里 `provider='cpa'` 的今日 token 强度(今日 vs 近 7 日均值)——credits 百分比因官方端点对消费级账号 403 且当前 CPA 构建无该端点而不可用,以强度条替代;管理密钥放 `~/.tui-token-monitor/cpa-key`.
- **Token 用量页**:今日/本周(ISO 周一起)/本月汇总 + 近 30 天按 Agent 表 + 近 14 天每日表,行可点击看明细.六路数据源(口径照抄 tokscale 的字段映射与去重规则):

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

交互:左侧圆角按钮点击切换页面(键盘 `↑↓`/`Tab`/`1·2·3` 等价)· 配额页右上 `⟳` 切换卡片/圆环(`v` 同效)· 圆环点击换聚焦窗口 · 用量页 `s` 切维度、`g` 切每日表格/趋势图 · `l` 切中英文 · `p` 循环四套主题(均持久化)· 点击行与色块看明细(`Esc` 收起)· `r` 刷新 · `q` 退出 · 每 60s 自动刷新.OMP 挂掉自动落 Codex 原生配额,Token 页不受影响.

字体:为中文对齐与盲文环形计,终端字体请用 **Sarasa Term SC**(已装,Windows Terminal `profiles.defaults.font.face`)——盲文点阵/块条/圆环符号全覆盖.

## 验证

```sh
cargo test                # 78 个测试:fixture 回放 + 内存 SQLite 构造 + 多帧 zstd + 聚合边界 + 盲文环弧形/大数字 + 双语切换 + 三页无头渲染
cargo run --example ledger_smoke   # 对本机真实数据源跑一遍 ledger,打印汇总
```

## 已知口径

- Token 页 token 计数(本地)与配额页百分比(服务商)不同源,对不上是正常的.
- 周一为每周起点;日界按本地时区;五路桶(input/output/cache_read/cache_write/reasoning)已归一化为互不重叠.
- chrono 钉 0.4.38:0.4.40+ 的 Windows Local 走 raw-dylib,在本机 WinLibs GCC 工具链上首次调用即崩(见 Cargo.toml 注释).
