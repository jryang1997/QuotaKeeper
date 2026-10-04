// 用量页:本机账本口径(与服务商配额分开)。
// 顶部摘要行(范围筛选驱动)+ 最近活动 + 可编辑模块板(原"面板"页 12 个模块并入此处)。
import ModuleBoard from './ModuleBoard.jsx';
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { STR, fill, fmtInt, fmtTokens, fmtRel } from './ui-data.js';
import { registerNativeCallbacks, sendCommand } from './gui-bridge.js';
import { aggregateIntraday, aggregateUsage, fmtDurationMs, localDate, speedBoard, usageBounds, usageMetric } from './usage-data.js';
import { formatCurrency } from './currency.js';
import AnimatedNumber from './AnimatedNumber.jsx';
import { PopoverSurface, SelectionGroup, SelectionHighlight, useMotionPreference } from './ui-motion.jsx';
import { FADE, LAYOUT_SPRING } from './motion-tuning.js';
import { PANEL_BODIES, PanelErrorBoundary, panelMeta } from './PanelBodies.jsx';
import { AchievementsPanel } from './Achievements.jsx';
import { PANEL_CATALOG, DEFAULT_PANELS } from './panel-layout.js';

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
  ['7d', 'period_7d'], ['30d', 'period_30d'], ['today', 'period_today'],
  ['yesterday', 'period_yesterday'], ['month', 'period_month'], ['custom', 'period_custom'],
];
const tr = (zh, cn, en) => (zh ? cn : en);
const valueText = (value, metric, currency) => (metric === 'cost' ? formatCurrency(value, currency) : fmtInt(value));
const monthDay = (date) => date?.slice(5).replace('-', '/') || '—';

function SummaryCard({ title, value, format, note, tone }) {
  return (
    <div className="usage-surface usage-stat" style={tone ? { '--stat-tone': tone } : undefined}>
      <span className="usage-stat-label">{title}</span>
      <strong className="usage-stat-value"><AnimatedNumber value={value} format={format} /></strong>
      <small className="usage-stat-note">{note}</small>
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

function TrendChart({ days, metric, agent, mode, names, zh, currency, lang }) {
  const reduced = useMotionPreference();
  const t = (key) => STR[key][lang];
  const [hover, setHover] = useState(null);
  const [pin, setPin] = useState(null);
  const bucketKeys = days.map((day) => day.date).join('|');
  useEffect(() => { setHover(null); setPin(null); }, [bucketKeys, metric, agent, mode]);
  const active = hover ?? pin;
  const selected = active == null ? null : days[active];
  const max = Math.max(1, ...days.map((day) => usageMetric(day.agg, metric)));
  const axisText = (n) => (metric === 'tokens' ? fmtTokens(n, zh) : valueText(n, metric, currency));
  const showTypes = !!agent && metric === 'tokens' && mode === 'type';
  const legend = showTypes
    ? TOKEN_PARTS.map(([, cn, en, color]) => [tr(zh, cn, en), color])
    : [...new Set(days.flatMap((day) => [...day.agents.keys()]))].map((client) => [names[client] || client, agentColor(client)]);
  const detail = selected && (showTypes
    ? TOKEN_PARTS.filter(([key]) => selected.agg[key] > 0).map(([key, cn, en, color]) => [tr(zh, cn, en), selected.agg[key], color])
    : [...selected.agents.entries()].filter(([, agg]) => usageMetric(agg, metric) > 0)
      .sort((a, b) => usageMetric(b[1], metric) - usageMetric(a[1], metric))
      .map(([client, agg]) => [names[client] || client, usageMetric(agg, metric), agentColor(client)]));
  return <div className="usage-trend" onKeyDown={(event) => { if (event.key === 'Escape') { setHover(null); setPin(null); } }}>
    <div className="usage-plot">
      <div className="usage-y-axis"><span>{axisText(max)}</span><span>{axisText(max / 2)}</span><span>0</span></div>
      <div className="usage-grid-lines"><i /><i /><i /></div>
      <div className="usage-bars" onMouseLeave={() => setHover(null)}>{days.map((day, index) => {
        const amount = usageMetric(day.agg, metric);
        const parts = showTypes
          ? TOKEN_PARTS.map(([key, , , color]) => [key, day.agg[key], color])
          : [...day.agents.entries()].map(([client, agg]) => [client, usageMetric(agg, metric), agentColor(client)]);
        return <button type="button" key={day.date} className={`usage-day${active === index ? ' is-active' : ''}`}
          onMouseEnter={() => setHover(index)} onFocus={() => setHover(index)} onBlur={() => setHover(null)}
          onClick={() => setPin((current) => (current === index ? null : index))}
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
        {detail.length === 0 && <small>{t('usage_day_empty')}</small>}
        {metric === 'cost' && selected.unpriced > 0 && <small>{fill(t('usage_unpriced'), { n: selected.unpriced })}</small>}
        {showTypes && selected.agg.reasoning > 0 && <small>{t('usage_reasoning_in_output')}</small>}
      </motion.div>}</AnimatePresence>
    </div>
    <div className="usage-x-axis"><span>{days[0]?.label || monthDay(days[0]?.date)}</span><span>{days.length > 2 ? days[Math.floor(days.length / 2)]?.label || monthDay(days[Math.floor(days.length / 2)]?.date) : ''}</span><span>{days.at(-1)?.label || monthDay(days.at(-1)?.date)}</span></div>
    {legend.length > 0 && <div className="usage-chart-legend">{legend.map(([name, color]) => <span key={name}><i style={{ background: color }} />{name}</span>)}</div>}
    {!days.some((day) => usageMetric(day.agg, metric) > 0) && <div className="usage-empty">{t('usage_no_records')}</div>}
  </div>;
}

function ModelExplorer({ ledger, zh, lang, part, selectedName, onSelect }) {
  const reduced = useMotionPreference();
  const t = (key) => STR[key][lang];
  const models = (ledger.by_model || []).slice(0, 7);
  const selected = models.find((item) => item.model === selectedName) || models[0];
  const max = Math.max(1, ...models.map((item) => item.agg.total));
  if (!selected) return null;
  return part === 'list' ? (<div className="usage-surface usage-model-list"><h3>{t('usage_models_title')}</h3>
      {models.map((item) => <button type="button" key={item.model} className={selected.model === item.model ? 'is-selected' : ''} onClick={() => onSelect(item.model)}>
        <span title={item.model}>{item.model}</span><i className="usage-model-track"><motion.i initial={false} animate={{ scaleX: item.agg.total / max }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING} style={{ width: '100%', transformOrigin: 'left' }} /></i><strong>{fmtTokens(item.agg.total, zh)}</strong>
      </button>)}
      {ledger.by_model?.length > models.length && <small className="usage-list-rest">{fill(t('usage_models_rest'), { n: ledger.by_model.length, k: models.length })}</small>}
    </div>) : (<div className="usage-surface usage-model-detail"><div className="usage-model-detail-head"><h3 title={selected.model}>{selected.model}</h3><strong>{fmtInt(selected.agg.total)} Tokens</strong></div>
      <p>{t('usage_token_mix')}</p><div className="usage-model-stack">{TOKEN_PARTS.map(([key, , , color]) => <motion.i key={key} initial={false} animate={{ flexGrow: selected.agg[key] }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING} style={{ background: color }} />)}</div>
      <div className="usage-model-parts">{TOKEN_PARTS.map(([key, cn, en, color]) => <div key={key}><i style={{ background: color }} /><span>{tr(zh, cn, en)}</span><strong>{fmtInt(selected.agg[key])}</strong></div>)}</div>
      <small>{t('usage_reasoning_note')}</small>
    </div>);
}

// 实测速度榜:tok/s 降序,行内附首字延迟与计时次数;数据来自会话文件时间戳,纯只读.
function SpeedBoard({ ledger, zh, lang }) {
  const reduced = useMotionPreference();
  const t = (key) => STR[key][lang];
  const board = useMemo(() => speedBoard(ledger?.by_model, 5), [ledger]);
  const max = Math.max(1, ...board.rows.map((row) => row.tokPerSec));
  return <div className="usage-surface usage-model-list usage-speed">
    <h3>{t('usage_speed_title')}</h3>
    {board.rows.length === 0 && <p className="usage-speed-empty">{t('usage_speed_empty')}</p>}
    {board.rows.map((row) => <div className="usage-speed-row" key={row.model}>
      <span title={row.model}>{row.model}</span>
      <i className="usage-model-track"><motion.i initial={false} animate={{ scaleX: row.tokPerSec / max }} transition={reduced ? { duration: 0 } : LAYOUT_SPRING} style={{ width: '100%', transformOrigin: 'left', background: 'var(--usage-part-output)' }} /></i>
      <strong>{fmtTokens(Math.round(row.tokPerSec), zh)}<em> t/s</em></strong>
      <small>{fill(t('usage_speed_meta'), { ttft: fmtDurationMs(row.ttftMs), n: row.timedCalls })}</small>
    </div>)}
    {board.unTimed > 0 && <small className="usage-list-rest">{fill(t('usage_speed_rest'), { n: board.unTimed })}</small>}
  </div>;
}

// 模块清单(id → 文案键):跳转面板按清单生成条目,与看板模块一一对应。
export const USAGE_MODULE_LABELS = [
  ['trend', 'usage_trend_title'], ['tokens', 'usage_panel_tokens'], ['requests', 'usage_panel_requests'],
  ['cost', 'usage_panel_cost'], ['value', 'usage_panel_value'], ['achievements', 'panel_achievements'],
  ['cache', 'panel_cache'], ['speed', 'usage_speed_title'], ['models', 'usage_models_title'],
  ['modelMix', 'usage_model_mix_title'], ['heat', 'panel_heat'], ['punch', 'panel_punch'],
  ['clock', 'panel_clock'], ['week', 'panel_week'], ['sources', 'panel_sources'],
  ['projects', 'panel_projects'], ['sessions', 'panel_sessions'], ['alltime', 'panel_alltime'],
  ['streak', 'panel_streak'], ['records', 'panel_records'],
];

export default function UsageDashboard({ ledger, monthlyValue, achItems, lang, currency, onOpenSettings, onOpenAchievements, editing, onToggleEditing }) {
  const zh = lang === 0;
  const t = (key) => STR[key][lang];
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
  useEffect(() => registerNativeCallbacks({ __onExportResult: (status) => setExportStatus(status) }), []);
  if (!ledger) return null;
  const recent = ledger.recent_by_agent?.[0];
  const clear = () => { setRange('7d'); setAgent(''); setCustomStart(''); setCustomEnd(''); };
  const exportUsage = (format) => {
    if (!summary.valid) return;
    setExportStatus('');
    sendCommand({ cmd: 'exportUsage', format, start: bounds.start, end: bounds.end, agent: agent || null, project: null });
  };
  const rangeNote = bounds.start && bounds.end
    ? (bounds.start.slice(0, 4) === bounds.end.slice(0, 4) ? `${bounds.start.slice(5)} → ${bounds.end.slice(5)}` : `${bounds.start} → ${bounds.end}`)
    : '—';
  const value = monthlyValue?.month_api_equivalent_usd || 0;
  const multiple = monthlyValue?.list_price_multiple || 0;

  const panelBodies = (id) => {
    const Body = PANEL_BODIES[id];
    if (id === 'achievements') return <AchievementsPanel items={achItems} lang={lang} onOpen={onOpenAchievements} />;
    return <PanelErrorBoundary>{Body ? <Body ledger={ledger} lang={lang} currency={currency} /> : null}</PanelErrorBoundary>;
  };

  const modules = [
    { id: 'tokens', title: t('usage_panel_tokens'), description: t('catalog_tokens'), w: 3, h: 1, minW: 3, minH: 1, bare: true,
      content: <SummaryCard title={t('usage_panel_tokens')} value={summary.total.total} format={(n) => fmtTokens(n, zh)} note={rangeNote} tone="var(--sum-purple)" /> },
    { id: 'requests', title: t('usage_panel_requests'), description: t('catalog_requests'), w: 3, h: 1, minW: 3, minH: 1, bare: true,
      content: <SummaryCard title={t('usage_panel_requests')} value={summary.total.messages} format={(n) => fmtInt(Math.round(n))} note={rangeNote} tone="var(--sum-orange)" /> },
    { id: 'cost', title: t('usage_panel_cost'), description: t('catalog_cost'), w: 3, h: 1, minW: 3, minH: 1, bare: true,
      content: <SummaryCard title={t('usage_summary_cost')} value={summary.total.cost} format={(n) => formatCurrency(n, currency)} note={summary.unpriced > 0 ? fill(t('usage_unpriced_partial'), { n: summary.unpriced }) : t('usage_summary_cost_hint')} tone="var(--usage-part-output)" /> },
    { id: 'value', title: t('usage_panel_value'), description: t('catalog_value'), w: 3, h: 1, minW: 3, minH: 1, bare: true,
      content: <SummaryCard title={t('usage_summary_value')} value={value} format={(n) => formatCurrency(n, currency)} note={value > 0 ? `${t('usage_summary_value_hint')} · ×${multiple.toFixed(1)}` : t('usage_summary_local')} tone="var(--color-text-green)" /> },
    { id: 'trend', title: t('usage_trend_title'), description: t('catalog_trend'), w: 12, h: 4, minW: 6, minH: 3, bare: true,
      content: (<section className="usage-surface usage-chart-card"><div className="usage-chart-heading"><div><h2>{t('usage_trend_title')}</h2><p>{t('usage_trend_hint')}</p></div><div className="usage-toggles"><MetricSelector label={t('usage_metric')} options={[['tokens', 'Tokens'], ['messages', t('usage_metric_messages')], ['cost', t('metric_cost')]]} value={metric} onChange={setMetric} />{agent && metric === 'tokens' && <MetricSelector label={t('usage_breakdown')} options={[['agent', t('usage_breakdown_agent')], ['type', t('usage_breakdown_type')]]} value={mode} onChange={setMode} />}</div></div>
      <TrendChart days={trendDays} metric={metric} agent={agent} mode={mode} names={names} zh={zh} currency={currency} lang={lang} /></section>) },
    { id: 'achievements', title: t('panel_achievements'), description: t('catalog_achievements'), w: 12, h: 3, minW: 6, minH: 2, content: panelBodies('achievements') },
    { id: 'cache', title: t('panel_cache'), description: t('catalog_cache'), w: 4, h: 2, minW: 3, minH: 2, content: panelBodies('cache') },
    { id: 'speed', title: t('usage_speed_title'), description: t('catalog_speed'), w: 8, h: 2, minW: 4, minH: 2, bare: true, content: <SpeedBoard ledger={ledger} zh={zh} lang={lang} /> },
    { id: 'models', title: t('usage_models_title'), description: t('catalog_models'), w: 6, h: 4, minW: 4, minH: 3, bare: true, content: <ModelExplorer part="list" lang={lang} zh={zh} selectedName={selectedModel} onSelect={setSelectedModel} ledger={ledger} /> },
    { id: 'modelMix', title: t('usage_model_mix_title'), description: t('catalog_modelMix'), w: 6, h: 4, minW: 4, minH: 3, bare: true, content: <ModelExplorer part="detail" lang={lang} zh={zh} selectedName={selectedModel} onSelect={setSelectedModel} ledger={ledger} /> },
    ...PANEL_CATALOG.filter((entry) => !['tokens', 'requests', 'cost', 'value', 'trend', 'achievements', 'cache', 'speed', 'models', 'modelMix'].includes(entry.id)).map((entry) => ({
      ...entry,
      title: STR[`panel_${entry.id}`][lang],
      description: STR[`catalog_${entry.id}`][lang],
      content: panelBodies(entry.id),
    })),
  ];

  return <div className="usage-page usage-dashboard">
    <div className="usage-head">
      <p className="usage-subtitle">{t('usage_subtitle')}</p>
      <div className="usage-activity"><i /><span className="usage-activity-main">{t('usage_recent')} · {recent ? `${recent.label} ${fmtTokens(recent.agg.total, zh)} Tokens` : t('usage_no_activity')}{ledger.burn_rate != null && <span> · {fmtTokens(Math.round(ledger.burn_rate), zh)} {t('usage_tokens_per_min')}</span>}</span>{ledger.last_activity_ms && <small>{t('last_activity')} {fmtRel(Date.now() - ledger.last_activity_ms)}</small>}</div>
    </div>
    <div className="usage-filters"><div className="usage-filter-left">
      <label className="usage-select"><span>{t('period_range')}</span><select value={range} onChange={(event) => setRange(event.target.value)} aria-label={t('period_range')}>{RANGES.map(([key, label]) => <option key={key} value={key}>{t(label)}</option>)}</select></label>
      <label className="usage-select"><span>Agent</span><select value={agent} onChange={(event) => setAgent(event.target.value)} aria-label="Agent"><option value="">{t('period_all')}</option>{agents.map(([key, name]) => <option key={key} value={key}>{name}</option>)}</select></label>
      {(range !== '7d' || agent) && <button type="button" className="usage-clear" onClick={clear}>{t('period_clear')}</button>}
    </div>
      <ExportMenu label={t('export_label')} valid={summary.valid} onExport={exportUsage} />
    </div>
    {range === 'custom' && <div className="usage-custom"><label>{t('period_from')}<input type="date" value={customStart} max={customEnd || today} onChange={(event) => setCustomStart(event.target.value)} /></label><label>{t('period_to')}<input type="date" value={customEnd} min={customStart || undefined} max={today} onChange={(event) => setCustomEnd(event.target.value)} /></label></div>}
    {!summary.valid && <p className="usage-feedback" role="alert">{t('period_invalid')}</p>}
    {exportStatus && <p className="usage-feedback" role="status">{{ saved: t('export_saved'), canceled: '', error: t('export_error'), unavailable: t('export_unavailable') }[exportStatus]}</p>}
    <ModuleBoard
      storageKey="token-monitor.usage-layout.v4"
      lang={lang}
      catalog={modules}
      defaults={DEFAULT_PANELS}
      editing={editing}
      meta={(id) => panelMeta(id, ledger, lang)}
      renderModule={(id) => modules.find((item) => item.id === id)?.content}
    />
  </div>;
}
