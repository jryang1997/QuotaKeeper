export const PROVIDER_ACCENT = {
  'opencode-go': 'var(--color-text-blue)',
  codex: 'var(--color-text-green)',
  goat: 'var(--color-text-purple)',
  antigravity: 'var(--color-text-orange)',
  claude: 'var(--color-text-orange)',
  cursor: 'var(--color-text-primary)',
  glm: 'var(--color-text-blue)',
  kimi: 'var(--color-text-orange)',
  deepseek: '#2764d6',
};

export const STATUS_COLOR = {
  ok: 'var(--color-success)',
  warning: 'var(--color-warning)',
  exhausted: 'var(--color-error)',
};

export const PROVIDER_COLOR_DEFAULTS = {
  'opencode-go': '#14b8a6', codex: '#8b5cf6', goat: '#f59e0b', antigravity: '#0ea5e9', claude: '#d97757', cursor: '#5a5a5a', glm: '#2f6bff', kimi: '#ff8d1a', deepseek: '#2764d6',
};
export const PROVIDERS = ['opencode-go', 'codex', 'goat', 'antigravity', 'claude', 'cursor', 'glm', 'kimi', 'deepseek'];
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
};
export const QUOTA_HINT = {
  'opencode-go': ['未连接 · 保存 Go API key 或登录 OpenCode', 'Not connected · save a Go API key or sign in to OpenCode'],
  codex: ['未连接 · 登录 Codex 后刷新', 'Not connected · sign in to Codex and refresh'],
  goat: ['未连接 · 保存 Command Code key 或登录 cmdc', 'Not connected · save a Command Code key or use cmdc login'],
  antigravity: ['未连接 · 启动 CPA 并放置管理密钥', 'Not connected · start CPA with its management key'],
  glm: ['未连接 · 设置 GLM_API_KEY 后重启', 'Not connected · set GLM_API_KEY and restart'],
  kimi: ['未连接 · 设置 KIMI_CODE_API_KEY 后重启', 'Not connected · set KIMI_CODE_API_KEY and restart'],
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
  title: ['Token 监控', 'Token Monitor'],
  nav_quota: ['配额', 'Quota'],
  nav_usage: ['Token 用量', 'Token Usage'],
  nav_heat: ['面板', 'Panels'],
  nav_settings: ['设置', 'Settings'],
  nav_share: ['分享卡', 'Share card'],
  refresh: ['刷新', 'Refresh'],
  refreshing: ['采集中…', 'Fetching…'],
  updated: ['更新于', 'Updated'],
  quota_error: ['额度读取失败', 'Quota fetch failed'],
  today: ['今日', 'Today'],
  week: ['本周', 'This week'],
  month: ['本月', 'This month'],
  burn: ['燃烧率', 'Burn rate'],
  burn_unit: ['token/分', 'tok/min'],
  resets_in: ['重置', 'resets'],
  by_agent: ['按 Agent', 'By agent'],
  by_model: ['按模型', 'By model'],
  by_session: ['按会话', 'By session'],
  by_project: ['按项目', 'By project'],
  by_day: ['按日', 'By day'],
  recent_hour: ['最近一小时', 'Last hour'],
  last_activity: ['最后活动', 'Last activity'],
  messages: ['次请求', 'msgs'],
  cost_est: ['~$', '~$'],
  no_data: ['暂无数据', 'No data'],
  loading: ['正在读取…', 'Loading…'],
  empty_quota: ['暂无配额数据', 'No quota data'],
  empty_trend: ['这段时间暂无用量', 'No usage in this range'],
  now: ['最近在用', 'Right now'],
  quota: ['已用最高', 'Highest used'],
  aggregate: ['总计', 'Total'],
  range_24h: ['24 小时', '24h'],
  range_14d: ['14 天', '14d'],
  metric_tokens: ['Token', 'Tokens'],
  metric_cost: ['成本', 'Cost'],
  dim_agent: ['Agent', 'Agent'],
  dim_session: ['会话', 'Session'],
  dim_project: ['项目', 'Project'],
  dim_model: ['模型', 'Model'],
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
  theme_ink: ['深海', 'Midnight'],
  theme_sand: ['暖纸', 'Warm paper'],
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
  settings_currency_hint: ['成本按最新参考汇率换算；原始价格与套餐基准以 USD 保存。', 'Costs use a reference rate; source prices and plan benchmark stay in USD.'],
  settings_currency_rate: ['参考汇率', 'Reference rate'],
  settings_currency_usd: ['原始计价单位 · USD', 'Source pricing unit · USD'],
  settings_currency_stale: ['离线缓存', 'Cached offline'],
  settings_currency_error: ['汇率暂不可用；金额暂不显示', 'Rate unavailable; converted amounts are hidden'],
  settings_currency_loading: ['正在获取汇率…', 'Loading exchange rate…'],
  settings_currency_source: ['汇率来源：', 'Rate source:'],
  settings_plan_hint: ['非实际省下的钱', 'Not actual cash saved'],
  settings_monthly: ['每月套餐费用', 'Monthly plan cost'],
  settings_default: ['默认基准', 'Default benchmark'],
  settings_save: ['保存', 'Save'],
  settings_saved: ['已保存', 'Saved'],
  settings_invalid_price: ['请输入 0.01 到 1,000,000 美元之间的金额。', 'Enter an amount from $0.01 to $1,000,000.'],
  settings_font_sample: ['用量一目了然', 'Usage at a glance'],
  settings_reset_all: ['恢复外观默认值', 'Reset appearance'],
  quota_overview: ['额度总览', 'Quota overview'],
  quota_attention: ['需要关注', 'Needs attention'],
  quota_all_clear: ['当前额度充足', 'Quotas look healthy'],
  quota_unavailable: ['暂无可用额度', 'No live quotas'],
  quota_used: ['已用', 'used'],
  usage_sum_tokens: ['TOTAL TOKENS', 'TOTAL TOKENS'],
  usage_sum_runs: ['TOTAL RUNS', 'TOTAL RUNS'],
  usage_unit_tokens: ['tokens', 'tokens'],
  usage_unit_runs: ['runs', 'runs'],
  usage_sum_official: ['当前账期 · Command Code 官方', 'Current billing period · Command Code'],
  usage_sum_local: ['本月 · 本机账本', 'This month · local ledger'],
  usage_card_monthly: ['MONTHLY', 'MONTHLY'],
  panels_add: ['添加图表', 'Add panel'],
  panels_lock_all: ['全部锁定', 'Lock all'],
  panels_unlock_all: ['全部解锁', 'Unlock all'],
  panels_reset: ['重置布局', 'Reset layout'],
  panel_heat: ['热力图', 'Heatmap · 26W'],
  panel_punch: ['作息矩阵', 'Punchcard · 30D'],
  panel_clock: ['编码时钟', 'Coding clock'],
  panel_week: ['星期强度', 'Weekday load'],
  panel_models: ['模型榜', 'Model board'],
  panel_sources: ['来源占比', 'By source'],
  panel_projects: ['项目榜', 'Project board'],
  panel_alltime: ['一生总量', 'All-time'],
  panel_streak: ['连击', 'Streak'],
  panel_records: ['本机纪录', 'Records'],
  panel_cache: ['缓存命中', 'Cache hit'],
  panel_daily: ['近 30 天', 'Daily · 30D'],
  catalog_heat: ['26 周每日强度', '26 weeks of daily intensity'],
  catalog_punch: ['星期 × 小时作息', 'Weekday × hour rhythm'],
  catalog_clock: ['24 小时放射分布', 'Radial 24-hour spread'],
  catalog_week: ['周内哪天最狠', 'Heaviest weekday'],
  catalog_models: ['按模型 token 排行', 'Tokens by model'],
  catalog_sources: ['当月各来源份额', 'This month by source'],
  catalog_projects: ['项目投入排行', 'Tokens by project'],
  catalog_alltime: ['累计 token / 请求 / 成本', 'Total tokens, runs, cost'],
  catalog_streak: ['连续编码天数', 'Consecutive active days'],
  catalog_records: ['单日峰值 · 最长会话 · 最贵模型', 'Peak day, top session, priciest model'],
  catalog_cache: ['命中率与五类 token 构成', 'Hit rate and token mix'],
  catalog_daily: ['每日消耗柱状', 'Daily bars'],
  panel_peak_hour: ['峰值', 'Peak'],
  panel_active_days: ['活跃', 'Active'],
  panel_current: ['当前', 'Current'],
  panel_longest: ['最长', 'Longest'],
  panel_busiest_day: ['单日峰值', 'Busiest day'],
  panel_top_session: ['最长会话', 'Top session'],
  panel_priciest_model: ['最贵模型', 'Priciest model'],
  panel_per_request: ['/req', '/req'],
  panel_cache_hit: ['命中率', 'Hit rate'],
  panel_token_mix: ['token 构成', 'token mix'],
  panel_cache_read: ['缓存读', 'cache read'],
  panel_cache_write: ['缓存写', 'cache write'],
  quota_remaining: ['剩余', 'remaining'],
  quota_next_reset: ['最近重置', 'Next reset'],
  source_health: ['本机数据源', 'Local sources'],
  source_ready: ['有记录', 'Active'],
  source_idle: ['暂无记录', 'No records'],
  source_error: ['读取失败', 'Read failed'],
  freshness_quota: ['服务额度', 'Service quotas'],
  freshness_local: ['本机用量', 'Local usage'],
  freshness_waiting: ['等待首次读取', 'Waiting for first read'],
  freshness_stale: ['数据已过期', 'Data is stale'],
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
  settings_pace_alerts: ['耗尽预测提醒', 'Pace forecast alerts'],
  settings_pace_alerts_hint: ['预测将提前耗尽时提醒', 'Alerts when a forecast turns early-exhaust'],
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
  agents_title: ['Agent 管理', 'Agent manager'],
  agents_hint: ['扫描本机 AI CLI Agent 的安装与版本，锚定来源一键升级', 'Scans installed AI CLI agents and upgrades them in place'],
  agents_scan: ['重新检测', 'Re-scan'],
  agents_scanning: ['检测中…', 'Scanning…'],
  agents_upgrade_all: ['全部升级', 'Upgrade all'],
  agents_upgrade: ['升级', 'Upgrade'],
  agents_confirm: ['确认执行', 'Run'],
  agents_cancel: ['取消', 'Cancel'],
  agents_not_installed: ['未安装', 'Not installed'],
  agents_up_to_date: ['已是最新', 'Up to date'],
  agents_outdated: ['可升级', 'Update available'],
  agents_unreadable: ['版本读取失败', 'Version unreadable'],
  agents_manual: ['来源不明 · 请按原方式更新', 'Unknown source · update it the way you installed it'],
  agents_confirm_hint: ['将静默执行以下命令', 'This command will run silently'],
  agents_running: ['升级中…', 'Upgrading…'],
  agents_update_ok: ['升级完成', 'Upgrade finished'],
  agents_update_fail: ['升级失败', 'Upgrade failed'],
  agents_empty: ['本机未检出任何 CLI Agent', 'No CLI agents detected on this machine'],
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
  share_export: ['导出 PNG', 'Export PNG'],
  share_exporting: ['生成中…', 'Rendering…'],
  share_saved: ['已保存 PNG', 'PNG saved'],
  share_canceled: ['已取消保存', 'Save canceled'],
  share_error: ['导出失败，请重试', 'Export failed, please retry'],
  share_palette: ['配色', 'Palette'],
  share_palette_deep: ['深海', 'Deep sea'],
  share_palette_paper: ['暖纸', 'Warm paper'],
  share_hint: ['拖拽 / 缩放模块 · 导出 1080 宽 PNG', 'Drag / resize modules · 1080px PNG'],
  share_empty: ['暂无数据', 'No data yet'],
  share_mod_hero: ['今日 Token', "Today's tokens"],
  share_mod_clock: ['活跃时段', 'Active hours'],
  share_mod_models: ['最常用模型', 'Top models'],
  share_mod_agents: ['Agent 分布', 'By agent'],
  share_mod_days: ['近 14 天', 'Last 14 days'],
  share_mod_value: ['本月价值', 'Monthly value'],
  share_mod_summary: ['今日之最', 'Highlights'],
  lan_title: ['局域网看板', 'LAN dashboard'],
  lan_hint: ['手机/平板浏览器访问，Standby 式只读看板', 'Read-only Standby-style board for phones and tablets'],
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
  cards_custom: ['配额卡片', 'Quota cards'],
  cards_custom_hint: ['固定重要卡片、隐藏无关卡片，并调整各来源颜色', 'Pin key cards, hide irrelevant cards, and set provider colors'],
  card_pin: ['固定', 'Pin'],
  card_unpin: ['取消固定', 'Unpin'],
  card_hide: ['隐藏', 'Hide'],
  card_show: ['显示', 'Show'],
  cards_restore: ['恢复卡片默认设置', 'Reset cards'],
  welcome_title: ['欢迎使用 Token Monitor', 'Welcome to Token Monitor'],
  welcome_intro: ['已检测本机来源与服务额度。', 'Local sources and service quotas have been checked.'],
  welcome_done: ['开始使用', 'Get started'],
  welcome_reopen: ['重新检测来源', 'Review sources'],
  welcome_repair: ['查看修复方法', 'How to fix'],
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
