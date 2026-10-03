# 四个新功能实施计划（待批准）

调研时间：2026-10-03。调研依据：本仓库源码摸底、CC Switch 源码（farion1231/cc-switch `src-tauri/src/commands/misc.rs`）、tw93/kami 排版规范、wry 0.53 API。

## 现状结论（可复用资产）

- 设置页扩展样板齐全：`IpcCommand` → ipc_handler 分支 → `UserEvent` 事件 → `evaluate_script` 回传，前端偏好存 localStorage（`gui-preferences.js`）。
- 自由拖拽缩放面板 = `ModuleBoard.jsx` + `panel-layout.js`（12 列网格、持久化、碰撞/对齐/惯性），换 `storageKey` 即可挂新页面。
- 分块环组件现成：`QuotaPage.jsx` 的 `QuotaRing`（24 段 SVG）、`PanelBodies.jsx` 的 `clockArc` 径向钟。
- 凭证已入 Windows 凭据管理器（`credential.rs`，GLM/Kimi 读写函数已备好未接线）；绝不明文落盘。
- 执行外部命令照抄 `backend.rs::run_omp`（piped + 12s 超时 + CREATE_NO_WINDOW + OEM 解码）。
- 分享卡数据几乎全部现成于 `LedgerReport`：今日/本周/月汇总、by_model、hourly[24]、weekday_hour、trend_24h、burn_rate、monthly_value、by_agent。
- wry 0.53 支持 `with_always_on_top` 与同事件循环多窗口。

---

## 功能 ① 设置页「Agent 一键检测 + 更新」（参考 CC Switch）

**范围**：扫描 6 个 CLI Agent —— claude / codex / gemini / qwen / iflow / opencode（可再加 crush、copilot，见待确认问题 A）。

**检测**（后端新模块 `gui/src/agents.rs` 或 `src/agents_scan.rs`）：
1. `where.exe $PATH:<tool>` 找默认入口（跳过 WindowsApps 执行别名，防项目目录恶意同名 exe——CC Switch 同款防注入思路）。
2. PATH 没有再扫候选目录：`%APPDATA%\npm`、`~\.local\bin`、`~\.<tool>\bin`、`%LOCALAPPDATA%\Programs\*`、scoop shims、nvm/volta/fnm/pnpm 目录。
3. 每处安装跑 `--version`（10s 超时、CREATE_NO_WINDOW、OEM 代码页解码），区分「未装 / 装了但跑不起来 / 正常」。
4. 最新版查 npm registry dist-tags（复用现有 `http.rs`），取不到显示"未知"不卡住。

**更新**：
- 按安装来源生成锚定命令（npm 装 → `npm i -g <pkg>@latest`；claude 原生 → `claude update`；codex 独立安装 → 重跑官方 installer 非交互模式；opencode Windows 上不走 `upgrade`（上游会挂死）走 npm）。
- 执行前弹确认框，明文展示将要执行的命令；命令由后端生成，不信任前端回传；写临时 .bat 串行执行；升级后复检版本。
- 多处安装（npm 与原生共存遮蔽）标"冲突/默认"徽章，只升级默认处并提示。

**前端**：SettingsPage 新增「Agents」区块——每工具一行：图标 + 本机版本 → 最新版 + 状态徽章 + 「升级」按钮 + 一键全部升级；扫描中显示 spinner。

**IPC**：`scanAgents` / `runAgentUpdate(tools)` 两个新命令。

## 功能 ② 社交分享卡（网易云年度报告风 + kami 排版语言）

**入口**：用量/图表页加「分享卡」入口，进全屏卡片编辑器（独立导航页）。

**编辑器 = 复用 ModuleBoard**：
- 模块目录（可勾选、可拖拽缩放布局）：今日 token 大数字、今日请求数、最活跃时段（hourly 众数）、最常用模型 + Top3 模型条、24h 时段环（clockArc 缩略）、26 周热力缩略图、burn rate、本月 API 等价价值（monthly_value）、按 agent 分布、人格化总结卡。
- 画布固定 **1080×1440（3:4，小红书最优比例）**，预览时 scale 缩放；支持追加下一屏拼成长图（首屏大数字开场 + 尾屏总结卡的 Wrapped 叙事结构）。
- 点缀：卡片配色跟应用主题（含 kami 式暖底方案 parchment #f5f4ed + 墨蓝 #1B365D 唯一强调色 + 暖灰文字梯度 + 500 字重 tabular-nums 大数字，禁冷灰）；布局与勾选存 localStorage。

**导出**：`evaluate_script` 注入 **SnapDOM**（SVG foreignObject 路线，现代、快、还原度高）→ dataURL → IPC 回传 Rust → rfd 存 PNG（复用 export.rs 的保存对话框）。图片全部内联 base64、字体用系统已加载字体，规避 CORS/字体两大坑。wry 0.53 无原生截图 API（issue #1358 未实现），此为最稳路线；CDP captureScreenshot 作备选。

## 功能 ③ 悬浮小窗（置顶三环 + 切换套餐）

- 同一 tao event loop 里开第二个小窗口（约 220×280），`always_on_top(true)`、无边框、半透明，前端小页渲染。
- 内容：**5 小时环 / 本周环 / 月环** 三枚分块环（复用 QuotaRing 风格缩小版），该 provider 没有的环自动隐藏；环下小字显示重置倒计时。
- **上/下按钮**在有实际数据的套餐之间循环切换（按 `ProviderId::all()` 顺序，缺失的跳过），切换即刷新该 provider 的三环。
- 开关入口：托盘菜单「悬浮窗」+ 可选全局快捷键；位置拖动后记住。

**IPC**：悬浮窗页面用精简版 push（只送当前 provider 的 limits），复用现有采集管线，不加新请求。

## 功能 ④ 套餐卡点开：可用模型清单 + API key 打码展示

**交互**：首页点击套餐卡 → 卡片背面/弹层展开：
1. **可用模型清单**：内置静态映射表（provider + plan_type → 官方模型名，来源各官方文档，标注核对日期，支持后续在 settings.json 扩展覆盖）。每行官方名称 + 「导入」按钮（导入去向见待确认问题 B）。
2. **API key 打码展示**：仅当应用在 Windows 凭据管理器里已存有该 provider 的 key 时显示。默认形态 `sk-a3****…****9f2c`（首 4 + 尾 4，中间等宽圆点/雪花点）；**hover 揭示明文，点击复制到剪贴板**，复制后 toast 提示。

**安全分析**：
- key 从凭据管理器读出后经 IPC 明文进前端内存——渲染进程内存可见，与现有设置页明文输入 key 的暴露面相当，本机单用户场景风险可接受；但**不落 localStorage、不进任何日志/快照缓存/导出文件**。
- 复制进剪贴板属新增暴露面，建议复制后 30 秒自动清剪贴板（可在设置关掉）。
- hover 揭示只是 CSS 遮罩，本机截图/共享屏幕时会露——加一个「揭示需双击且 3 秒后自动重新打码」的更保守档位可选。
- 结论：可实现，风险点均可控；不新增网络发送。

---

## 实施顺序（每步独立提交，英文 type + 中文正文）

1. ① Agent 扫描与更新（设置页）
2. ③ 悬浮小窗三环
3. ④ 套餐卡模型清单 + key 打码
4. ② 分享卡编辑器与导出（体量最大放最后）

每步完成后 `npm run build`（dist 进 exe 必须）+ `cargo build`，GUI 验证走 CDP 截图不抢前台。

## 待确认问题

- **A. Agent 扫描范围**：推荐 6 个（claude/codex/gemini/qwen/iflow/opencode）；要不要加 crush、copilot？
- **B. 模型清单「一键导入」导入到哪里**：是导入应用的模型价格/别名表（pricing），还是生成一段可贴进 agent 配置的模型列表，还是别的？此点仓库里没有现成落点，需要 Jerry 指个方向。
- **C. API key 揭示交互**：hover 揭示即可，还是要更保守的双击揭示 + 3 秒自动回打码？复制后 30s 自动清剪贴板要不要默认开？
