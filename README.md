# tui-token-monitor

终端里的三页监测台:宽窗口用左侧按钮切换,窄窗口自动让内容占满宽度;配额页用卡片和横向进度条,Token 用量页显示最近一小时活动、Agent/会话/项目三维度和可切换的堆叠趋势图,热力图页是 GitHub 风格贡献格 + 工作时段 Profile。中英双语(`l` 键),四套主题(`p` 键)。

## Windows 桌面版

桌面版是 `gui/` 下的 WebView2 程序。先在 `gui/frontend/` 执行 `npm ci` 和 `npm run build`，再在仓库根目录执行 `cargo build --release --manifest-path gui/Cargo.toml`；`run_gui.bat` 会启动构建好的 Release 程序。GNU 构建会自动把 `WebView2Loader.dll` 复制到程序旁边，分发时需将两者放在同一目录。再次启动只唤醒已有窗口。

- 默认使用 Segoe UI Variable / Microsoft YaHei UI 字体，设置中可切换为像素字体，并可调整信息密度及减少动态效果。
- 主工具栏的主题按钮或 `P` 键循环“跟随系统、浅色、深色、深海、暖纸”；设置页可直接指定主题，选择会保存在本机。系统模式随 Windows 明暗模式变化。
- 首次启动会检测服务额度和六路本机来源，并提供逐项修复链接。配额页显示最紧额度、额度与本机用量各自的上次成功时间；缓存过期会标记。每张卡可展开查看真实来源、窗口与计算方法，Antigravity 上游配额与回退的本机活动强度按来源区分。
- 真实额度窗口显示相对匀速的超前／余量及能否撑到重置；样本不足、额度过期时不显示耗尽预测。阈值提醒和预测提醒分别开关，支持 22:00–08:00 夜间静默。
- 用量页支持自选日期范围、上一等长区间对比、Agent／项目筛选，以及所选范围的 CSV／JSON 导出。导出包含本地时区、来源、报告金额、估算金额和缺价标记；月度金额遇缺价会标“部分汇总”。原有 24 小时／14 天趋势和 Agent／会话／项目／模型拆分继续可用。
- 托盘图标以颜色显示风险级别。左键显隐窗口，右键可刷新、打开设置或退出；可选关闭窗口时退出。设置中可启用 `Ctrl+Alt+U` 全局显隐快捷键和当前 Windows 用户登录启动，也可固定／隐藏卡片、调整来源颜色及显示密度。
- 配额默认由各服务独立采集：OpenCode Go 直连 usage API，Codex 使用官方 `app-server`，GOAT 直连其 billing API，Antigravity 通过 CPA 代查 Google 上游配额。设置页可把 Go 和 GOAT API key 分别存入 Windows 凭据管理器；不保存到前端本地存储。OMP 只给 Go/Codex 提供配额兼容兜底，并可为 CPA 卡提供本机活动强度。每张卡的“来源与口径”显示实际命中的路径。详见[一手来源与实现边界](docs/research/native-quota-sources.md)和[CPA Antigravity 读取路径](docs/research/cpa-antigravity-usage-paths.md)。

竞品来源与功能取舍见 [Windows 桌面版竞品功能审计](docs/research/competitor-feature-audit.md)。

## 范围(已确认,2026-09-26)

- **配额页**:OpenCode Go、Codex、Command Code GOAT 的真实可用窗口按账号采集；Antigravity 经 CPA 代查 Google 账号的 Gemini 和 Claude/GPT 额度窗口。上游不可读时回退显示代理状态与可选本机活动量。每条真实额度用文字和 █░ 横向进度条显示，行尾附窗口内匀速外推的预计耗尽时间。
- **配额阈值通知**:任额度跨越 80% / 100% / 重置时弹 Windows toast(状态机去重,不重复轰炸).
- **采集链路**:Go usage API（应用凭据／环境变量／OpenCode 登录态）；Codex 官方 `app-server`（无可用结果时使用未公开 wham 接口，再以 OMP 兜底）；GOAT 直连 billing API（API key 可存应用凭据、使用 `COMMAND_CODE_API_KEY` 或官方 CLI 登录态）。四家单独刷新，一家失败不影响别家。GOAT 月度百分比由已识别订阅套餐上限与服务端余额推算；订阅接口失败或类型未知时不显示月度百分比。**Antigravity 卡**从 CPA `auth-files` 取得账号索引和项目，再经 CPA `api-call` 调 Google 内部 `retrieveUserQuotaSummary`，返回剩余比例与重置时间；可选 `loadCodeAssist` 读取套餐。Google 内部接口无公开稳定性承诺。失败时才从 CPA 凭证健康与可选 OMP stats.db 显示本机活动强度，这不代表 Google 配额或 credits。CPA 管理密钥位于 `~/.tui-token-monitor/cpa-key`。
- **Token 用量页**:顶部显示近 60 分钟本机用量、用量最多的 Agent、最近记录和最紧张的真实订阅额度；下方保留今日/本周(ISO 周一起)/本月汇总与近 30 天按 Agent/会话/项目表。月度 API 等值由来源报告金额与 LiteLLM 对缺少来源成本记录的估价合计，并显示相对月费基准的倍数。默认基准是三项基础美元月标价合计 `$40`；桌面 GUI 可点“修改”填入自己的月费，TUI 与 GUI 共用本机设置。API 等值不代表实际省下的钱。价格核验和估价边界见[官方价格核验记录](docs/research/official-pricing-validation.md)。趋势图可切换滚动 24 小时或近 14 天、Token 或金额，按 Agent 堆叠并可点击时段看明细；`g` 切回每日表。六路数据源:

| Agent | 来源 | 成本 |
|---|---|---|
| Codex | `~/.codex/sessions/**/rollout-*.jsonl`(`last_token_usage` 增量) | LiteLLM 估价 |
| OMP | `~/.omp/stats.db` `messages`(官方预聚合) | OMP 来源报告价(未与用户账单核对) |
| OpenCode | `~/.local/share/opencode/opencode.db`(v1/v2 双 schema) | LiteLLM 估价 |
| ZCode | `~/.zcode/cli/db/db.sqlite` `model_usage`(exclusive 归一化) | LiteLLM 估价 |
| DeepSeek Harness | `~/.dsh/sessions/**/session*.jsonl.zstd`(多帧 zstd) | LiteLLM 估价 |
| Claude Code | `~/.claude/projects/**/*.jsonl`(`(message.id, requestId)` 去重) | LiteLLM 估价 |

- **热力图页**:近 26 周 × 7 天网格,每格 "██" 两字宽按当日 token 五级绿阶上色(GitHub light 色阶),月标签 + 图例,26 周合计;点击色块下钻当日按 agent 明细;网格下方为**工作时段 Profile**(清晨/白天/傍晚/深夜 + 星期分布 + 峰值小时,近 30 天).
- **会话/项目维度**:用量页顶部三标签(Agent|会话|项目,`s` 键循环);会话表显示短 id/项目/来源/总量/成本/最近活跃,项目表按项目聚合(会话数/总量/成本).
- **双语**:界面中/英文,`l` 键切换并持久化(`~/.tui-token-monitor/lang`);所有 UI 文案集中在 `ui/i18n.rs`.
- 美元估价:未提供来源价的记录使用 LiteLLM 第三方公开价目数据(jsDelivr 优先、7 天缓存:`~/.tui-token-monitor/litellm-pricing.json`)；先按完整模型名匹配，尾部名称只有唯一提供方时才用于估价，同名费率冲突则显示无价格，不随机挑一个。该价目表未逐项由模型厂商核验，也无法反映时段、地区、上下文档位、工具费或订阅折扣；OMP 的来源报告价与 LiteLLM 估价仍分开记录。
- 架构:`backend(独立采集 + 按卡兼容兜底)` → `model(归一化、来源标签)` → `ui`;`ledger/` 六 collector → 估价 → 聚合,JSONL 源带进程级 mtime+size 文件缓存.
- 登录:Codex 需要其官方程序已登录；Go／GOAT 可在 Windows 设置页保存自己的 API key，也可使用环境变量或对应第一方客户端登录态。Claude 无日志时在“源”行标黄提示。

## 运行

```sh
cargo run
```

或直接双击桌面上的 **Token 用量监测** 快捷方式(图标在 `assets/app.ico`):它调 `run_tui.bat`,脚本自己把会话交给 Windows Terminal 再跑 TUI(只有 WT 会用 Sarasa Term SC;`WT_SESSION` 已在则就地运行,`nowt` 参数防套娃).`run_tui.bat` 本身双击同样可用;正常 `q` 退出窗口即关,非 0 退出会暂停以显示报错.

工具链已全局固化(`~/.cargo/config.toml` + rustup default GNU + repo `rust-toolchain.toml`),新终端一遍过,无前缀.

交互:宽窗口可点击左侧按钮切换页面;`↑`/`k` 上一页,`↓`/`j`/`Tab` 下一页,`1·2·3` 直达页面 · 用量页 `s` 切维度、`g` 切每日表格/趋势图、`t` 切 24h/14d、`c` 切 Token/金额 · `l` 切中英文 · `p` 循环四套主题(均持久化)· `?` 显示快捷键 · 点击行与柱条看明细(`Esc` 收起)· `r` 刷新 · `q` 退出 · 每 60s 自动刷新.各家额度独立刷新,Token 页不受配额请求影响.

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
