import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { STR, fill, fmtInt, fmtTokens } from './ui-data.js';
import { registerNativeCallbacks, sendCommand } from './gui-bridge.js';
import { aggregateIntraday, aggregateUsage, localDate, seriesMatchesTotal, usageBounds, usageCoverage, usageMetric } from './usage-data.js';
import { formatCurrency } from './currency.js';
import AnimatedNumber from './AnimatedNumber.jsx';
import { PopoverSurface, SelectionGroup, SelectionHighlight, useMotionPreference } from './ui-motion.jsx';
import { FADE, LAYOUT_SPRING } from './motion-tuning.js';

const TOKEN_PARTS = [
  ['input', '输入', 'Input', 'var(--usage-part-input)'],
  ['cache_read', '缓存读取', 'Cache read', 'var(--usage-part-cache-read)'],
  ['cache_write', '缓存写入', 'Cache write', 'var(--usage-part-cache-write)'],
  ['output', '输出', 'Output', 'var(--usage-part-output)'],
];
const BUCKETS = [
  ['input', '输入', 'Input', 'var(--usage-part-input)'],
  ['output', '输出', 'Output', 'var(--usage-part-output)'],
  ['cache_read', '缓存读取', 'Cache read', 'var(--usage-part-cache-read)'],
  ['cache_write', '缓存写入', 'Cache write', 'var(--usage-part-cache-write)'],
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
  ['today', 'period_today'], ['yesterday', 'period_yesterday'], ['7d', 'period_7d'],
  ['30d', 'period_30d'], ['month', 'period_month'], ['custom', 'period_custom'],
];
const tr = (zh, cn, en) => (zh ? cn : en);
const valueText = (value, metric, currency) => (metric === 'cost' ? formatCurrency(value, currency) : fmtInt(value));
const monthDay = (date) => date?.slice(5).replace('-', '/') || '—';
const modelName = (model, zh) => (model === 'unknown' ? tr(zh, '未知模型', 'Unknown model') : model);

function SummaryCard({ title, value, format, note, measured }) {
  return (
    <div className="usage-surface usage-summary-card">
      <span>{title}</span>
      <strong>{measured ? <AnimatedNumber value={value} format={format} /> : '—'}</strong>
      <small>{note}</small>
    </div>
  );
}

function MetricSelector({ label, options, value, onChange }) {
  return <SelectionGroup><div className="usage-segment" role="group" aria-label={label}>{options.map(([key, name]) => <button type="button" key={key} className={value === key ? 'is-selected' : ''} onClick={() => onChange(key)} aria-pressed={value === key}>
    {value === key && <SelectionHighlight />}<span className="segment-label">{name}</span>
  </button>)}</div></SelectionGroup>;
}

function ExportMenu({ label, valid, onExport }) {
  const [open, setOpen] = useState(false);
  const root = useRef(null);
  useEffect(() => { if (!valid) setOpen(false); }, [valid]);
  useEffect(() => {
    if (!open) return undefined;
    const close = (event) => { if (event.type === 'keydown' ? event.key === 'Escape' : !root.current?.contains(event.target)) setOpen(false); };
    document.addEventListener('pointerdown', close); document.addEventListener('keydown', close);
    return () => { document.removeEventListener('pointerdown', close); document.removeEventListener('keydown', close); };
  }, [open]);
  return <div className="usage-export" ref={root}><button type="button" className="usage-export-trigger" disabled={!valid} aria-expanded={open} onClick={() => setOpen((value) => !value)}>{label}</button>
    <AnimatePresence>{open && <PopoverSurface className="usage-export-menu">{['csv', 'json'].map((format) => <button type="button" key={format} disabled={!valid} onClick={() => { onExport(format); setOpen(false); }}>{format.toUpperCase()}</button>)}</PopoverSurface>}</AnimatePresence>
  </div>;
}

function TrendChart({ days, metric, agent, model, mode, names, zh, currency, lang }) {
  const reduced = useMotionPreference();
  const t = (key) => STR[key][lang];
  const [hover, setHover] = useState(null);
  const [pin, setPin] = useState(null);
  const bucketKeys = days.map((day) => day.date).join('|');
  useEffect(() => { setHover(null); setPin(null); }, [bucketKeys, metric, agent, model, mode]);
  const active = hover ?? pin;
  const selected = active == null ? null : days[active];
  const max = Math.max(1, ...days.map((day) => usageMetric(day.agg, metric, day.unpriced) ?? 0));
  const allUnpriced = metric === 'cost' && days.some((day) => day.unpriced > 0) && days.every((day) => day.agg.messages <= day.unpriced);
  const axisText = (n) => (allUnpriced ? '—' : metric === 'tokens' ? fmtTokens(n, zh) : valueText(n, metric, currency));
  const showTypes = !!agent && metric === 'tokens' && mode === 'type';
  const legend = showTypes
    ? TOKEN_PARTS.map(([, cn, en, color]) => [tr(zh, cn, en), color])
    : [...new Set(days.flatMap((day) => [...day.agents.keys()]))].map((client) => [names[client] || client, agentColor(client)]);
  const detail = selected && (showTypes
    ? TOKEN_PARTS.filter(([key]) => selected.agg[key] > 0).map(([key, cn, en, color]) => [tr(zh, cn, en), selected.agg[key], color])
    : [...selected.agents.entries()].filter(([, item]) => usageMetric(item, metric) > 0)
      .sort((a, b) => usageMetric(b[1], metric) - usageMetric(a[1], metric))
      .map(([client, item]) => [names[client] || client, usageMetric(item, metric), agentColor(client)]));
  let emptyLabel = t('usage_no_records');
  if (allUnpriced) emptyLabel = t('usage_not_priced');
  else if (metric === 'cost' && days.some((day) => day.agg.messages > 0)) emptyLabel = tr(zh, 'API 等值为 0', 'API equivalent is zero');
  return <div className="usage-trend" onKeyDown={(event) => { if (event.key === 'Escape') { setHover(null); setPin(null); } }}>
    <div className="usage-plot">
      <div className="usage-y-axis"><span>{axisText(max)}</span><span>{axisText(max / 2)}</span><span>{allUnpriced ? '—' : '0'}</span></div>
      <div className="usage-grid-lines"><i /><i /><i /></div>
      <div className="usage-bars" onMouseLeave={() => setHover(null)}>{days.map((day, index) => {
        const amount = usageMetric(day.agg, metric, day.unpriced);
        const barParts = showTypes
          ? TOKEN_PARTS.map(([key, , , color]) => [key, day.agg[key], color])
          : [...day.agents.entries()].map(([client, item]) => [client, usageMetric(item, metric), agentColor(client)]);
        return <button type="button" key={day.date} className={`usage-day${active === index ? ' is-active' : ''}`}
          onMouseEnter={() => setHover(index)} onFocus={() => setHover(index)} onBlur={() => setHover(null)}
          onClick={() => setPin((current) => (current === index ? null : index))}
          aria-label={`${day.date} · ${valueText(amount, metric, currency)}`} aria-pressed={pin === index}>
          <motion.span className="usage-day-bar" initial={false} animate={{ height: amount > 0 ? `${Math.max(3, amount / max * 100)}%` : '0%' }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING}>
            {barParts.filter(([, value]) => value > 0).map(([key, value, color]) => <motion.i key={key} initial={false} animate={{ flexGrow: value }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING} style={{ background: color }} />)}
          </motion.span>
        </button>;
      })}</div>
      {selected && <div className="usage-guide" style={{ left: `calc(48px + (100% - 48px) * ${(active + 0.5) / days.length})` }} />}
      <AnimatePresence>{selected && <motion.div className="usage-popover" style={{ left: `clamp(156px, ${(active + 0.5) / days.length * 100}%, calc(100% - 156px))`, x: '-50%' }} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={FADE} role="status">
        <div className="usage-popover-head"><strong>{selected.date}</strong><strong>{valueText(usageMetric(selected.agg, metric, selected.unpriced), metric, currency)}</strong></div>
        {detail.map(([name, amount, color]) => <div className="usage-popover-line" key={name}><i style={{ background: color }} /><span>{name}</span><strong>{valueText(amount, metric, currency)}</strong></div>)}
        {detail.length === 0 && selected.agg.messages === 0 && selected.unpriced === 0 && <small>{t('usage_day_empty')}</small>}
        {metric === 'cost' && selected.unpriced > 0 && <small>{fill(t('usage_unpriced'), { n: selected.unpriced })}</small>}
        {showTypes && selected.agg.reasoning > 0 && <small>{t('usage_reasoning_in_output')}</small>}
      </motion.div>}</AnimatePresence>
    </div>
    <div className="usage-x-axis"><span>{days[0]?.label || monthDay(days[0]?.date)}</span><span>{days.length > 2 ? days[Math.floor(days.length / 2)]?.label || monthDay(days[Math.floor(days.length / 2)]?.date) : ''}</span><span>{days.at(-1)?.label || monthDay(days.at(-1)?.date)}</span></div>
    {legend.length > 0 && <div className="usage-chart-legend">{legend.map(([name, color]) => <span key={name}><i style={{ background: color }} />{name}</span>)}</div>}
    {!days.some((day) => usageMetric(day.agg, metric, day.unpriced) > 0) && <div className="usage-empty">{emptyLabel}</div>}
  </div>;
}

export default function UsageDashboard({ ledger, stale, model, onModelChange: setModel, lang, currency, onOpenSettings }) {
  const zh = lang === 0;
  const t = (key) => STR[key][lang];
  const [range, setRange] = useState('today');
  const [agent, setAgent] = useState('');
  const [distribution, setDistribution] = useState('model');
  const [metric, setMetric] = useState('tokens');
  const [mode, setMode] = useState('agent');
  const [customStart, setCustomStart] = useState('');
  const [customEnd, setCustomEnd] = useState('');
  const [exportStatus, setExportStatus] = useState('');
  const today = localDate();
  const bounds = usageBounds(range, today, customStart, customEnd);
  const summary = useMemo(() => aggregateUsage(ledger?.analysis_rows, bounds, agent, model), [ledger, bounds.start, bounds.end, agent, model]);
  const models = useMemo(() => [...new Set([...(ledger?.analysis_rows || []).map((row) => row.model), ...(model ? [model] : [])])].sort(), [ledger, model]);
  const coverage = useMemo(() => usageCoverage(ledger?.sources, agent, summary), [ledger, agent, summary]);
  const trendDays = useMemo(() => {
    if (range !== 'today' && range !== 'yesterday') return summary.days;
    const buckets = aggregateIntraday(ledger?.intraday_rows, bounds.start, agent, model);
    return seriesMatchesTotal(buckets, summary.total, summary.unpriced) ? buckets : summary.days;
  }, [ledger, range, bounds.start, agent, model, summary]);
  const agents = (ledger?.sources || []).map((source) => [source.client, source.label]);
  const names = Object.fromEntries(agents);
  useEffect(() => registerNativeCallbacks({ __onExportResult: (status) => setExportStatus(status) }), []);
  useEffect(() => { setExportStatus(''); }, [ledger?.report_id]);
  if (!ledger) return <div className="usage-page usage-dashboard"><p className="usage-feedback" role="status">{t('usage_refresh_required')}</p></div>;
  const measured = summary.valid && coverage.state !== 'unknown';
  const reportId = !stale && exportStatus !== 'refresh_required' ? ledger.report_id : null;
  const canExport = measured && !!reportId;
  const clear = () => { setRange('today'); setAgent(''); setModel(''); setCustomStart(''); setCustomEnd(''); };
  const exportUsage = (format) => {
    if (!canExport) return;
    setExportStatus('');
    sendCommand({ cmd: 'exportUsage', format, start: bounds.start, end: bounds.end, agent: agent || null, project: null, model: model || null, reportId });
  };
  const rangeNote = bounds.start && bounds.end
    ? (bounds.start.slice(0, 4) === bounds.end.slice(0, 4) ? `${bounds.start.slice(5)} → ${bounds.end.slice(5)}` : `${bounds.start} → ${bounds.end}`)
    : '—';
  const withheld = summary.valid
    ? tr(zh, '采集未完成，不能当成 0', 'Collection incomplete, not a real zero')
    : t('period_invalid');
  const notBill = tr(zh, '不是订阅扣费', 'Not a subscription charge');
  const costKnown = usageMetric(summary.total, 'cost', summary.unpriced) !== null;
  let priceNote = notBill;
  if (!measured) priceNote = withheld;
  else if (!costKnown) priceNote = `${t('usage_not_priced')} · ${fill(t('usage_unpriced'), { n: summary.unpriced })} · ${notBill}`;
  else if (summary.unpriced > 0) priceNote = `${fill(t('usage_unpriced_partial'), { n: summary.unpriced })} · ${notBill}`;
  const failedText = coverage.failed.map((client) => names[client] || client).join(zh ? '、' : ', ');
  const collectionCopy = coverage.state === 'unknown'
    ? (failedText
      ? tr(zh, `${failedText} 没读成，空结果不是用量为 0。`, `${failedText} failed to read, so an empty result is not zero usage.`)
      : tr(zh, '没有采集状态，空着不代表用量为 0。', 'Collection status is missing, so a blank is not zero usage.'))
    : (failedText
      ? tr(zh, `${failedText} 没读成，数字只含已读到的记录。`, `${failedText} failed to read. Figures include only collected records.`)
      : tr(zh, '有采集源没读成，数字只含已读到的记录。', 'A source failed to read. Figures include only collected records.'));
  const exportMessage = stale ? t('export_stale') : { saved: t('export_saved'), canceled: '', error: t('export_error'), unavailable: t('export_unavailable'), refresh_required: t('export_refresh_required') }[exportStatus] || '';
  const showChartTypes = agent && metric === 'tokens';

  return <div className="usage-page usage-dashboard">
    <header className="usage-head">
      <p className="usage-subtitle">{t('usage_subtitle')}</p>
      <p className="usage-disclaimer">{tr(zh, 'API 等值按本机记录里已有的价格估算，不是订阅扣费，也不能和额度页的剩余量相加。', 'API equivalent is priced from local records. It is not a subscription charge and is not added to remaining quota.')}</p>
    </header>
    <div className="usage-filters"><div className="usage-filter-left">
      <label className="usage-select"><span>{t('period_range')}</span><select value={range} onChange={(event) => setRange(event.target.value)} aria-label={t('period_range')}>{RANGES.map(([key, label]) => <option key={key} value={key}>{t(label)}</option>)}</select></label>
      <label className="usage-select"><span>{t('period_agent')}</span><select value={agent} onChange={(event) => setAgent(event.target.value)} aria-label={t('period_agent')}><option value="">{t('period_all')}</option>{agents.map(([key, name]) => <option key={key} value={key}>{name}</option>)}</select></label>
      <label className="usage-select"><span>{t('period_model')}</span><select value={model} onChange={(event) => setModel(event.target.value)} aria-label={t('period_model')}><option value="">{t('period_all')}</option>{models.map((name) => <option key={name} value={name}>{modelName(name, zh)}</option>)}</select></label>
      {(range !== 'today' || agent || model) && <button type="button" className="usage-clear" onClick={clear}>{t('period_clear')}</button>}
    </div>
      <ExportMenu label={t('export_label')} valid={canExport} onExport={exportUsage} />
    </div>
    {range === 'custom' && <div className="usage-custom"><label>{t('period_from')}<input type="date" value={customStart} max={customEnd || today} onChange={(event) => setCustomStart(event.target.value)} /></label><label>{t('period_to')}<input type="date" value={customEnd} min={customStart || undefined} max={today} onChange={(event) => setCustomEnd(event.target.value)} /></label></div>}
    {!summary.valid && <p className="usage-feedback" role="alert">{t('period_invalid')}</p>}
    {(coverage.state === 'partial' || coverage.state === 'unknown') && <p className="usage-feedback" role="status"><span>{collectionCopy}</span><button type="button" className="usage-diagnostics" onClick={() => onOpenSettings?.()}>{t('source_diagnostic')}</button></p>}
    {exportMessage && <p className="usage-feedback" role="status">{exportMessage}</p>}
    <div className="usage-kpis">
      <SummaryCard title={t('usage_summary_tokens')} value={summary.total.total} format={(n) => fmtTokens(n, zh)} note={measured ? `${rangeNote} · ${t('usage_total_note')}` : withheld} measured={measured} />
      <SummaryCard title={t('usage_summary_requests')} value={summary.total.messages} format={(n) => fmtInt(Math.round(n))} note={measured ? t('usage_record_note') : withheld} measured={measured} />
      <SummaryCard title={tr(zh, 'API 等值参考', 'API equivalent')} value={summary.total.cost} format={(n) => formatCurrency(n, currency)} note={priceNote} measured={measured && costKnown} />
    </div>
    <section className="usage-surface usage-chart-card">
      <div className="usage-chart-heading">
        <div><h2>{t('usage_trend_title')}</h2><p>{t('usage_trend_hint')}</p></div>
        <div className="usage-toggles">
          <MetricSelector label={t('usage_metric')} options={[['tokens', 'Tokens'], ['messages', t('usage_metric_messages')], ['cost', t('metric_cost')]]} value={metric} onChange={setMetric} />
          {showChartTypes && <MetricSelector label={t('usage_breakdown')} options={[['agent', t('usage_breakdown_agent')], ['type', t('usage_breakdown_type')]]} value={mode} onChange={setMode} />}
        </div>
      </div>
      {measured ? <TrendChart days={trendDays} metric={metric} agent={agent} model={model} mode={mode} names={names} zh={zh} currency={currency} lang={lang} /> : <p className="usage-placeholder">—</p>}
    </section>
    <div className="usage-lower">
      <section className="usage-surface usage-buckets-card">
        <div className="usage-section-heading"><h2>{t('usage_token_mix')}</h2><p>{t('usage_reasoning_note')}</p></div>
        {measured && summary.total.total > 0 && <div className="usage-bucket-bar" aria-hidden="true">{BUCKETS.map(([key, , , color]) => summary.total[key] > 0 && <i key={key} style={{ flexGrow: summary.total[key], background: color }} />)}</div>}
        <div className="usage-buckets">{BUCKETS.map(([key, cn, en, color]) => <div className="usage-bucket" key={key}>
          <span><i style={{ background: color }} />{tr(zh, cn, en)}</span>
          <strong>{measured ? fmtInt(summary.total[key]) : '—'}</strong>
        </div>)}</div>
        {measured && summary.total.reasoning > 0 && <p className="usage-reasoning-count">{t('usage_reasoning_in_output')} · {fmtInt(summary.total.reasoning)}</p>}
      </section>
      <section className="usage-surface usage-clients-card">
        <div className="usage-chart-heading">
          <div className="usage-section-heading"><h2>{distribution === 'model' ? tr(zh, '模型分布', 'By model') : tr(zh, '采集源分布', 'By collector')}</h2><p>{tr(zh, '与上面同一筛选', 'Same filters as the totals')}</p></div>
          <MetricSelector label={tr(zh, '分布维度', 'Distribution')} options={[['model', t('period_model')], ['client', t('period_agent')]]} value={distribution} onChange={setDistribution} />
        </div>
        {!measured && <p className="usage-placeholder">—</p>}
        {measured && summary.byClient.length === 0 && <p className="usage-placeholder">{t('usage_no_records')}</p>}
        {measured && distribution === 'model' && <div className="usage-models">{summary.byModel.map((row) => {
          const share = summary.total.total > 0 ? row.agg.total / summary.total.total * 100 : 0;
          const name = modelName(row.model, zh);
          return <div className="usage-model" key={row.model}>
            <button type="button" className="usage-model-filter" onClick={() => setModel(row.model)} aria-pressed={model === row.model} aria-label={tr(zh, `筛选模型 ${name}`, `Filter model ${name}`)}>
              <span title={name}>{name}</span><strong>{fmtTokens(row.agg.total, zh)} <small>Tokens</small></strong>
            </button>
            <i className="usage-client-track" aria-hidden="true"><i style={{ width: `${share}%`, background: 'var(--usage-blue)' }} /></i>
            <div className="usage-model-totals"><span>{tr(zh, '占比', 'Share')} {share.toFixed(1)}%</span><span>{t('usage_summary_requests')} {fmtInt(row.agg.messages)}</span><span>{t('metric_cost')} {valueText(usageMetric(row.agg, 'cost', row.unpriced), 'cost', currency)}</span></div>
            <details className="usage-model-details"><summary>{tr(zh, '原始名称、采集源与缓存', 'Raw names, collectors and cache')}</summary>
              <dl>
                <div><dt>{tr(zh, '原始名称', 'Raw names')}</dt><dd>{row.rawModels.map((raw) => <code key={raw}>{raw === '' ? tr(zh, '空名称', 'Empty name') : raw}</code>)}</dd></div>
                <div><dt>{t('period_agent')}</dt><dd>{row.clients.map((client) => names[client] || client).join(zh ? '、' : ', ')}</dd></div>
                <div><dt>{tr(zh, '缓存读取', 'Cache read')}</dt><dd>{fmtInt(row.agg.cache_read)}</dd></div>
                <div><dt>{tr(zh, '缓存写入', 'Cache write')}</dt><dd>{fmtInt(row.agg.cache_write)}</dd></div>
              </dl>
              {row.unpriced > 0 && <p>{fill(t('usage_unpriced'), { n: row.unpriced })}</p>}
            </details>
          </div>;
        })}</div>}
        {measured && distribution === 'client' && summary.byClient.length > 0 && <div className="usage-clients">{summary.byClient.map((row) => {
          const share = summary.total.total > 0 ? row.agg.total / summary.total.total * 100 : 0;
          return <div className="usage-client" key={row.client}>
            <span><i style={{ background: agentColor(row.client) }} />{names[row.client] || row.client}</span>
            <i className="usage-client-track" aria-hidden="true"><i style={{ width: `${share}%`, background: agentColor(row.client) }} /></i>
            <strong>{fmtTokens(row.agg.total, zh)}</strong>
            <small title={t('usage_summary_requests')}>{fmtInt(row.agg.messages)}</small>
          </div>;
        })}</div>}
      </section>
    </div>
  </div>;
}
