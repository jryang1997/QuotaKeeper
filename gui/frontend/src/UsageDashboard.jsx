import ModuleBoard from './ModuleBoard.jsx';
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { fmtInt, fmtTokens, fmtRel } from './ui-data.js';
import { registerNativeCallbacks, sendCommand } from './gui-bridge.js';
import { aggregateIntraday, aggregateUsage, localDate, usageBounds, usageMetric } from './usage-data.js';
import { formatCurrency } from './currency.js';
import AnimatedNumber from './AnimatedNumber.jsx';
import { MotionDisclosure, PopoverSurface, SelectionGroup, SelectionHighlight, useMotionPreference } from './ui-motion.jsx';
import { FADE, LAYOUT_SPRING } from './motion-tuning.js';

const TOKEN_PARTS = [
  ['input', '输入', 'Input', 'var(--usage-part-input)'],
  ['cache_read', '缓存读取', 'Cache read', 'var(--usage-part-cache-read)'],
  ['cache_write', '缓存写入', 'Cache write', 'var(--usage-part-cache-write)'],
  ['output', '输出', 'Output', 'var(--usage-part-output)'],
];
const AGENT_COLORS = {
  codex: 'var(--color-text-purple)',
  omp: 'var(--color-text-blue)',
  opencode: 'var(--color-text-orange)',
  zcode: 'var(--color-text-green)',
  dsh: 'var(--usage-part-output)',
  claude: 'var(--usage-part-cache-write)',
};
const agentColor = (client) => AGENT_COLORS[client] || 'var(--usage-blue)';
const RANGES = [
  ['7d', '近 7 天', 'Last 7 days'], ['30d', '近 30 天', 'Last 30 days'],
  ['today', '今天', 'Today'], ['yesterday', '昨天', 'Yesterday'],
  ['month', '本月', 'This month'], ['custom', '自定义', 'Custom'],
];
const tr = (zh, cn, en) => zh ? cn : en;
const valueText = (value, metric, currency) => metric === 'cost' ? formatCurrency(value, currency) : fmtInt(value);
const monthDay = (date) => date?.slice(5).replace('-', '/') || '—';

function SummaryCard({ title, value, format, note, reduceMotion }) {
  return <div className="usage-summary-card"><span>{title}</span><strong><AnimatedNumber value={value} format={format} reduceMotion={reduceMotion} /></strong><small>{note}</small></div>;
}

function MetricSelector({ label, options, value, onChange }) {
  return <SelectionGroup><div className="usage-segment" role="group" aria-label={label}>{options.map(([key, name]) => <button type="button" key={key} className={value === key ? 'is-selected' : ''} onClick={() => onChange(key)} aria-pressed={value === key}>
    {value === key && <SelectionHighlight />}<span className="segment-label">{name}</span>
  </button>)}</div></SelectionGroup>;
}

function ExportMenu({ label, valid, onExport }) {
  const [open, setOpen] = useState(false);
  const root = useRef(null);
  useEffect(() => {
    if (!open) return;
    const close = (event) => { if (event.type === 'keydown' ? event.key === 'Escape' : !root.current?.contains(event.target)) setOpen(false); };
    document.addEventListener('pointerdown', close); document.addEventListener('keydown', close);
    return () => { document.removeEventListener('pointerdown', close); document.removeEventListener('keydown', close); };
  }, [open]);
  return <div className="usage-export" ref={root}><button type="button" className="usage-export-trigger" aria-expanded={open} onClick={() => setOpen((value) => !value)}>{label}</button>
    <AnimatePresence>{open && <PopoverSurface className="usage-export-menu">{['csv', 'json'].map((format) => <button type="button" key={format} disabled={!valid} onClick={() => { onExport(format); setOpen(false); }}>{format.toUpperCase()}</button>)}</PopoverSurface>}</AnimatePresence>
  </div>;
}

function TrendChart({ days, metric, agent, mode, names, zh, currency }) {
  const reduced = useMotionPreference();
  const [hover, setHover] = useState(null);
  const [pin, setPin] = useState(null);
  const bucketKeys = days.map((day) => day.date).join('|');
  useEffect(() => { setHover(null); setPin(null); }, [bucketKeys, metric, agent, mode]);
  const active = hover ?? pin;
  const selected = active == null ? null : days[active];
  const max = Math.max(1, ...days.map((day) => usageMetric(day.agg, metric)));
  const axisText = (n) => metric === 'tokens' ? fmtTokens(n) : valueText(n, metric, currency);
  const showTypes = !!agent && metric === 'tokens' && mode === 'type';
  const legend = showTypes
    ? TOKEN_PARTS.map(([, cn, en, color]) => [tr(zh, cn, en), color])
    : [...new Set(days.flatMap((day) => [...day.agents.keys()]))].map((client) => [names[client] || client, metric === 'cost' && agent ? 'var(--color-text-orange)' : agentColor(client)]);
  const detail = selected && (showTypes
    ? TOKEN_PARTS.filter(([key]) => selected.agg[key] > 0).map(([key, cn, en, color]) => [tr(zh, cn, en), selected.agg[key], color])
    : [...selected.agents.entries()].filter(([, agg]) => usageMetric(agg, metric) > 0)
      .sort((a, b) => usageMetric(b[1], metric) - usageMetric(a[1], metric))
      .map(([client, agg]) => [names[client] || client, usageMetric(agg, metric), metric === 'cost' && agent ? 'var(--color-text-orange)' : agentColor(client)]));
  return <div className="usage-trend" onKeyDown={(event) => { if (event.key === 'Escape') { setHover(null); setPin(null); } }}>
    <div className="usage-plot">
      <div className="usage-y-axis"><span>{axisText(max)}</span><span>{axisText(max / 2)}</span><span>0</span></div>
      <div className="usage-grid-lines"><i /><i /><i /></div>
      <div className="usage-bars" onMouseLeave={() => setHover(null)}>{days.map((day, index) => {
        const amount = usageMetric(day.agg, metric);
        const parts = showTypes
          ? TOKEN_PARTS.map(([key, , , color]) => [key, day.agg[key], color])
          : [...day.agents.entries()].map(([client, agg]) => [client, usageMetric(agg, metric), metric === 'cost' && agent ? 'var(--color-text-orange)' : agentColor(client)]);
        return <button type="button" key={day.date} className={`usage-day${active === index ? ' is-active' : ''}`}
          onMouseEnter={() => setHover(index)} onFocus={() => setHover(index)} onBlur={() => setHover(null)}
          onClick={() => setPin((current) => current === index ? null : index)}
          aria-label={`${day.date} · ${valueText(amount, metric, currency)}`} aria-pressed={pin === index}>
          <motion.span className="usage-day-bar" initial={false} animate={{ height: amount > 0 ? `${Math.max(3, amount / max * 100)}%` : '0%' }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING}>
            {parts.filter(([, value]) => value > 0).map(([key, value, color]) => <motion.i key={key} initial={false} animate={{ flexGrow: value }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING} style={{ background: color }} />)}
          </motion.span>
        </button>;
      })}</div>
      {selected && <div className="usage-guide" style={{ left: `calc(48px + (100% - 48px) * ${(active + 0.5) / days.length})` }} />}
      <AnimatePresence>{selected && <motion.div className="usage-popover" style={{ left: `clamp(156px, ${(active + 0.5) / days.length * 100}%, calc(100% - 156px))`, x: '-50%' }} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={FADE} role="status">
        <div className="usage-popover-head"><strong>{selected.date}</strong><strong>{valueText(usageMetric(selected.agg, metric), metric, currency)}</strong></div>
        {detail.map(([name, amount, color]) => <div className="usage-popover-line" key={name}><i style={{ background: color }} /><span>{name}</span><strong>{valueText(amount, metric, currency)}</strong></div>)}
        {detail.length === 0 && <small>{tr(zh, '此日暂无记录', 'No records for this day')}</small>}
        {metric === 'cost' && selected.unpriced > 0 && <small>{tr(zh, `${selected.unpriced} 条记录缺价`, `${selected.unpriced} records without pricing`)}</small>}
        {showTypes && selected.agg.reasoning > 0 && <small>{tr(zh, '推理 Token 已包含在输出中', 'Reasoning is included in output')}</small>}
      </motion.div>}</AnimatePresence>
    </div>
    <div className="usage-x-axis"><span>{days[0]?.label || monthDay(days[0]?.date)}</span><span>{days.length > 2 ? days[Math.floor(days.length / 2)]?.label || monthDay(days[Math.floor(days.length / 2)]?.date) : ''}</span><span>{days.at(-1)?.label || monthDay(days.at(-1)?.date)}</span></div>
    {legend.length > 0 && <div className="usage-chart-legend">{legend.map(([name, color]) => <span key={name}><i style={{ background: color }} />{name}</span>)}</div>}
    {!days.some((day) => usageMetric(day.agg, metric) > 0) && <div className="usage-empty">{tr(zh, '所选范围暂无记录', 'No records in this range')}</div>}
  </div>;
}

function ModelExplorer({ ledger, zh, part, selectedName, onSelect }) {
  const reduced = useMotionPreference();
  const models = (ledger.by_model || []).slice(0, 7);
  const selected = models.find((item) => item.model === selectedName) || models[0];
  const max = Math.max(1, ...models.map((item) => item.agg.total));
  if (!selected) return null;
  return part === 'list' ? (<div className="usage-surface usage-model-list"><h3>{tr(zh, '模型用量', 'Model usage')}</h3>
      {models.map((item) => <button type="button" key={item.model} className={selected.model === item.model ? 'is-selected' : ''} onClick={() => onSelect(item.model)}>
        <span title={item.model}>{item.model}</span><i className="usage-model-track"><motion.i initial={false} animate={{ scaleX: item.agg.total / max }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING} style={{ width: '100%', transformOrigin: 'left' }} /></i><strong>{fmtTokens(item.agg.total)}</strong>
      </button>)}
    </div>) : (<div className="usage-surface usage-model-detail"><div className="usage-model-detail-head"><h3 title={selected.model}>{selected.model}</h3><strong>{fmtInt(selected.agg.total)} Tokens</strong></div>
      <p>{tr(zh, 'Token 构成', 'Token mix')}</p><div className="usage-model-stack">{TOKEN_PARTS.map(([key, , , color]) => <motion.i key={key} initial={false} animate={{ flexGrow: selected.agg[key] }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING} style={{ background: color }} />)}</div>
      <div className="usage-model-parts">{TOKEN_PARTS.map(([key, cn, en, color]) => <div key={key}><i style={{ background: color }} /><span>{tr(zh, cn, en)}</span><strong>{fmtInt(selected.agg[key])}</strong></div>)}</div>
      <small>{tr(zh, '推理 Token 已计入输出；不同工具的缓存记录能力可能不同。', 'Reasoning is included in output; cache reporting varies by tool.')}</small>
    </div>);
}

export default function UsageDashboard({ ledger, cards = [], monthlyValue, lang, currency, reduceMotion, onOpenSettings }) {
  const zh = lang === 0;
  const [selectedModel, setSelectedModel] = useState('');
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
  const trendDays = useMemo(() => (range === 'today' || range === 'yesterday')
    ? aggregateIntraday(ledger?.intraday_rows, bounds.start, agent)
    : summary.days, [ledger, range, bounds.start, agent, summary.days]);
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
  const modules = [
    { id: 'quota', title: tr(zh, '当前最紧张额度', 'Current quota'), w: 6, h: 2, minW: 4, minH: 2, content: (<div className="usage-surface usage-hero"><span>{tr(zh, '当前最紧张额度', 'Tightest current quota')}</span><strong className="usage-quota-value" title={quota ? `${quota.provider} · ${Math.round(quota.used_pct)}%` : '—'}><span>{quota?.provider || '—'}</span>{quota && <span className="usage-quota-percent">· <AnimatedNumber value={Math.round(quota.used_pct)} format={(n) => `${Math.round(n)}%`} reduceMotion={reduceMotion} /></span>}</strong><small>{quota?.label || tr(zh, '暂无可用配额数据', 'No quota data available')}</small></div>) },
    { id: 'month', title: tr(zh, '本月本机用量', 'Monthly usage'), w: 6, h: 2, minW: 4, minH: 2, content: (<div className="usage-surface usage-hero"><span>{tr(zh, '本月本机用量', 'Local usage this month')}</span><strong className="usage-hero-number"><AnimatedNumber value={ledger.month?.total || 0} format={(n) => fmtInt(Math.round(n))} reduceMotion={reduceMotion} /> <em>Tokens</em></strong><small>{monthlyValue?.month_api_equivalent_usd > 0 ? <>{tr(zh, 'API 等值约', 'Approx. API equivalent')} <AnimatedNumber value={monthlyValue.month_api_equivalent_usd} format={(n) => formatCurrency(n, currency)} reduceMotion={reduceMotion} /></> : tr(zh, '按本机记录统计', 'From local records')}{monthlyValue && <button type="button" onClick={onOpenSettings}>{tr(zh, '月费基准设置', 'Plan benchmark')}</button>}</small></div>) },
    { id: 'tokens', title: tr(zh, 'Tokens', 'Tokens'), w: 4, h: 2, minW: 3, minH: 2, content: (<SummaryCard title="Tokens" value={summary.total.total} format={(n) => fmtInt(Math.round(n))} reduceMotion={reduceMotion} note={`${bounds.start || '—'} — ${bounds.end || '—'}`} />) },
    { id: 'requests', title: tr(zh, '请求／消息数', 'Requests / messages'), w: 4, h: 2, minW: 3, minH: 2, content: (<SummaryCard title={tr(zh, '请求／消息数', 'Requests / messages')} value={summary.total.messages} format={(n) => fmtInt(Math.round(n))} reduceMotion={reduceMotion} note={tr(zh, '本机记录的调用', 'Calls recorded locally')} />) },
    { id: 'cost', title: tr(zh, '已知成本', 'Known cost'), w: 4, h: 2, minW: 3, minH: 2, content: (<SummaryCard title={tr(zh, '已知成本 ≈', 'Known cost ≈')} value={summary.total.cost} format={(n) => formatCurrency(n, currency)} reduceMotion={reduceMotion} note={summary.unpriced > 0 ? tr(zh, `${summary.unpriced} 条记录缺价 · 部分汇总`, `${summary.unpriced} unpriced · partial total`) : tr(zh, '来源报告价与估算价', 'Reported and estimated cost')} />) },
    { id: 'trend', title: tr(zh, '用量趋势', 'Usage trend'), w: 12, h: 4, minW: 6, minH: 3, content: (<section className="usage-surface usage-chart-card"><div className="usage-chart-heading"><div><h2>{tr(zh, '用量趋势', 'Usage trend')}</h2><p>{tr(zh, '悬浮或点击柱子查看构成', 'Hover or select a bar for details')}</p></div><div className="usage-toggles"><MetricSelector label={tr(zh, '图表指标', 'Chart metric')} options={[[ 'tokens', 'Tokens' ], [ 'messages', tr(zh, '请求', 'Calls') ], [ 'cost', tr(zh, '成本', 'Cost') ]]} value={metric} onChange={setMetric} />{agent && metric === 'tokens' && <MetricSelector label={tr(zh, '图表拆分', 'Chart breakdown')} options={[[ 'agent', 'Agent' ], [ 'type', tr(zh, '类型', 'Type') ]]} value={mode} onChange={setMode} />}</div></div>
      <TrendChart days={trendDays} metric={metric} agent={agent} mode={mode} names={names} zh={zh} currency={currency} /></section>) },
    { id: 'models', title: tr(zh, '模型用量 · 近 30 天', 'Model usage · 30 days'), w: 6, h: 4, minW: 4, minH: 3, content: (<ModelExplorer part="list" selectedName={selectedModel} onSelect={setSelectedModel} ledger={ledger} zh={zh} />) },
    { id: 'model-detail', title: tr(zh, '模型 Token 构成', 'Model token mix'), w: 6, h: 4, minW: 4, minH: 3, content: (<ModelExplorer part="detail" selectedName={selectedModel} onSelect={setSelectedModel} ledger={ledger} zh={zh} />) },
    { id: 'details', title: tr(zh, '项目与会话', 'Projects and sessions'), w: 12, h: 3, minW: 6, minH: 2, content: (<MotionDisclosure className="usage-more" label={tr(zh, '更多明细 · 项目与会话（近 30 天）', 'More details · projects and sessions (30 days)')}><div className="usage-more-grid"><section><h3>{tr(zh, '项目', 'Projects')}</h3>{(ledger.by_project || []).slice(0, 12).map((row) => <p key={row.project}><span title={row.project}>{row.project}</span><strong>{fmtTokens(row.agg.total)}</strong></p>)}</section><section><h3>{tr(zh, '会话', 'Sessions')}</h3>{(ledger.by_session || []).slice(0, 12).map((row) => <p key={`${row.client}:${row.session}`}><span title={row.session}>{row.label} · {row.session}</span><strong>{fmtTokens(row.agg.total)}</strong></p>)}</section></div></MotionDisclosure>) }
  ];
  return <div className="usage-page usage-dashboard">
    <p className="usage-subtitle">{tr(zh, '本机账本 · 与服务商配额口径不同', 'Local ledger · separate from provider quotas')}</p>
    <div className="usage-activity"><i />{tr(zh, '最近 60 分钟', 'Last 60 minutes')} · {recent ? `${recent.label} ${fmtTokens(recent.agg.total)} Tokens` : tr(zh, '暂无活动', 'No activity')}{ledger.burn_rate != null && <span>· {Math.round(ledger.burn_rate)} {tr(zh, 'Tokens/分钟', 'Tokens/min')}</span>}{ledger.last_activity_ms && <small>{tr(zh, '上次活动', 'Last activity')} {fmtRel(Date.now() - ledger.last_activity_ms)}</small>}</div>
    <div className="usage-filters"><div className="usage-filter-left"><label className="usage-select"><span>{tr(zh, '时间范围', 'Range')}</span><select value={range} onChange={(event) => setRange(event.target.value)} aria-label={tr(zh, '时间范围', 'Range')}>{RANGES.map(([key, cn, en]) => <option key={key} value={key}>{tr(zh, cn, en)}</option>)}</select></label>
      <label className="usage-select"><span>Agent</span><select value={agent} onChange={(event) => setAgent(event.target.value)} aria-label="Agent"><option value="">{tr(zh, '全部', 'All')}</option>{agents.map(([key, name]) => <option key={key} value={key}>{name}</option>)}</select></label>
      {(range !== '7d' || agent) && <button type="button" className="usage-clear" onClick={clear}>{tr(zh, '清除筛选', 'Clear filters')}</button>}</div>
      <ExportMenu label={tr(zh, '导出', 'Export')} valid={summary.valid} onExport={exportUsage} />
    </div>
    {range === 'custom' && <div className="usage-custom"><label>{tr(zh, '开始', 'From')}<input type="date" value={customStart} max={customEnd || today} onChange={(event) => setCustomStart(event.target.value)} /></label><label>{tr(zh, '结束', 'To')}<input type="date" value={customEnd} min={customStart || undefined} max={today} onChange={(event) => setCustomEnd(event.target.value)} /></label></div>}
    {!summary.valid && <p className="usage-feedback" role="alert">{tr(zh, '请选择有效日期，最长 366 天', 'Choose valid dates within 366 days')}</p>}
    {exportStatus && <p className="usage-feedback" role="status">{{ saved: tr(zh, '文件已保存', 'File saved'), canceled: '', error: tr(zh, '导出失败', 'Export failed'), unavailable: tr(zh, '暂无数据', 'No data') }[exportStatus]}</p>}
    <ModuleBoard storageKey="token-monitor.usage-layout.v1" lang={lang} catalog={modules} defaults={USAGE_LAYOUT} renderModule={(id) => modules.find((item) => item.id === id)?.content} />
  </div>;
}

const USAGE_LAYOUT = [{'id': 'quota', 'x': 0, 'y': 0, 'w': 6, 'h': 2, 'locked': false}, {'id': 'month', 'x': 6, 'y': 0, 'w': 6, 'h': 2, 'locked': false}, {'id': 'tokens', 'x': 0, 'y': 2, 'w': 4, 'h': 2, 'locked': false}, {'id': 'requests', 'x': 4, 'y': 2, 'w': 4, 'h': 2, 'locked': false}, {'id': 'cost', 'x': 8, 'y': 2, 'w': 4, 'h': 2, 'locked': false}, {'id': 'trend', 'x': 0, 'y': 4, 'w': 12, 'h': 4, 'locked': false}, {'id': 'models', 'x': 0, 'y': 8, 'w': 6, 'h': 4, 'locked': false}, {'id': 'model-detail', 'x': 6, 'y': 8, 'w': 6, 'h': 4, 'locked': false}, {'id': 'details', 'x': 0, 'y': 12, 'w': 12, 'h': 3, 'locked': false}];
