import React, { lazy, Suspense, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import '@astryxdesign/core/reset.css';
import '@astryxdesign/core/astryx.css';
import '@astryxdesign/theme-neutral/theme.css';
import './app.css';

import { Card } from '@astryxdesign/core/Card';
import { Text } from '@astryxdesign/core/Text';
import { Button } from '@astryxdesign/core/Button';
import { Spinner } from '@astryxdesign/core/Spinner';
import { SegmentedControl } from '@astryxdesign/core/SegmentedControl';
import { SegmentedControlItem } from '@astryxdesign/core/SegmentedControl';
import { Tooltip } from '@astryxdesign/core/Tooltip';
import { Toast } from '@astryxdesign/core/Toast';

const PageTransition = lazy(() => import('./PageTransition.jsx'));
const AnalysisPanel = lazy(() => import('./AnalysisPanel.jsx'));

/* ---------------- 常量(与 Rust ipc.rs 对齐) ---------------- */

const PROVIDER_ACCENT = {
  'opencode-go': 'var(--color-text-blue)',
  codex: 'var(--color-text-green)',
  goat: 'var(--color-text-purple)',
  antigravity: 'var(--color-text-orange)',
};

const STATUS_COLOR = {
  ok: 'var(--color-success)',
  warning: 'var(--color-warning)',
  exhausted: 'var(--color-error)',
};

const CLIENT_COLORS = {
  codex: '#7c3aed',
  omp: '#0ea5e9',
  opencode: '#f59e0b',
  zcode: '#10b981',
  dsh: '#3b82f6',
  claude: '#ef6c4d',
};
const PROVIDER_COLOR_DEFAULTS = {
  'opencode-go': '#14b8a6', codex: '#8b5cf6', goat: '#f59e0b', antigravity: '#0ea5e9',
};
const PROVIDERS = ['opencode-go', 'codex', 'goat', 'antigravity'];
const PROVIDER_LOGOS = {
  'opencode-go': 'logos/opencode-go.svg',
  codex: 'logos/codex.svg',
  goat: 'logos/commandcode.svg',
  antigravity: 'logos/antigravity.png',
};

const SOURCE_PATHS = {
  codex: '~/.codex/sessions/',
  omp: '~/.omp/stats.db',
  opencode: '~/.local/share/opencode/opencode.db',
  zcode: '~/.zcode/cli/db/db.sqlite',
  dsh: '~/.dsh/sessions/',
  claude: '~/.claude/projects/',
};
const SOURCE_REPAIR = {
  codex: ['先在 Codex 中运行一次会话，再检查 ~/.codex/sessions/ 下是否有 rollout 日志。', 'Run a Codex session, then check for rollout logs under ~/.codex/sessions/.'],
  omp: ['启动 OMP 并完成登录；运行 omp usage --json 检查额度，stats.db 用于本机用量。', 'Start OMP and sign in. Run omp usage --json for quotas; stats.db holds local usage.'],
  opencode: ['在 OpenCode 中运行一次会话；确认本机数据库已生成。', 'Run an OpenCode session and check that its local database exists.'],
  zcode: ['在 ZCode 中运行一次会话；确认本机数据库已生成。', 'Run a ZCode session and check that its local database exists.'],
  dsh: ['在 DeepSeek Harness 中运行一次会话；检查压缩会话日志。', 'Run a DeepSeek Harness session and check its compressed session logs.'],
  claude: ['在 Claude Code 中运行一次会话；检查 ~/.claude/projects/ 下的 JSONL 日志。', 'Run a Claude Code session and check JSONL logs under ~/.claude/projects/.'],
};
const QUOTA_REPAIR = {
  'opencode-go': ['在设置页保存 Go API key，或设置 OPENCODE_GO_API_KEY；也可读取 OpenCode 登录态。', 'Save your Go API key in Settings, set OPENCODE_GO_API_KEY, or use an OpenCode login.'],
  codex: ['登录 Codex 官方程序后刷新；应用通过 app-server 读取限额。', 'Sign in to the official Codex app and refresh; quotas come from app-server.'],
  goat: ['在设置页保存 Command Code API key，或配置 COMMAND_CODE_API_KEY；也可使用 cmdc 登录态。', 'Save a Command Code API key in Settings, set COMMAND_CODE_API_KEY, or use a cmdc login.'],
  antigravity: ['启动本机 CPA，并将管理密钥放在 ~/.tui-token-monitor/cpa-key；应用会通过 CPA 代查 Google 上游配额。', 'Start local CPA and place the management key in ~/.tui-token-monitor/cpa-key. The app reads upstream Google quotas through CPA.'],
};
const QUOTA_HINT = {
  'opencode-go': ['未连接 · 保存 Go API key 或登录 OpenCode', 'Not connected · save a Go API key or sign in to OpenCode'],
  codex: ['未连接 · 登录 Codex 后刷新', 'Not connected · sign in to Codex and refresh'],
  goat: ['未连接 · 保存 Command Code key 或登录 cmdc', 'Not connected · save a Command Code key or use cmdc login'],
  antigravity: ['未连接 · 启动 CPA 并放置管理密钥', 'Not connected · start CPA with its management key'],
};

const NAV_ITEMS = ['quota', 'usage', 'heat', 'settings'];

function NavIcon({ page }) {
  if (page === 'quota') {
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 15a8 8 0 1 1 16 0" /><path d="m12 12 4-4" /><path d="M8 19h8" /></svg>;
  }
  if (page === 'usage') {
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 19V5" /><path d="M4 19h16" /><path d="M8 16v-4" /><path d="M12 16V8" /><path d="M16 16v-7" /></svg>;
  }
  if (page === 'settings') {
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3" /><path d="m19.4 15 .1.1a1.7 1.7 0 0 1-2.4 2.4l-.1-.1a1.7 1.7 0 0 0-2.9 1.2v.2a1.7 1.7 0 0 1-3.4 0v-.2a1.7 1.7 0 0 0-2.9-1.2l-.1.1a1.7 1.7 0 1 1-2.4-2.4l.1-.1a1.7 1.7 0 0 0-1.2-2.9H4a1.7 1.7 0 0 1 0-3.4h.2a1.7 1.7 0 0 0 1.2-2.9l-.1-.1a1.7 1.7 0 1 1 2.4-2.4l.1.1a1.7 1.7 0 0 0 2.9-1.2V2a1.7 1.7 0 0 1 3.4 0v.2a1.7 1.7 0 0 0 2.9 1.2l.1-.1a1.7 1.7 0 1 1 2.4 2.4l-.1.1a1.7 1.7 0 0 0 1.2 2.9h.2a1.7 1.7 0 0 1 0 3.4h-.2a1.7 1.7 0 0 0-1.2 2.9Z" /></svg>;
  }
  return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="5" height="5" rx="1" /><rect x="10" y="4" width="5" height="5" rx="1" /><rect x="16" y="4" width="5" height="5" rx="1" /><rect x="4" y="10" width="5" height="5" rx="1" /><rect x="10" y="10" width="5" height="5" rx="1" /><rect x="16" y="10" width="5" height="5" rx="1" /><rect x="4" y="16" width="5" height="5" rx="1" /><rect x="10" y="16" width="5" height="5" rx="1" /><rect x="16" y="16" width="5" height="5" rx="1" /></svg>;
}

function PrimaryNav({ lang, page, onNavigate, variant }) {
  return (
    <nav className={`primary-nav primary-nav-${variant}`} aria-label={lang === 0 ? '主导航' : 'Main navigation'}>
      {NAV_ITEMS.map((key) => {
        const active = page === key;
        return (
          <button
            key={key}
            className={`nav-item${active ? ' nav-item-active' : ''}`}
            onClick={() => onNavigate(key)}
            aria-current={active ? 'page' : undefined}
          >
            {active && <span className="nav-active-indicator" aria-hidden="true" />}
            <NavIcon page={key} />
            <span className="nav-label">{STR[`nav_${key}`][lang]}</span>
          </button>
        );
      })}
    </nav>
  );
}

/* ---------------- 双语 ---------------- */

const STR = {
  title: ['Token 监控', 'Token Monitor'],
  nav_quota: ['配额', 'Quota'],
  nav_usage: ['Token 用量', 'Token Usage'],
  nav_heat: ['面板', 'Panels'],
  nav_settings: ['设置', 'Settings'],
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
  font_plex: ['Plex 现代终端', 'Plex modern terminal'],
  font_pixel: ['复古像素', 'Retro pixel'],
  font_system: ['清晰字体', 'Clear type'],
  settings_density: ['信息密度', 'Density'],
  density_comfortable: ['舒展', 'Comfortable'],
  density_compact: ['紧凑', 'Compact'],
  settings_motion: ['动态效果', 'Motion'],
  motion_system: ['跟随系统', 'Follow system'],
  motion_reduce: ['减少动态', 'Reduce motion'],
  settings_plan: ['套餐基准', 'Plan benchmark'],
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

/* ---------------- 工具 ---------------- */

function fmtInt(n) {
  if (n == null) return '—';
  return n.toLocaleString('zh-CN', { maximumFractionDigits: 0 });
}

function fmtTokens(n) {
  if (n == null) return '—';
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
  if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
  return String(n);
}

function fmtCost(v) {
  if (v == null) return '—';
  const est = '$';
  if (v >= 100) return est + v.toFixed(0);
  if (v >= 1) return est + v.toFixed(2);
  if (v > 0) return est + v.toFixed(4);
  return est + '0.00';
}

function fmtCountdown(ms) {
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

function fmtRel(msAgo) {
  if (msAgo == null) return '—';
  const m = Math.floor(msAgo / 60000);
  if (m < 1) return '<1m';
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  return `${Math.floor(h / 24)}d`;
}

function fmtClock(ms) {
  if (!ms) return '—';
  return new Date(ms).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
}

/* ---------------- 大数字点阵 ---------------- */

function DotMatrix({ text, size = 42, className = '' }) {
  // 5x7 点阵数字,方块字符用 CSS 网格渲染,保持 █░ 的审美
  const GLYPHS = {
    '0': ['01110','10001','10011','10101','11001','10001','01110'],
    '1': ['00100','01100','00100','00100','00100','00100','01110'],
    '2': ['01110','10001','00001','00110','01000','10000','11111'],
    '3': ['11110','00001','00001','01110','00001','00001','11110'],
    '4': ['00010','00110','01010','10010','11111','00010','00010'],
    '5': ['11111','10000','11110','00001','00001','10001','01110'],
    '6': ['00110','01000','10000','11110','10001','10001','01110'],
    '7': ['11111','00001','00010','00100','01000','01000','01000'],
    '8': ['01110','10001','10001','01110','10001','10001','01110'],
    '9': ['01110','10001','10001','01111','00001','00010','01100'],
    'K': ['10001','10010','10100','11000','10100','10010','10001'],
    'M': ['10001','11011','10101','10101','10001','10001','10001'],
    'B': ['11110','10001','10001','11110','10001','10001','11110'],
    '.': ['00000','00000','00000','00000','00000','00110','00110'],
  };
  const chars = String(text).split('');
  const cell = size / 7;
  return (
    <div className={`dotmatrix${className ? ` ${className}` : ''}`} style={{ '--cell': `${cell}px` }} role="img" aria-label={text}>
      {chars.map((ch, i) => {
        const g = GLYPHS[ch] || GLYPHS['0'];
        return (
          <svg key={i} viewBox="0 0 5 7" className="dotmatrix-glyph" aria-hidden>
            {g.flatMap((row, y) =>
              row.split('').map((bit, x) =>
                bit === '1' ? <rect key={`${x}-${y}`} x={x + 0.08} y={y + 0.08} width={0.84} height={0.84} rx={0.18} /> : null,
              ),
            )}
          </svg>
        );
      })}
    </div>
  );
}

function quotaPace(limit) {
  const label = limit.label.toLowerCase();
  const now = new Date();
  let totalMs;
  if (/5\s*h|5\s*hour|5\s*小时/.test(label)) totalMs = 5 * 3600_000;
  else if (/7\s*d|7\s*day|week|周/.test(label)) totalMs = 7 * 86400_000;
  else if (/month|月/.test(label)) totalMs = new Date(now.getFullYear(), now.getMonth() + 1, 1).getTime() - new Date(now.getFullYear(), now.getMonth(), 1).getTime();
  else return null;
  const resetMs = limit.resets_in_ms;
  if (resetMs == null || resetMs <= 0 || resetMs >= totalMs || limit.used_pct < 5) return null;
  const elapsed = totalMs - resetMs;
  if (elapsed < totalMs * 0.1) return null;
  if (limit.used_pct >= 100) return { eta: 'exhausted', delta: limit.used_pct - elapsed / totalMs * 100 };
  const eta = elapsed * (100 - limit.used_pct) / limit.used_pct;
  if (!Number.isFinite(eta) || eta <= 0) return null;
  return { eta: eta < resetMs ? eta : 'sustainable', delta: limit.used_pct - elapsed / totalMs * 100 };
}

function ObservedDotMatrix({ value, size = 34, typeface = 'system' }) {
  const previousValue = useRef(value);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (previousValue.current === value) return;
    previousValue.current = value;
    setRevision((current) => current + 1);
  }, [value]);

  if (typeface !== 'pixel') return <div className="stat-number">{fmtTokens(value)}</div>;
  return (
    <DotMatrix
      key={revision}
      text={fmtTokens(value)}
      size={size}
      className={revision > 0 ? 'dotmatrix-update' : ''}
    />
  );
}

/* ---------------- 迷你条形(五桶拆分) ---------------- */

function MiniBars({ agg }) {
  const parts = [
    ['input', agg.input, 'var(--color-text-blue)'],
    ['output', agg.output, 'var(--color-text-green)'],
    ['cache read', agg.cache_read, 'var(--color-text-purple)'],
    ['cache write', agg.cache_write, 'var(--color-text-orange)'],
  ];
  const total = parts.reduce((s, [, v]) => s + v, 0) || 1;
  return (
    <div className="minibars">
      <div className="minibars-track">
        {parts.map(([k, v, color]) => (
          <Tooltip key={k} content={`${k} ${fmtTokens(v)}`}>
            <div className="minibars-seg" style={{ flexGrow: v, background: color }} />
          </Tooltip>
        ))}
      </div>
    </div>
  );
}

/* ---------------- 当月用量总览(Command Code 风格) ---------------- */

function TrendBars({ values, color }) {
  const max = Math.max(1, ...values);
  return (
    <div className="trend-bars" aria-hidden="true">
      {values.map((v, i) => (
        <span
          key={i}
          style={{
            height: `${Math.max(4, (v / max) * 100)}%`,
            background: color,
            opacity: i === values.length - 1 ? 1 : 0.45,
          }}
        />
      ))}
    </div>
  );
}

function UsageSummaryRow({ ledger, goat, lang }) {
  const t = (k) => STR[k][lang];
  const zh = lang === 0;
  const now = new Date();
  const monthPrefix = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
  const monthDays = (ledger?.daily_totals || []).filter((d) => d.date.startsWith(monthPrefix));
  const tokens = goat ? goat.tokens : ledger?.month.total ?? 0;
  const requests = goat ? goat.requests : ledger?.month.messages ?? 0;
  const caption = goat ? t('usage_sum_official') : t('usage_sum_local');
  return (
    <div className="usage-summary-row">
      <div className="usage-summary-card">
        <div className="usage-summary-head" style={{ color: 'var(--sum-purple)' }}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 8v8M8 12h8" /></svg>
          <span>{t('usage_sum_tokens')}</span>
        </div>
        <div className="usage-summary-number">{fmtTokens(tokens)}<span>{t('usage_unit_tokens')}</span></div>
        <TrendBars values={monthDays.map((d) => d.tokens)} color="var(--sum-purple)" />
        <div className="usage-summary-caption">{caption}</div>
      </div>
      <div className="usage-summary-card">
        <div className="usage-summary-head" style={{ color: 'var(--sum-orange)' }}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13l11-6.5z" /></svg>
          <span>{t('usage_sum_runs')}</span>
        </div>
        <div className="usage-summary-number">{fmtInt(requests)}<span>{t('usage_unit_runs')}</span></div>
        <TrendBars values={monthDays.map((d) => d.messages)} color="var(--sum-orange)" />
        <div className="usage-summary-caption">{caption}</div>
      </div>
    </div>
  );
}

/* ---------------- 配额卡片 ---------------- */

const BAR_BLOCKS = 20;

function BlockBar({ pct, color }) {
  const clamped = Math.max(0, Math.min(100, pct));
  const filled = clamped > 0 ? Math.max(1, Math.round(clamped / 100 * BAR_BLOCKS)) : 0;
  return (
    <div className="block-bar" style={{ '--bar-tone': color }} aria-hidden="true">
      {Array.from({ length: BAR_BLOCKS }, (_, i) => (
        <span key={i} className={`block-bar-cell${i < filled ? ' is-filled' : ''}`} />
      ))}
    </div>
  );
}

function ProviderLogo({ provider }) {
  const src = PROVIDER_LOGOS[provider];
  if (!src) return <span className="quota-logo quota-logo-fallback" aria-hidden="true" />;
  return <img className="quota-logo" src={src} alt="" aria-hidden="true" draggable="false" />;
}

function paceChipText(pace, lang) {
  const zh = lang === 0;
  if (pace.eta === 'sustainable') return zh ? '可撑到重置' : 'lasts to reset';
  if (pace.eta === 'exhausted') return zh ? '已耗尽' : 'exhausted';
  const d = Math.abs(pace.delta).toFixed(0);
  const eta = `≈${fmtCountdown(pace.eta)}`;
  return zh
    ? `${pace.delta >= 0 ? '快' : '慢'} ${d}pt · ${eta} 后耗尽`
    : `${d}pt ${pace.delta >= 0 ? 'ahead' : 'behind'} · ${eta} to empty`;
}

const QUOTA_CLIENT_OF = { 'opencode-go': 'opencode', codex: 'codex' };

function QuotaCards({ cards, lang, allowPace = false, pinnedCards = [], hiddenCards = [], providerColors = {}, monthByClient = [], goatUsage = null, onTogglePin, onToggleHidden }) {
  const t = (k) => STR[k][lang];
  const ordered = [...cards].sort((a, b) => Number(pinnedCards.includes(b.provider)) - Number(pinnedCards.includes(a.provider)));
  return (
    <div className="quota-grid">
      {ordered.filter((c) => !hiddenCards.includes(c.provider)).map((c) => {
        const accent = providerColors[c.provider] || PROVIDER_ACCENT[c.provider] || 'var(--color-text-primary)';
        const isActivity = c.method === 'activity_vs_average';
        const monthlyTokens = c.kind !== 'Live' ? undefined
          : c.provider === 'goat'
            ? goatUsage?.tokens
            : (() => {
              const client = QUOTA_CLIENT_OF[c.provider];
              const row = client && monthByClient.find((r) => r.client === client);
              return row?.agg.total;
            })();
        return (
          <Card key={c.provider} className={`quota-card${isActivity ? ' quota-card-activity' : ''}`} role="group" aria-label={c.title}>
            <div className="quota-head">
              <ProviderLogo provider={c.provider} />
              <Text weight="medium" className="quota-title">{c.title}</Text>
              <span className="quota-plan-type" title={c.kind === 'Live' && c.plan_type ? c.plan_type : undefined}>{c.kind === 'Live' ? c.plan_type || '' : ''}</span>
              <div className="quota-card-actions">
                <button type="button" aria-label={`${pinnedCards.includes(c.provider) ? t('card_unpin') : t('card_pin')} ${c.title}`} title={pinnedCards.includes(c.provider) ? t('card_unpin') : t('card_pin')} onClick={() => onTogglePin?.(c.provider)}>{pinnedCards.includes(c.provider) ? '◆' : '◇'}</button>
                <button type="button" aria-label={`${t('card_hide')} ${c.title}`} title={t('card_hide')} onClick={() => onToggleHidden?.(c.provider)}>×</button>
              </div>
            </div>
            {c.kind === 'Live' ? (
              <div className="quota-limits">
                {c.limits.length === 0 && <Text type="supporting" className="quota-missing-hint">{lang === 0 ? '暂无额度窗口' : 'No quota windows'}</Text>}
                {c.limits.map((l) => {
                  const pace = allowPace && !isActivity ? quotaPace(l) : null;
                  const pct = Math.round(l.used_pct);
                  return (
                  <div key={l.label} className="quota-limit">
                    <div className="quota-limit-row">
                      <span className="quota-limit-label">{l.label}</span>
                      <span className="quota-limit-pct" style={{ color: isActivity ? accent : STATUS_COLOR[l.status] }}>
                        {isActivity
                          ? (l.used_pct >= 100 ? t('activity_at_least_average') : `${pct}% ${lang === 0 ? 'vs' : 'of'} ${t('activity_vs_average')}`)
                          : l.used_pct >= 100 ? '100%' : `${pct}%`}
                      </span>
                    </div>
                    <BlockBar pct={l.used_pct} color={isActivity ? accent : STATUS_COLOR[l.status]} />
                    <div className="quota-limit-sub">
                      <span>{isActivity ? t('activity_today') : l.resets_in_ms != null ? `${t('resets_in')} ${fmtCountdown(l.resets_in_ms)}` : ' '}</span>
                      {pace != null && <span className={`quota-pace${pace.eta !== 'sustainable' ? ' quota-pace-warning' : ''}`}>{paceChipText(pace, lang)}</span>}
                    </div>
                  </div>
                );})}
              </div>
            ) : (
              <div className="quota-missing">
                <Text type="supporting" className="quota-missing-hint">{QUOTA_HINT[c.provider]?.[lang] || c.hint}</Text>
              </div>
            )}
            {monthlyTokens != null && monthlyTokens > 0 && (
              <div className="quota-monthly">
                <span className="quota-monthly-label">{t('usage_card_monthly')}</span>
                <span className="quota-monthly-value">{fmtTokens(monthlyTokens)} {t('usage_unit_tokens')}</span>
              </div>
            )}
          </Card>
        );
      })}
    </div>
  );
}

function QuotaPage({ cards, sources, ledger, goatUsage, quotaFetchedAtMs, quotaStale, now, lang, preferences, onTogglePin, onToggleHidden, onOpenSettings }) {
  const t = (key) => STR[key][lang];
  const [selectedSource, setSelectedSource] = useState(null);
  const sourceDetail = sources?.find((source) => source.client === selectedSource);
  const elapsed = quotaFetchedAtMs ? Math.max(0, now - quotaFetchedAtMs) : 0;
  const currentCards = cards.map((card) => card.kind === 'Live' ? {
    ...card,
    limits: card.limits.map((limit) => ({ ...limit, resets_in_ms: limit.resets_in_ms == null ? null : Math.max(0, limit.resets_in_ms - elapsed) })),
  } : card);
  const live = currentCards.filter((card) => card.kind === 'Live' && card.method !== 'activity_vs_average' && !preferences.hiddenCards.includes(card.provider));
  const limits = live.flatMap((card) => card.limits.map((limit) => ({ ...limit, provider: card.title.split(' ·')[0] })));
  const tightest = limits.reduce((best, current) => !best || current.used_pct > best.used_pct ? current : best, null);
  const nextReset = limits.filter((limit) => limit.resets_in_ms > 0).sort((a, b) => a.resets_in_ms - b.resets_in_ms)[0];
  const attention = limits.filter((limit) => limit.status !== 'ok').length;
  const pct = tightest ? Math.max(0, Math.min(100, tightest.used_pct)) : 0;
  return <div className="quota-page">
    <section className="quota-overview" aria-label={t('quota_overview')}>
      <div className="quota-overview-copy">
        <span className="eyebrow">// {t('quota_overview')}</span>
        <h1>{tightest ? `${tightest.provider} · ${tightest.label}` : t('quota_unavailable')}</h1>
        <p>{tightest ? `${attention ? `${attention} ${t('quota_attention')}` : t('quota_all_clear')} · ${nextReset ? `${t('quota_next_reset')} ${fmtCountdown(nextReset.resets_in_ms)}` : '—'}` : t('empty_quota')}</p>
      </div>
      <div className="quota-overview-meter" style={{ '--quota-pct': `${pct}%`, '--quota-tone': STATUS_COLOR[tightest?.status] || 'var(--color-text-blue)' }} aria-label={tightest ? `${Math.round(pct)}% ${t('quota_used')}` : undefined}>
        <strong>{tightest ? `${Math.round(pct)}%` : '—'}</strong>
        <span>{t('quota_used')}</span>
      </div>
    </section>
    <UsageSummaryRow ledger={ledger} goat={goatUsage} lang={lang} />
    <QuotaCards cards={currentCards} lang={lang} allowPace={!!quotaFetchedAtMs && !quotaStale} pinnedCards={preferences.pinnedCards} hiddenCards={preferences.hiddenCards} providerColors={preferences.providerColors} monthByClient={ledger?.month_by_client || []} goatUsage={goatUsage} onTogglePin={onTogglePin} onToggleHidden={onToggleHidden} />
    {preferences.hiddenCards.length > 0 && <button className="quota-hidden-link" type="button" onClick={onOpenSettings}>{lang === 0 ? `已隐藏 ${preferences.hiddenCards.length} 张卡片 · 在设置中恢复` : `${preferences.hiddenCards.length} hidden cards · restore in Settings`}</button>}
    {sources?.length > 0 && <section className="source-section" aria-label={t('source_health')}>
      <div className="source-heading"><h2>{t('source_health')}</h2></div>
      <div className="source-grid">{sources.map((source) => {
        const state = source.error ? 'error' : source.records > 0 ? 'ready' : 'idle';
        return <button type="button" className={`source-item${selectedSource === source.client ? ' is-selected' : ''}`} key={source.client} onClick={() => setSelectedSource((current) => current === source.client ? null : source.client)} aria-expanded={selectedSource === source.client}>
          <span className={`source-indicator source-indicator-${state}`} />
          <span className="source-name">{source.label}</span>
          <span className="source-count">{source.error ? t('source_error') : source.records > 0 ? `${fmtInt(source.records)} ${t('source_ready')}` : t('source_idle')}</span>
        </button>;
      })}</div>
      {sourceDetail && <div className="source-detail" role="status">
        <strong>{sourceDetail.label}</strong>
        <span>{t('source_location')} · <code>{SOURCE_PATHS[sourceDetail.client] || '—'}</code></span>
        <span>{t('source_diagnostic')} · {sourceDetail.error || (sourceDetail.records ? t('source_ok_detail') : t('source_no_records'))}</span>
      </div>}
    </section>}
  </div>;
}

/* ---------------- 用量页 ---------------- */

function tightestQuota(cards) {
  const limits = cards
    .filter((card) => card.kind === 'Live' && card.method !== 'activity_vs_average')
    .flatMap((card) => card.limits.map((limit) => ({ provider: card.title.split(' ·')[0], ...limit })));
  return limits.sort((a, b) => b.used_pct - a.used_pct)[0] || null;
}

function UsagePage({ ledger, cards, monthlyValue, lang, onOpenSettings, typeface }) {
  const t = (k) => STR[k][lang];
  const [range, setRange] = useState('24h');
  const [metric, setMetric] = useState('tokens');
  const [dim, setDim] = useState('agent');
  const [selectedBucket, setSelectedBucket] = useState(null);
  if (!ledger) return null;

  const series = range === '24h' ? ledger.trend_24h : ledger.by_day;
  const value = (b) => (metric === 'tokens' ? b.agg.total : b.agg.cost);
  const hasTrend = series.some((bucket) => value(bucket) > 0);
  const maxV = Math.max(1, ...series.map(value));
  const peak = series.length ? series.reduce((m, b) => (value(b) > value(m) ? b : m), series[0]) : null;
  const periodLabel = (b) => range === '24h' ? fmtClock(b?.start_ms) : (b?.date?.slice(5) || '—');
  const quota = tightestQuota(cards || []);
  const recent = ledger.recent_by_agent?.[0];
  const planBenchmark = monthlyValue?.list_price_benchmark_usd
    ?? monthlyValue?.default_list_price_benchmark_usd
    ?? 40;
  const planBenchmarkIsCustom = monthlyValue?.benchmark_is_custom ?? false;
  const planMultiple = monthlyValue && planBenchmark > 0
    ? monthlyValue.month_api_equivalent_usd / planBenchmark
    : 0;
  const planLabel = planBenchmarkIsCustom
    ? (lang === 0 ? '我的月费' : 'my plan/mo')
    : (lang === 0 ? '月基准' : 'list/mo');
  const monthUnpriced = monthlyValue && ledger.month.total > 0 && monthlyValue.month_api_equivalent_usd <= 0;
  const currentMonth = `${new Date().getFullYear()}-${String(new Date().getMonth() + 1).padStart(2, '0')}`;
  const monthUnpricedCount = (ledger.analysis_rows || []).filter((row) => row.date.startsWith(currentMonth)).reduce((count, row) => count + row.unpriced_messages, 0);
  const selected = !hasTrend || selectedBucket == null ? null : series[selectedBucket];
  const legend = [...new Set(series.flatMap((b) => (b.per_client || []).map(([client]) => client)))];

  const dimRows =
    dim === 'agent' ? ledger.by_agent.map((a) => ({ label: a.label, color: CLIENT_COLORS[a.client] || 'var(--color-text-secondary)', agg: a.agg, key: a.client }))
    : dim === 'session' ? ledger.by_session.map((s) => ({ label: s.session, color: CLIENT_COLORS[s.client] || 'var(--color-text-secondary)', agg: s.agg, key: `${s.client}:${s.session}` }))
    : dim === 'model' ? ledger.by_model.map((m) => ({
      label: m.model,
      clients: m.clients || [],
      color: 'var(--color-text-blue)',
      agg: m.agg,
      key: m.model,
      isModel: true,
    }))
    : ledger.by_project.map((p) => ({ label: p.project, color: 'var(--color-text-secondary)', agg: p.agg, key: p.project }));

  const dimTotal = dimRows.reduce((s, r) => s + r.agg.total, 0) || 1;

  return (
    <div className="usage-page">
      <Suspense fallback={<Card className="analysis-panel"><Text type="supporting">{t('loading')}</Text></Card>}><AnalysisPanel ledger={ledger} lang={lang} /></Suspense>
      <Card className="now-card">
        <div className="now-main">
          <Text type="supporting" className="mono-label">{t('now')} · {t('recent_hour')}</Text>
          <strong>{recent ? `${recent.label} ${fmtTokens(recent.agg.total)}` : t('no_data')}</strong>
          <span className="now-sub">{ledger.burn_rate != null ? `${Math.round(ledger.burn_rate)} ${t('burn_unit')}` : '—'}</span>
        </div>
        <div className="now-quota">
          <Text type="supporting" className="mono-label">{t('quota')}</Text>
          <strong>{quota ? `${quota.provider} ${Math.round(quota.used_pct)}%` : '—'}</strong>
          <span className="now-sub">{quota?.label || ''}</span>
        </div>
      </Card>
      <div className="stat-row">
        <Card className="stat-card">
          <Text type="supporting" className="mono-label">{t('today')}</Text>
          <ObservedDotMatrix value={ledger.today.total} typeface={typeface} />
          <div className="stat-sub">
            <span>{fmtCost(ledger.today.cost)}</span>
            <span>·</span>
            <span>{fmtInt(ledger.today.messages)} {t('messages')}</span>
          </div>
          <MiniBars agg={ledger.today} />
        </Card>
        <Card className="stat-card">
          <Text type="supporting" className="mono-label">{t('week')}</Text>
          <ObservedDotMatrix value={ledger.week.total} typeface={typeface} />
          <div className="stat-sub">
            <span>{fmtCost(ledger.week.cost)}</span>
          </div>
        </Card>
        <Card className="stat-card">
          <Text type="supporting" className="mono-label">{t('month')}</Text>
          <ObservedDotMatrix value={ledger.month.total} typeface={typeface} />
          <div className="stat-sub monthly-value-sub">
            {monthlyValue ? (
              <>
                <span className="monthly-value-readout">
                  {monthUnpriced
                    ? `API— · —×${fmtCost(planBenchmark)} ${planLabel}`
                    : `API≈${fmtCost(monthlyValue.month_api_equivalent_usd)} · ${planMultiple.toFixed(2)}×${fmtCost(planBenchmark)} ${planLabel}`}
                </span>
                <button
                  className="plan-benchmark-edit"
                  type="button"
                  onClick={onOpenSettings}
                  aria-label={lang === 0 ? '打开设置修改每月套餐费用' : 'Open settings to edit monthly plan cost'}
                  title={lang === 0 ? '在设置中修改月费' : 'Edit monthly cost in Settings'}
                >
                  {lang === 0 ? '设置' : 'Settings'}
                </button>
              </>
            ) : <span>{fmtCost(ledger.month.cost)}</span>}
          </div>
          {monthlyValue && monthUnpricedCount > 0 && <div className="price-provenance">
            <span className="cost-partial">{lang === 0 ? `${monthUnpricedCount} 条缺价` : `${monthUnpricedCount} unpriced`}</span>
          </div>}
        </Card>
      </div>

      <Card className="trend-card">
        <div className="trend-toolbar">
          <Text weight="medium" className="mono-label">{range === '24h' ? t('range_24h') : t('range_14d')}</Text>
          <div className="trend-controls">
            <SegmentedControl
              size="sm"
              label={lang === 0 ? '趋势时间范围' : 'Trend range'}
              value={range}
              onChange={(next) => { setRange(next); setSelectedBucket(null); }}
            >
              <SegmentedControlItem label={t('range_24h')} value="24h" />
              <SegmentedControlItem label={t('range_14d')} value="14d" />
            </SegmentedControl>
            <SegmentedControl
              size="sm"
              label={lang === 0 ? '趋势数值' : 'Trend metric'}
              value={metric}
              onChange={(next) => { setMetric(next); setSelectedBucket(null); }}
            >
              <SegmentedControlItem label={t('metric_tokens')} value="tokens" />
              <SegmentedControlItem label={t('metric_cost')} value="cost" />
            </SegmentedControl>
          </div>
        </div>
        {!hasTrend ? <Text type="supporting">{metric === 'cost' ? t('no_data') : t('empty_trend')}</Text> : <div className="trend-chart">
          {series.map((b, i) => {
            const v = value(b);
            const h = Math.max(v > 0 ? 3 : 0, (v / maxV) * 100);
            const isPeak = b === peak && v > 0;
            const parts = b.per_client?.length
              ? b.per_client
              : null;
            return (
              <Tooltip key={i} content={`${periodLabel(b)} · ${fmtTokens(b.agg.total)} / ${fmtCost(b.agg.cost)}`}>
                <button
                  type="button"
                  className={`trend-col${isPeak ? ' trend-col-peak' : ''}${selectedBucket === i ? ' trend-col-selected' : ''}`}
                  style={{ height: `${h}%` }}
                  onClick={() => setSelectedBucket((current) => current === i ? null : i)}
                  aria-label={`${periodLabel(b)} · ${fmtTokens(b.agg.total)} · ${fmtCost(b.agg.cost)}`}
                  aria-pressed={selectedBucket === i}
                  disabled={v <= 0}
                >
                  {parts && metric === 'tokens'
                    ? parts.map(([c, g]) => (
                        <div key={c} style={{ flexGrow: g.total, background: CLIENT_COLORS[c] || 'var(--color-text-secondary)' }} />
                      ))
                    : <div style={{ flexGrow: 1, background: 'var(--color-text-blue)' }} />}
                </button>
              </Tooltip>
            );
          })}
        </div>}
        {hasTrend && <div className="trend-axis">
          <span>{periodLabel(series[0])}</span>
          <span>{periodLabel(series[series.length - 1])}</span>
        </div>}
        {hasTrend && metric === 'tokens' && legend.length > 0 && <div className="trend-legend">
          {legend.map((client) => <span key={client}><i style={{ background: CLIENT_COLORS[client] || 'var(--color-text-secondary)' }} />{client}</span>)}
        </div>}
        {hasTrend && metric === 'cost' && <Text type="supporting">{t('aggregate')}</Text>}
        {selected && <div className="trend-detail">
          <strong>{periodLabel(selected)} · {metric === 'tokens' ? fmtTokens(selected.agg.total) : fmtCost(selected.agg.cost)}</strong>
          <span>{(selected.per_client || []).slice(0, 3).map(([client, agg]) => `${client} ${metric === 'tokens' ? fmtTokens(agg.total) : fmtCost(agg.cost)}`).join(' · ')}</span>
        </div>}
      </Card>

      <div className="usage-columns">
        <Card className="dim-card">
          <div className="trend-toolbar">
            <Text weight="medium" className="mono-label">
              {dim === 'agent' ? t('by_agent') : dim === 'session' ? t('by_session') : dim === 'project' ? t('by_project') : t('by_model')} · 30d
            </Text>
            <SegmentedControl
              size="sm"
              label={lang === 0 ? '用量拆分维度' : 'Usage dimension'}
              value={dim}
              onChange={setDim}
            >
              <SegmentedControlItem label={t('dim_agent')} value="agent" />
              <SegmentedControlItem label={t('dim_session')} value="session" />
              <SegmentedControlItem label={t('dim_project')} value="project" />
              <SegmentedControlItem label={t('dim_model')} value="model" />
            </SegmentedControl>
          </div>
          <div className="dim-rows">
            {dimRows.length === 0 ? <Text type="supporting">{t('no_data')}</Text> : dimRows.map((r) => (
                <div key={r.key} className="dim-row">
                  <span className="dim-dot" style={{ background: r.color }} />
                  <span
                    className="dim-label"
                    title={r.isModel ? `${r.label} · ${(r.clients || []).join(', ')}` : r.label}
                    aria-label={r.isModel ? `${r.label}, ${(r.clients || []).join(', ')}` : r.label}
                  >{r.label}</span>
                  <div className="dim-bar">
                    <BlockBar pct={(r.agg.total / dimTotal) * 100} color={r.color} />
                  </div>
                  <span className={`dim-value${r.isModel ? ' dim-model-value' : ''}`} title={r.isModel ? `${fmtInt(r.agg.total)} tokens` : undefined}>
                    {r.isModel ? fmtInt(r.agg.total) : fmtTokens(r.agg.total)}
                  </span>
                </div>
            ))}
          </div>
        </Card>

        <Card className="recent-card">
          <Text weight="medium" className="mono-label">{t('recent_hour')}</Text>
          {ledger.recent_by_agent.length === 0 ? (
            <Text type="supporting">{t('no_data')}</Text>
          ) : (
            <div className="dim-rows">
              {ledger.recent_by_agent.map((r) => (
                <div key={r.client} className="dim-row">
                  <span className="dim-dot" style={{ background: CLIENT_COLORS[r.client] || 'var(--color-text-secondary)' }} />
                  <span className="dim-label">{r.label}</span>
                  <span className="dim-value">{fmtTokens(r.agg.total)}</span>
                  <span className="dim-sub">{fmtRel(Date.now() - r.last_ts)}</span>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}

function SettingsChoice({ label, value, onChange, options }) {
  return (
    <div className={`settings-choice${options.length > 3 ? ' settings-choice-many' : ''}`} role="group" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          aria-pressed={value === option.value}
          className={`settings-choice-option${value === option.value ? ' is-selected' : ''}`}
          onClick={() => onChange(option.value)}
        >
          {option.icon && <span className={`settings-choice-icon ${option.icon}`} aria-hidden="true" />}
          <span>{option.label}</span>
        </button>
      ))}
    </div>
  );
}

function SettingsSection({ title, hint, children, className = '' }) {
  return (
    <section className={`settings-card ${className}`}>
      <header className="settings-card-heading">
        <div>
          <h2>{title}</h2>
          {hint && <p>{hint}</p>}
        </div>
      </header>
      {children}
    </section>
  );
}

function SettingsRow({ title, hint, children, className = '' }) {
  return (
    <div className={`settings-row ${className}`}>
      <div className="settings-row-copy">
        <div className="settings-row-title">{title}</div>
        {hint && <div className="settings-row-hint">{hint}</div>}
      </div>
      <div className="settings-row-control">{children}</div>
    </div>
  );
}

function SettingsPage({ lang, preferences, onPreferenceChange, onResetAppearance, onResetCards, onShowWelcome, monthlyValue, nativeError, goKeyStored, goKeyFeedback, onSaveGoKey, onDeleteGoKey, goatKeyStored, goatKeyFeedback, onSaveGoatKey, onDeleteGoatKey }) {
  const t = (key) => STR[key][lang];
  const initialBenchmark = monthlyValue?.list_price_benchmark_usd ?? monthlyValue?.default_list_price_benchmark_usd ?? 40;
  const [benchmarkOverride, setBenchmarkOverride] = useState(null);
  const [planDraft, setPlanDraft] = useState(initialBenchmark.toFixed(2));
  const [planError, setPlanError] = useState('');
  const [saved, setSaved] = useState(false);
  const [goKeyDraft, setGoKeyDraft] = useState('');
  const [goKeyError, setGoKeyError] = useState('');
  const [goatKeyDraft, setGoatKeyDraft] = useState('');
  const [goatKeyError, setGoatKeyError] = useState('');
  const saveGoKey = (event) => {
    event.preventDefault();
    const key = goKeyDraft.trim();
    if (!key || /\s/.test(key)) { setGoKeyError(t('settings_go_key_invalid')); return; }
    onSaveGoKey(key);
    setGoKeyDraft('');
    setGoKeyError('');
  };
  const saveGoatKey = (event) => {
    event.preventDefault();
    const key = goatKeyDraft.trim();
    if (!key || /\s/.test(key)) { setGoatKeyError(t('settings_go_key_invalid')); return; }
    onSaveGoatKey(key);
    setGoatKeyDraft('');
    setGoatKeyError('');
  };
  useEffect(() => {
    setBenchmarkOverride(null);
    setPlanDraft(initialBenchmark.toFixed(2));
  }, [initialBenchmark]);

  const benchmark = benchmarkOverride?.amount ?? initialBenchmark;
  const isCustom = benchmarkOverride?.isCustom ?? monthlyValue?.benchmark_is_custom ?? false;
  const saveBenchmark = (event) => {
    event.preventDefault();
    const amount = Number(planDraft);
    if (!Number.isFinite(amount) || amount < 0.01 || amount > 1_000_000) {
      setPlanError(t('settings_invalid_price'));
      setSaved(false);
      return;
    }
    window.ipc?.postMessage?.(JSON.stringify({ cmd: 'setPlanBenchmark', amount }));
    setBenchmarkOverride({ amount, isCustom: true });
    setPlanError('');
    setSaved(true);
  };
  const resetBenchmark = () => {
    const amount = monthlyValue?.default_list_price_benchmark_usd ?? 40;
    window.ipc?.postMessage?.(JSON.stringify({ cmd: 'resetPlanBenchmark' }));
    setBenchmarkOverride({ amount, isCustom: false });
    setPlanDraft(amount.toFixed(2));
    setPlanError('');
    setSaved(true);
  };

  return (
    <div className="settings-page">
      <header className="settings-intro">
        <Text type="supporting" className="settings-kicker">{t('settings_kicker')}</Text>
        <h1>{t('nav_settings')}</h1>
      </header>

      <div className="settings-grid">
        <SettingsSection title={t('settings_appearance')} className="settings-appearance">
          <SettingsRow title={t('settings_language')}>
            <SettingsChoice
              label={t('settings_language')}
              value={lang}
              onChange={(value) => onPreferenceChange('lang', value)}
              options={[{ value: 0, label: '中文' }, { value: 1, label: 'English' }]}
            />
          </SettingsRow>
          <SettingsRow title={t('settings_theme')} hint={t('settings_theme_hint')}>
            <SettingsChoice
              label={t('settings_theme')}
              value={preferences.theme}
              onChange={(value) => onPreferenceChange('theme', value)}
              options={[
                { value: 'system', label: t('theme_system'), icon: 'theme-icon-system' },
                { value: 'light', label: t('theme_light'), icon: 'theme-icon-light' },
                { value: 'dark', label: t('theme_dark'), icon: 'theme-icon-dark' },
                { value: 'ink', label: t('theme_ink'), icon: 'theme-icon-ink' },
                { value: 'sand', label: t('theme_sand'), icon: 'theme-icon-sand' },
              ]}
            />
          </SettingsRow>
          <div className="settings-theme-preview" aria-hidden="true">
            <span className="settings-preview-window"><i /><i /><i /><b /><b /><b /></span>
            <span className="settings-preview-copy"><strong>{t('settings_font_sample')}</strong><small>12,480 tokens · $3.84</small></span>
            <span className="settings-preview-spark" />
          </div>
        </SettingsSection>

        <div className="settings-column">
          <SettingsSection title={t('settings_go_key')} hint={t('settings_go_key_hint')} className="settings-go-key">
            <p className="settings-go-key-status">{goKeyStored ? t('settings_go_key_stored') : t('settings_go_key_missing')}</p>
            <form className="settings-go-key-form" onSubmit={saveGoKey}>
              <input type="password" autoComplete="off" spellCheck="false" value={goKeyDraft} onChange={(event) => { setGoKeyDraft(event.target.value); setGoKeyError(''); }} placeholder={t('settings_go_key_placeholder')} aria-label={t('settings_go_key_placeholder')} />
              <button className="settings-save" type="submit">{t('settings_save')}</button>
              <button className="settings-reset" type="button" disabled={!goKeyStored} onClick={onDeleteGoKey}>{t('settings_go_key_remove')}</button>
            </form>
            {(goKeyError || goKeyFeedback) && <p className="settings-form-error" role="status">{goKeyError || goKeyFeedback}</p>}
          </SettingsSection>
          <SettingsSection title={t('settings_goat_key')} hint={t('settings_goat_key_hint')} className="settings-go-key">
            <p className="settings-go-key-status">{goatKeyStored ? t('settings_go_key_stored') : (lang === 0 ? '尚未保存专用密钥；仍可读取环境变量或 cmdc 登录态。' : 'No dedicated key saved; an environment variable or cmdc login may still work.')}</p>
            <form className="settings-go-key-form" onSubmit={saveGoatKey}>
              <input type="password" autoComplete="off" spellCheck="false" value={goatKeyDraft} onChange={(event) => { setGoatKeyDraft(event.target.value); setGoatKeyError(''); }} placeholder="Command Code API key" aria-label={`${t('settings_goat_key')} API key`} />
              <button className="settings-save" type="submit">{t('settings_save')}</button>
              <button className="settings-reset" type="button" disabled={!goatKeyStored} onClick={onDeleteGoatKey}>{t('settings_go_key_remove')}</button>
            </form>
            {(goatKeyError || goatKeyFeedback) && <p className="settings-form-error" role="status">{goatKeyError || goatKeyFeedback}</p>}
          </SettingsSection>
          <SettingsSection title={t('settings_type')} className="settings-type">
            <SettingsRow title={t('settings_font')}>
              <SettingsChoice
                label={t('settings_font')}
                value={preferences.typeface}
                onChange={(value) => onPreferenceChange('typeface', value)}
                options={[
                  { value: 'plex', label: t('font_plex') },
                  { value: 'system', label: t('font_system') },
                  { value: 'pixel', label: t('font_pixel') },
                ]}
              />
            </SettingsRow>
            <div className={`settings-font-sample${preferences.typeface === 'pixel' ? ' is-pixel' : preferences.typeface === 'plex' ? ' is-plex' : ''}`}>
              <span className="settings-font-sample-label">TYPE SPECIMEN · 16</span>
              <div className="settings-font-sample-en">Token usage, at a glance</div>
              <div className="settings-font-sample-zh">令牌用量，一目了然</div>
              <div className="settings-font-sample-numbers">12,480 tokens · $3.84</div>
            </div>
            <SettingsRow title={t('settings_density')}>
              <SettingsChoice
                label={t('settings_density')}
                value={preferences.density}
                onChange={(value) => onPreferenceChange('density', value)}
                options={[
                  { value: 'comfortable', label: t('density_comfortable') },
                  { value: 'compact', label: t('density_compact') },
                ]}
              />
            </SettingsRow>
            <SettingsRow title={t('settings_motion')}>
              <SettingsChoice
                label={t('settings_motion')}
                value={preferences.motion}
                onChange={(value) => onPreferenceChange('motion', value)}
                options={[
                  { value: 'system', label: t('motion_system') },
                  { value: 'reduced', label: t('motion_reduce') },
                ]}
              />
            </SettingsRow>
          </SettingsSection>

          <SettingsSection title={t('settings_plan')} hint={t('settings_plan_hint')} className="settings-plan">
            <form className="settings-plan-form" onSubmit={saveBenchmark}>
              <label htmlFor="settings-plan-input">{t('settings_monthly')}</label>
              <div className="settings-plan-input-wrap">
                <span>$</span>
                <input
                  id="settings-plan-input"
                  type="number"
                  min="0.01"
                  max="1000000"
                  step="0.01"
                  required
                  value={planDraft}
                  onChange={(event) => { setPlanDraft(event.target.value); setSaved(false); setPlanError(''); }}
                  aria-label={`${t('settings_monthly')} (USD)`}
                />
                <span className="settings-plan-period">/ mo</span>
              </div>
              <div className="settings-plan-actions">
                <button className="settings-save" type="submit">{saved ? t('settings_saved') : t('settings_save')}</button>
                <button className="settings-reset" type="button" onClick={resetBenchmark} disabled={!isCustom}>{t('settings_default')}</button>
              </div>
              {planError && <p className="settings-form-error" role="alert">{planError}</p>}
            </form>
            <div className="settings-plan-note">
              <span className="settings-plan-note-dot" />
              {isCustom ? `${t('settings_monthly')}: ${fmtCost(benchmark)}` : `${t('settings_default')}: ${fmtCost(benchmark)}`}
            </div>
          </SettingsSection>

          <SettingsSection title={t('settings_desktop')} hint={t('settings_desktop_hint')} className="settings-desktop">
            <SettingsRow title={t('settings_close_behavior')}>
              <SettingsChoice label={t('settings_close_behavior')} value={preferences.closeBehavior} onChange={(value) => onPreferenceChange('closeBehavior', value)} options={[{ value: 'tray', label: t('settings_close_tray') }, { value: 'exit', label: t('settings_close_exit') }]} />
            </SettingsRow>
            <SettingsRow title={t('settings_shortcut')} hint={t('settings_shortcut_hint')}>
              <SettingsChoice label={t('settings_shortcut')} value={preferences.globalShortcut} onChange={(value) => onPreferenceChange('globalShortcut', value)} options={[{ value: false, label: t('settings_alerts_off') }, { value: true, label: t('settings_alerts_on') }]} />
            </SettingsRow>
            <SettingsRow title={t('settings_autostart')} hint={t('settings_autostart_hint')}>
              <SettingsChoice label={t('settings_autostart')} value={preferences.autostart} onChange={(value) => onPreferenceChange('autostart', value)} options={[{ value: false, label: t('settings_alerts_off') }, { value: true, label: t('settings_alerts_on') }]} />
            </SettingsRow>
            {nativeError && <p className="settings-form-error" role="alert">{nativeError}</p>}
            <SettingsRow title={t('settings_notifications')} hint={t('settings_notifications_hint')}>
              <SettingsChoice label={t('settings_notifications')} value={preferences.notifications} onChange={(value) => onPreferenceChange('notifications', value)} options={[{ value: 'on', label: t('settings_alerts_on') }, { value: 'off', label: t('settings_alerts_off') }]} />
            </SettingsRow>
            <SettingsRow title={t('settings_pace_alerts')} hint={t('settings_pace_alerts_hint')}>
              <SettingsChoice label={t('settings_pace_alerts')} value={preferences.paceAlerts} onChange={(value) => onPreferenceChange('paceAlerts', value)} options={[{ value: true, label: t('settings_alerts_on') }, { value: false, label: t('settings_alerts_off') }]} />
            </SettingsRow>
            <SettingsRow title={t('settings_quiet_hours')} hint={t('settings_quiet_hours_hint')}>
              <SettingsChoice label={t('settings_quiet_hours')} value={preferences.quietHours} onChange={(value) => onPreferenceChange('quietHours', value)} options={[{ value: false, label: t('settings_alerts_off') }, { value: true, label: t('settings_alerts_on') }]} />
            </SettingsRow>
            <button type="button" className="settings-quit" onClick={() => window.ipc?.postMessage?.(JSON.stringify({ cmd: 'quit' }))}>{t('settings_quit')}</button>
          </SettingsSection>

          <SettingsSection title={t('cards_custom')} hint={t('cards_custom_hint')} className="settings-cards">
            {PROVIDERS.map((provider) => <div className="settings-provider-row" key={provider}>
              <span>{provider}</span>
              <label><span>{lang === 0 ? '颜色' : 'Color'}</span><input type="color" aria-label={`${provider} ${lang === 0 ? '颜色' : 'color'}`} value={preferences.providerColors[provider] || PROVIDER_COLOR_DEFAULTS[provider]} onChange={(event) => onPreferenceChange('providerColors', { ...preferences.providerColors, [provider]: event.target.value })} /></label>
              <button type="button" onClick={() => onPreferenceChange('pinnedCards', preferences.pinnedCards.includes(provider) ? preferences.pinnedCards.filter((item) => item !== provider) : [...preferences.pinnedCards, provider])}>{preferences.pinnedCards.includes(provider) ? t('card_unpin') : t('card_pin')}</button>
              <button type="button" onClick={() => onPreferenceChange('hiddenCards', preferences.hiddenCards.includes(provider) ? preferences.hiddenCards.filter((item) => item !== provider) : [...preferences.hiddenCards, provider])}>{preferences.hiddenCards.includes(provider) ? t('card_show') : t('card_hide')}</button>
            </div>)}
            <div className="settings-card-footer"><button type="button" onClick={onResetCards}>{t('cards_restore')}</button><button type="button" onClick={onShowWelcome}>{t('welcome_reopen')}</button></div>
          </SettingsSection>

          <div className="settings-footer">
            <span>Token Monitor <span className="settings-footer-dot">·</span> v0.1</span>
            <button type="button" onClick={onResetAppearance}>{t('settings_reset_all')}</button>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ---------------- 面板仪表板 ---------------- */

const PANEL_COLS = 12;
const PANEL_ROW_H = 84;
const PANEL_GAP = 14;
const PANELS_KEY = 'token-monitor.panels.v1';
const PANELS_MAX = 14;

const PANEL_CATALOG = [
  { id: 'heat', w: 12, h: 4 },
  { id: 'alltime', w: 4, h: 1 },
  { id: 'streak', w: 4, h: 1 },
  { id: 'records', w: 4, h: 1 },
  { id: 'cache', w: 4, h: 2 },
  { id: 'punch', w: 8, h: 2 },
  { id: 'clock', w: 4, h: 3 },
  { id: 'week', w: 4, h: 2 },
  { id: 'daily', w: 4, h: 3 },
  { id: 'models', w: 4, h: 3 },
  { id: 'sources', w: 4, h: 2 },
  { id: 'projects', w: 4, h: 3 },
];
const DEFAULT_PANELS = [
  { id: 'heat', x: 0, y: 0, w: 12, h: 4, locked: false },
  { id: 'alltime', x: 0, y: 4, w: 4, h: 1, locked: false },
  { id: 'streak', x: 4, y: 4, w: 4, h: 1, locked: false },
  { id: 'records', x: 8, y: 4, w: 4, h: 1, locked: false },
  { id: 'cache', x: 0, y: 5, w: 4, h: 2, locked: false },
  { id: 'punch', x: 4, y: 5, w: 8, h: 2, locked: false },
  { id: 'clock', x: 0, y: 7, w: 4, h: 3, locked: false },
  { id: 'week', x: 4, y: 7, w: 4, h: 2, locked: false },
  { id: 'daily', x: 8, y: 7, w: 4, h: 3, locked: false },
  { id: 'models', x: 4, y: 9, w: 4, h: 3, locked: false },
  { id: 'sources', x: 8, y: 9, w: 4, h: 2, locked: false },
  { id: 'projects', x: 0, y: 10, w: 4, h: 3, locked: false },
];

const panelOverlap = (a, b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
const panelOverlapArea = (a, b) =>
  Math.max(0, Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x)) *
  Math.max(0, Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y));

/* 锁定卡是钉子,其余自由卡按原序落到不重叠的最小行 */
function compactPanels(panels) {
  const locked = panels.filter((p) => p.locked);
  const free = panels.filter((p) => !p.locked).sort((a, b) => a.y - b.y || a.x - b.x);
  const placed = [...locked];
  for (const p of free) {
    let y = 0;
    while (y < 400 && placed.some((q) => panelOverlap({ ...p, y }, q))) y++;
    placed.push({ ...p, y });
  }
  return placed.sort((a, b) => a.y - b.y || a.x - b.x);
}

function resolvePanelMove(panels, id, target) {
  const origin = panels.find((p) => p.id === id);
  const others = panels.filter((p) => p.id !== id);
  const overlapping = others.filter((p) => panelOverlap(target, p) && !p.locked);
  if (overlapping.length === 0) return compactPanels([target, ...others]);
  const first = overlapping.reduce((best, p) => (panelOverlapArea(target, p) > panelOverlapArea(target, best) ? p : best), overlapping[0]);
  const swapped = { ...first, x: origin.x, y: origin.y };
  const rest = others.filter((p) => p.id !== first.id);
  return compactPanels([target, swapped, ...rest]);
}

function resolvePanelResize(panels, id, target) {
  const others = panels.filter((p) => p.id !== id);
  let { w, h } = target;
  while (w > 1 && others.some((p) => panelOverlap({ ...target, w }, p))) w--;
  while (h > 1 && others.some((p) => panelOverlap({ ...target, w, h }, p))) h--;
  return { ...target, w, h };
}
void resolvePanelResize;

function collectAlignLines(panels, id, nx, ny) {
  const v = [];
  const h = [];
  panels.forEach((p) => {
    if (p.id === id) return;
    if (p.x === nx) v.push(p.x);
    if (p.x + p.w === nx) v.push(p.x + p.w);
    if (p.y === ny) h.push(p.y);
    if (p.y + p.h === ny) h.push(p.y + p.h);
  });
  return { v: [...new Set(v)], h: [...new Set(h)] };
}

function readPanelState() {
  try {
    const raw = JSON.parse(window.localStorage.getItem(PANELS_KEY) || 'null');
    if (!raw || !Array.isArray(raw.panels) || !Array.isArray(raw.enabled)) return null;
    const ok = raw.panels.filter((p) => PANEL_CATALOG.some((c) => c.id === p.id) && Number.isFinite(p.x) && Number.isFinite(p.y) && p.w >= 1 && p.h >= 1);
    const enabled = raw.enabled.filter((id) => PANEL_CATALOG.some((c) => c.id === id));
    return { panels: ok, enabled };
  } catch {
    return null;
  }
}

const WEEKDAY_ZH = ['一', '二', '三', '四', '五', '六', '日'];
const WEEKDAY_EN = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const MONTH_EN = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

function HeatPanel({ ledger, lang }) {
  const totals = ledger?.daily_totals || [];
  const [sel, setSel] = useState(null);
  const days = [...totals];
  while (days.length % 7 !== 0) days.unshift({ date: '', tokens: 0, cost: 0, empty: true });
  const weeks = [];
  for (let i = 0; i < days.length; i += 7) weeks.push(days.slice(i, i + 7));
  const maxDay = Math.max(1, ...totals.map((d) => d.tokens));
  const now = new Date();
  const todayStr = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
  const level = (v) => (v <= 0 ? 0 : Math.min(6, Math.ceil((v / maxDay) * 6)));
  const monthOf = (w) => (w[0]?.date ? Number(w[0].date.slice(5, 7)) : null);
  const detail = sel ? ledger?.daily_detail?.[sel] : null;
  const selTotal = totals.find((d) => d.date === sel);
  const dayNames = lang === 0 ? WEEKDAY_ZH : ['M', 'T', 'W', 'T', 'F', 'S', 'S'];
  return (
    <>
      <div className="heat-wrap">
        <div className="heat-days">{dayNames.map((d, i) => <span key={i}>{d}</span>)}</div>
        <div className="heat-flex">
          <div className="heat-months">{weeks.map((w, wi) => {
            const m = monthOf(w);
            const prev = wi > 0 ? monthOf(weeks[wi - 1]) : null;
            const label = m == null || m === prev ? '' : lang === 0 ? `${m}月` : MONTH_EN[m - 1];
            return <span key={wi}>{label}</span>;
          })}</div>
          <div className="heat-cols">
            {weeks.map((w, wi) => (
              <div className="heat-col" key={wi}>
                {w.map((d, di) => d.empty || !d.date ? <div key={di} className="heat-cell is-empty" /> : (
                  <Tooltip key={di} content={`${d.date} · ${fmtTokens(d.tokens)} / ${fmtCost(d.cost)}`}>
                    <button
                      type="button"
                      className={`heat-cell${sel === d.date ? ' is-sel' : ''}${d.date === todayStr ? ' is-today' : ''}`}
                      style={d.tokens > 0 ? { background: `color-mix(in srgb, var(--color-text-blue) ${Math.round(level(d.tokens) * 15)}%, transparent)` } : undefined}
                      onClick={() => setSel((s) => (s === d.date ? null : d.date))}
                      aria-label={`${d.date} · ${fmtTokens(d.tokens)}`}
                      aria-pressed={sel === d.date}
                    />
                  </Tooltip>
                ))}
              </div>
            ))}
          </div>
        </div>
      </div>
      <div className="heat-foot">
        <div className="heat-legend">{[1, 2, 3, 4, 5, 6].map((lv) => <i key={lv} style={{ background: `color-mix(in srgb, var(--color-text-blue) ${lv * 15}%, transparent)` }} />)}</div>
        {selTotal && <span className="heat-sel">{selTotal.date} · {fmtTokens(selTotal.tokens)} · {fmtCost(selTotal.cost)}</span>}
      </div>
      {sel && detail && (
        <div className="heat-drill">
          {detail.map(([c, g]) => (
            <span key={c} className="heat-drill-row"><i style={{ background: CLIENT_COLORS[c] || 'var(--color-text-secondary)' }} />{c}<b>{fmtTokens(g.total)}</b></span>
          ))}
        </div>
      )}
    </>
  );
}

function PunchPanel({ ledger, lang }) {
  const wh = ledger?.weekday_hour || Array.from({ length: 7 }, () => Array(24).fill(0));
  const max = Math.max(1, ...wh.flat());
  const dayNames = lang === 0 ? WEEKDAY_ZH : WEEKDAY_EN;
  const dayFull = lang === 0 ? ['周一', '周二', '周三', '周四', '周五', '周六', '周日'] : WEEKDAY_EN;
  return (
    <div className="punch">
      <div className="punch-row punch-row-head">
        <span className="punch-day" />
        {Array.from({ length: 24 }, (_, h) => <span key={h} className="punch-hour">{h % 6 === 0 ? h : ''}</span>)}
      </div>
      {wh.map((row, di) => (
        <div className="punch-row" key={di}>
          <span className="punch-day">{dayNames[di]}</span>
          {row.map((v, hi) => (
            <Tooltip key={hi} content={`${dayFull[di]} ${String(hi).padStart(2, '0')}:00 · ${fmtTokens(v)}`}>
              <i className="punch-cell" style={v > 0 ? { background: `color-mix(in srgb, var(--color-text-purple) ${Math.round(14 + Math.ceil((v / max) * 5) * 17)}%, transparent)` } : undefined} />
            </Tooltip>
          ))}
        </div>
      ))}
    </div>
  );
}

function clockArc(r0, r1, a0, a1) {
  const rad = (a) => (a * Math.PI) / 180;
  const p = (r, a) => `${(r * Math.cos(rad(a))).toFixed(2)} ${(r * Math.sin(rad(a))).toFixed(2)}`;
  const large = a1 - a0 > 180 ? 1 : 0;
  return `M ${p(r0, a0)} L ${p(r1, a0)} A ${r1} ${r1} 0 ${large} 1 ${p(r1, a1)} L ${p(r0, a1)} A ${r0} ${r0} 0 ${large} 0 ${p(r0, a0)} Z`;
}

function ClockPanel({ ledger }) {
  const hourly = ledger?.hourly || Array(24).fill(0);
  const max = Math.max(1, ...hourly);
  const peak = hourly.indexOf(Math.max(...hourly));
  return (
    <div className="clock">
      <svg viewBox="-60 -60 120 120" role="img" aria-label="hourly distribution">
        {hourly.map((v, h) => {
          if (v <= 0) return null;
          const a0 = -90 + h * 15 + 1.2;
          const a1 = -90 + (h + 1) * 15 - 1.2;
          const len = 26 + (v / max) * 28;
          return <path key={h} d={clockArc(26, len, a0, a1)} fill="var(--color-text-blue)" opacity={0.3 + (v / max) * 0.7} />;
        })}
        {[0, 6, 12, 18].map((h) => {
          const rad = ((-90 + h * 15 + 7.5) * Math.PI) / 180;
          return <text key={h} x={57 * Math.cos(rad)} y={57 * Math.sin(rad) + 3} textAnchor="middle" fontSize="7">{h}</text>;
        })}
        <text className="clock-peak" x="0" y="-2" textAnchor="middle" fontSize="12">{max > 1 ? `${String(peak).padStart(2, '0')}:00` : '—'}</text>
        <text x="0" y="10" textAnchor="middle" fontSize="6.5">PEAK</text>
      </svg>
    </div>
  );
}

function BoardPanel({ rows, color = 'var(--color-text-blue)', showRank = false }) {
  const top = rows.slice(0, 6);
  const topV = top[0]?.v || 1;
  if (top.length === 0) return <div className="board-empty">—</div>;
  return (
    <div className="board">
      {top.map((r, i) => (
        <div className="board-row" key={r.key}>
          {showRank && <span className="board-rank">{String(i + 1).padStart(2, '0')}</span>}
          <span className="board-label" title={r.label}>{r.label}</span>
          <BlockBar pct={(r.v / topV) * 100} color={r.color || color} />
          <span className="board-value">{fmtTokens(r.v)}</span>
        </div>
      ))}
    </div>
  );
}

function WeekPanel({ ledger, lang }) {
  const weekday = ledger?.weekday || Array(7).fill(0);
  const max = Math.max(1, ...weekday);
  const peak = weekday.indexOf(Math.max(...weekday));
  const names = lang === 0 ? WEEKDAY_ZH : WEEKDAY_EN;
  return (
    <div className="board">
      {weekday.map((v, i) => (
        <div className="board-row" key={i}>
          <span className="board-label">{names[i]}</span>
          <BlockBar pct={(v / max) * 100} color={i === peak && v > 0 ? 'var(--sum-orange)' : 'var(--color-text-blue)'} />
          <span className="board-value">{fmtTokens(v)}</span>
        </div>
      ))}
    </div>
  );
}

function ModelsPanel({ ledger }) {
  const rows = [...(ledger?.by_model || [])]
    .sort((a, b) => b.agg.total - a.agg.total)
    .slice(0, 6)
    .map((m) => ({ key: m.model, label: m.model, v: m.agg.total }));
  return <BoardPanel rows={rows} color="var(--color-text-purple)" showRank />;
}

function SourcesPanel({ ledger }) {
  const rows = ledger?.month_by_client || [];
  const total = rows.reduce((s, r) => s + r.agg.total, 0) || 1;
  return (
    <div className="share">
      <div className="share-track">
        {rows.map((r) => (
          <div key={r.client} style={{ flexGrow: Math.max(1, r.agg.total), background: CLIENT_COLORS[r.client] || 'var(--color-text-secondary)' }} title={`${r.client} ${fmtTokens(r.agg.total)}`} />
        ))}
      </div>
      <div className="share-legend">
        {rows.map((r) => (
          <span key={r.client}><i style={{ background: CLIENT_COLORS[r.client] || 'var(--color-text-secondary)' }} />{r.client} {Math.round((r.agg.total / total) * 100)}%</span>
        ))}
      </div>
    </div>
  );
}

function ProjectsPanel({ ledger }) {
  const rows = [...(ledger?.by_project || [])]
    .sort((a, b) => b.agg.total - a.agg.total)
    .slice(0, 6)
    .map((p) => ({ key: p.project, label: p.project || '—', v: p.agg.total, sub: p.sessions }));
  const topV = rows[0]?.v || 1;
  if (rows.length === 0) return <div className="board-empty">—</div>;
  return (
    <div className="board">
      {rows.map((r, i) => (
        <div className="board-row" key={r.key}>
          <span className="board-rank">{String(i + 1).padStart(2, '0')}</span>
          <span className="board-label" title={r.label}>{r.label}</span>
          <BlockBar pct={(r.v / topV) * 100} color="var(--sum-orange)" />
          <span className="board-value">{fmtTokens(r.v)}</span>
        </div>
      ))}
    </div>
  );
}

function AllTimePanel({ ledger }) {
  const days = ledger?.daily_totals || [];
  const sum = (f) => days.reduce((s, d) => s + (f(d) || 0), 0);
  const active = days.filter((d) => d.tokens > 0).length;
  const cells = [
    [fmtTokens(sum((d) => d.tokens)), 'TOKENS'],
    [fmtInt(sum((d) => d.messages)), 'RUNS'],
    [`~${fmtCost(sum((d) => d.cost))}`, 'COST'],
    [String(active), 'DAYS'],
  ];
  return (
    <div className="big4">
      {cells.map(([v, label]) => <div key={label}><b>{v}</b><span>{label}</span></div>)}
    </div>
  );
}

function StreakPanel({ ledger }) {
  const days = ledger?.daily_totals || [];
  let cur = 0;
  for (let i = days.length - 1; i >= 0 && days[i].tokens > 0; i--) cur++;
  let best = 0;
  let run = 0;
  for (const d of days) {
    run = d.tokens > 0 ? run + 1 : 0;
    best = Math.max(best, run);
  }
  const active = days.filter((d) => d.tokens > 0).length;
  return (
    <div className="big4">
      <div><b>{cur}<i className="big4-unit">d</i></b><span>{'CURRENT'}</span></div>
      <div><b>{best}<i className="big4-unit">d</i></b><span>{'LONGEST'}</span></div>
      <div><b>{active}</b><span>{'ACTIVE'}</span></div>
      <div><b>{fmtTokens(days.reduce((s, d) => s + d.tokens, 0))}</b><span>{'26W'}</span></div>
    </div>
  );
}

function RecordsPanel({ ledger }) {
  const days = ledger?.daily_totals || [];
  const maxDay = days.reduce((m, d) => (d.tokens > (m?.tokens || 0) ? d : m), null);
  const topSession = (ledger?.by_session || [])[0];
  const priciest = (ledger?.by_model || [])
    .filter((m) => m.agg.messages > 0)
    .reduce((best, m) => {
      const r = m.agg.cost / m.agg.messages;
      return !best || r > best.r ? { label: m.model, r } : best;
    }, null);
  return (
    <div className="records">
      <div className="records-row"><span className="records-label">BUSIEST DAY</span><span className="records-value">{maxDay ? maxDay.date.slice(5) : '—'}</span><span className="records-sub">{fmtTokens(maxDay?.tokens)}</span></div>
      <div className="records-row"><span className="records-label">TOP SESSION</span><span className="records-value">{topSession ? topSession.session.slice(0, 18) : '—'}</span><span className="records-sub">{fmtTokens(topSession?.agg.total)}</span></div>
      <div className="records-row"><span className="records-label">PRICIEST MODEL</span><span className="records-value">{priciest ? priciest.label : '—'}</span><span className="records-sub">{priciest ? `$${priciest.r.toFixed(4)}/req` : ''}</span></div>
    </div>
  );
}

function CachePanel({ ledger, lang }) {
  const m = ledger?.month;
  const hit = m && m.input + m.cache_read > 0 ? (m.cache_read / (m.input + m.cache_read)) * 100 : 0;
  const mix = m ? [
    ['input', m.input, 'var(--color-text-blue)'],
    ['output', m.output, 'var(--color-text-green)'],
    [STR.panel_cache_read[lang], m.cache_read, 'var(--color-text-purple)'],
    [STR.panel_cache_write[lang], m.cache_write, 'var(--color-text-orange)'],
    ['reasoning', m.reasoning, 'var(--color-text-secondary)'],
  ] : [];
  const mixTotal = mix.reduce((s, [, v]) => s + v, 0) || 1;
  return (
    <div className="cache">
      <div className="cache-hit"><b>{hit.toFixed(1)}<i className="big4-unit">%</i></b><span>{'CACHE HIT'}</span></div>
      <div className="cache-mix">
        <div className="cache-track">
          {mix.map(([k, v, color]) => <div key={k} style={{ flexGrow: Math.max(1, v), background: color }} title={`${k} ${fmtTokens(v)}`} />)}
        </div>
        <div className="share-legend">
          {mix.map(([k, v, color]) => <span key={k}><i style={{ background: color }} />{k} {Math.round((v / mixTotal) * 100)}%</span>)}
        </div>
      </div>
    </div>
  );
}

function DailyPanel({ ledger }) {
  const last30 = (ledger?.daily_totals || []).slice(-30);
  const max = Math.max(1, ...last30.map((d) => d.tokens));
  return (
    <div className="daily">
      <div className="daily-bars">
        {last30.map((d) => (
          <Tooltip key={d.date} content={`${d.date} · ${fmtTokens(d.tokens)} / ${fmtCost(d.cost)}`}>
            <div className="daily-bar" style={{ height: `${Math.max(2, (d.tokens / max) * 100)}%` }} />
          </Tooltip>
        ))}
      </div>
      <div className="daily-axis">
        <span>{last30[0]?.date?.slice(5) || ''}</span>
        <span>{last30[last30.length - 1]?.date?.slice(5) || ''}</span>
      </div>
    </div>
  );
}

const PANEL_BODIES = {
  heat: HeatPanel,
  punch: PunchPanel,
  clock: ClockPanel,
  week: WeekPanel,
  models: ModelsPanel,
  sources: SourcesPanel,
  projects: ProjectsPanel,
  alltime: AllTimePanel,
  streak: StreakPanel,
  records: RecordsPanel,
  cache: CachePanel,
  daily: DailyPanel,
};

class PanelErrorBoundary extends React.Component {
  constructor(props) { super(props); this.state = { err: null }; }
  static getDerivedStateFromError(err) { return { err }; }
  render() {
    if (this.state.err) return <div className="panel-error">PANEL ERROR · {String(this.state.err?.message || this.state.err).slice(0, 160)}</div>;
    return this.props.children;
  }
}

function panelMeta(id, ledger, lang) {
  const L = ledger;
  if (!L) return '';
  switch (id) {
    case 'heat': {
      const total = (L.daily_totals || []).reduce((s, d) => s + d.tokens, 0);
      return fmtTokens(total);
    }
    case 'punch': {
      const flat = (L.weekday_hour || []).flat();
      const max = Math.max(0, ...flat);
      return max > 0 ? fmtTokens(max) : '';
    }
    case 'clock': {
      const peak = (L.hourly || []).indexOf(Math.max(0, ...(L.hourly || [0])));
      return Math.max(0, ...(L.hourly || [0])) > 0 ? `${String(peak).padStart(2, '0')}:00` : '';
    }
    case 'week': {
      const names = lang === 0 ? WEEKDAY_ZH : WEEKDAY_EN;
      const idx = (L.weekday || []).indexOf(Math.max(0, ...(L.weekday || [0])));
      return (L.weekday || [0]).some((v) => v > 0) ? names[idx] : '';
    }
    case 'models': {
      const top = [...(L.by_model || [])].sort((a, b) => b.agg.total - a.agg.total)[0];
      return top?.model || '';
    }
    case 'sources': return `${(L.month_by_client || []).length}`;
    case 'projects': {
      const top = [...(L.by_project || [])].sort((a, b) => b.agg.total - a.agg.total)[0];
      return top?.project ? top.project.slice(0, 16) : '';
    }
    case 'alltime': return fmtTokens((L.daily_totals || []).reduce((s, d) => s + d.tokens, 0));
    case 'streak': {
      let cur = 0;
      const days = L.daily_totals || [];
      for (let i = days.length - 1; i >= 0 && days[i].tokens > 0; i--) cur++;
      return `${cur}d`;
    }
    case 'records': {
      const maxDay = (L.daily_totals || []).reduce((m, d) => (d.tokens > (m?.tokens || 0) ? d : m), null);
      return maxDay ? maxDay.date.slice(5) : '';
    }
    case 'cache': {
      const m = L.month;
      return m && m.input + m.cache_read > 0 ? `${Math.round((m.cache_read / (m.input + m.cache_read)) * 100)}%` : '';
    }
    case 'daily': return fmtTokens((L.daily_totals || []).slice(-30).reduce((s, d) => s + d.tokens, 0));
    default: return '';
  }
}

function PanelsPage({ ledger, lang }) {
  const t = (k) => STR[k][lang];
  const gridRef = useRef(null);
  const panelsRef = useRef(null);
  const dragRef = useRef(null);
  const [panels, setPanels] = useState(() => readPanelState()?.panels || DEFAULT_PANELS.map((p) => ({ ...p })));
  const [enabled, setEnabled] = useState(() => readPanelState()?.enabled || PANEL_CATALOG.map((c) => c.id));
  const [drag, setDrag] = useState(null);
  const [align, setAlign] = useState({ v: [], h: [] });
  const [menuOpen, setMenuOpen] = useState(false);
  useEffect(() => { panelsRef.current = panels; }, [panels]);
  useEffect(() => {
    try { window.localStorage.setItem(PANELS_KEY, JSON.stringify({ panels, enabled })); } catch { /* local only */ }
  }, [panels, enabled]);
  useEffect(() => {
    if (!menuOpen) return;
    const close = () => setMenuOpen(false);
    document.addEventListener('click', close);
    return () => document.removeEventListener('click', close);
  }, [menuOpen]);

  const startDrag = (e, id, mode) => {
    const list = panelsRef.current || [];
    const panel = list.find((p) => p.id === id);
    if (!panel || (mode === 'move' && panel.locked)) return;
    e.preventDefault();
    const grid = gridRef.current?.getBoundingClientRect();
    if (!grid) return;
    const colW = (grid.width - (PANEL_COLS - 1) * PANEL_GAP) / PANEL_COLS;
    dragRef.current = { id, mode, panel, startX: e.clientX, startY: e.clientY, colW, stepY: PANEL_ROW_H + PANEL_GAP };
    setDrag({ id, mode, dx: 0, dy: 0, swapWith: null, w: panel.w, h: panel.h, nx: panel.x, ny: panel.y });
  };

  useEffect(() => {
    if (!drag) return;
    const onMove = (e) => {
      const d = dragRef.current;
      if (!d) return;
      const list = panelsRef.current || [];
      const dx = e.clientX - d.startX;
      const dy = e.clientY - d.startY;
      if (d.mode === 'move') {
        // 指针所在格位;拖拽卡跟手浮起,其余卡不动,仅当重叠时让一张卡平移到原位(iOS 式让位)
        const nx = Math.max(0, Math.min(PANEL_COLS - d.panel.w, d.panel.x + Math.round(dx / (d.colW + PANEL_GAP))));
        const ny = Math.max(0, d.panel.y + Math.round(dy / d.stepY));
        const target = { ...d.panel, x: nx, y: ny };
        const overlapping = list.filter((p) => p.id !== d.id && !p.locked && panelOverlap(target, p));
        const swapWith = overlapping.length ? overlapping.reduce((best, p) => (panelOverlapArea(target, p) > panelOverlapArea(target, best) ? p : best), overlapping[0]).id : null;
        setAlign(collectAlignLines(list, d.id, nx, ny));
        setDrag((cur) => (cur ? { ...cur, dx, dy, nx, ny, swapWith } : cur));
      } else {
        const nw = Math.max(1, Math.min(PANEL_COLS - d.panel.x, d.panel.w + Math.round(dx / (d.colW + PANEL_GAP))));
        const nh = Math.max(1, Math.min(10, d.panel.h + Math.round(dy / d.stepY)));
        setDrag((cur) => (cur ? { ...cur, w: nw, h: nh } : cur));
      }
    };
    const onUp = () => {
      const d = dragRef.current;
      const cur = drag;
      if (d && cur) {
        const list = panelsRef.current || [];
        if (d.mode === 'move') {
          const target = { ...d.panel, x: cur.nx, y: cur.ny };
          setPanels(resolvePanelMove(list, d.id, target));
        } else {
          setPanels(compactPanels(list.map((p) => (p.id === d.id ? { ...p, w: cur.w, h: cur.h } : p))));
        }
      }
      dragRef.current = null;
      setDrag(null);
      setAlign({ v: [], h: [] });
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  }, [drag]);

  const removePanel = (id) => {
    setEnabled((es) => es.filter((x) => x !== id));
    setPanels((ps) => ps.filter((p) => p.id !== id));
  };
  const addPanel = (id) => {
    if (enabled.includes(id)) { removePanel(id); return; }
    if (enabled.length >= PANELS_MAX) return;
    setEnabled((es) => [...es, id]);
    const cat = PANEL_CATALOG.find((c) => c.id === id);
    const maxBottom = Math.max(0, ...(panelsRef.current || []).map((p) => p.y + p.h));
    setPanels((ps) => compactPanels([...ps.filter((p) => p.id !== id), { id, x: 0, y: maxBottom, w: cat.w, h: cat.h, locked: false }]));
  };
  const toggleLock = (id) => setPanels((ps) => ps.map((p) => (p.id === id ? { ...p, locked: !p.locked } : p)));
  const allLocked = panels.length > 0 && panels.every((p) => p.locked);
  const toggleAllLock = () => setPanels((ps) => ps.map((p) => ({ ...p, locked: !allLocked })));
  const resetLayout = () => {
    setPanels(DEFAULT_PANELS.map((p) => ({ ...p })));
    setEnabled(PANEL_CATALOG.map((c) => c.id));
  };

  const origin = drag ? panels.find((p) => p.id === drag.id) : null;
  const renderList = panels
    .filter((p) => enabled.includes(p.id))
    .map((p) => {
      if (drag?.mode === 'move' && drag.swapWith && p.id === drag.swapWith && origin) {
        return { ...p, x: origin.x, y: origin.y };
      }
      if (drag?.mode === 'resize' && p.id === drag.id) {
        return { ...p, w: drag.w, h: drag.h };
      }
      return p;
    });
  const bottomPanels = drag?.mode === 'move' && origin ? renderList.filter((p) => p.id !== drag.id) : renderList;
  const maxBottom = Math.max(3, ...renderList.map((p) => p.y + p.h));
  const lockTitle = lang === 0 ? '锁定/解锁拖拽' : 'Lock/unlock';
  const removeTitle = lang === 0 ? '移除面板' : 'Remove panel';
  const posStyle = (p) => ({
    '--x': p.x, '--y': p.y, '--w': p.w, '--h': p.h,
  });
  return (
    <div className="panel-page">
      <div className="panel-tools">
        <span className="panel-hint">{`${renderList.length}/${PANELS_MAX}`}</span>
        <button type="button" className="panel-tool" onClick={toggleAllLock}>{allLocked ? t('panels_unlock_all') : t('panels_lock_all')}</button>
        <button type="button" className="panel-tool" onClick={resetLayout}>{t('panels_reset')}</button>
        <div className="panel-add" onClick={(e) => e.stopPropagation()}>
          <button type="button" className="panel-tool panel-tool-primary" onClick={() => setMenuOpen((o) => !o)}>+ {t('panels_add')}</button>
          {menuOpen && (
            <div className="panel-menu">
              {PANEL_CATALOG.map((c) => {
                const on = enabled.includes(c.id);
                return (
                  <button key={c.id} type="button" className={`panel-menu-item${on ? ' is-on' : ''}`} onClick={() => addPanel(c.id)} disabled={!on && enabled.length >= PANELS_MAX}>
                    <span className="panel-menu-copy"><b>{t(`panel_${c.id}`)}</b><small>{t(`catalog_${c.id}`)}</small></span>
                    <i className={`panel-menu-dot${on ? ' is-on' : ''}`} />
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>
      <div ref={gridRef} className="panel-grid" style={{ height: maxBottom * PANEL_ROW_H + (maxBottom - 1) * PANEL_GAP }}>
        {align.v.map((v, i) => <i key={`v${i}`} className="panel-align-v" style={{ left: `calc(${(v / PANEL_COLS) * 100}% + ${v * PANEL_GAP}px)` }} />)}
        {align.h.map((y, i) => <i key={`h${i}`} className="panel-align-h" style={{ top: y * (PANEL_ROW_H + PANEL_GAP) }} />)}
        <PanelErrorBoundary>
          {drag?.mode === 'move' && origin && <div className="panel-ghost" style={posStyle(origin)} />}
          {bottomPanels.map((p) => {
            const Body = PANEL_BODIES[p.id];
            return (
              <div key={p.id} className={`panel-card${drag?.mode === 'resize' && drag.id === p.id ? ' is-resizing' : ''}${p.locked ? ' is-locked' : ''}`} style={posStyle(p)}>
                <div className="panel-head" onPointerDown={(e) => startDrag(e, p.id, 'move')}>
                  <span className="panel-title">{t(`panel_${p.id}`)}</span>
                  <span className="panel-meta">{panelMeta(p.id, ledger, lang)}</span>
                  <span className="panel-head-btns" onPointerDown={(e) => e.stopPropagation()}>
                    <button type="button" className={`panel-btn${p.locked ? ' is-on' : ''}`} onClick={() => toggleLock(p.id)} title={lockTitle} aria-label={lockTitle}>
                      <svg viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="9" rx="2" /><path d="M8 11V7a4 4 0 0 1 8 0v4" /></svg>
                    </button>
                    <button type="button" className="panel-btn" onClick={() => removePanel(p.id)} title={removeTitle} aria-label={removeTitle}>×</button>
                  </span>
                </div>
                <div className="panel-body">{ledger ? <Body ledger={ledger} lang={lang} /> : null}</div>
                <div className="panel-resize" onPointerDown={(e) => startDrag(e, p.id, 'resize')} aria-hidden="true" />
              </div>
            );
          })}
          {drag && origin && (() => {
            const Body = PANEL_BODIES[origin.id];
            return (
              <div className={`panel-card is-dragging${drag.mode === 'resize' ? ' is-resizing' : ''}`} style={{ ...posStyle(origin), transform: `translate(${drag.dx}px, ${drag.dy}px) scale(1.025)` }}>
                <div className="panel-head">
                  <span className="panel-title">{t(`panel_${origin.id}`)}</span>
                  <span className="panel-meta">{panelMeta(origin.id, ledger, lang)}</span>
                </div>
                <div className="panel-body">{ledger ? <Body ledger={ledger} lang={lang} /> : null}</div>
              </div>
            );
          })()}
          {renderList.length === 0 && <div className="panel-empty">{t('no_data')}</div>}
        </PanelErrorBoundary>
      </div>
    </div>
  );
}

/* ---------------- App ---------------- */

function SourceWelcome({ data, lang, onDone }) {
  const t = (key) => STR[key][lang];
  const firstButton = useRef(null);
  useEffect(() => { firstButton.current?.focus(); }, []);
  const sources = data.ledger?.sources || [];
  const cards = data.cards || [];
  return <div className="welcome-backdrop"><section className="welcome-panel" role="dialog" aria-modal="true" aria-labelledby="welcome-title">
    <div className="welcome-head"><div><span className="eyebrow">TOKEN MONITOR</span><h1 id="welcome-title">{t('welcome_title')}</h1><p>{t('welcome_intro')}</p></div><button ref={firstButton} type="button" onClick={onDone} aria-label={t('welcome_done')}>×</button></div>
    <h2>{t('freshness_quota')}</h2>
    <div className="welcome-source-list">{cards.map((card) => <div className="welcome-source" key={card.provider}>
      <div><span className={`source-indicator source-indicator-${card.kind === 'Live' ? 'ready' : 'error'}`} /><strong>{card.title}</strong><span>{card.kind === 'Live' ? t('source_ready') : t('missing')}</span></div>
      <a href={`#repair-quota-${card.provider}`} className="welcome-repair-link" onClick={(event) => { event.preventDefault(); event.currentTarget.nextElementSibling.open = true; }}>{t('welcome_repair')}</a>
      <details id={`repair-quota-${card.provider}`}><summary>{lang === 0 ? '修复步骤' : 'Repair steps'}</summary><p>{QUOTA_REPAIR[card.provider]?.[lang] || card.hint}</p></details>
    </div>)}</div>
    <h2>{t('freshness_local')}</h2>
    <div className="welcome-source-list">{sources.map((source) => <div className="welcome-source" key={source.client}>
      <div><span className={`source-indicator source-indicator-${source.error ? 'error' : source.records > 0 ? 'ready' : 'idle'}`} /><strong>{source.label}</strong><span>{source.error ? t('source_error') : source.records ? `${source.records} ${t('source_ready')}` : t('source_idle')}</span></div>
      <small>{SOURCE_PATHS[source.client]}</small>
      <a href={`#repair-local-${source.client}`} className="welcome-repair-link" onClick={(event) => { event.preventDefault(); event.currentTarget.nextElementSibling.open = true; }}>{t('welcome_repair')}</a>
      <details id={`repair-local-${source.client}`}><summary>{lang === 0 ? '修复步骤' : 'Repair steps'}</summary><p>{SOURCE_REPAIR[source.client]?.[lang]}</p>{source.error && <p>{source.error}</p>}</details>
    </div>)}</div>
    <button className="welcome-done" type="button" onClick={onDone}>{t('welcome_done')}</button>
  </section></div>;
}

const GUI_PREFERENCES_KEY = 'token-monitor.gui-preferences.v1';
const WELCOME_DONE_KEY = 'token-monitor.welcome-done.v1';
const THEME_ORDER = ['system', 'light', 'dark', 'ink', 'sand'];
const DEFAULT_GUI_PREFERENCES = {
  lang: 0,
  theme: 'system',
  typeface: 'plex',
  density: 'comfortable',
  motion: 'system',
  notifications: 'on',
  paceAlerts: false,
  quietHours: false,
  closeBehavior: 'tray',
  globalShortcut: false,
  autostart: false,
  pinnedCards: [],
  hiddenCards: [],
  providerColors: {},
};

function readGuiPreferences() {
  try {
    const stored = JSON.parse(window.localStorage.getItem(GUI_PREFERENCES_KEY) || '{}');
    return {
      lang: stored.lang === 1 ? 1 : 0,
      theme: THEME_ORDER.includes(stored.theme) ? stored.theme : DEFAULT_GUI_PREFERENCES.theme,
      typeface: ['plex', 'pixel', 'system'].includes(stored.typeface) ? stored.typeface : DEFAULT_GUI_PREFERENCES.typeface,
      density: ['comfortable', 'compact'].includes(stored.density) ? stored.density : DEFAULT_GUI_PREFERENCES.density,
      motion: ['system', 'reduced'].includes(stored.motion) ? stored.motion : DEFAULT_GUI_PREFERENCES.motion,
      notifications: ['on', 'off'].includes(stored.notifications) ? stored.notifications : DEFAULT_GUI_PREFERENCES.notifications,
      paceAlerts: stored.paceAlerts === true,
      quietHours: stored.quietHours === true,
      closeBehavior: ['tray', 'exit'].includes(stored.closeBehavior) ? stored.closeBehavior : 'tray',
      globalShortcut: stored.globalShortcut === true,
      autostart: stored.autostart === true,
      pinnedCards: Array.isArray(stored.pinnedCards) ? stored.pinnedCards.filter((item) => PROVIDERS.includes(item)) : [],
      hiddenCards: Array.isArray(stored.hiddenCards) ? stored.hiddenCards.filter((item) => PROVIDERS.includes(item)) : [],
      providerColors: Object.fromEntries(Object.entries(stored.providerColors || {}).filter(([key, value]) => PROVIDERS.includes(key) && /^#[0-9a-fA-F]{6}$/.test(value))),
    };
  } catch {
    return DEFAULT_GUI_PREFERENCES;
  }
}

function App() {
  const [preferences, setPreferences] = useState(readGuiPreferences);
  const [showWelcome, setShowWelcome] = useState(() => { try { return window.localStorage.getItem(WELCOME_DONE_KEY) !== '1'; } catch { return true; } });
  const [nativeError, setNativeError] = useState('');
  const [goKeyStored, setGoKeyStored] = useState(false);
  const [goKeyFeedback, setGoKeyFeedback] = useState('');
  const [goatKeyStored, setGoatKeyStored] = useState(false);
  const [goatKeyFeedback, setGoatKeyFeedback] = useState('');
  const lang = preferences.lang;
  const [page, setPage] = useState('quota');
  const [data, setData] = useState(null);
  const [refreshing, setRefreshing] = useState(false);
  const [now, setNow] = useState(Date.now);
  const dataRef = useRef(null);
  const refreshRequestedAt = useRef(0);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 5_000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    window.__onPush = (payload) => {
      dataRef.current = payload;
      setData(payload);
      if (!refreshRequestedAt.current || (payload.fetched_at_ms >= refreshRequestedAt.current && payload.quota_checked_at_ms >= refreshRequestedAt.current)) {
        refreshRequestedAt.current = 0;
        setRefreshing(false);
      }
    };
    window.__showSettings = () => setPage('settings');
    window.__onNativeState = (state) => {
      setGoKeyStored(!!state.goKeyStored);
      setGoatKeyStored(!!state.goatKeyStored);
      setPreferences((current) => {
        const next = { ...current, autostart: !!state.autostart };
        try { window.localStorage.setItem(GUI_PREFERENCES_KEY, JSON.stringify(next)); } catch { /* local preference only */ }
        return next;
      });
    };
    window.__onAutostartResult = (success) => {
      if (!success) {
        setNativeError('Windows 启动项写入失败 / Failed to update Windows startup');
        window.ipc?.postMessage?.(JSON.stringify({ cmd: 'queryNativePreferences' }));
      } else setNativeError('');
    };
    window.__onGoKeyResult = ({ success, stored }) => {
      setGoKeyStored(!!stored);
      setGoKeyFeedback(success ? '' : STR.settings_go_key_failed[readGuiPreferences().lang]);
    };
    window.__onGoatKeyResult = ({ success, stored }) => {
      setGoatKeyStored(!!stored);
      setGoatKeyFeedback(success ? '' : STR.settings_go_key_failed[readGuiPreferences().lang]);
    };
    window.__onShortcutStatus = (available) => {
      if (!available) {
        setNativeError('Ctrl+Alt+U 已被其他应用占用 / Ctrl+Alt+U is already in use');
        setPreferences((current) => {
          const next = { ...current, globalShortcut: false };
          try { window.localStorage.setItem(GUI_PREFERENCES_KEY, JSON.stringify(next)); } catch { /* local preference only */ }
          return next;
        });
      } else setNativeError('');
    };
    window.ipc?.postMessage?.(JSON.stringify({ cmd: 'queryNativePreferences' }));
    return () => { delete window.__onPush; delete window.__showSettings; delete window.__onNativeState; delete window.__onAutostartResult; delete window.__onGoKeyResult; delete window.__onGoatKeyResult; delete window.__onShortcutStatus; };
  }, []);

  const refresh = useCallback(() => {
    refreshRequestedAt.current = Date.now();
    setRefreshing(true);
    // wry 在 WebView2 上注入 window.ipc.postMessage(见 wry webview2/mod.rs 注入脚本)
    window.ipc?.postMessage?.(JSON.stringify({ cmd: 'refresh' }));
  }, []);

  const updatePreference = useCallback((key, value) => {
    if (key === 'autostart') window.ipc?.postMessage?.(JSON.stringify({ cmd: 'setAutostart', autostart: value }));
    setPreferences((current) => {
      const next = { ...current, [key]: value };
      try { window.localStorage.setItem(GUI_PREFERENCES_KEY, JSON.stringify(next)); } catch { /* Preferences stay available for this session. */ }
      return next;
    });
  }, []);

  const resetAppearance = useCallback(() => {
    setPreferences((current) => {
      const next = { ...DEFAULT_GUI_PREFERENCES, notifications: current.notifications, paceAlerts: current.paceAlerts, quietHours: current.quietHours, closeBehavior: current.closeBehavior, globalShortcut: current.globalShortcut, autostart: current.autostart, pinnedCards: current.pinnedCards, hiddenCards: current.hiddenCards, providerColors: current.providerColors };
      try { window.localStorage.setItem(GUI_PREFERENCES_KEY, JSON.stringify(next)); } catch { /* Preferences stay available for this session. */ }
      return next;
    });
  }, []);

  const resetCards = useCallback(() => {
    setPreferences((current) => {
      const next = { ...current, pinnedCards: [], hiddenCards: [], providerColors: {} };
      try { window.localStorage.setItem(GUI_PREFERENCES_KEY, JSON.stringify(next)); } catch { /* local preference only */ }
      return next;
    });
  }, []);

  const dismissWelcome = useCallback(() => {
    setShowWelcome(false);
    try { window.localStorage.setItem(WELCOME_DONE_KEY, '1'); } catch { /* session only */ }
  }, []);

  const toggleCard = useCallback((key, provider) => {
    setPreferences((current) => {
      const values = current[key].includes(provider) ? current[key].filter((item) => item !== provider) : [...current[key], provider];
      const next = { ...current, [key]: values };
      try { window.localStorage.setItem(GUI_PREFERENCES_KEY, JSON.stringify(next)); } catch { /* local preference only */ }
      return next;
    });
  }, []);

  useEffect(() => {
    window.ipc?.postMessage?.(JSON.stringify({ cmd: 'setNotifications', enabled: preferences.notifications === 'on', paceAlerts: preferences.paceAlerts, quietHours: preferences.quietHours, lang: preferences.lang }));
  }, [preferences.notifications, preferences.paceAlerts, preferences.quietHours, preferences.lang]);

  useEffect(() => {
    window.ipc?.postMessage?.(JSON.stringify({ cmd: 'setCloseBehavior', closeBehavior: preferences.closeBehavior }));
  }, [preferences.closeBehavior]);

  useEffect(() => {
    window.ipc?.postMessage?.(JSON.stringify({ cmd: 'setGlobalShortcut', globalShortcut: preferences.globalShortcut }));
  }, [preferences.globalShortcut]);

  const cycleTheme = useCallback(() => {
    const index = THEME_ORDER.indexOf(preferences.theme);
    updatePreference('theme', THEME_ORDER[(index + 1) % THEME_ORDER.length]);
  }, [preferences.theme, updatePreference]);

  useLayoutEffect(() => {
    if (preferences.theme === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = ['dark', 'ink'].includes(preferences.theme) ? 'dark' : 'light';
    document.documentElement.lang = lang === 0 ? 'zh-CN' : 'en';
  }, [preferences.theme, lang]);

  useEffect(() => {
    const onKeyDown = (event) => {
      const target = event.target;
      if (event.altKey || event.ctrlKey || event.metaKey || target?.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName || '')) return;
      if (event.key === '1') setPage('quota');
      else if (event.key === '2') setPage('usage');
      else if (event.key === '3') setPage('heat');
      else if (event.key === '4') setPage('settings');
      else if (event.key.toLowerCase() === 'r' && !refreshing) refresh();
      else if (event.key.toLowerCase() === 'p') cycleTheme();
      else return;
      event.preventDefault();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [refresh, refreshing, cycleTheme]);

  const t = useCallback((k) => STR[k][lang], [lang]);

  const age = data ? Math.floor((now - data.fetched_at_ms) / 1000) : null;
  const quotaAge = data?.quota_fetched_at_ms ? now - data.quota_fetched_at_ms : null;
  const ledgerAge = data?.fetched_at_ms ? now - data.fetched_at_ms : null;
  const quotaStale = data && (data.quotaError || quotaAge == null || quotaAge > 120_000);
  const ledgerStale = data && (ledgerAge == null || ledgerAge > 20_000);
  const pageContent = page === 'settings' ? (
    <SettingsPage
      lang={lang}
      preferences={preferences}
      onPreferenceChange={updatePreference}
      onResetAppearance={resetAppearance}
      onResetCards={resetCards}
      onShowWelcome={() => setShowWelcome(true)}
      nativeError={nativeError}
      goKeyStored={goKeyStored}
      goKeyFeedback={goKeyFeedback}
      onSaveGoKey={(goApiKey) => { setGoKeyFeedback(''); window.ipc?.postMessage?.(JSON.stringify({ cmd: 'setGoApiKey', goApiKey })); }}
      onDeleteGoKey={() => { setGoKeyFeedback(''); window.ipc?.postMessage?.(JSON.stringify({ cmd: 'deleteGoApiKey' })); }}
      goatKeyStored={goatKeyStored}
      goatKeyFeedback={goatKeyFeedback}
      onSaveGoatKey={(goatApiKey) => { setGoatKeyFeedback(''); window.ipc?.postMessage?.(JSON.stringify({ cmd: 'setGoatApiKey', goatApiKey })); }}
      onDeleteGoatKey={() => { setGoatKeyFeedback(''); window.ipc?.postMessage?.(JSON.stringify({ cmd: 'deleteGoatApiKey' })); }}
      monthlyValue={data?.monthly_value}
    />
  ) : data ? (
    <>
      {page === 'quota' && (data.cards?.length ? <QuotaPage cards={data.cards} sources={data.ledger?.sources} ledger={data.ledger} goatUsage={data.goat_usage} quotaFetchedAtMs={data.quota_fetched_at_ms} quotaStale={quotaStale} now={now} lang={lang} preferences={preferences} onTogglePin={(provider) => toggleCard('pinnedCards', provider)} onToggleHidden={(provider) => toggleCard('hiddenCards', provider)} onOpenSettings={() => setPage('settings')} /> : <Card className="empty-card"><Text type="supporting">{t('empty_quota')}</Text></Card>)}
      {page === 'usage' && <UsagePage ledger={data.ledger} cards={data.cards} monthlyValue={data.monthly_value} lang={lang} typeface={preferences.typeface} onOpenSettings={() => setPage('settings')} />}
      {page === 'heat' && <PanelsPage ledger={data.ledger} lang={lang} />}
    </>
  ) : null;

  return (
    <div
      className="app"
      data-astryx-theme="neutral"
      data-language={lang === 0 ? 'zh' : 'en'}
      data-typeface={preferences.typeface}
      data-palette={preferences.theme}
      data-density={preferences.density}
      data-motion={preferences.motion}
    >
      <aside className="sidebar">
        <div className="sidebar-title">
          <span className="logo-block" />
          <Text weight="medium">{t('title')}</Text>
        </div>
        <PrimaryNav lang={lang} page={page} onNavigate={setPage} variant="desktop" />
        <div className="sidebar-foot">
          <div className="sidebar-status">
            {refreshing ? (
              <><Spinner size="sm" /> <span>{t('refreshing')}</span></>
            ) : (
              <span className="sidebar-age">
                {t('updated')} {age != null ? fmtRel(age * 1000) : '—'}
              </span>
            )}
          </div>
        </div>
      </aside>

      <main className="main">
        <div className="main-toolbar">
          <Text weight="medium" className="page-title">{t(page === 'quota' ? 'nav_quota' : page === 'usage' ? 'nav_usage' : page === 'heat' ? 'nav_heat' : 'nav_settings')}</Text>
          <div className="main-toolbar-spacer" />
          <button className="toolbar-theme" type="button" onClick={cycleTheme} title={`${t('theme_quick')} (P)`} aria-label={`${t('theme_quick')}: ${t(`theme_${preferences.theme}`)}`}>
            <span className={`toolbar-theme-swatch toolbar-theme-${preferences.theme}`} aria-hidden="true" />
            <span>{t(`theme_${preferences.theme}`)}</span>
          </button>
          <Button size="sm" onClick={refresh} disabled={refreshing}>
            {t('refresh')}
          </Button>
        </div>
        {page !== 'settings' && data && <div className="freshness-strip" role="status">
          <span className={quotaStale ? 'freshness-badge is-stale' : 'freshness-badge'} title={data.quota_fetched_at_ms ? new Date(data.quota_fetched_at_ms).toLocaleString() : undefined}><i />{t('freshness_quota')} · {quotaStale && `${t('freshness_stale')} · `}{quotaAge == null ? t('freshness_waiting') : `${t('freshness_last_success')} ${fmtRel(quotaAge)}`}</span>
          <span className={ledgerStale ? 'freshness-badge is-stale' : 'freshness-badge'} title={data.fetched_at_ms ? new Date(data.fetched_at_ms).toLocaleString() : undefined}><i />{t('freshness_local')} · {ledgerStale && `${t('freshness_stale')} · `}{ledgerAge == null ? t('freshness_waiting') : `${t('freshness_last_success')} ${fmtRel(ledgerAge)}`}</span>
        </div>}
        {page !== 'settings' && data?.quotaError && (
          <div className="banner-error">
            <Toast kind="error">{t('quota_error')}: {data.quotaError}</Toast>
          </div>
        )}
        {page !== 'settings' && !data ? <Card className="empty-card" role="status"><Spinner size="sm" /> <Text type="supporting">{t('loading')}</Text></Card> : (
          <Suspense fallback={<div className="page-content">{pageContent}</div>}>
            <PageTransition page={page} reduceMotion={preferences.motion === 'reduced'}>{pageContent}</PageTransition>
          </Suspense>
        )}
      </main>
      <PrimaryNav lang={lang} page={page} onNavigate={setPage} variant="mobile" />
      {showWelcome && data?.ledger && data.quota_checked_at_ms > 0 && <SourceWelcome data={data} lang={lang} onDone={dismissWelcome} />}
    </div>
  );
}

window.addEventListener('error', (e) => {
  document.title = `E ${String(e.message).slice(0, 90)} @${e.lineno}:${e.colno}`;
});
window.addEventListener('unhandledrejection', () => {
  document.title = 'E promise';
});

createRoot(document.getElementById('root')).render(<App />);
