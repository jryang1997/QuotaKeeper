# Codebase 审计（2026-10-01）

基线：`340f30b56ae0cb30d278765f2119d544250e3252`，包含审计开始时已有的 Kimi 未提交改动。审计和竞品研究以当前源码为准；原有工作不覆盖、不提交。

## 修复依据

- Codex rollout 的 `SessionMeta.id` 是当前 thread 标识；新格式的 `session_id` 可指向 root thread，旧格式可能只有 `id`。账本应优先读取 `id`，兼容只有 `session_id` 的记录，避免会话缺失或把 fork 合并。[Codex 协议源码](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/protocol.rs)
- ZCode 的 inclusive 数据中 output 包含 reasoning；tokscale 会扣 reasoning，因为它使用五个互不重叠桶。本项目明确使用四桶、reasoning 是 output 子集，因此照搬扣除会漏算；additive 数据若 total 单独包含 reasoning，则需并回 output。[tokscale ZCode 解析源码](https://github.com/junhoyeo/tokscale/blob/main/crates/tokscale-core/src/sessions/zcode.rs)、本项目 `src/ledger/types.rs`
- 定价目录中的缓存价格字段是可选字段，缺失不等于免费。消耗了缺价桶时应返回未估价；`Some(0)` 才表示明确零价。原始 provider-qualified 名的精确匹配（含大小写匹配）应先于剥 provider 的聚合别名。[LiteLLM 价目源码](https://github.com/BerriAI/litellm/blob/main/model_prices_and_context_window.json)
- 账本应统一只接纳 `0 < ts_ms <= now_ms`，避免今日/本月/趋势/导出采用不同截止点。OpenCode 已识别表的 SQL 失败应向上传递；缺少支持的表应标记来源错误，不能当作读取成功的空账本。

以上是本地源码和合成记录可复现的问题，不依赖真实服务额度。

## 整体判断

项目已经具有完整产品的结构：同一 Rust 核心库拥有配额采集、账本归一化和聚合，TUI 与 Windows GUI 复用结果，前端拥有布局和展示。六种本地账本来源、八张配额/余额卡、导出、托盘和提醒都已有实现。这是合理的 module 划分，保留它比更换 GUI 框架或引入通用 provider 插件系统更有价值。

目前更准确的成熟度描述是“功能完整，正确性与交付证据仍需加强”。本轮可复现的问题集中在数据语义和异常状态，不是模块数量不足。修复继续通过已有 interface 完成，让复杂实现留在核心 module 内，获得 locality 和 leverage；不增加新依赖，不改变入口结构。

## 范围与方法

审阅覆盖核心采集器与定价/聚合、配额适配器、凭据与 HTTP、设置、TUI 展示与通知、GUI 壳与 IPC、前端展示/布局/筛选、CSV/JSON、构建与 CI、双语文档。先看提交历史和频繁变化的账本、配额、GUI，再沿数据从文件/HTTP 到 `LedgerReport`/`Report`，继续追到 TUI、IPC、导出和前端。仓库未提供 `CONTEXT.md` 或 ADR；本报告沿用源码的四桶、reasoning、来源状态和配额窗口术语。

采用三类证据：源码调用链、人工计算预期的合成输入、竞品一手源码/文档。独立 architecture audit 辅助检查另一条调用路径；修改后以固定补丁执行 Standards 和 Spec 两轴复核。没有进行正式渗透测试、真实供应商账单对账、完整 GUI 人工操作或大型历史压测，因此“审计覆盖整个代码库”不代表每条平台与外部服务分支都已运行验收。

审计开始时已有 Kimi 工作保留；`src/kimi_quota.rs` 内容校验未变。对已修改的 `model.rs`、`ui-data.js`、`ipc.rs` 只叠加必要修复。既有差异另存到系统临时目录；本轮未创建提交、推送或发布。

## 本轮已落地的修复

优先级表示本项目的实施顺序，不是正式安全漏洞评级。P0：可能改变数字或成功/失败语义；P1：安全、可用性和交付质量。

| 优先级 | 问题与原行为 | 最小修复 | 回归证据 |
|---|---|---|---|
| P0 | Codex 只读 `session_id`，旧格式丢会话，新格式 fork 可能并到 root | 优先当前 thread 的 `id`，保留 legacy 回退 | id-only、fork 与 legacy 三种输入 |
| P0 | ZCode 扣除 reasoning，但项目总 token 已将其包含在 output 中，造成漏算 | inclusive 只扣 input 缓存；additive 独立 reasoning 并回 output；保持 reasoning 子集 | inclusive、additive、legacy 非零 reasoning 的人工总量 |
| P0 | 未来记录进入部分总量和分析，其他视图又排除，页面数字不一致 | 在共享 `aggregate` 入口统一保留 `0 < ts_ms <= now_ms` | totals、趋势、会话、项目、小时与星期视图共同截止 |
| P0 | 已消费缓存桶缺价时仍给出不完整金额；缺字段价格缓存可变成免费 | 缺所需单价返回未估价；显式零价有效；必需 input/output 字段校验；拒绝负数与非有限价 | 缺缓存价、双零、缺必需字段、非法价；先失败再通过 |
| P0 | 大小写不同的完整 provider/model 名先被去 provider，可能采用另一渠道价格 | 完整名称的大小写匹配先于 canonical 聚合别名；众数键保留 `None` 与零的区别 | 同名 bare 与 qualified 不同单价 |
| P0 | OpenCode 的 SQL/未知 schema 错误可伪装成成功空账本 | 识别支持的表；有效空表与错误分开；v2/v1 读取合并，保持 v2 优先去重 | 空支持表、缺表、坏 schema 与既有双版本去重 |
| P0 | OMP/GOAT 缺 `used` 默认零，GOAT 空/部分 summary 可覆盖本地结果 | 必需数据用 `Option`/Serde 校验；过滤无效窗口；没有有效窗口返回错误 | 缺/负/非有限用量、显式零、其他有效 provider 保留、summary 缺字段 |
| P0 | GUI 把绝对订阅到期时间又减去已流逝时间，日期随刷新漂移 | 绝对到期时间直接使用；相对 reset 保持倒计时 | 固定时钟下到期日期稳定，reset 正常递减的渲染检查 |
| P1 | ureq 2 的环境代理不识别 NO_PROXY，本地 CPA 管理请求可能经代理 | 共享 HTTP 按解析后的 host 选择直连 Agent；环回禁止重定向，外部继续使用代理 | 隔离子进程与假代理验证 GET/POST，含 IPv6、127.1、伪装域名 |
| P1 | Windows toast 文本中的单引号可闭合 PowerShell 字符串；占位符二次替换可串入正文 | XML 实体编码 ASCII/智能引号；用一次格式化插入 title/body | 引号、XML 字符和 `__BODY__` 文本保留断言 |
| P1 | 项目/会话等 CSV 字符串可被表格软件当成公式 | 共用 `csv_field` 对公式/控制字符前缀加单引号并保持 CSV 转义；JSON 原文保留 | 合成危险项目文本的 CSV/JSON 对照 |
| P1 | 冷启动可能同步访问两个价格源，共等待约 40 秒；IPC 测试依赖真实日志 | 定价 load 只读缓存与内置补价，使用已有后台刷新；IPC/导出测试改为合成账本 | 核心、IPC、导出回归；GUI 测试不再扫描用户账本 |
| P1 | GLM/Kimi 缺配置提示指向并不存在的 Settings 输入；README 数量/缓存周期过时；Windows 持久化范围说错 | 修正双语配置指引、8 卡清单与 24h 后台刷新说明；更正凭据持久化文档 | 源码/文档交叉检查与 Windows 官方定义 |

相关实现位于 [ledger](../../src/ledger/mod.rs)、[pricing](../../src/ledger/pricing.rs)、[配额模型](../../src/model.rs)、[GOAT](../../src/goat.rs)、[HTTP](../../src/http.rs)、[通知](../../src/ui/notify.rs)、[导出](../../gui/src/export.rs) 和 [配额页面](../../gui/frontend/src/QuotaPage.jsx)。重构主要删除重复读取/时间校验与同步网络路径，其他新增代码集中在输入校验和能捕获旧行为的回归检查。

## 架构上的取舍

### 已深化：共享账本 module

`aggregate(records, now)` 是真正被 TUI、GUI、导出复用的 interface。把时间截止、四桶语义和缺价政策集中在这条路径，能同时纠正多个入口，具有直接 leverage。OpenCode 两个真实 adapter（两代 SQLite 格式）继续位于同一 module，删除重复循环，不为每张表增加 interface 或类型框架。重复 home 解析改用已有 `paths::home_dir`。

这也通过 deletion test：删除重复分支后，复杂性集中在必要的表选择和归一化中；没有把相同判断转移到调用方。未把整个账本合并为一个大文件，来源自身的解析语义仍保留 locality。

### 已深化：配额和出站请求 module

保留 `Report`/`Card` interface，在 OMP 与 GOAT adapter 的解析 seam 上区分未知值和有效零。共享 HTTP module 吸收代理与环回策略，调用方继续使用 `http::get/post`，避免每个 provider 独立写凭据保护。不同来源单位继续保持：配额百分比、余额与 token/min 不强制混合。

### 下一候选：来源诊断

当前已有 `SourceStatus.error` 与 Missing 卡，但 provider 详细失败常在 `fill_report` 被丢弃，配置错误、401、429、超时和 schema 变动最终表现接近。下一次深化应沿现有 `Report`/`Card` 和 `SourceStatus` seam 保留可行动的失败原因，并明确部分读取；先通过这一个 interface 验收，再考虑扩大类型。已有真实 adapter 足够说明 seam，无需通用插件机制或新日志数据库。

## 尚需关注的项目

这些是保留项，不是本轮已经修好的能力。

| 顺序 | 事实或风险 | 下一步与验收 |
|---|---|---|
| 1 | provider 失败原因仍会降级成笼统 Missing；JSONL 坏行、目录遍历失败可能未体现完整性 | 保留诊断；区分截断尾行与中间坏行。导出分别描述“缺价”与“采集不完整”，不共用一个 partial 含义 |
| 1 | Kimi 是审计前已有未提交接入；缺 `remaining` 按未使用处理，是否属于该接口合法省略未获真实证据 | 发布前用脱敏响应确认缺字段语义；若不是合法省略，应返回未知。既有测试只能证明当前实现，不能证明供应商约定 |
| 1 | 后缀众数/家族价仍是启发式；price provenance、长上下文档位与历史价尚未进入结果 | 先公开价目更新时间、匹配依据、覆盖范围；现价估算不能宣称历史实付。内置补价的每个价格还需独立核验 |
| 1 | collector 单测和 IPC 契约已有，但缺一个人工核算、多来源的贯穿语料 | 用小型脱敏语料验证 parser → aggregate → IPC/export，涵盖重写、去重、缺价、reasoning 和本地日界；预期不能由被测函数生成 |
| 2 | 只有构建/CI，缺可直接下载的稳定发行；无一次性 CLI JSON 报告 | 先 Windows TUI/GUI zip 与 SHA256、空目录验收；随后复用核心聚合增加机器输出。详见竞品报告 |
| 2 | macOS 没有同等级 CI；GUI 最低 Rust 1.90 本机未验证 | 保持既有 CI 最低工具链门槛；若继续明确 macOS 支持，补核心测试，不先换 GUI 框架 |
| 2 | 已保存 GUI 布局不自动加入新 catalog 模块 | 现有用户可手动选择模块/重置布局；需要自动迁移时应保留自定义位置，验证旧布局升级，不直接覆盖 |
| 条件触发 | 文件变化整文件解析、每轮克隆/全量聚合、SQLite 全读和全局缓存锁 | 先测冷启动、空轮询、单条追加与 RSS；确实慢才做增量，不先建持久归档 |
| 条件触发 | OMP 子进程先等待退出再读取 pipe；很大输出有填满 pipe 的可能 | 出现大输出超时时再调整并行 drain，使用合成大输出复现；本轮没有确认用户遇到此故障 |
| 维护项 | Rustfmt/严格 Clippy 基线不干净；前端主 bundle 大于 500kB | 独立小批清理或按实测首屏性能拆包，不把无关全库格式变化混入本轮 |

本轮缓存加载变为离线读取：首次启动没有价目时先使用内置补价，其他模型暂时缺价，后台更新成功后随后续采集生效。错误降级更严格：坏 schema/空配额不再得到“成功零值”。CSV 会对包括时区在内的危险前缀字符串添加保护前缀；JSON 保留原值。环回请求不再跟随重定向。这些是有意的行为变化，双语 README 已同步。

## 对标与补足顺序

[独立竞品研究](codebase-audit-landscape-2026-10-01.md) 固定查询日期、commit 和 release，比较 CodexBar、ccusage、tokscale、CodeBurn。它纠正“配额+账本/热力图独有”和“全部竞品只用同一价格源”等过度概括；本项目的实际价值是 Windows 使用场景、共享 Rust 核心和一体化本地监测。

建议顺序：本轮正确性修复 → 来源诊断/价格证据 → Windows 可下载发行 → 一次性 JSON → 按实际使用补来源或优化性能。增加更多卡片、MCP、多账户、同步、自动修改 Agent 配置或持久缓存，都不是本轮架构完善的必要条件。竞品功能存在是需求线索，不是本项目应复制的证明。

## 验证

| 检查 | 审计起点 | 最终结果 |
|---|---|---|
| `cargo test --locked` | 核心 151 通过、2 忽略 | 158 通过、2 忽略；二进制与 doc tests 通过 |
| `cargo test --locked --manifest-path gui/Cargo.toml` | 10 通过 | 11 通过 |
| `npm --prefix gui/frontend test` | 18 通过 | 19 通过 |
| `npm --prefix gui/frontend run build` | — | 通过，内嵌 dist 已同步；保留 >500kB bundle 警告 |
| `git diff --check` | — | 通过 |
| 严格 Clippy / 全库格式 | 已有问题 | 起点即有约 21 个 Clippy 警告和多处 rustfmt 差异，本轮未全库清理；不宣称这两项通过 |
| 最低核心 Rust 1.88 本机检查 | — | GNU 工具链依赖链接报 `_DllMainCRTStartup`，MSVC 缺 `link.exe`；本机不能确认通过，未改工具链或 MSRV |

忽略的两项需要用户已登录的 Codex 与运行中的 CPA。本轮没有启动这两项真实服务验证，未向第三方发送用户日志或凭据。回归输入为合成记录、内存 SQLite、固定时钟和本机假服务器；前端渲染检查不等于完整 WebView2 人工操作。运行环境为 Windows GNU stable；Linux/macOS 与最低 GUI Rust 的结果仍由既有 CI 验证，不能拿本机成功代替。

安全依据：[OWASP CSV Injection](https://community.owasp.org/attacks/CSV_Injection)、[PowerShell 引号规则](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_quoting_rules?view=powershell-7.6)、[Windows 凭据持久化定义](https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw)。CSV 防护仍取决于读取程序与后续保存/重导入方式；JSON 是保留原始文本的出口。HTTP 判断使用当前锁定 ureq 的 URL 解析，未自建解析器。

## Standards

独立复核结论：**0 项硬标准违反，0 项可行动启发式问题**。检查依据为 `CONTRIBUTING.md`、`SECURITY.md` 与 code-review 的 Fowler smell baseline；重点检查共享 HTTP、时间/错误路径、合成测试和新 delta 的价格字段校验。原有 Kimi 改动排除本轮归因。复核基于固定 `review.patch` 及 `review-final.patch`，没有重新采集真实数据。

## Spec

独立复核结论：**0 项发现**。用户的“整个 codebase 审计、适当修改、研究差距并探索补足”分别对应共享路径修复、回归和两份报告；没有发现未经授权的架构重写或功能扩张。补足路线明确标为后续，不声称已经交付。复核者未独立运行真实服务、安装包或压测；主报告和可视报告由主代理完成。

两轴汇总：Standards 0，无最高严重项；Spec 0，无最高严重项。此结论针对本轮修改，不表示上述保留项或全库历史问题不存在。

## 用户要求提交后的 CI 修复（2026-10-01）

检查 [GitHub 最近失败的运行](https://github.com/jryang1997/tui-token-monitor/actions/runs/36736933802) 后，定位到 Windows GNU、Windows MSVC、Linux 共同失败的 `renders_monthly_value_and_plan_multiple_at_80_columns`。产品在没有设置文件时正确显示 `$40.00月基准`，测试却硬编码 `$40.00我的月费`；本机保存的自定义月费使此前本地检查通过。空配置目录的子进程可复现同一失败。

修复只调整测试：根据读入的 benchmark 验证整个渲染结果，金额复用现有 `format_usd`；再固定默认 40、自定义 25、100、0.5 的预期标签与倍率，覆盖整数和四位小数格式。保留正确的默认/自定义产品文案，不删除失败的检查。最终验证分别在隔离的空配置和自定义设置中运行，不改用户设置。

同一旧运行还有 Node 20 action runtime 弃用警告，将 checkout/setup-node 从 v4 升至 v5；两者官方实现使用 Node 24，应用构建仍使用既有 Node 22。依据：[checkout v5](https://github.com/actions/checkout/blob/v5/README.md)、[setup-node v5](https://github.com/actions/setup-node/blob/v5/README.md)。独立 Standards 和 Spec 复核发现并修正了第一版预期金额格式的重复实现；最终两轴均无未解决发现。

审计阶段的“未提交/未 push”是当时状态；用户随后已授权将当前改进、原有 Kimi 接入和本节 CI 修复一起提交并推送。远端最新检查结果以该提交对应的 GitHub Actions 运行记录为准，不将历史红色运行当作新的 push 被拒绝。
