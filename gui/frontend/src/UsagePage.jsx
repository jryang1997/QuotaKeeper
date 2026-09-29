import React, { lazy, Suspense, useEffect, useRef, useState } from 'react';
import { Card } from '@astryxdesign/core/Card';
import { Text } from '@astryxdesign/core/Text';
import { Tooltip } from '@astryxdesign/core/Tooltip';
import { SegmentedControl, SegmentedControlItem } from '@astryxdesign/core/SegmentedControl';
import { CLIENT_COLORS, STR, fmtInt, fmtTokens, fmtCost, fmtRel, fmtClock } from './ui-data.js';
import BlockBar from './BlockBar.jsx';
const AnalysisPanel = lazy(() => import('./AnalysisPanel.jsx'));

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

export default function UsagePage({ ledger, cards, monthlyValue, lang, onOpenSettings, typeface }) {
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
