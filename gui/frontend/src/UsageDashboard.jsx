import React, { useEffect, useMemo, useState } from 'react';
import { CLIENT_COLORS, fmtCost, fmtInt, fmtTokens, fmtRel } from './ui-data.js';
import { registerNativeCallbacks, sendCommand } from './gui-bridge.js';
import { aggregateUsage, localDate, usageBounds, usageMetric } from './usage-data.js';

const TOKEN_PARTS = [
  ['input', '输入', 'Input', '#a3d9f5'],
  ['cache_read', '缓存读取', 'Cache read', '#65b4f5'],
  ['cache_write', '缓存写入', 'Cache write', '#8164eb'],
  ['output', '输出', 'Output', '#2174e8'],
];
const RANGES = [
  ['7d', '近 7 天', 'Last 7 days'], ['30d', '近 30 天', 'Last 30 days'],
  ['today', '今天', 'Today'], ['yesterday', '昨天', 'Yesterday'],
  ['month', '本月', 'This month'], ['custom', '自定义', 'Custom'],
];
const tr = (zh, cn, en) => zh ? cn : en;
const valueText = (value, metric) => metric === 'cost' ? fmtCost(value) : fmtInt(value);
const monthDay = (date) => date?.slice(5).replace('-', '/') || '—';

function SummaryCard({ title, value, note }) {
  return <div className="usage-summary-card"><span>{title}</span><strong>{value}</strong><small>{note}</small></div>;
}

function TrendChart({ days, metric, agent, mode, names, zh }) {
  const [hover, setHover] = useState(null);
  const [pin, setPin] = useState(null);
  useEffect(() => { setHover(null); setPin(null); }, [days, metric, agent, mode]);
  const active = hover ?? pin;
  const selected = active == null ? null : days[active];
  const max = Math.max(1, ...days.map((day) => usageMetric(day.agg, metric)));
  const axisText = (n) => metric === 'tokens' ? fmtTokens(n) : valueText(n, metric);
  const showTypes = !!agent && metric === 'tokens' && mode === 'type';
  const legend = showTypes
    ? TOKEN_PARTS.map(([, cn, en, color]) => [tr(zh, cn, en), color])
    : [...new Set(days.flatMap((day) => [...day.agents.keys()]))].map((client) => [names[client] || client, metric === 'cost' && agent ? '#ff5b20' : CLIENT_COLORS[client] || '#347ded']);
  const detail = selected && (showTypes
    ? TOKEN_PARTS.filter(([key]) => selected.agg[key] > 0).map(([key, cn, en, color]) => [tr(zh, cn, en), selected.agg[key], color])
    : [...selected.agents.entries()].filter(([, agg]) => usageMetric(agg, metric) > 0)
      .sort((a, b) => usageMetric(b[1], metric) - usageMetric(a[1], metric))
      .map(([client, agg]) => [names[client] || client, usageMetric(agg, metric), metric === 'cost' && agent ? '#ff5b20' : CLIENT_COLORS[client] || '#347ded']));
  return <div className="usage-trend" onKeyDown={(event) => { if (event.key === 'Escape') { setHover(null); setPin(null); } }}>
    <div className="usage-plot">
      <div className="usage-y-axis"><span>{axisText(max)}</span><span>{axisText(max / 2)}</span><span>0</span></div>
      <div className="usage-grid-lines"><i /><i /><i /></div>
      <div className="usage-bars" onMouseLeave={() => setHover(null)}>{days.map((day, index) => {
        const amount = usageMetric(day.agg, metric);
        const parts = showTypes
          ? TOKEN_PARTS.map(([key, , , color]) => [key, day.agg[key], color])
          : [...day.agents.entries()].map(([client, agg]) => [client, usageMetric(agg, metric), metric === 'cost' && agent ? '#ff5b20' : CLIENT_COLORS[client] || '#347ded']);
        return <button type="button" key={day.date} className={`usage-day${active === index ? ' is-active' : ''}`}
          onMouseEnter={() => setHover(index)} onFocus={() => setHover(index)} onBlur={() => setHover(null)}
          onClick={() => setPin((current) => current === index ? null : index)}
          aria-label={`${day.date} · ${valueText(amount, metric)}`} aria-pressed={pin === index}>
          <span className="usage-day-bar" style={{ height: amount > 0 ? `${Math.max(3, amount / max * 100)}%` : 0 }}>
            {parts.filter(([, value]) => value > 0).map(([key, value, color]) => <i key={key} style={{ flexGrow: value, background: color }} />)}
          </span>
        </button>;
      })}</div>
      {selected && <div className="usage-guide" style={{ left: `calc(48px + (100% - 48px) * ${(active + 0.5) / days.length})` }} />}
      {selected && <div className="usage-popover" style={{ left: `clamp(156px, ${(active + 0.5) / days.length * 100}%, calc(100% - 156px))` }} role="status">
        <div className="usage-popover-head"><strong>{selected.date}</strong><strong>{valueText(usageMetric(selected.agg, metric), metric)}</strong></div>
        {detail.map(([name, amount, color]) => <div className="usage-popover-line" key={name}><i style={{ background: color }} /><span>{name}</span><strong>{valueText(amount, metric)}</strong></div>)}
        {detail.length === 0 && <small>{tr(zh, '此日暂无记录', 'No records for this day')}</small>}
        {metric === 'cost' && selected.unpriced > 0 && <small>{tr(zh, `${selected.unpriced} 条记录缺价`, `${selected.unpriced} records without pricing`)}</small>}
        {showTypes && selected.agg.reasoning > 0 && <small>{tr(zh, '推理 Token 已包含在输出中', 'Reasoning is included in output')}</small>}
      </div>}
    </div>
    <div className="usage-x-axis"><span>{monthDay(days[0]?.date)}</span><span>{days.length > 2 ? monthDay(days[Math.floor(days.length / 2)]?.date) : ''}</span><span>{monthDay(days.at(-1)?.date)}</span></div>
    {legend.length > 0 && <div className="usage-chart-legend">{legend.map(([name, color]) => <span key={name}><i style={{ background: color }} />{name}</span>)}</div>}
    {!days.some((day) => usageMetric(day.agg, metric) > 0) && <div className="usage-empty">{tr(zh, '所选范围暂无记录', 'No records in this range')}</div>}
  </div>;
}

function ModelExplorer({ ledger, zh }) {
  const models = (ledger.by_model || []).slice(0, 7);
  const [selectedName, setSelectedName] = useState('');
  const selected = models.find((item) => item.model === selectedName) || models[0];
  const max = Math.max(1, ...models.map((item) => item.agg.total));
  if (!selected) return null;
  return <section className="usage-models"><div className="usage-section-heading"><h2>{tr(zh, '按模型查看', 'Explore by model')}</h2><p>{tr(zh, '近 30 天 · 本机记录', 'Last 30 days · local records')}</p></div>
    <div className="usage-model-grid"><div className="usage-surface usage-model-list"><h3>{tr(zh, '模型用量', 'Model usage')}</h3>
      {models.map((item) => <button type="button" key={item.model} className={selected.model === item.model ? 'is-selected' : ''} onClick={() => setSelectedName(item.model)}>
        <span title={item.model}>{item.model}</span><i className="usage-model-track"><i style={{ width: `${item.agg.total / max * 100}%` }} /></i><strong>{fmtTokens(item.agg.total)}</strong>
      </button>)}
    </div><div className="usage-surface usage-model-detail"><div className="usage-model-detail-head"><h3 title={selected.model}>{selected.model}</h3><strong>{fmtInt(selected.agg.total)} Tokens</strong></div>
      <p>{tr(zh, 'Token 构成', 'Token mix')}</p><div className="usage-model-stack">{TOKEN_PARTS.filter(([key]) => selected.agg[key] > 0).map(([key, , , color]) => <i key={key} style={{ flexGrow: selected.agg[key], background: color }} />)}</div>
      <div className="usage-model-parts">{TOKEN_PARTS.map(([key, cn, en, color]) => <div key={key}><i style={{ background: color }} /><span>{tr(zh, cn, en)}</span><strong>{fmtInt(selected.agg[key])}</strong></div>)}</div>
      <small>{tr(zh, '推理 Token 已计入输出；不同工具的缓存记录能力可能不同。', 'Reasoning is included in output; cache reporting varies by tool.')}</small>
    </div></div>
  </section>;
}

export default function UsageDashboard({ ledger, cards = [], monthlyValue, lang, onOpenSettings }) {
  const zh = lang === 0;
  const [range, setRange] = useState('7d');
  const [agent, setAgent] = useState('');
  const [metric, setMetric] = useState('tokens');
  const [mode, setMode] = useState('agent');
  const [customStart, setCustomStart] = useState('');
  const [customEnd, setCustomEnd] = useState('');
  const [exportStatus, setExportStatus] = useState('');
  const today = localDate();
  const bounds = usageBounds(range, today, customStart, customEnd);
  const summary = useMemo(() => aggregateUsage(ledger?.analysis_rows, bounds, agent), [ledger, bounds.start, bounds.end, agent]);
  const agents = (ledger?.sources || []).map((source) => [source.client, source.label]);
  const names = Object.fromEntries(agents);
  const quota = (cards || []).filter((card) => card.kind === 'Live' && card.method !== 'activity_vs_average')
    .flatMap((card) => card.limits.map((limit) => ({ ...limit, provider: card.title.split(' ·')[0] })))
    .sort((a, b) => b.used_pct - a.used_pct)[0];
  useEffect(() => registerNativeCallbacks({ __onExportResult: (status) => setExportStatus(status) }), []);
  if (!ledger) return null;
  const recent = ledger.recent_by_agent?.[0];
  const clear = () => { setRange('7d'); setAgent(''); setCustomStart(''); setCustomEnd(''); };
  const exportUsage = (format) => {
    if (!summary.valid) return;
    setExportStatus('');
    sendCommand({ cmd: 'exportUsage', format, start: bounds.start, end: bounds.end, agent: agent || null, project: null });
  };
  return <div className="usage-page usage-dashboard">
    <p className="usage-subtitle">{tr(zh, '本机账本 · 与服务商配额口径不同', 'Local ledger · separate from provider quotas')}</p>
    <div className="usage-hero-row"><div className="usage-surface usage-hero"><span>{tr(zh, '当前最紧张额度', 'Tightest current quota')}</span><strong>{quota ? `${quota.provider} · ${Math.round(quota.used_pct)}%` : '—'}</strong><small>{quota?.label || tr(zh, '暂无可用配额数据', 'No quota data available')}</small></div>
      <div className="usage-surface usage-hero"><span>{tr(zh, '本月本机用量', 'Local usage this month')}</span><strong>{fmtInt(ledger.month?.total || 0)} <em>Tokens</em></strong><small>{monthlyValue?.month_api_equivalent_usd > 0 ? `${tr(zh, 'API 等值约', 'Approx. API equivalent')} ${fmtCost(monthlyValue.month_api_equivalent_usd)}` : tr(zh, '按本机记录统计', 'From local records')}{monthlyValue && <button type="button" onClick={onOpenSettings}>{tr(zh, '月费基准设置', 'Plan benchmark')}</button>}</small></div></div>
    <div className="usage-activity"><i />{tr(zh, '最近 60 分钟', 'Last 60 minutes')} · {recent ? `${recent.label} ${fmtTokens(recent.agg.total)} Tokens` : tr(zh, '暂无活动', 'No activity')}{ledger.burn_rate != null && <span>· {Math.round(ledger.burn_rate)} {tr(zh, 'Tokens/分钟', 'Tokens/min')}</span>}{ledger.last_activity_ms && <small>{tr(zh, '上次活动', 'Last activity')} {fmtRel(Date.now() - ledger.last_activity_ms)}</small>}</div>
    <div className="usage-filters"><div className="usage-filter-left"><label className="usage-select"><span>{tr(zh, '时间范围', 'Range')}</span><select value={range} onChange={(event) => setRange(event.target.value)} aria-label={tr(zh, '时间范围', 'Range')}>{RANGES.map(([key, cn, en]) => <option key={key} value={key}>{tr(zh, cn, en)}</option>)}</select></label>
      <label className="usage-select"><span>Agent</span><select value={agent} onChange={(event) => setAgent(event.target.value)} aria-label="Agent"><option value="">{tr(zh, '全部', 'All')}</option>{agents.map(([key, name]) => <option key={key} value={key}>{name}</option>)}</select></label>
      {(range !== '7d' || agent) && <button type="button" className="usage-clear" onClick={clear}>{tr(zh, '清除筛选', 'Clear filters')}</button>}</div>
      <details className="usage-export"><summary>{tr(zh, '导出', 'Export')}</summary><div><button type="button" disabled={!summary.valid} onClick={() => exportUsage('csv')}>CSV</button><button type="button" disabled={!summary.valid} onClick={() => exportUsage('json')}>JSON</button></div></details>
    </div>
    {range === 'custom' && <div className="usage-custom"><label>{tr(zh, '开始', 'From')}<input type="date" value={customStart} max={customEnd || today} onChange={(event) => setCustomStart(event.target.value)} /></label><label>{tr(zh, '结束', 'To')}<input type="date" value={customEnd} min={customStart || undefined} max={today} onChange={(event) => setCustomEnd(event.target.value)} /></label></div>}
    {!summary.valid && <p className="usage-feedback" role="alert">{tr(zh, '请选择有效日期，最长 366 天', 'Choose valid dates within 366 days')}</p>}
    {exportStatus && <p className="usage-feedback" role="status">{{ saved: tr(zh, '文件已保存', 'File saved'), canceled: '', error: tr(zh, '导出失败', 'Export failed'), unavailable: tr(zh, '暂无数据', 'No data') }[exportStatus]}</p>}
    <div className="usage-kpis"><SummaryCard title="Tokens" value={fmtInt(summary.total.total)} note={`${bounds.start || '—'} — ${bounds.end || '—'}`} /><SummaryCard title={tr(zh, '请求／消息数', 'Requests / messages')} value={fmtInt(summary.total.messages)} note={tr(zh, '本机记录的调用', 'Calls recorded locally')} /><SummaryCard title={tr(zh, '已知成本 ≈', 'Known cost ≈')} value={fmtCost(summary.total.cost)} note={summary.unpriced > 0 ? tr(zh, `${summary.unpriced} 条记录缺价 · 部分汇总`, `${summary.unpriced} unpriced · partial total`) : tr(zh, '来源报告价与估算价', 'Reported and estimated cost')} /></div>
    <section className="usage-surface usage-chart-card"><div className="usage-chart-heading"><div><h2>{tr(zh, '用量趋势', 'Usage trend')}</h2><p>{tr(zh, '悬浮或点击柱子查看构成', 'Hover or select a bar for details')}</p></div><div className="usage-toggles"><div className="usage-segment" aria-label={tr(zh, '图表指标', 'Chart metric')}>{[['tokens', 'Tokens'], ['messages', tr(zh, '请求', 'Calls')], ['cost', tr(zh, '成本', 'Cost')]].map(([key, name]) => <button type="button" key={key} className={metric === key ? 'is-selected' : ''} onClick={() => setMetric(key)} aria-pressed={metric === key}>{name}</button>)}</div>{agent && metric === 'tokens' && <div className="usage-segment" aria-label={tr(zh, '图表拆分', 'Chart breakdown')}>{[['agent', 'Agent'], ['type', tr(zh, '类型', 'Type')]].map(([key, name]) => <button type="button" key={key} className={mode === key ? 'is-selected' : ''} onClick={() => setMode(key)} aria-pressed={mode === key}>{name}</button>)}</div>}</div></div>
      <TrendChart days={summary.days} metric={metric} agent={agent} mode={mode} names={names} zh={zh} /></section>
    <ModelExplorer ledger={ledger} zh={zh} />
    <details className="usage-more"><summary>{tr(zh, '更多明细 · 项目与会话（近 30 天）', 'More details · projects and sessions (30 days)')}</summary><div><section><h3>{tr(zh, '项目', 'Projects')}</h3>{(ledger.by_project || []).slice(0, 12).map((row) => <p key={row.project}><span title={row.project}>{row.project}</span><strong>{fmtTokens(row.agg.total)}</strong></p>)}</section><section><h3>{tr(zh, '会话', 'Sessions')}</h3>{(ledger.by_session || []).slice(0, 12).map((row) => <p key={`${row.client}:${row.session}`}><span title={row.session}>{row.label} · {row.session}</span><strong>{fmtTokens(row.agg.total)}</strong></p>)}</section></div></details>
  </div>;
}
