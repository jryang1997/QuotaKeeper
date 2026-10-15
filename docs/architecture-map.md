# 架构地图（2026-10-15）

面向后续维护的导航文档。产品收口取舍见 [ADR 0002](adr/0002-readonly-quota-and-token-focus.md)。测试与运行结果记录在对应研究文档中，不在此缓存测试数量。

## 一句话定位

Rust 单核心库 + 两个前端外壳的本地 AI coding 用量监测台：**配额**从各服务商官方/登录态接口直采，**token 账本**读本机会话文件，**美元估价**用 LiteLLM 公开价目。全部数据留在本机，无遥测。同一份 Rust 数据层同时驱动终端 TUI（ratatui）和 Windows 桌面 GUI（WebView2 + React）。

## 三个 crate / 工程单元

| 单元 | 路径 | 角色 |
|---|---|---|
| `quotakeeper`（核心库与 TUI） | `src/` | 配额采集、账本归一化、聚合、定价、TUI 渲染、通知状态机。TUI 功能冻结。 |
| `quotakeeper-gui`（桌面壳） | `gui/src/` | wry 与 tao 外壳、DTO、IPC、导出、托盘、单实例、启动缓存和局域网只读服务。 |
| 前端 | `gui/frontend/src/` | React 19、motion 与 Vite。订阅额度、Token 用量、设置三页，产物内嵌进 GUI。 |

GUI 通过 `quotakeeper = { path = ".." }` 复用核心库，调用 `backend::fetch_snapshot`、`ledger::collect` 与 `deepseek_balance::fetch`。TUI 和 GUI 共用采集口径，GUI 只推送当前页面需要的数据。

## 核心数据流

```
配额（9 provider 并行）                    账本（6 源）
backend::fetch_snapshot()                 ledger::collect(now)
  std::thread::scope 并发                    JSONL/zstd 源: 文件级 mtime+size 缓存
  每家一个 fetch_report → Report             SQLite 源: 每轮只读 SELECT
  失败隔离 → Card::Missing                   归一化 → UsageRecord[]
  全 Missing 才 bail                         → Pricing::estimate 补 cost
        │                                    → aggregate() 聚合出所有视图
        ▼                                            │
     Snapshot{ cards: [Card; 9] }                    ▼
        │                                    LedgerReport{ today/week/month,
        │                                      by_agent/by_model/by_session/
        │                                      by_project, trend_24h, by_day,
        │                                      daily_totals(热力图), hourly/
        │                                      weekday(时段), burn_rate ... }
        └──────────────┬─────────────────────────────┘
                       ▼
        TUI: main.rs 事件循环直接渲染（mpsc 回传，UI 不阻塞）
        GUI: refresh.rs 采集 → ipc.rs 转 DTO(PushDto) → evaluate_script 推 JSON → React
```

### 运行时线程模型
- **TUI**（`src/main.rs`）：主线程跑 ratatui 事件循环；每轮 `spawn_refresh` 起两个后台线程（配额、账本），经 `mpsc` 回传 `Msg`；`pending_refreshes` 计数两者都到位才结束 refreshing 态；60s 自动刷新（`next_refresh`）。
- **GUI**（`gui/src/refresh.rs` + `main.rs`）：tao 事件循环为主线程；配额轮询 60s（`QUOTA_PERIOD`）、账本轮询 5s（`LEDGER_PERIOD`）分开限频；`AtomicBool` 防重入 + rerun 标志合并并发请求；结果经 `mpsc` → 主线程合并最新快照 → `evaluate_script` 推给 WebView。启动先回放 `gui-last-push.json`（打 `stale:true`）做 stale-while-revalidate。

## 关键类型（改造时的锚点）

- **配额侧**（`src/model.rs`）：`ProviderId`、`Report`、`Limit`、`Card::{Live,Missing}` 与 `Snapshot`。`used_pct` 是已用百分比，`resets_in_ms` 是采集时刻的相对时间。`quota-data.js` 只扣一次经过时间；缺失、空窗口和活动均值分别处理，不推算成剩余额度。
- **账本侧**（`src/ledger/types.rs`）：`TokenBreakdown`（input/output/cache_read/cache_write **四桶互不重叠**，reasoning 是 output 的信息性子集不另计）、`UsageRecord`（client/model/ts_ms/tokens/cost/cost_is_estimate/session/project/duration_ms/ttft_ms）、`Agg`（累加器，`cost_reported` 与 `cost_estimated` 分列）。
- **账本报告**（`src/ledger/mod.rs` `LedgerReport`）：所有 UI 视图的数据都在这一个结构里预聚合好，TUI 和 GUI 都消费它。**这是账本侧改造的中枢。**
- **IPC DTO**（`gui/src/ipc.rs`）：`PushDto` 带读取时间、配额错误、卡片、DeepSeek 余额和账本。`LedgerDto` 只保留 sources、today、week、month、analysis_rows、intraday_rows、generated_at_ms；字段沿用 snake_case。导出直接读 Rust 账本，不依赖浏览器缓存。

## 文件地图（按子系统）

### 配额采集（每家一个适配器文件）
- `src/backend.rs` — 并发编排 + OMP 兜底 + Card 填充（`fetch_snapshot`）
- `src/opencode_go.rs`、`src/goat.rs`、`src/claude_quota.rs`、`src/codex_app_server.rs`、`src/cursor_quota.rs`、`src/glm_quota.rs`、`src/kimi_quota.rs`、`src/minimax_quota.rs` 负责各 provider 的 `fetch_report`。
- `src/deepseek_balance.rs` — DeepSeek API 余额（与订阅配额不同口径，单独余额卡）
- `src/pace.rs` 保留 TUI 的原有预测。GUI 通知使用 `AlertTracker::quota_only`，不计算或发送窗口名称推算的耗尽预测。
- **新增一家服务商**：加 `ProviderId` 变体 + `title`/`missing_hints` → 新建 `xxx_quota.rs` 出 `fetch_report` → 在 `backend.rs` 的 `thread::scope` 里加一路 → `ipc.rs` 加 slug → 前端 `ui-data.js` 的 `PROVIDERS`/`SOURCE_PATHS`/logo。

### 账本（`src/ledger/`）
- `mod.rs` — `collect`（编排 6 源 + 文件缓存 + 定价）、`aggregate`（所有视图聚合，**最长的函数**）
- `codex.rs` `claude.rs` `dsh.rs`（JSONL/zstd，走文件缓存）；`ompstats.rs` `opencode.rs` `zcode.rs`（SQLite，每轮直查）
- `pricing.rs` — LiteLLM 价目：磁盘缓存 `~/.tui-token-monitor/litellm-pricing.json`、`spawn_refresher` 24h 后台刷新、`estimate`（6 级回退：精确→大小写→canonical 别名→厂商官方补桶→反向别名→后缀众数/家族兜底）
- `model_alias.rs` — 模型名 canonical 归一
- `agentenv.rs` — agent 环境探测
- `types.rs` — 统一数据模型（见上）
- **新增一个账本源**：实现 `scan_paths`/`parse_file`（JSONL）或 `db_path`/`collect`（SQLite），归一化到 `UsageRecord` → 在 `mod.rs` `collect` 里注册 → `types.rs` `CLIENTS` 加标签。

### TUI 渲染（`src/ui/`）
- `mod.rs` — 壳（顶栏/侧栏/内容/底栏）+ `Page` 枚举 + `HitRegions`（渲染时填点击命中区，主循环查）
- `overview.rs` `quota.rs` `usage.rs`（1315 行，最大）`trend.rs` `heatmap.rs` — 各页/图
- `i18n.rs`（中英，`l` 切换）、`theme.rs`（4 主题，`p` 循环）、`notify.rs`（toast + 阈值状态机 `AlertTracker`）
- `src/settings.rs` — 只存 `monthly_plan_usd` 基准（非敏感），`~/.tui-token-monitor/settings.json`
- `src/paths.rs` — 家目录与各 CLI 数据目录解析（支持 `CODEX_HOME`/`CLAUDE_CONFIG_DIR` 重定向）
- `src/credential.rs` — Windows 凭据管理器读写（Go/GOAT/DeepSeek/GLM/Kimi key），**不落 settings.json**
- `src/http.rs` — 出站 HTTP 统一入口：环境代理 + 环回强制直连不重定向（CPA 管理密钥不经代理）

### GUI 壳（`gui/src/`）
- `main.rs` 负责事件循环、自定义协议、IPC、托盘、快捷键、悬浮窗和 LAN 服务。
- `ipc.rs` 只序列化当前界面需要的数据。
- `refresh.rs` — 刷新引擎；`cache.rs` — 启动回放缓存；`export.rs` — CSV/JSON（Windows 保存对话框，CSV 对公式前缀加单引号）；`desktop.rs` — 自启注册表/全局快捷键；`single_instance.rs` — 单实例

### 前端（`gui/frontend/src/`）
- `main.jsx` 负责导航 quota、usage、settings，原生回调和读取状态。
- `LocalUsageSummary.jsx` 在额度首屏展示本机今日、本周、本月摘要。`QuotaPage.jsx` 固定展示额度卡，`quota-data.js` 统一百分比与重置投影。
- `UsageDashboard.jsx` 使用固定布局；`usage-data.js` 根据日期和采集源统一聚合摘要、趋势、Token 桶和采集源分布。
- `SettingsPage.jsx` 包含外观、采集源与凭据、桌面通知、关于四组；`LanSettings.jsx` 控制只读局域网展示。
- `gui-bridge.js` 封装 `window.ipc.postMessage`；`ui-data.js` 放共享文案与格式，`gui-preferences.js` 放本地偏好。
- `app.css`、`themes.css` 保留用户外观；`quota-focus.css`、`focus.css`、`usage-dashboard.css` 服务当前页面，`ia.css` 保留设置目录、菜单和命令面板。
- 前端测试 `*.test.mjs` 跑 `node --test`

## 工程基建
- **CI**（`.github/workflows/ci.yml`）：windows-msvc（前端 npm ci/test/build + dist 一致性检查 + cargo test + GUI check）、windows-gnu（TUI+GUI 测试，作者本机第一公民）、linux（核心 crate 跨平台门槛）、minimum-rust（1.88）/minimum-gui-rust（1.90）。**前端 `dist/` 必须提交，CI 校验与源码一致**；改完 `gui/frontend/` 先 `npm run build` 再 `cargo build --release`（dist 经 `include_dir!` 编进 exe）。
- 启动脚本 `run_tui.bat`/`run_gui.bat`；`scripts/new-gui-shortcut.ps1` 生成桌面快捷方式。
- `.cargo/config.toml` 固化 GNU 工具链；`chrono` 钉 `=0.4.38`（0.4.40+ raw-dylib 在 WinLibs GCC 崩，见根 `Cargo.toml` 注释）。

## 改造时必知的坑与约束
1. **四桶口径不可破**：input/output/cache_read/cache_write 互不重叠，reasoning 含在 output 里。ZCode inclusive 数据扣 reasoning 会漏算（2026-10-01 审计修过）。改任何 collector 都要守这个归一化。
2. **时间截止统一**：`aggregate` 只接纳 `0 < ts_ms <= now_ms`，所有视图共用同一截止点，避免今日/本月/趋势数字不一致。
3. **报告价 vs 估价分列**：`cost_reported`（如 OMP）与 `cost_estimated`（LiteLLM）不能混。缺价记录返回 None 标"未估价"，**不按免费处理**。
4. **凭据不落盘**：API key 只进 Windows 凭据管理器；`settings.json` 只有月费基准。环回地址（本机 CPA）强制直连不重定向，管理密钥不发给环境代理。
5. **配额百分比与本地 token 不同源**：配额页百分比来自服务商，用量页 token 来自本地账本，对不上是正常的（README「已知口径」）。
6. **未文档化接口会失效**：Codex wham、GOAT `/alpha/billing`、Google `retrieveUserQuotaSummary` 无稳定性承诺；失败降级为 Missing 卡，不影响其他卡与本地账本。
7. **GUI 改动要重建前端**：改 `gui/frontend/` 后必须 `npm run build` 并提交 `dist/`，否则 CI 的一致性检查失败，且 GUI exe 内嵌的是旧产物。
8. **`LedgerReport` 是账本中枢**：加新视图字段要同时动 `mod.rs`（聚合）、`ipc.rs`（DTO）、前端（消费）。三处对齐才不会漏。

## 与竞品对标（详见 `docs/research/codebase-audit-landscape-2026-10-01.md`）
2026-10-01 审计判断：功能已完整，**没有证据支持为"架构更高级"而重写**。与成熟项目（CodexBar/ccusage/tokscale/CodeBurn）的主要差距是：数字正确性的证据、免构建发行、脚本出口（一次性 JSON）、来源可诊断性、大历史性能边界——其次才是来源数量。建议顺序：共享数据路径正确性 → 安装发行 → 本地 JSON/来源诊断 → 按实际使用补来源或性能。**不建议**追覆盖数量、换 GUI 框架、做通用插件系统、仅因文件长就机械拆分。

## 验证命令
```sh
cargo test                                    # 核心 crate（fixture 回放/内存 SQLite/zstd/时间边界/无头渲染）
cargo test --manifest-path gui/Cargo.toml     # GUI 壳（IPC/导出/注册表）
npm --prefix gui/frontend test                # 前端（布局/偏好/通信契约）
cargo run --example ledger_smoke              # 对本机真实数据跑 ledger
cargo run --example quota_smoke               # 对本机真实凭据跑配额快照
```
