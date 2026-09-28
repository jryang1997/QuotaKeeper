# Windows 桌面版竞品功能审计

调研日期：2026-09-27。对象是本项目的 Windows GUI；竞品事实只取项目自己的 README、配置文档或 Microsoft 的 Windows 设计文档。以下“本项目现状”为调研时的基线，最新实施状态见文末。此文是功能与交互参考，不把竞品宣传中的准确度视为已独立验证。

## 先看结论

1. **主题、字体与密度是基础体验。** 工作树的 GUI 已有“跟随系统／浅色／深色”、像素／系统字体、舒适／紧凑密度和减少动态效果选项，但像素字体仍是默认值。Windows 官方建议 UI 以 Segoe UI Variable 为主，并将 Microsoft YaHei UI 作为简体中文界面字体；默认改成系统字体、保留像素风格作为可选项，最直接回应当前视觉问题。[本项目 GUI 源码](../../gui/frontend/src/main.jsx) · [本项目 CSS](../../gui/frontend/src/app.css) · [Microsoft Windows typography](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography)
2. **随手查看是桌面应用的关键。** 两个 Windows 同类工具都把托盘图标与弹出面板作为主要入口；其中 CodexBar for Windows 还提供最紧张额度图标、全局快捷键、更新时间、可选任务栏停靠条。Token Monitor 提供托盘／悬浮泡、全局显隐快捷键、窗口置顶或桌面固定模式。本项目当前 GUI 是普通窗口，关闭即退出，尚无这些入口。[CodexBar for Windows README](https://github.com/dontcallmejames/CodexBar-Windows#features) · [另一个 CodexBar for Windows README](https://github.com/SameRainbows/CodexBar-For-Windows#features) · [Token Monitor 配置文档](https://github.com/Javis603/token-monitor/blob/main/docs/configuration.md) · [本项目窗口源码](../../gui/src/main.rs)
3. **有用的预测要说明依据。** CodexBar 展示相对可持续速率的“超前／余量”和“能撑到重置／预计耗尽”，且窗口开头样本不足时不显示估计。Claude Code Usage Monitor 把官方额度、本地估计、实验数据分开标记。项目已有按窗口已过时间线性外推的额度耗尽时间和近 60 分钟本机 token 速率，但两者口径不同，GUI 应显式展示来源、更新时间与估计条件。[CodexBar UI 文档：Pace tracking](https://github.com/steipete/CodexBar/blob/main/docs/ui.md#pace-tracking) · [Claude Code Usage Monitor README：Machine-readable Usage Protocol](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor#machine-readable-usage-protocol) · [本项目 README](../../README.md)

## 一手资料中的优秀做法

| 维度 | 官方资料中的具体做法 | 对本项目的判断 |
|---|---|---|
| 视觉、主题、字体 | Windows 版 CodexBar 使用 Fluent、Mica/Acrylic，并跟随系统主题与强调色；Token Monitor 允许浅色、深色及自定义颜色、各工具颜色、玻璃透明度和自定义字体；Claude Code Usage Monitor 提供 auto/light/dark/classic，并持久化明确选择的主题。[CodexBar for Windows README](https://github.com/dontcallmejames/CodexBar-Windows#codexbar-for-windows) · [Token Monitor README](https://github.com/Javis603/token-monitor#features) · [Claude Code Usage Monitor README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor#available-command-line-parameters) | 沿用现有 WebView 与设计令牌即可先做好系统／浅／深模式、对比度和字号；不必为材质效果更换 GUI 框架。把系统字体设为默认，像素字体当作明确的个性化选择。 |
| 配额预测 | CodexBar 把实际使用与窗口内可持续用量比较，显示“in deficit／in reserve”、预计耗尽或能撑到重置；样本太早时抑制预测。Claude Code Usage Monitor 计算近期 burn rate，并在机器可读快照中标注 official／local_estimate 等来源。[CodexBar UI 文档](https://github.com/steipete/CodexBar/blob/main/docs/ui.md#pace-tracking) · [Claude Code Usage Monitor README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor#machine-readable-usage-protocol) | 先在每个真实额度窗口旁显示进度与时间的关系；估计值必须标“按当前窗口线性估算”。低样本、过期或缺失重置时间时显示“暂无可靠估计”。不要用 token/min 直接推算不同服务的百分比额度。 |
| 通知 | CodexBar 配置含会话、阈值和预测 pace 提醒及通知时间窗；Windows 版 CodexBar 把更新提示发成 Windows 通知；另一个 Claude 专用监控工具按 5h／7d 阈值及预计耗尽提醒，并按重置窗口去重。[CodexBar 配置文档](https://github.com/steipete/CodexBar/blob/main/docs/configuration.md) · [CodexBar for Windows README](https://github.com/dontcallmejames/CodexBar-Windows#features) · [claude-usage-monitor README](https://github.com/SteveHuang27GitHub/claude-usage-monitor#what-it-does) | 本项目 TUI 已有 80%、100%、重置 Windows toast 与去重；GUI 目前未接这条提醒链。GUI 应先复用现有事件判定，再加开关、静默时段和“读取失败／数据过期”选项；预测提醒只在估计可靠时发。 |
| 托盘与快捷操作 | CodexBar for Windows 的托盘图标显示最紧张 provider，左键弹出、右键 Settings/About/Quit，支持全局 Ctrl+Alt+U、可选任务栏停靠条；另一个 Windows 实现支持托盘右键刷新及单实例 `--show`。Token Monitor 允许托盘模式、悬浮泡、全局显隐快捷键和窗口模式。[CodexBar for Windows README](https://github.com/dontcallmejames/CodexBar-Windows#first-run) · [另一个 CodexBar for Windows README](https://github.com/SameRainbows/CodexBar-For-Windows#quick-start) · [Token Monitor 配置文档](https://github.com/Javis603/token-monitor/blob/main/docs/configuration.md) | 首个迭代做托盘图标、左键显示／隐藏、右键刷新／设置／退出、单实例唤醒。第二步增加可配置全局快捷键及开机启动。任务栏停靠条和悬浮泡属高级模式，待基础入口稳定后考虑。 |
| 刷新与新鲜度 | CodexBar 提供手动、固定间隔与自适应刷新；刷新失败会让图标变暗并在菜单提示过期／错误。CodexBar for Windows 按 provider 独立退避，某一路遇限流不拖慢其他来源；弹出面板显示“Ns ago”。[CodexBar refresh-loop 文档](https://github.com/steipete/CodexBar/blob/main/docs/refresh-loop.md) · [CodexBar for Windows README](https://github.com/dontcallmejames/CodexBar-Windows#features) | GUI 已将本机账本与额度分别按 5 秒／60 秒刷新，也有总的更新时间与配额错误横幅。应继续拆成各类数据的上次成功时间和状态，并保证失败时旧值被标为“旧数据”。[本项目刷新源码](../../gui/src/refresh.rs) · [本项目 GUI 源码](../../gui/frontend/src/main.jsx) |
| 隐私 | CodexBar 明确只查已知配置和日志路径，额外进程活动扫描要用户同意；Token Monitor 声明本地处理日志、不向维护者发送遥测，跨设备同步单机默认不需要；Windows 版 CodexBar 将手填 cookie 通过 DPAPI 加密。[CodexBar README：Privacy note](https://github.com/steipete/CodexBar#privacy-note) · [Token Monitor README：Privacy](https://github.com/Javis603/token-monitor#privacy) · [CodexBar for Windows README：Privacy](https://github.com/dontcallmejames/CodexBar-Windows#privacy) | 设置页应展示“从哪里读、哪些请求会发出、凭据放在哪、如何清理缓存”。目前本机账本源与价格缓存说明主要在 README；GUI 应有可点开的来源说明。涉及密钥时避免在界面、日志或导出里泄漏。 |
| 数据分析与对比 | ccusage 提供日期范围、时区、项目筛选、模型拆分、JSON 和无成本模式；Token Monitor 可重排 Home 模块，按工具／设备／模型／会话／项目看用量，提供 CSV/JSON 导出与订阅支出登记；CodexBar 的 Usage & Spend 支持按日明细、部分价格标记和本地复制图片。[ccusage README](https://github.com/ccusage/ccusage#features) · [Token Monitor README](https://github.com/Javis603/token-monitor#features) · [CodexBar UI 文档](https://github.com/steipete/CodexBar/blob/main/docs/ui.md#daily-spend-ledger) | 项目已有 Agent／会话／项目、趋势、热力图和月费基准。下一步加入明确的时间范围与两段时间对比、CSV/JSON 导出及价格缺失标记；对比要保持“来源报告成本／估算成本”分列。 |
| 首次启动与来源诊断 | CodexBar for Windows 的初次欢迎页让用户启用 provider，每项给凭据来源 Help，并可测试凭据；Token Use 提供 `doctor` 自检。本项目已有 missing 卡与来源错误提示，适合做“检测来源”页，列出路径、上次成功、数据条数、错误及修复动作。[CodexBar for Windows README](https://github.com/dontcallmejames/CodexBar-Windows#first-run) · [Token Use README](https://github.com/russmckendrick/tokenuse) · [本项目 README](../../README.md) | 这项比再接入许多 provider 更能减少空白页面的困惑。 |

## 建议执行顺序与验收标准

| 优先级 | 工作包 | 可验证的完成标准 |
|---|---|---|
| P0 | 视觉基线与主题 | GUI 默认使用 `Segoe UI Variable, Microsoft YaHei UI` 的系统字体栈，数字使用 tabular lining；保留像素风格可选。系统／浅／深主题可即时切换且重启保留，跟随系统时 Windows 外观变化会更新。常用信息在 100%、125%、150% 缩放与 360px 窄宽度仍可读。Microsoft 建议正文一般不小于 12px regular，中文尤其要避免现在部分 2xs 的视觉负担。[Microsoft Windows typography](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography) |
| P0 | 新鲜度与来源说明 | 配额、本机账本分别显示上次成功时间；失败后继续展示缓存值时标为过期。每张配额卡可查看真实来源、窗口和估计方法；Antigravity 的 token 强度始终与官方额度百分比视觉区分。[CodexBar refresh-loop 文档](https://github.com/steipete/CodexBar/blob/main/docs/refresh-loop.md) · [Claude Code Usage Monitor README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor#machine-readable-usage-protocol) |
| P1 | Windows 常驻入口 | 托盘图标显示风险级别，点击显隐 GUI、右键刷新／设置／退出；关闭窗口时用户可选择退出或留在托盘，首次行为清楚说明；再次启动只唤醒已有窗口。提醒事件复用现有去重规则。[CodexBar for Windows README](https://github.com/dontcallmejames/CodexBar-Windows#first-run) · [另一个 CodexBar for Windows README](https://github.com/SameRainbows/CodexBar-For-Windows#quick-start) |
| P1 | 可操作的 pace | 对有真实百分比和重置时间的窗口显示“比匀速使用快／慢多少”、预计能否撑到重置；样本不足或数据过期时隐藏 ETA。阈值与预测提醒有独立开关和静默时段。[CodexBar UI 文档](https://github.com/steipete/CodexBar/blob/main/docs/ui.md#pace-tracking) · [CodexBar 配置文档](https://github.com/steipete/CodexBar/blob/main/docs/configuration.md) |
| P2 | 使用分析出口 | 支持自选日期范围、与上一等长区间对比、按 Agent／项目筛选、导出 CSV/JSON；输出带时区、来源与估算标记。针对缺价数据，金额显示部分汇总标识。[ccusage README](https://github.com/ccusage/ccusage#features) · [Token Monitor README](https://github.com/Javis603/token-monitor#features) |
| P2 | 个性化与引导 | 首次启动检测每个可用来源并给修复链接；支持固定／隐藏卡片、显示密度与 provider 颜色。全局快捷键和登录启动可选。保留设置重置入口。[CodexBar for Windows README](https://github.com/dontcallmejames/CodexBar-Windows#first-run) · [Token Monitor 配置文档](https://github.com/Javis603/token-monitor/blob/main/docs/configuration.md) |

## 取舍依据

- **先改善阅读质量，再加装饰。** Microsoft 的 Windows 字体规范以清晰层级和可读性为目标，明确建议 Segoe UI Variable、中文用 Microsoft YaHei UI；项目当前默认的 Sixtyfour 与秋叶圆体适合作为风格选项，长表格和金额主界面则应以系统字形为基线。[Microsoft Windows typography](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography) · [本项目 CSS](../../gui/frontend/src/app.css)
- **不把“预测”包装成官方额度。** CodexBar 的 pace 是窗口用量与时间关系；Claude Code Usage Monitor 刻意区分官方、当地估计和实验信号。项目的 Antigravity 卡当前是 CPA 凭据健康与 token 强度，不能画成 credits 百分比。[CodexBar UI 文档](https://github.com/steipete/CodexBar/blob/main/docs/ui.md#pace-tracking) · [Claude Code Usage Monitor README](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor#machine-readable-usage-protocol) · [本项目 README](../../README.md)
- **不复制所有设置。** Token Monitor 的同步、悬浮泡、窗口固定、货币与多账户设置覆盖很广，但本项目应优先让已有四张额度卡与本地账本在 Windows 上更容易打开、看懂和信任。[Token Monitor 配置文档](https://github.com/Javis603/token-monitor/blob/main/docs/configuration.md)

## 本轮实施记录

P0–P2 六个工作包已在当前工作树逐项落地：系统字体、持久主题、窄屏布局；独立成功时间、过期状态及每张配额卡的来源／窗口／计算口径；风险色托盘、可选关闭行为、单实例唤醒；相对匀速差值与可靠性门槛、独立预测提醒；任意日期范围与上一等长区间比较、Agent／项目筛选、带时区／采集器标识／成本口径／缺价标记的 CSV／JSON 导出；首次来源检测与修复链接、卡片固定／隐藏、来源颜色、可选全局快捷键及登录启动。隐藏的卡片不会参与顶部额度摘要，采集失败时立即隐藏耗尽预测；过期标记与上次成功时间并列显示。见 [桌面版说明](../../README.md#windows-桌面版) 与 [GUI 源码](../../gui/)。

验证记录：根 crate 79 项测试及 GUI 9 项测试通过；前端生产构建通过，两个 crate 的 Clippy 严格检查通过。浏览器中以 360px CSS 宽度和 100%／125%／150% 设备缩放渲染配额与用量页，正文最小 12px，无横向溢出；系统深色偏好及手动浅色／深色主题切换可改变实际界面颜色，手动选择在页面重载后保留。Windows GUI Debug 和 Release 程序加载成功，第二次启动以退出码 0 唤醒单实例；登录启动注册表项的写入／查询／删除经过隔离值测试。GNU 构建现自动复制 WebView2Loader.dll；分发时需与 exe 一起提供。真实 Windows 显示设置的多显示器 DPI、系统登录后的自启时机和通知弹出仍应在目标机器上做人工验收。

本轮未纳入 P0–P2 验收的可选扩展包括自由配置全局快捷键组合、按 provider 自适应退避、任务栏停靠条和悬浮泡。
