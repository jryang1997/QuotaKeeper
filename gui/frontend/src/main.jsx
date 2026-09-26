import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
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
import { Tooltip } from '@astryxdesign/core/Tooltip';
import { Toast } from '@astryxdesign/core/Toast';

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

/* ---------------- 双语 ---------------- */

const STR = {
  title: ['Token 监控', 'Token Monitor'],
  nav_quota: ['配额', 'Quota'],
  nav_usage: ['Token 用量', 'Token Usage'],
  nav_heat: ['热力图', 'Heatmap'],
  refresh: ['刷新', 'Refresh'],
  refreshing: ['采集中…', 'Fetching…'],
  updated: ['更新于', 'Updated'],
  quota_error: ['配额采集失败(用量页不受影响)', 'Quota fetch failed (usage pages unaffected)'],
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
  theme: ['主题', 'Theme'],
  lang: ['语言', 'Lang'],
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
  sources: ['数据源', 'Sources'],
  missing: ['未接入', 'Not connected'],
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

function DotMatrix({ text, size = 42 }) {
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
    <div className="dotmatrix" style={{ '--cell': `${cell}px` }}>
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

/* ---------------- 配额卡片 ---------------- */

function QuotaCards({ cards, lang }) {
  const t = (k) => STR[k][lang];
  return (
    <div className="quota-grid">
      {cards.map((c) => {
        const accent = PROVIDER_ACCENT[c.provider] || 'var(--color-text-primary)';
        return (
          <Card key={c.provider} className="quota-card" role="group" aria-label={c.title}>
            <div className="quota-head">
              <span className="quota-dot" style={{ background: accent }} />
              <Text weight="medium" className="quota-title">{c.title}</Text>
            </div>
            {c.kind === 'Live' ? (
              <div className="quota-limits">
                {c.limits.map((l) => (
                  <div key={l.label} className="quota-limit">
                    <div className="quota-limit-row">
                      <span className="quota-limit-label">{l.label}</span>
                      <span className="quota-limit-pct" style={{ color: STATUS_COLOR[l.status] }}>
                        {l.used_pct >= 100 ? '100%' : `${Math.round(l.used_pct)}%`}
                      </span>
                    </div>
                    <div className="quota-bar">
                      <div
                        className="quota-bar-fill"
                        style={{
                          width: `${Math.min(100, l.used_pct)}%`,
                          background: STATUS_COLOR[l.status],
                          transition: 'width 700ms cubic-bezier(0.22,1,0.36,1)',
                        }}
                      />
                    </div>
                    <div className="quota-limit-sub">
                      {l.resets_in_ms != null ? `${t('resets_in')} ${fmtCountdown(l.resets_in_ms)}` : ' '}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="quota-missing">
                <Text variant="supporting" className="quota-missing-hint">{c.hint}</Text>
              </div>
            )}
          </Card>
        );
      })}
    </div>
  );
}

/* ---------------- 用量页 ---------------- */

function tightestQuota(cards) {
  const limits = cards
    .filter((card) => card.kind === 'Live' && card.provider !== 'antigravity')
    .flatMap((card) => card.limits.map((limit) => ({ provider: card.title.split(' ·')[0], ...limit })));
  return limits.sort((a, b) => b.used_pct - a.used_pct)[0] || null;
}

function UsagePage({ ledger, cards, lang }) {
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
  const selected = !hasTrend || selectedBucket == null ? null : series[selectedBucket];
  const legend = [...new Set(series.flatMap((b) => (b.per_client || []).map(([client]) => client)))];

  const dimRows =
    dim === 'agent' ? ledger.by_agent.map((a) => ({ label: a.label, color: CLIENT_COLORS[a.client] || 'var(--color-text-secondary)', agg: a.agg, key: a.client }))
    : dim === 'session' ? ledger.by_session.map((s) => ({ label: s.session, color: CLIENT_COLORS[s.client] || 'var(--color-text-secondary)', agg: s.agg, key: `${s.client}:${s.session}` }))
    : ledger.by_project.map((p) => ({ label: p.project, color: 'var(--color-text-secondary)', agg: p.agg, key: p.project }));

  const dimTotal = dimRows.reduce((s, r) => s + r.agg.total, 0) || 1;

  return (
    <div className="usage-page">
      <Card className="now-card">
        <div className="now-main">
          <Text variant="supporting">{t('now')} · {t('recent_hour')}</Text>
          <strong>{recent ? `${recent.label} ${fmtTokens(recent.agg.total)}` : t('no_data')}</strong>
          <span className="now-sub">{ledger.burn_rate != null ? `${Math.round(ledger.burn_rate)} ${t('burn_unit')}` : '—'}</span>
        </div>
        <div className="now-quota">
          <Text variant="supporting">{t('quota')}</Text>
          <strong>{quota ? `${quota.provider} ${Math.round(quota.used_pct)}%` : '—'}</strong>
          <span className="now-sub">{quota?.label || ''}</span>
        </div>
      </Card>
      <div className="stat-row">
        <Card className="stat-card">
          <Text variant="supporting">{t('today')}</Text>
          <DotMatrix text={fmtTokens(ledger.today.total)} size={34} />
          <div className="stat-sub">
            <span>{fmtCost(ledger.today.cost)}</span>
            <span>·</span>
            <span>{fmtInt(ledger.today.messages)} {t('messages')}</span>
          </div>
          <MiniBars agg={ledger.today} />
        </Card>
        <Card className="stat-card">
          <Text variant="supporting">{t('week')}</Text>
          <DotMatrix text={fmtTokens(ledger.week.total)} size={34} />
          <div className="stat-sub">
            <span>{fmtCost(ledger.week.cost)}</span>
          </div>
        </Card>
        <Card className="stat-card">
          <Text variant="supporting">{t('month')}</Text>
          <DotMatrix text={fmtTokens(ledger.month.total)} size={34} />
          <div className="stat-sub">
            <span>{fmtCost(ledger.month.cost)}</span>
          </div>
        </Card>
      </div>

      <Card className="trend-card">
        <div className="trend-toolbar">
          <Text weight="medium">{range === '24h' ? t('range_24h') : t('range_14d')}</Text>
          <div className="trend-controls">
            <SegmentedControl
              size="small"
              value={range}
              onChange={(next) => { setRange(next); setSelectedBucket(null); }}
              options={[
                { label: t('range_24h'), value: '24h' },
                { label: t('range_14d'), value: '14d' },
              ]}
            />
            <SegmentedControl
              size="small"
              value={metric}
              onChange={(next) => { setMetric(next); setSelectedBucket(null); }}
              options={[
                { label: t('metric_tokens'), value: 'tokens' },
                { label: t('metric_cost'), value: 'cost' },
              ]}
            />
          </div>
        </div>
        {!hasTrend ? <Text variant="supporting">{metric === 'cost' ? t('no_data') : t('empty_trend')}</Text> : <div className="trend-chart">
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
        {hasTrend && metric === 'cost' && <Text variant="supporting">{t('aggregate')}</Text>}
        {selected && <div className="trend-detail">
          <strong>{periodLabel(selected)} · {metric === 'tokens' ? fmtTokens(selected.agg.total) : fmtCost(selected.agg.cost)}</strong>
          <span>{(selected.per_client || []).slice(0, 3).map(([client, agg]) => `${client} ${metric === 'tokens' ? fmtTokens(agg.total) : fmtCost(agg.cost)}`).join(' · ')}</span>
        </div>}
      </Card>

      <div className="usage-columns">
        <Card className="dim-card">
          <div className="trend-toolbar">
            <Text weight="medium">{t('by_agent')} · 30d</Text>
            <SegmentedControl
              size="small"
              value={dim}
              onChange={setDim}
              options={[
                { label: t('dim_agent'), value: 'agent' },
                { label: t('dim_session'), value: 'session' },
                { label: t('dim_project'), value: 'project' },
              ]}
            />
          </div>
          <div className="dim-rows">
            {dimRows.slice(0, 12).map((r) => (
              <div key={r.key} className="dim-row">
                <span className="dim-dot" style={{ background: r.color }} />
                <span className="dim-label" title={r.label}>{r.label}</span>
                <div className="dim-bar">
                  <div className="dim-bar-fill" style={{ width: `${(r.agg.total / dimTotal) * 100}%`, background: r.color }} />
                </div>
                <span className="dim-value">{fmtTokens(r.agg.total)}</span>
              </div>
            ))}
          </div>
        </Card>

        <Card className="recent-card">
          <Text weight="medium">{t('recent_hour')}</Text>
          {ledger.recent_by_agent.length === 0 ? (
            <Text variant="supporting">{t('no_data')}</Text>
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

/* ---------------- 热力图页 ---------------- */

const HEAT_LEVELS = [
  'var(--color-border)',
  'var(--heat-1)',
  'var(--heat-2)',
  'var(--heat-3)',
  'var(--heat-4)',
];

function heatLevel(tokens) {
  if (tokens <= 0) return 0;
  if (tokens < 100e3) return 1;
  if (tokens < 400e3) return 2;
  if (tokens < 1.5e6) return 3;
  return 4;
}

function HeatmapPage({ ledger, lang }) {
  const t = (k) => STR[k][lang];
  const [sel, setSel] = useState(null);
  if (!ledger) return null;
  const weeks = [];
  const totals = ledger.daily_totals;
  // 末尾对齐:最后一天为终点,往前切 26 周,起点对齐周一
  const days = [...totals];
  while (days.length % 7 !== 0) days.unshift({ date: '', tokens: 0, cost: 0, empty: true });
  for (let i = 0; i < days.length; i += 7) weeks.push(days.slice(i, i + 7));
  const detail = sel ? (ledger.daily_detail?.[sel] || null) : null;
  const selTotal = totals.find((d) => d.date === sel);

  return (
    <div className="heat-page">
      {totals.every((day) => day.tokens <= 0) ? <Card className="empty-card"><Text variant="supporting">{t('no_data')}</Text></Card> : <Card className="heat-card">
        <div className="heat-weeks" role="group" aria-label={t('nav_heat')}>
          {weeks.map((w, wi) => (
            <div key={wi} className="heat-col">
              {w.map((d, di) =>
                d.empty || !d.date ? (
                  <div key={di} className="heat-cell heat-cell-empty" />
                ) : (
                  <Tooltip key={di} content={`${d.date} · ${fmtTokens(d.tokens)} / ${fmtCost(d.cost)}`}>
                    <button
                      type="button"
                      className={`heat-cell${sel === d.date ? ' heat-cell-sel' : ''}`}
                      style={{ background: HEAT_LEVELS[heatLevel(d.tokens)] }}
                      onClick={() => setSel((s) => (s === d.date ? null : d.date))}
                      aria-label={`${d.date} · ${fmtTokens(d.tokens)} · ${fmtCost(d.cost)}`}
                      aria-pressed={sel === d.date}
                    />
                  </Tooltip>
                ),
              )}
            </div>
          ))}
        </div>
        <div className="heat-legend">
          <span>{t('by_day')}</span>
          {HEAT_LEVELS.map((c, i) => <span key={i} className="heat-legend-cell" style={{ background: c }} />)}
        </div>
      </Card>}
      {sel && (
        <Card className="heat-detail">
          <div className="trend-toolbar">
            <Text weight="medium">{sel}</Text>
            {selTotal && <Text variant="supporting">{fmtTokens(selTotal.tokens)} · {fmtCost(selTotal.cost)}</Text>}
          </div>
          {detail ? (
            <div className="dim-rows">
              {detail.map(([c, g]) => (
                <div key={c} className="dim-row">
                  <span className="dim-dot" style={{ background: CLIENT_COLORS[c] || 'var(--color-text-secondary)' }} />
                  <span className="dim-label">{c}</span>
                  <span className="dim-value">{fmtTokens(g.total)}</span>
                  <span className="dim-sub">{fmtCost(g.cost)}</span>
                </div>
              ))}
            </div>
          ) : (
            <Text variant="supporting">{t('no_data')}</Text>
          )}
        </Card>
      )}
    </div>
  );
}

/* ---------------- App ---------------- */

function App() {
  const [lang, setLang] = useState(0);
  const [page, setPage] = useState('quota');
  const [data, setData] = useState(null);
  const [refreshing, setRefreshing] = useState(false);
  const dataRef = useRef(null);

  useEffect(() => {
    window.__onPush = (payload) => {
      dataRef.current = payload;
      setData(payload);
      setRefreshing(false);
    };
    return () => { delete window.__onPush; };
  }, []);

  const refresh = useCallback(() => {
    setRefreshing(true);
    // wry 在 WebView2 上注入 window.ipc.postMessage(见 wry webview2/mod.rs 注入脚本)
    window.ipc?.postMessage?.(JSON.stringify({ cmd: 'refresh' }));
  }, []);

  useEffect(() => {
    const onKeyDown = (event) => {
      const target = event.target;
      if (event.altKey || event.ctrlKey || event.metaKey || target?.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName || '')) return;
      if (event.key === '1') setPage('quota');
      else if (event.key === '2') setPage('usage');
      else if (event.key === '3') setPage('heat');
      else if (event.key.toLowerCase() === 'r' && !refreshing) refresh();
      else return;
      event.preventDefault();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [refresh, refreshing]);

  const t = useCallback((k) => STR[k][lang], [lang]);

  const now = Date.now();
  const age = data ? Math.floor((now - data.fetched_at_ms) / 1000) : null;

  return (
    <div className="app" data-astryx-theme="neutral">
      <aside className="sidebar">
        <div className="sidebar-title">
          <span className="logo-block" />
          <Text weight="medium">{t('title')}</Text>
        </div>
        <nav className="sidebar-nav">
          {[
            ['quota', t('nav_quota')],
            ['usage', t('nav_usage')],
            ['heat', t('nav_heat')],
          ].map(([k, label]) => (
            <button
              key={k}
              className={`nav-item${page === k ? ' nav-item-active' : ''}`}
              onClick={() => setPage(k)}
              aria-current={page === k ? 'page' : undefined}
            >
              {label}
            </button>
          ))}
        </nav>
        <div className="sidebar-foot">
          <Button size="small" variant="secondary" onClick={() => setLang((l) => 1 - l)}>
            {lang === 0 ? 'EN' : '中文'}
          </Button>
          <div className="sidebar-status">
            {refreshing ? (
              <><Spinner size="small" /> <span>{t('refreshing')}</span></>
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
          <Text weight="medium" className="page-title">{t(page === 'quota' ? 'nav_quota' : page === 'usage' ? 'nav_usage' : 'nav_heat')}</Text>
          <div className="main-toolbar-spacer" />
          <Button size="small" onClick={refresh} disabled={refreshing}>
            {t('refresh')}
          </Button>
        </div>
        {data?.quotaError && (
          <div className="banner-error">
            <Toast kind="error">{t('quota_error')}: {data.quotaError}</Toast>
          </div>
        )}
        {!data ? <Card className="empty-card" role="status"><Spinner size="small" /> <Text variant="supporting">{t('loading')}</Text></Card> : <>
          {page === 'quota' && (data.cards?.length ? <QuotaCards cards={data.cards} lang={lang} /> : <Card className="empty-card"><Text variant="supporting">{t('empty_quota')}</Text></Card>)}
          {page === 'usage' && <UsagePage ledger={data.ledger} cards={data.cards} lang={lang} />}
          {page === 'heat' && <HeatmapPage ledger={data.ledger} lang={lang} />}
        </>}
      </main>
    </div>
  );
}

createRoot(document.getElementById('root')).render(<App />);
