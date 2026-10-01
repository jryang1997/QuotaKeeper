# 代码库审计补充：成熟度、竞品差距与补足顺序

查询日期：**2026-10-01（Asia/Shanghai）**。本文补充整个代码库审计中的外部对标；建议项不是本轮全部实施的承诺。

## 结论

本项目已是功能完整的个人用量监测台：共享 Rust 数据层同时服务 TUI 与 Windows GUI，包含多服务配额、本机账本、活动分析、筛选导出、系统提醒与常驻入口。对当前定位而言，没有证据支持为了“架构更高级”重写这套结构。与成熟项目的主要差距是**数字正确性的证据、免构建发行、脚本出口、来源可诊断性，以及大历史数据的性能边界**，其次才是来源数量。

这是一项工程判断，不是性能排名。本文没有运行竞品或独立核验真实服务商账单，也不把 README 宣传、测试文件数量或 star 数当作准确性证明。最值得学习的是成熟项目如何保持数据口径、交付可安装产物、处理不完整历史；不是复制它们全部功能。

## 取样与证据范围

对标四个各有代表性的项目：CodexBar 的配额采集与桌面常驻，ccusage 的可组合报表，tokscale 的多来源 Rust TUI，CodeBurn 的 Windows 发行与完整产品出口。功能以查询到的一手 README、文档和源码为准；下列默认分支快照可能领先于公开 release，不能据此认定未发布代码已经交付用户。

| 项目 | 本轮默认分支 commit | GitHub latest release API 返回的 tag | 可核验来源 |
|---|---|---|---|
| CodexBar | `2ef5bb6305dda4570d8d080f348cf76a45a392bd` | `v0.70.0`（2026-09-30 UTC） | [commit](https://github.com/steipete/CodexBar/commit/2ef5bb6305dda4570d8d080f348cf76a45a392bd)、[release](https://github.com/steipete/CodexBar/releases/tag/v0.70.0) |
| ccusage | `f537be9dbc677589b1357b7c089e6e43e22800ef` | `v20.0.26`（2026-09-27 UTC） | [commit](https://github.com/ccusage/ccusage/commit/f537be9dbc677589b1357b7c089e6e43e22800ef)、[release](https://github.com/ccusage/ccusage/releases/tag/v20.0.26) |
| tokscale | `fe72e1f9b5a2b1a927108b353a9730a5c631a196` | `v4.17.0`（2026-09-15 UTC） | [commit](https://github.com/junhoyeo/tokscale/commit/fe72e1f9b5a2b1a927108b353a9730a5c631a196)、[release](https://github.com/junhoyeo/tokscale/releases/tag/v4.17.0) |
| CodeBurn | `986d72ce1efbfaf40b3c9d4050d9ecdbc24be986` | `mac-v0.9.25`（2026-09-21 UTC） | [commit](https://github.com/getagentseal/codeburn/commit/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986)、[release](https://github.com/getagentseal/codeburn/releases/tag/mac-v0.9.25) |

元数据通过公开 GitHub API 查询；CodeBurn 的 latest release 是 macOS tag，不能把它当作所有平台统一版本。ccusage 根 README 在该 commit 是指向 `apps/ccusage/README.md` 的链接，本文检查了实际文档。

本项目基线取自本轮阅读时的工作树，包括 [backend.rs](../../src/backend.rs)、[ledger](../../src/ledger/)、[GUI 导出](../../gui/src/export.rs)、[刷新引擎](../../gui/src/refresh.rs)、[桌面入口](../../gui/src/desktop.rs)、[IPC](../../gui/src/ipc.rs)、[前端](../../gui/frontend/src/)、[CI](../../.github/workflows/ci.yml)。同期审计修复完成后，以实际代码和主审计验证记录为准。

### 成熟度判断标准

| 维度 | 需要回答的问题 |
|---|---|
| 正确性 | 同一 fixture 在采集、聚合、界面、导出中是否一致？重复事件、缓存桶、时区、缺价和不完整扫描是否有明确语义？ |
| 可交付性 | 新用户是否需要编译？发布产物是否与测试过的源码、前端、依赖版本对应？ |
| 故障恢复 | 某个来源失败、登录过期、文件损坏或 schema 改变时，是否保留其他来源并指出受影响范围？ |
| 可组合性 | 能否在无 GUI、无交互终端下输出稳定结构、错误状态和采集时间？ |
| 性能 | 固定规模语料的冷启动、无变化刷新、追加日志刷新，是否有可重复测量？ |
| 维护成本 | 每项新来源和平台是否有真实用户需求、可回放样本与可维护的适配边界？ |

CodeBurn 明确区分源码检查、冻结 fixture、实际安装包和真实交互证据，并要求同一语料核对不同入口的数字。这是有用的工程参考，但**存在发布验收文档不等于每次发布都已通过这些验收**。[CodeBurn release acceptance](https://github.com/getagentseal/codeburn/blob/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986/docs/release-acceptance/README.md)

## 当前已经具备的能力

| 已核对能力 | 源码依据 | 意义与边界 |
|---|---|---|
| 8 张配额卡：OpenCode Go、Codex、GOAT、Antigravity、Claude、Cursor、GLM、Kimi；另有 DeepSeek 余额 | [backend.rs](../../src/backend.rs)、[model.rs](../../src/model.rs)、[DeepSeek](../../src/deepseek_balance.rs) | 比 README 仍写的“六卡”更完整；配额、API 余额、本机 token 是不同指标。 |
| 6 路账本：Codex、Claude、DSH、OMP、OpenCode、ZCode | [collect](../../src/ledger/mod.rs) | 已覆盖本人主要工作流；不能把“支持某个 CLI 的账本”推导成“支持该服务所有配额”。 |
| JSONL/zstd 文件按 mtime 与大小复用；SQLite 只读；来源独立报错 | [ledger/mod.rs](../../src/ledger/mod.rs)、[SQLite collectors](../../src/ledger/opencode.rs) | 已有实际缓存与失败隔离；无需先创建通用 collector 插件框架。 |
| 报告价与估价分列，缺价记录数，CSV/JSON 日期、Agent、项目筛选 | [export.rs](../../gui/src/export.rs)、[types.rs](../../src/ledger/types.rs) | 导出 JSON 已有 `schema_version: 2`、来源标识、生成时间、USD 口径和时区偏移；不能再建议“从零添加导出”。 |
| 近期活动、24h/14d 趋势、模型/会话/项目拆分、26 周热力图、工作时段矩阵 | [ledger/mod.rs](../../src/ledger/mod.rs)、[UsageDashboard](../../gui/frontend/src/UsageDashboard.jsx) | 已具备个人分析所需的多数维度；不必为匹配竞品页数再增加页面。 |
| GUI 风险色托盘、单实例、通知、自启、主题、来源说明、区间对比 | [GUI](../../gui/src/main.rs)、[desktop.rs](../../gui/src/desktop.rs)、[前端](../../gui/frontend/src/main.jsx) | 早期竞品审计提出的常驻与视觉基础工作已经落地。 |
| Windows MSVC/GNU、Linux 与最低 Rust 版本 CI，Rust 和前端测试 | [ci.yml](../../.github/workflows/ci.yml)、[package.json](../../gui/frontend/package.json) | 测试体系已存在；下一步是补关键跨层断言和发行验证，而不是换测试框架。macOS 暂无同等级 CI 证据。 |

本项目的可保留优势是：Windows 使用场景明确；运行 TUI 不要求 Node/Python；GUI 使用 WebView2 并内嵌前端；中文与英文、订阅配额与本机账本同屏；维护范围相对有限。这里描述的是现有组合价值，**不声称其中任何单项或组合在整个市场独有**。

### 旧调研应如何解读

现有 [9 月 25 日对标](token-usage-monitor-benchmark.md)、[GUI 审计](competitor-feature-audit.md) 与 [9 月 29 日 landscape](codeburn-tokenscale-landscape.md) 是历史快照；它们的功能建议和实施记录不能直接当作今天的缺口清单。尤其需要更正以下判断：

- **不能继续称 OpenCode Go / GOAT 所属 Command Code / Antigravity 配额为竞品未覆盖领域。** CodexBar 当前 README 已列出三者。具体 GOAT 套餐字段和 CPA 路径可能与本项目不同，但“相同服务没有竞品接入”已不成立。[CodexBar providers](https://github.com/steipete/CodexBar/blob/2ef5bb6305dda4570d8d080f348cf76a45a392bd/README.md)
- **本地热力图不是独占能力。** tokscale 的 Stats 视图就是 contribution graph，另有 JSON graph 导出；不应把它的全部分析能力都归类为线上排行榜。[tokscale TUI 与 graph](https://github.com/junhoyeo/tokscale/blob/fe72e1f9b5a2b1a927108b353a9730a5c631a196/README.md)
- **不能再写“这些项目全用 LiteLLM 单一价格源”。** CodexBar 文档使用 models.dev 与内置兜底；ccusage 源码包含 LiteLLM 和 models.dev 的嵌入数据与补充逻辑；tokscale 也有多价目来源。[CodexBar pricing](https://github.com/steipete/CodexBar/blob/2ef5bb6305dda4570d8d080f348cf76a45a392bd/docs/model-pricing.md)、[ccusage pricing](https://github.com/ccusage/ccusage/blob/f537be9dbc677589b1357b7c089e6e43e22800ef/rust/crates/ccusage-core/src/pricing.rs)、[tokscale pricing](https://github.com/junhoyeo/tokscale/tree/fe72e1f9b5a2b1a927108b353a9730a5c631a196/crates/tokscale-core/src/pricing)
- **“桌面用户必须装 Node”不能由 CLI 安装要求推导。** CodeBurn 的 CLI 确有 Node 要求，Windows 桌面提供 Store 和安装包；比较运行时依赖应按具体交付入口讨论。[CodeBurn Windows distribution](https://github.com/getagentseal/codeburn/blob/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986/README.md)

## 四个代表项目分别值得学习什么

| 项目 | 本轮一手资料可确认的做法 | 对本项目最有用的借鉴 |
|---|---|---|
| CodexBar | 广泛配额接入，provider 配置、来源回退、状态与自适应刷新；macOS 主应用和 macOS/Linux CLI。[README](https://github.com/steipete/CodexBar/blob/2ef5bb6305dda4570d8d080f348cf76a45a392bd/README.md)、[refresh loop](https://github.com/steipete/CodexBar/blob/2ef5bb6305dda4570d8d080f348cf76a45a392bd/docs/refresh-loop.md) | 每个来源的更新、降级和成本覆盖范围比继续加配额卡更值得打磨。官方 Windows 主应用仍通过相关移植项目提供，不能忽略本项目 Windows 形态的实用价值。 |
| ccusage | 多 CLI 的一次性日/周/月/会话报表，JSON、日期/时区筛选，Claude blocks 与 statusline。[实际 README](https://github.com/ccusage/ccusage/blob/f537be9dbc677589b1357b7c089e6e43e22800ef/apps/ccusage/README.md) | 无界面 JSON 报表即可服务脚本和状态栏；无需先建 Web 服务或 MCP。 |
| tokscale | 多客户端 Rust TUI、工作区分组、worktree 合并、JSON 导出、贡献图，以及跨平台预编译产物。[README](https://github.com/junhoyeo/tokscale/blob/fe72e1f9b5a2b1a927108b353a9730a5c631a196/README.md)、[build matrix](https://github.com/junhoyeo/tokscale/blob/fe72e1f9b5a2b1a927108b353a9730a5c631a196/.github/workflows/build-native.yml) | 安装发行、来源格式兼容与项目身份解析。覆盖数字不代表每个客户端都有同等完整的 token 证据。 |
| CodeBurn | Windows Store/安装包、WSL 双环境读取、周期比较、任务/分支分析、quota、可选 guard/optimize 与本地 MCP。[README](https://github.com/getagentseal/codeburn/blob/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986/README.md)、[MCP](https://github.com/getagentseal/codeburn/blob/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986/docs/mcp.md) | 实际安装包验收与来源覆盖说明。自动修改配置的 optimize 和停止会话的 guard 改变产品风险与维护成本，不是本项目必须追随的功能。 |

## 真正的差距与最小补足方案

优先级含义：**P0 是数字或状态正确性；P1 是对当前用户有直接用途的交付改进；P2 需要需求或测量证据才做。** 下表源码风险是审计起点，不替代针对具体输入的复现与本轮修复记录。

### P0：确保“未知”和“部分”不会变成完整金额或成功

**成本桶与匹配语义。** 审计起点的 [pricing.rs](../../src/ledger/pricing.rs) 只在缓存桶单价存在时累加费用；已消耗缓存桶缺价时，仍可返回一个金额。其后缀众数及去掉模型 minor 版本的家族回退，也无法证明请求采用了那个 provider 或旧版本的价格。当前 `cost_is_estimate` 只能说明金额是估价，不能说明匹配依据。应优先检查这些路径，而不是增加更多硬编码单价。

最小做法：在共享估价函数守住已消费桶的单价完整性；未知模型或无法确认的跨版本匹配保持缺价，或明确标成近似价格匹配；补一条能失败的缓存缺价回放断言。若将来确实要按渠道/长上下文档位计价，再保留 provider 与单次请求信息。CodexBar 把 provider+model 作为定价范围，并明确规定 consumed cache class 缺价不生成完整金额；ccusage/tokscale 已有长上下文档位处理。这些是口径边界，不是必须复制的价目框架。[CodexBar model pricing](https://github.com/steipete/CodexBar/blob/2ef5bb6305dda4570d8d080f348cf76a45a392bd/docs/model-pricing.md)、[ccusage pricing source](https://github.com/ccusage/ccusage/blob/f537be9dbc677589b1357b7c089e6e43e22800ef/rust/crates/ccusage-core/src/pricing.rs)、[tokscale lookup source](https://github.com/junhoyeo/tokscale/blob/fe72e1f9b5a2b1a927108b353a9730a5c631a196/crates/tokscale-core/src/pricing/lookup.rs)

**不完整来源的可见性。** 本项目已有 `SourceStatus.error`，但 [Claude](../../src/ledger/claude.rs)、[Codex](../../src/ledger/codex.rs) 的逐行容错会跳过不能解释的行；这是活跃 JSONL 的合理策略，却还不足以证明扫描完整。GUI 导出的 `cost_is_partial` 目前指“缺价记录”，不代表“文件读到一半或来源缺失”。最小做法是先让报告/导出携带来源状态，区分缺价和采集不完整；截断尾行与中间坏行采用不同的诊断语义。无需为了此事增加日志数据库。[本项目 export](../../gui/src/export.rs)

**跨层正确性回放。** 现有 parser、SQLite、zstd、时间边界与 GUI 契约测试是良好基础。建议再固定一份小型、脱敏、人工计算过的六来源语料，核对 token 四桶、reasoning 子集、去重、报告价/估价、缺价、未来时间与本地日界，贯穿 `aggregate → IPC → export`。测试断言必须独立算出预期，不能拿同一个聚合函数生成答案。不要把配额百分比强行与 token/min 对齐；它们没有相同单位。[本项目 types](../../src/ledger/types.rs)、[aggregate](../../src/ledger/mod.rs)、[CodeBurn correctness acceptance](https://github.com/getagentseal/codeburn/blob/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986/docs/release-acceptance/README.md)

### P1：交付和可组合性

| 差距 | 本项目事实 | 最小方案与验收 |
|---|---|---|
| 免构建发行 | 安装仍以 `cargo install --path .` 或 GUI 前端/Rust 构建为主；仓库工作流只有 CI。[README](../../README.md)、[CI](../../.github/workflows/ci.yml) | 首先产生 Windows MSVC TUI exe 与 GUI zip，带版本、许可证、SHA256 和 WebView2 前置说明；从空目录解包验证，确保 GUI 包里的前端正是该 tag 的构建。自动 Release 工作流比立即同时申请 Store、winget、Scoop 更小。参考 [CodexBar CLI tarballs](https://github.com/steipete/CodexBar/blob/2ef5bb6305dda4570d8d080f348cf76a45a392bd/docs/cli.md) 与 [ccusage release workflow](https://github.com/ccusage/ccusage/blob/f537be9dbc677589b1357b7c089e6e43e22800ef/.github/workflows/release.yaml)。 |
| 无界面 JSON 出口 | GUI 能保存 CSV/JSON；TUI `main` 直接进入终端循环，尚无同等级的一次性机器输出。[main.rs](../../src/main.rs)、[export.rs](../../gui/src/export.rs) | 复用账本聚合，加一个 `--json`/一次性报告入口；stdout 只放版本化 JSON，stderr 放诊断，包含来源错误、金额覆盖范围和时间。首版以本地账本为默认，配额读取显式选择；若需要离线标志，先定义价格刷新策略。只把可共享编码逻辑移进核心库，不移动 Windows 文件对话框。对齐 [ccusage JSON](https://github.com/ccusage/ccusage/blob/f537be9dbc677589b1357b7c089e6e43e22800ef/apps/ccusage/README.md) 与 [CodexBar CLI cost coverage](https://github.com/steipete/CodexBar/blob/2ef5bb6305dda4570d8d080f348cf76a45a392bd/docs/cli.md)。 |
| 诊断与文档一致性 | 审计起点源码 8 卡、README 6 卡，本轮已同步；来源状态有 records/error，但没有每路完整覆盖证明。[backend](../../src/backend.rs)、[SourceStatus](../../src/ledger/mod.rs) | 复用已有状态输出“检测到/无记录/不可读/部分读取”，附针对性的修复方式，避免新用户空白页面。provider 失败原因尽量保留到 UI，而不是只剩 Missing。 |
| 价格可核验、可覆盖 | 缓存记录 fetched time，但估值输出缺少所用价格来源/版本与匹配方法；现价重算不等于历史账单。[pricing](../../src/ledger/pricing.rs)、[export](../../gui/src/export.rs) | 先在导出添加价目更新时间和来源，明确“以当前价目重估”；实际遇到代理改名或私人渠道价再加少量本机 override。不要把月费倍数称为实付节省。CodeBurn 提供 alias/override，也明确按读入时的现价重算；并非所有竞品都保存历史价格。[CodeBurn configuration](https://github.com/getagentseal/codeburn/blob/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986/docs/configuration.md)、[how it works](https://github.com/getagentseal/codeburn/blob/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986/docs/how-it-works.md)。 |
| 支持声明的验证 | Windows/Linux CI 已有；macOS 账本/终端行为与 WSL 双 home 读取没有同等级覆盖。[CI](../../.github/workflows/ci.yml)、[paths](../../src/paths.rs) | 若 README 继续声称 macOS 支持，先加 macOS 核心测试；若本人使用 WSL，再支持显式附加数据根并处理重复扫描。不能直接把 Windows 与 WSL 总量相加而忽略拷贝/镜像会话。CodeBurn 的双环境读取是可用需求例子，不是其去重已经被本轮独立验证的证据。[CodeBurn WSL](https://github.com/getagentseal/codeburn/blob/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986/README.md)。 |

### P2：有证据才扩展

**性能与归档。** 本项目已缓存未变化的文件，但文件变化后仍整文件解析，每轮克隆记录并全量聚合，SQLite 每 5 秒全量读；缓存又是进程内的，重启后要重新扫描。这说明复杂度边界，尚不能说明当前用户数据已经慢。先用冻结语料分别测冷启动、无变化轮询、追加一个事件的耗时和 RSS，再选择是否做追加式解析、查询增量或持久缓存。不要先上统一 SQLite 归档、文件 watcher、多级缓存。

tokscale 的缓存包含版本与 Codex consumed offset，并关联 SQLite WAL 等依赖；CodeBurn 日级缓存保留了复杂迁移与历史保留语义。它们说明持久缓存会引入真实维护成本。先借鉴测量和失效原则，再借鉴实现。[tokscale message cache](https://github.com/junhoyeo/tokscale/blob/fe72e1f9b5a2b1a927108b353a9730a5c631a196/crates/tokscale-core/src/message_cache.rs)、[CodeBurn daily cache](https://github.com/getagentseal/codeburn/blob/986d72ce1efbfaf40b3c9d4050d9ecdbc24be986/src/daily-cache.ts)

**来源广度与项目归并。** Gemini CLI、Copilot CLI、Cursor 本地用量、更多 DSH/ZCode 版本是可见覆盖差距；先按本人确实使用的工具补一个来源和 fixture。Cursor 配额已有不等于 Cursor 用量账本已有。项目跨 Agent 合并、worktree 归并也应以真实日志身份为依据；Claude 的目录 slug 与真实路径不能只靠字符串替换就假定等价。[本项目 collectors](../../src/ledger/mod.rs)、[tokscale source list 与 workspace grouping](https://github.com/junhoyeo/tokscale/blob/fe72e1f9b5a2b1a927108b353a9730a5c631a196/README.md)

**可选的下一层能力。** statusline 可以在一次性 JSON 输出稳定后添加；MCP 有明确 Agent 消费需求再做。多账户、跨设备同步、组织账单、网页服务、任务/PR 成本、自动停止和优化配置，都需要独立产品需求与可靠的归因数据。它们并不是“成熟用量监控器”必须全部具备的标准答案。

## 实施取舍

建议顺序是：**共享数据路径的正确性修复 → 安装发行 → 本地 JSON/来源诊断 → 根据实际使用补来源或性能**。架构上继续让核心 Rust 库拥有采集与聚合，GUI 壳拥有 Windows 生命周期，前端拥有展示。确实需要跨入口复用时才抽出编码函数或小型共享类型，保留现有明确的 collector 模块和 provider 枚举。

本阶段不建议追覆盖数量、排行榜、遥测、在线账户、跨平台 GUI 框架替换、通用插件系统，也不建议仅因模块较长就机械拆文件。已有共享核心、静态来源列表和文件缓存能解决当前问题；只有新入口或独立变化边界证明需要更深封装时，才扩大改动。

本次外部调研未读取真实凭据、未请求真实配额、未安装或运行竞品，也未向任何第三方发送本机使用数据。竞品事实是对应快照的源码/文档事实，补足优先级与本项目定位判断是本文的推断；实际修改和测试结果由主代码审计另行记录。
