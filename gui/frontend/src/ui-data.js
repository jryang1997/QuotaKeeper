export const PROVIDER_ACCENT = {
  'opencode-go': 'var(--color-text-blue)',
  codex: 'var(--color-text-green)',
  goat: 'var(--color-text-purple)',
  antigravity: 'var(--color-text-orange)',
  claude: 'var(--color-text-orange)',
  cursor: 'var(--color-text-primary)',
  glm: 'var(--color-text-blue)',
  kimi: 'var(--color-text-orange)',
  minimax: '#f0476a',
  deepseek: '#2764d6',
};

export const STATUS_COLOR = {
  ok: 'var(--color-success)',
  warning: 'var(--color-warning)',
  exhausted: 'var(--color-error)',
};

export const PROVIDER_COLOR_DEFAULTS = {
  'opencode-go': '#14b8a6', codex: '#8b5cf6', goat: '#f59e0b', antigravity: '#0ea5e9', claude: '#d97757', cursor: '#5a5a5a', glm: '#2f6bff', kimi: '#ff8d1a', minimax: '#f0476a', deepseek: '#2764d6',
};
export const PROVIDERS = ['opencode-go', 'codex', 'goat', 'antigravity', 'claude', 'cursor', 'glm', 'kimi', 'minimax', 'deepseek'];
export const PROVIDER_LOGOS = {
  'opencode-go': 'logos/opencode-go.svg',
  codex: 'logos/codex.svg',
  goat: 'logos/commandcode.svg',
  antigravity: 'logos/antigravity.png',
  claude: 'logos/claude.png',
  cursor: 'logos/cursor.png',
  deepseek: 'logos/deepseek.svg',
};

export const SOURCE_PATHS = {
  codex: '~/.codex/sessions/',
  omp: '~/.omp/stats.db',
  opencode: '~/.local/share/opencode/opencode.db',
  zcode: '~/.zcode/cli/db/db.sqlite',
  dsh: '~/.dsh/sessions/',
  claude: '~/.claude/projects/',
};
export const SOURCE_REPAIR = {
  codex: ['先在 Codex 中运行一次会话，再检查 ~/.codex/sessions/ 下是否有 rollout 日志。', 'Run a Codex session, then check for rollout logs under ~/.codex/sessions/.'],
  omp: ['启动 OMP 并完成登录；运行 omp usage --json 检查额度，stats.db 用于本机用量。', 'Start OMP and sign in. Run omp usage --json for quotas; stats.db holds local usage.'],
  opencode: ['在 OpenCode 中运行一次会话；确认本机数据库已生成。', 'Run an OpenCode session and check that its local database exists.'],
  zcode: ['在 ZCode 中运行一次会话；确认本机数据库已生成。', 'Run a ZCode session and check that its local database exists.'],
  dsh: ['在 DeepSeek Harness 中运行一次会话；检查压缩会话日志。', 'Run a DeepSeek Harness session and check its compressed session logs.'],
  claude: ['在 Claude Code 中运行一次会话；检查 ~/.claude/projects/ 下的 JSONL 日志。', 'Run a Claude Code session and check JSONL logs under ~/.claude/projects/.'],
};
export const QUOTA_REPAIR = {
  'opencode-go': ['在设置页保存 Go API key，或设置 OPENCODE_GO_API_KEY；也可读取 OpenCode 登录态。', 'Save your Go API key in Settings, set OPENCODE_GO_API_KEY, or use an OpenCode login.'],
  codex: ['登录 Codex 官方程序后刷新；应用通过 app-server 读取限额。', 'Sign in to the official Codex app and refresh; quotas come from app-server.'],
  goat: ['在设置页保存 Command Code API key，或配置 COMMAND_CODE_API_KEY；也可使用 cmdc 登录态。', 'Save a Command Code API key in Settings, set COMMAND_CODE_API_KEY, or use a cmdc login.'],
  antigravity: ['启动本机 CPA，并将管理密钥放在 ~/.tui-token-monitor/cpa-key；应用会通过 CPA 代查 Google 上游配额。', 'Start local CPA and place the management key in ~/.tui-token-monitor/cpa-key. The app reads upstream Google quotas through CPA.'],
  glm: ['设置环境变量 GLM_API_KEY 后重启应用（bigmodel.cn 或 api.z.ai 控制台创建）；国际站还需 GLM_API_BASE=https://api.z.ai。', 'Set GLM_API_KEY and restart the app (create a key in the bigmodel.cn or api.z.ai console); the global region also needs GLM_API_BASE=https://api.z.ai.'],
  kimi: ['设置环境变量 KIMI_CODE_API_KEY 后重启应用（kimi.com 控制台创建，sk-kimi- 开头；与 Moonshot 开放平台 key 不通用）。', 'Set KIMI_CODE_API_KEY and restart the app (from kimi.com, sk-kimi- prefix; not interchangeable with Moonshot platform keys).'],
  minimax: ['在凭据管理器添加 TuiTokenMonitor/MiniMax（sk-cp- 编程套餐密钥），或设置 MINIMAX_CODING_API_KEY 后重启应用。', 'Add TuiTokenMonitor/MiniMax in Credential Manager (sk-cp- coding plan key), or set MINIMAX_CODING_API_KEY and restart the app.'],
};
export const QUOTA_HINT = {
  'opencode-go': ['未连接 · 保存 Go API key 或登录 OpenCode', 'Not connected · save a Go API key or sign in to OpenCode'],
  codex: ['未连接 · 登录 Codex 后刷新', 'Not connected · sign in to Codex and refresh'],
  goat: ['未连接 · 保存 Command Code key 或登录 cmdc', 'Not connected · save a Command Code key or use cmdc login'],
  antigravity: ['未连接 · 启动 CPA 并放置管理密钥', 'Not connected · start CPA with its management key'],
  glm: ['未连接 · 设置 GLM_API_KEY 后重启', 'Not connected · set GLM_API_KEY and restart'],
  kimi: ['未连接 · 设置 KIMI_CODE_API_KEY 后重启', 'Not connected · set KIMI_CODE_API_KEY and restart'],
  minimax: ['未连接 · 添加 MiniMax 编程套餐密钥后重启', 'Not connected · add a MiniMax coding plan key and restart'],
};

export const CLIENT_COLORS = {
  codex: '#7c3aed',
  omp: '#0ea5e9',
  opencode: '#f59e0b',
  zcode: '#10b981',
  dsh: '#3b82f6',
  claude: '#ef6c4d',
};

export const STR = {
  title: ['QuotaKeeper', 'QuotaKeeper'],
  nav_quota: ['订阅额度', 'Subscriptions'],
  nav_usage: ['Token 用量', 'Token usage'],
  nav_settings: ['设置', 'Settings'],
  refresh: ['刷新', 'Refresh'],
  refreshing: ['采集中…', 'Fetching…'],
  updated: ['更新于', 'Updated'],
  quota_error: ['额度读取失败', 'Quota fetch failed'],
  startup_write_failed: ['Windows 启动项写入失败', 'Failed to update Windows startup'],
  shortcut_conflict: ['Ctrl+Alt+U 已被其他应用占用', 'Ctrl+Alt+U is already in use'],
  today: ['今日', 'Today'],
  week: ['本周', 'This week'],
  month: ['本月', 'This month'],
  resets_in: ['重置', 'resets'],
  messages: ['次请求', 'requests'],
  cost_est: ['~$', '~$'],
  no_data: ['暂无数据', 'No data'],
  loading: ['正在读取…', 'Loading…'],
  empty_quota: ['暂无配额数据', 'No quota data'],
  empty_trend: ['这段时间暂无用量', 'No usage in this range'],
  metric_cost: ['API 等值', 'API equiv.'],
  sources: ['数据源', 'Sources'],
  missing: ['未接入', 'Not connected'],
  settings_kicker: ['偏好设置', 'PREFERENCES'],
  settings_appearance: ['外观', 'Appearance'],
  settings_language: ['界面语言', 'Language'],
  settings_theme: ['主题', 'Theme'],
  settings_theme_hint: ['跟随 Windows 外观', 'Follows your Windows look'],
  theme_system: ['跟随系统', 'System'],
  theme_light: ['浅色', 'Light'],
  theme_dark: ['深色', 'Dark'],
  theme_cassette: ['卡带未来', 'Cassette Future'],
  theme_quick: ['切换主题', 'Switch theme'],
  settings_type: ['字体与布局', 'Type & layout'],
  settings_font: ['字体风格', 'Typeface'],
  font_sarasa: ['更纱黑体', 'Sarasa Gothic'],
  font_plex: ['Plex 现代终端', 'Plex modern terminal'],
  settings_density: ['信息密度', 'Density'],
  density_comfortable: ['舒展', 'Comfortable'],
  density_compact: ['紧凑', 'Compact'],
  settings_motion: ['动态效果', 'Motion'],
  motion_system: ['跟随系统', 'Follow system'],
  motion_reduce: ['减少动态', 'Reduce motion'],
  settings_plan: ['套餐基准', 'Plan benchmark'],
  settings_currency: ['显示货币', 'Display currency'],
  settings_currency_hint: ['API 等值参考按汇率换算；原始价格以 USD 记录。', 'API equivalents use a reference exchange rate; source prices are recorded in USD.'],
  settings_currency_rate: ['参考汇率', 'Reference rate'],
  settings_currency_usd: ['原始计价单位 · USD', 'Source pricing unit · USD'],
  settings_currency_stale: ['离线缓存', 'Cached offline'],
  settings_currency_error: ['汇率暂不可用；金额暂不显示', 'Rate unavailable; converted amounts are hidden'],
  settings_currency_loading: ['正在获取汇率…', 'Loading exchange rate…'],
  settings_currency_source: ['汇率来源：', 'Rate source:'],
  settings_save: ['保存', 'Save'],
  settings_saved: ['已保存', 'Saved'],
  settings_font_sample: ['用量一目了然', 'Usage at a glance'],
  settings_reset_all: ['恢复默认偏好', 'Reset preferences'],
  quota_overview: ['额度总览', 'Quota overview'],
  quota_attention: ['需要关注', 'Needs attention'],
  quota_all_clear: ['当前额度充足', 'Quotas look healthy'],
  quota_unavailable: ['暂无可用额度', 'No live quotas'],
  quota_used: ['已用', 'Used'],
  usage_sum_tokens: ['TOTAL TOKENS', 'TOTAL TOKENS'],
  usage_sum_runs: ['TOTAL RUNS', 'TOTAL RUNS'],
  usage_unit_tokens: ['tokens', 'tokens'],
  usage_unit_runs: ['runs', 'runs'],
  usage_sum_official: ['当前账期 · Command Code 官方', 'Current billing period · Command Code'],
  usage_sum_local: ['本月 · 本机账本', 'This month · local ledger'],
  usage_card_monthly: ['MONTHLY', 'MONTHLY'],
  quota_remaining: ['剩余', 'Remaining'],
  quota_next_reset: ['最近重置', 'Next reset'],
  source_health: ['本机数据源', 'Local sources'],
  source_ready: ['有记录', 'Active'],
  source_idle: ['暂无记录', 'No records'],
  source_error: ['读取失败', 'Read failed'],
  freshness_quota: ['服务额度', 'Service quotas'],
  freshness_local: ['本机用量', 'Local usage'],
  freshness_waiting: ['等待首次读取', 'Waiting for first read'],
  freshness_stale: ['上次读数 · 待更新', 'Previous reading · refresh needed'],
  freshness_last_success: ['上次成功', 'Last success'],
  cache_replay: ['显示上次留存的数据 · 正在同步最新…', 'Showing your last synced data · refreshing…'],
  activity_vs_average: ['7 日均值', '7-day avg'],
  activity_at_least_average: ['≥ 7 日均值', '≥ 7-day avg'],
  activity_today: ['本机今日活动', 'Local activity today'],
  settings_desktop: ['桌面常驻', 'Desktop presence'],
  settings_desktop_hint: ['关闭后驻留系统托盘', 'Keeps running in the tray'],
  settings_quit: ['退出应用', 'Quit app'],
  settings_notifications: ['额度通知', 'Quota alerts'],
  settings_notifications_hint: ['80% / 100% / 重置时通知', 'Alerts at 80%, 100% and reset'],
  settings_close_behavior: ['关闭窗口时', 'When closing the window'],
  settings_close_tray: ['留在托盘', 'Stay in tray'],
  settings_close_exit: ['退出应用', 'Quit app'],
  settings_shortcut: ['全局显隐快捷键', 'Global show/hide shortcut'],
  settings_shortcut_hint: ['Ctrl+Alt+U 全局呼出', 'Toggle from anywhere with Ctrl+Alt+U'],
  settings_autostart: ['登录 Windows 时启动', 'Start when signing into Windows'],
  settings_autostart_hint: ['为当前 Windows 用户添加启动项', 'Adds a startup entry for this Windows user'],
  settings_go_key: ['OpenCode Go 直连', 'OpenCode Go direct access'],
  settings_go_key_hint: ['密钥存入 Windows 凭据管理器', 'Stored in Windows Credential Manager'],
  settings_go_key_stored: ['已保存专用密钥', 'Dedicated key saved'],
  settings_go_key_missing: ['未保存密钥 · 可用环境变量或登录态', 'No key saved · env vars or a login may work'],
  settings_go_key_placeholder: ['粘贴 Go API key', 'Paste Go API key'],
  settings_go_key_remove: ['移除密钥', 'Remove key'],
  settings_go_key_failed: ['Windows 凭据管理器操作失败', 'Windows Credential Manager operation failed'],
  settings_go_key_invalid: ['请输入不含空格的 API key', 'Enter an API key without spaces'],
  card_details: ['套餐详情', 'Plan details'],
  models_title: ['可用模型', 'Available models'],
  models_import: ['导入', 'Import'],
  models_imported: ['已导入', 'Imported'],
  models_copy: ['复制列表', 'Copy list'],
  models_copied: ['已复制到剪贴板', 'Copied to clipboard'],
  models_source: ['官方名称 · 核对于', 'Official names · verified'],
  key_stored: ['API key（凭据管理器）', 'API key (Credential Manager)'],
  key_mask_hint: ['悬停显示 · 点击复制', 'Hover to reveal · click to copy'],
  key_clip_clear: ['复制后 30s 清空剪贴板', 'Clear clipboard 30s after copy'],
  key_copy_done: ['已复制 · 30s 后自动清空剪贴板', 'Copied · clipboard clears in 30s'],
  key_missing: ['未在应用内存有该套餐的 key', 'No key stored for this plan in the app'],
  lan_title: ['局域网看板', 'LAN dashboard'],
  lan_hint: ['手机浏览器打开，只看各窗口还剩多少。可在浏览器菜单里添加到主屏幕。', 'Open it on your phone. Each window shows how much is left. Add it to the home screen from the browser menu.'],
  lan_on: ['开启', 'On'],
  lan_off: ['关闭', 'Off'],
  lan_running: ['运行中 · 手机访问', 'Running · open on your device'],
  lan_stopped: ['已停用', 'Stopped'],
  lan_copy: ['复制地址', 'Copy URL'],
  lan_copied: ['已复制', 'Copied'],
  lan_security: ['仅限受信任局域网 · 数据只读、不含密钥', 'Trusted networks only · read-only, no secrets'],
  settings_goat_key: ['Command Code GOAT 直连', 'Command Code GOAT direct access'],
  settings_goat_key_hint: ['密钥存入 Windows 凭据管理器', 'Stored in Windows Credential Manager'],
  settings_alerts_on: ['开启', 'On'],
  settings_alerts_off: ['关闭', 'Off'],
  settings_quiet_hours: ['夜间静默', 'Quiet hours'],
  settings_quiet_hours_hint: ['22:00–08:00 不弹通知', 'No alerts from 22:00 to 08:00'],
  source_location: ['默认读取位置', 'Default read location'],
  source_diagnostic: ['采集状态', 'Collection status'],
  source_ok_detail: ['本次读取成功', 'Read succeeded'],
  source_no_records: ['无本机记录', 'No local records'],
  export_title: ['导出本机账本', 'Export local ledger'],
  export_hint: ['导出当前筛选下的逐日数据', 'Exports daily rows for the current filters'],
  export_saved: ['文件已保存', 'File saved'],
  export_error: ['导出失败，请重试', 'Export failed, please retry'],
  export_unavailable: ['暂无可导出的数据', 'No data to export yet'],
  export_refresh_required: ['用量报告已更新，导出已暂停。请点击顶部「刷新」后重试。', 'The usage report changed. Export is paused; use Refresh at the top, then retry.'],
  export_stale: ['当前显示上次读取的用量，暂不能导出。请点击顶部「刷新」。', 'Showing previously collected usage. Export is disabled; use Refresh at the top.'],
  usage_refresh_required: ['这份用量数据不兼容，已暂不显示。请点击顶部「刷新」重新读取；订阅额度仍可查看。', 'This usage data is incompatible and has been withheld. Use Refresh at the top; subscription quotas are still available.'],
  cards_custom: ['配额卡片', 'Quota cards'],
  cards_custom_hint: ['固定重要卡片、隐藏无关卡片，并调整各来源颜色', 'Pin key cards, hide irrelevant cards, and set provider colors'],
  card_pin: ['固定', 'Pin'],
  card_unpin: ['取消固定', 'Unpin'],
  card_hide: ['隐藏', 'Hide'],
  card_show: ['显示', 'Show'],
  cards_restore: ['恢复卡片默认设置', 'Reset cards'],
  welcome_title: ['欢迎使用 QuotaKeeper', 'Welcome to QuotaKeeper'],
  welcome_intro: ['订阅剩余取自服务商，本机 Token 取自会话记录。两类数据分别统计。', 'Subscription quotas come from providers. Local tokens come from session records. They are tracked separately.'],
  welcome_done: ['开始使用', 'Get started'],
  welcome_reopen: ['重新检测来源', 'Review sources'],
  welcome_repair: ['查看修复方法', 'How to fix'],

  // ---- 2026-10 信息架构收口:周期词表、口径标注、布局开关、状态词 ----
  period_today: ['今天', 'Today'],
  period_yesterday: ['昨天', 'Yesterday'],
  period_7d: ['近 7 天', 'Last 7 days'],
  period_30d: ['近 30 天', 'Last 30 days'],
  period_month: ['本月', 'This month'],
  period_custom: ['自定义', 'Custom'],
  period_range: ['时间范围', 'Range'],
  period_agent: ['采集源', 'Collector'],
  period_model: ['模型', 'Model'],
  period_all: ['全部', 'All'],
  period_clear: ['清除筛选', 'Clear filters'],
  period_from: ['开始', 'From'],
  period_to: ['结束', 'To'],
  period_invalid: ['请选择有效日期，最长 366 天', 'Choose valid dates within 366 days'],
  export_label: ['导出', 'Export'],
  status_ok: ['正常', 'OK'],
  status_warning: ['关注', 'Attention'],
  status_exhausted: ['耗尽', 'Exhausted'],
  quota_local_month: ['本机本月', 'Local this month'],
  quota_official_period: ['官方账期', 'Official period'],
  quota_balance: ['账户余额', 'Balance'],
  quota_requests: ['请求', 'requests'],
  quota_hidden_link: ['已隐藏 {n} 张卡片 · 管理', '{n} hidden cards · manage'],
  quota_connect: ['还没有可用的额度数据 · 去连接', 'No live quota yet · connect sources'],
  card_menu: ['卡片操作', 'Card actions'],
  card_details_hide: ['收起详情', 'Hide details'],

  // ---- 用量页 ----
  usage_subtitle: ['本机账本 · 与服务商配额口径不同', 'Local ledger · separate from provider quotas'],
  usage_recent: ['最近 60 分钟', 'Last 60 minutes'],
  usage_no_activity: ['暂无活动', 'No activity'],
  usage_tokens_per_min: ['Tokens/分钟', 'Tokens/min'],
  usage_summary_tokens: ['总 Token', 'Total tokens'],
  usage_summary_requests: ['调用记录数', 'Usage records'],
  usage_total_note: ['含缓存读取和写入', 'Includes cache reads and writes'],
  usage_record_note: ['采集源记录数，未必等于 API 请求数', 'Source records, not guaranteed API request counts'],
  usage_summary_cost: ['已知成本 ≈', 'Known cost ≈'],
  usage_summary_cost_hint: ['来源报告价与估算价', 'Reported and estimated cost'],
  usage_summary_local: ['按本机记录统计', 'From local records'],
  usage_panel_tokens: ['Tokens', 'Tokens'],
  usage_panel_requests: ['调用记录数', 'Usage records'],
  usage_panel_cost: ['已知成本', 'Known cost'],
  usage_trend_title: ['用量趋势', 'Usage trend'],
  usage_trend_hint: ['悬浮或点击柱子查看构成', 'Hover or select a bar for details'],
  usage_metric: ['图表指标', 'Chart metric'],
  usage_breakdown: ['图表拆分', 'Chart breakdown'],
  usage_metric_messages: ['调用记录数', 'Usage records'],
  usage_breakdown_agent: ['采集源', 'Collector'],
  usage_breakdown_type: ['类型', 'Type'],
  usage_token_mix: ['Token 构成', 'Token mix'],
  usage_reasoning_note: ['推理 Token 已计入输出；不同工具的缓存记录能力可能不同。', 'Reasoning is included in output; cache reporting varies by tool.'],
  usage_no_records: ['所选范围暂无记录', 'No records in this range'],
  usage_day_empty: ['此日暂无记录', 'No records for this day'],
  usage_unpriced: ['{n} 条记录缺价', '{n} records without pricing'],
  usage_not_priced: ['未估价', 'Unpriced'],
  usage_reasoning_in_output: ['推理 Token 已包含在输出中', 'Reasoning is included in output'],
  usage_unpriced_partial: ['{n} 条缺价 · 部分汇总', '{n} unpriced · partial'],
  price_title: ['金额出处', 'Amount sources'],
  price_reported: ['来源报告', 'Source-reported'],
  price_estimated: ['估算', 'Estimated'],
  price_unpriced: ['缺价，未计入金额', 'Unpriced, excluded from amount'],
  price_records: ['{n} 条记录', '{n} records'],
  price_reported_hint: ['本机日志给出的金额；不是已扣费凭证。', 'Amount from local logs; not proof of a charge.'],
  price_estimated_hint: ['按本次价目估算。', 'Estimated using this report’s prices.'],
  price_disclaimer: ['API 等值含来源报告与估算金额，不是订阅扣费，也不能推算订阅剩余额度。', 'API equivalent combines source-reported and estimated amounts. It is not a subscription charge or a measure of remaining quota.'],
  price_history_hint: ['估算的历史记录使用本报告的价目快照，不恢复历史账单。来源报告来自本机日志，不是已扣费凭证。', 'Historical estimates use this report’s price snapshot, not historical bills. Source-reported amounts come from local logs, not proof of charges.'],
  price_catalogue_hint: ['目录条目的前缀不代表实际付费渠道或账号。', 'Catalogue prefixes do not identify the paid channel or account.'],
  price_raw_names: ['原始名称', 'Raw names'],
  price_empty_name: ['空名称', 'Empty name'],
  price_adopted_entry: ['采用条目', 'Adopted entry'],
  price_attempted_entry: ['尝试匹配的条目（未计入金额）', 'Attempted entry (not included in amount)'],
  price_match: ['匹配方式', 'Name matching'],
  price_family: ['借用同系列模型价格（近似）', 'Uses a related model’s price (approximate)'],
  price_cache_fill: ['缓存单价由另一条目补齐（近似）', 'Cache rate filled from another entry (approximate)'],
  price_rate_unit: ['单价 / 百万 Token', 'Rates per million tokens'],
  price_bucket_input: ['输入', 'Input'],
  price_bucket_output: ['输出', 'Output'],
  price_bucket_cache_read: ['缓存读取', 'Cache read'],
  price_bucket_cache_write: ['缓存写入', 'Cache write'],
  price_no_rate: ['未提供单价', 'No rate provided'],
  price_origin_upstream: ['LiteLLM 公开价目', 'LiteLLM public prices'],
  price_origin_supplement: ['内置补充价', 'Built-in supplement'],
  price_origin_legacy_unknown: ['旧缓存，原始来源不明', 'Legacy cache, original source unknown'],
  price_match_exact: ['原始名称命中', 'Original name matched'],
  price_match_case_insensitive: ['忽略大小写后命中', 'Matched ignoring letter case'],
  price_match_explicit_alias: ['已知别名命中', 'Known alias matched'],
  price_match_reverse_alias: ['目录使用另一种模型别名', 'Catalogue uses another model alias'],
  price_match_canonical: ['归并名称后匹配', 'Grouped name matched'],
  price_match_official_reference: ['采用公开目录中的同模型参考条目；不证明实际付费渠道', 'Uses a same-model reference in the public catalogue; not proof of the paid channel'],
  price_match_unique_suffix: ['借用同名模型价目（近似）', 'Uses a same-name model’s price (approximate)'],
  price_match_majority_reference: ['借用同名模型的多数价（近似）', 'Uses the majority price for the same model name (approximate)'],
  price_missing_unknown_model: ['价目里找不到对应模型，未估算。', 'No matching model in the prices; not estimated.'],
  price_missing_ambiguous_name: ['名称能对应不同价格，未估算。', 'The name matches different prices; not estimated.'],
  price_missing_ambiguous_suffix: ['名称能对应不同价格，未估算。', 'The name matches different prices; not estimated.'],
  price_missing_missing_cache_read: ['缺缓存读取单价，未估算。', 'Cache-read rate missing; not estimated.'],
  price_missing_missing_cache_write: ['缺缓存写入单价，未估算。', 'Cache-write rate missing; not estimated.'],
  price_missing_missing_cache_read_and_write: ['缺缓存读取和写入单价，未估算。', 'Cache-read and cache-write rates missing; not estimated.'],
  price_missing_non_finite_amount: ['金额无法计算，未估算。', 'Amount could not be calculated; not estimated.'],
  price_snapshot: ['本次价目', 'This report’s prices'],
  price_cache_state: ['加载时的缓存状态', 'Cache state at load'],
  price_fetched: ['记录的获取时间', 'Recorded fetch time'],
  price_fetch_age: ['获取至加载间隔', 'Fetch-to-load age'],
  price_loaded: ['本次加载时间', 'Loaded at'],
  price_dataset: ['价目来源', 'Price dataset'],
  price_time_unknown: ['时间不明', 'Unknown time'],
  price_age_unknown: ['获取时间不明或时钟异常', 'Fetch time unknown or clock inconsistent'],
  price_load_hint: ['时间按本机时区显示。加载不是价目生效日，也不表示本轮联网获取；缓存新不代表价格仍有效。', 'Times use this device’s timezone. Load time is not a tariff date or proof of a fetch now; a fresh cache does not guarantee a valid tariff.'],
  price_source_cache_v2: ['缓存价目', 'Cached prices'],
  price_source_legacy_cache: ['旧缓存，原始来源不明', 'Legacy cache, original source unknown'],
  price_source_supplements_only: ['仅内置补充价', 'Built-in supplements only'],
  price_cache_fresh: ['获取未满 24 小时', 'Fetched less than 24 hours before load'],
  price_cache_stale: ['获取已满 24 小时', 'Fetched at least 24 hours before load'],
  price_cache_unknown_age: ['获取时间不明或时钟异常', 'Fetch time unknown or clock inconsistent'],
  price_cache_missing: ['没有价目缓存', 'Price cache missing'],
  price_cache_corrupt: ['价目缓存损坏', 'Price cache corrupt'],
  price_cache_not_loaded: ['未加载价目缓存', 'Price cache not loaded'],
  price_currency_fallback: ['没有有效汇率，金额显示为「—」；单价暂按 USD 显示。', 'No valid exchange rate; amounts show “—” and unit rates are shown in USD.'],
  price_export_hint: ['导出保留 USD 原始精度与本报告的价目快照。', 'Exports keep full-precision USD and this report’s price snapshot.'],

  // ---- 设置分组 ----
  settings_group_appearance: ['外观', 'Appearance'],
  settings_group_sources: ['数据源与凭据', 'Sources & keys'],
  settings_group_desktop: ['桌面与通知', 'Desktop & alerts'],
  settings_group_about: ['关于与维护', 'About & maintenance'],
  settings_providers_title: ['服务商直连', 'Provider keys'],
  settings_providers_hint: ['密钥存入 Windows 凭据管理器；也可以使用环境变量或登录态。', 'Keys live in Windows Credential Manager; env vars or logins also work.'],
  settings_provider_ready: ['已保存专用密钥', 'Dedicated key saved'],
  settings_provider_missing: ['未保存密钥 · 可使用环境变量或登录态', 'No key saved · env vars or a login may work'],
  settings_sources_title: ['本机数据源', 'Local sources'],
  settings_sources_hint: ['决定账本能看到哪些本机用量；点开条目看路径与修复方法。', 'Which local usage the ledger can read; open a row for its path and repair steps.'],
  settings_clipboard: ['复制 key 后清空剪贴板', 'Clear clipboard after copying a key'],
  settings_clipboard_hint: ['30 秒后自动清空', 'Clears 30 seconds after copy'],
  settings_overlay: ['悬浮窗', 'Overlay'],
  settings_overlay_hint: ['置顶显示当前套餐的 5H / 周 / 月三环；也可从托盘打开。', 'Always-on-top rings for the current plan; also available from the tray.'],
  settings_overlay_toggle: ['显示 / 隐藏', 'Show / hide'],
  settings_version: ['版本', 'Version'],
  settings_about_hint: ['维护动作与退出', 'Maintenance actions'],
  settings_colors: ['服务商配色', 'Provider colors'],
  settings_colors_hint: ['配额卡与图表的强调色', 'Accent colors for quota cards and charts'],
  settings_deepseek_key: ['DeepSeek 官方余额', 'DeepSeek official balance'],
  settings_deepseek_hint: ['使用官方 API key 查询充值与赠送余额；密钥存入 Windows 凭据管理器。', 'Read prepaid and granted balances with an official API key; it lives in Credential Manager.'],
  settings_source_other: ['其他服务商（登录态 / 环境变量）', 'Other providers (login or env var)'],
  settings_provider_live: ['额度可读', 'Quota readable'],
  settings_provider_unreadable: ['额度不可读', 'Quota unreadable'],

  // ---- 快速跳转 ----
  jump_title: ['快速跳转', 'Quick jump'],
  jump_placeholder: ['跳转到页面、设置、服务商…', 'Jump to a page, setting or provider…'],
  jump_group_page: ['页面', 'Pages'],
  jump_group_settings: ['设置分组', 'Settings'],
  jump_group_provider: ['服务商卡片', 'Providers'],
  jump_group_action: ['动作', 'Actions'],
  jump_move: ['选择', 'move'],
  jump_run: ['执行', 'run'],
  jump_empty: ['没有匹配项', 'No matches'],
  jump_open: ['快速跳转 (Ctrl+K)', 'Quick jump (Ctrl+K)'],
  action_refresh: ['刷新数据', 'Refresh data'],
  action_theme: ['切换主题', 'Cycle theme'],
  action_overlay: ['显示 / 隐藏悬浮窗', 'Show or hide the overlay'],
  sidebar_hide: ['隐藏侧边栏', 'Hide sidebar'],
  sidebar_show: ['显示侧边栏', 'Show sidebar'],

  // ---- 数据状态条 ----
  freshness_title: ['数据状态', 'Data status'],
  freshness_open: ['查看数据状态', 'View data status'],

  local_usage_title: ['本机 Token 消耗', 'Local token usage'],
  local_usage_hint: ['来自本机会话记录，与订阅剩余额度分别统计。', 'From local session records, tracked separately from subscription quotas.'],
  local_usage_details: ['查看用量明细', 'Usage details'],
  local_usage_missing: ['尚未读取到会话记录。请在设置中检查采集源。', 'No session records found. Check sources in Settings.'],
  local_usage_partial: ['部分采集源读取失败，统计可能不完整。', 'Some sources failed; totals may be incomplete.'],
  local_usage_previous: ['当前显示上次读取的记录。', 'Showing the last recorded totals.'],
  settings_sources_empty: ['尚未完成本机采集。可先配置凭据，或返回订阅页刷新。', 'Local collection has not completed. Configure credentials now, or refresh from Subscriptions.'],
};


// 各订阅套餐官方可用模型(内置快照,核对日期见 PLAN_MODELS_VERIFIED,来源:
// docs.z.ai/devpack/overview、platform.kimi.ai/docs/models、platform.claude.com
// 模型对照表、learn.chatgpt.com/docs/models、cursor.com/docs/models-and-pricing、
// antigravity.google/docs/models)。Antigravity 名单官方页未完整公开,仅列交叉印证项。
// 「导入」写 settings.json(plan_models),「复制列表」直接进剪贴板。
export const PLAN_MODELS_VERIFIED = '2026-10-03';
export const PLAN_MODELS = {
  glm: ['glm-5.3', 'glm-5.3-flash'],
  kimi: ['kimi-k3', 'kimi-k2.7-code', 'kimi-k2.7-code-highspeed', 'kimi-k2.6'],
  claude: ['claude-opus-5-5', 'claude-sonnet-5-5', 'claude-haiku-4-5', 'claude-fable-5-1'],
  codex: ['gpt-6.1-sol', 'gpt-6-astra', 'gpt-6-luna'],
  cursor: ['composer-2.5', 'grok-4.7', 'grok-4.6', 'grok-4.5'],
  antigravity: ['gemini-3-pro'],
};


export function fmtInt(n) {
  if (n == null) return '—';
  return n.toLocaleString('zh-CN', { maximumFractionDigits: 0 });
}

// 参数化文案:"共 {n} 个模型 · 显示前 {k}" → fill(STR.usage_models_rest[lang], { n, k })
export function fill(template, values) {
  return String(template).replace(/\{(\w+)\}/g, (match, key) => (values[key] == null ? match : values[key]));
}

export function fmtTokens(n, zh = false) {
  if (n == null) return '—';
  // 中文环境用 亿/万 计数(1.33 亿 / 68.1 万),万以下原样,与英文 B/M/K 对应
  if (zh) {
    if (n >= 1e8) return (n / 1e8).toFixed(2) + '亿';
    if (n >= 1e4) return (n / 1e4).toFixed(1) + '万';
    return String(n);
  }
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
  if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
  return String(n);
}

export function fmtCost(v) {
  if (v == null) return '—';
  const est = '$';
  if (v >= 100) return est + v.toFixed(0);
  if (v >= 1) return est + v.toFixed(2);
  if (v > 0) return est + v.toFixed(4);
  return est + '0.00';
}

export function fmtCountdown(ms) {
  if (ms == null || ms < 0) return '—';
  let s = Math.floor(ms / 1000);
  const d = Math.floor(s / 86400); s %= 86400;
  const h = Math.floor(s / 3600); s %= 3600;
  const m = Math.floor(s / 60); s %= 60;
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

export function fmtRel(msAgo) {
  if (msAgo == null) return '—';
  const m = Math.floor(msAgo / 60000);
  if (m < 1) return '<1m';
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  return `${Math.floor(h / 24)}d`;
}

export function fmtClock(ms) {
  if (!ms) return '—';
  return new Date(ms).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
}

const LEDGER_SCHEMA_VERSION = 1;
const nonEmptyString = (value) => typeof value === 'string' && value.length > 0;
const nullableString = (value) => value === null || typeof value === 'string';

function compatibleRate(value) {
  return Number.isFinite(value?.usd_per_token) && value.usd_per_token >= 0
    && typeof value.entry === 'string' && ['upstream', 'supplement', 'legacy_unknown'].includes(value.origin);
}

function compatibleEstimate(value) {
  return typeof value?.matched_entry === 'string'
    && ['exact', 'case_insensitive', 'explicit_alias', 'canonical', 'official_reference', 'reverse_alias', 'unique_suffix', 'majority_reference'].includes(value.match_method)
    && typeof value.family_fallback === 'boolean' && typeof value.cache_fill === 'boolean'
    && ['input', 'output'].every((key) => compatibleRate(value[key]))
    && ['cache_read', 'cache_write'].every((key) => value[key] === null || compatibleRate(value[key]));
}

function compatibleBasis(value) {
  if (value?.kind === 'reported') return true;
  if (value?.kind === 'estimated') return compatibleEstimate(value);
  return value?.kind === 'unpriced'
    && ['unknown_model', 'ambiguous_name', 'ambiguous_suffix', 'missing_cache_read', 'missing_cache_write', 'missing_cache_read_and_write', 'non_finite_amount'].includes(value.reason)
    && (value.attempted === null || compatibleEstimate(value.attempted));
}

export function readLedger(payload) {
  const ledger = payload?.ledger;
  if (ledger?.schema_version !== LEDGER_SCHEMA_VERSION || !nonEmptyString(ledger.report_id)
    || !nonEmptyString(ledger.timezone_offset) || !Number.isInteger(ledger.generated_at_ms)) return null;
  const snapshot = ledger.price_snapshot;
  if (!nonEmptyString(snapshot?.version) || !Number.isInteger(snapshot.loaded_at_ms)
    || !(snapshot.fetched_at_ms === null || Number.isInteger(snapshot.fetched_at_ms))
    || !nullableString(snapshot.dataset_url)
    || !['cache_v2', 'legacy_cache', 'supplements_only'].includes(snapshot.source)
    || !['fresh', 'stale', 'unknown_age', 'missing', 'corrupt', 'not_loaded'].includes(snapshot.cache_state)) return null;
  if (!Array.isArray(ledger.price_bases)) return null;
  const ids = new Set();
  for (const entry of ledger.price_bases) {
    if (!Number.isSafeInteger(entry?.id) || entry.id < 0 || ids.has(entry.id) || !compatibleBasis(entry.basis)) return null;
    ids.add(entry.id);
  }
  const validAgg = (agg) => ['input', 'output', 'cache_read', 'cache_write', 'reasoning', 'total'].every((key) => Number.isInteger(agg?.[key]))
    && Number.isInteger(agg.messages) && agg.messages >= 0
    && ['cost', 'cost_reported', 'cost_estimated'].every((key) => Number.isFinite(agg[key]))
    && typeof agg.cost_is_estimate === 'boolean';
  const validRows = (rows, hourly) => Array.isArray(rows) && rows.every((row) => typeof row?.date === 'string'
    && typeof row.client === 'string' && nonEmptyString(row.model) && typeof row.raw_model === 'string'
    && nullableString(row.project) && Number.isSafeInteger(row.price_basis_id) && ids.has(row.price_basis_id)
    && Number.isInteger(row.unpriced_messages) && row.unpriced_messages >= 0 && validAgg(row.agg)
    && (!hourly || (Number.isInteger(row.hour) && row.hour >= 0 && row.hour < 24)));
  if (!['today', 'week', 'month'].every((key) => validAgg(ledger[key]))
    || !Array.isArray(ledger.sources) || !ledger.sources.every((source) => typeof source?.client === 'string'
      && typeof source.label === 'string' && Number.isInteger(source.records) && source.records >= 0 && nullableString(source.error))
    || !validRows(ledger.analysis_rows, false) || !validRows(ledger.intraday_rows, true)) return null;
  return ledger;
}
