import React, { useEffect, useMemo, useState } from 'react';

const dayNumber = (date) => {
  const [year, month, day] = date.split('-').map(Number);
  return Date.UTC(year, month - 1, day) / 86400_000;
};
const dateFromDay = (day) => new Date(day * 86400_000).toISOString().slice(0, 10);
const localToday = () => {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
};
const compact = (n) => new Intl.NumberFormat(undefined, { notation: 'compact', maximumFractionDigits: 1 }).format(n);
const usd = (n) => `$${Number(n).toFixed(2)}`;
const summary = (rows) => rows.reduce((sum, row) => ({
  tokens: sum.tokens + row.agg.total,
  messages: sum.messages + row.agg.messages,
  reported: sum.reported + row.agg.cost_reported,
  estimated: sum.estimated + row.agg.cost_estimated,
  unpriced: sum.unpriced + row.unpriced_messages,
}), { tokens: 0, messages: 0, reported: 0, estimated: 0, unpriced: 0 });

export default function AnalysisPanel({ ledger, lang }) {
  const zh = lang === 0;
  const rows = ledger.analysis_rows || [];
  const [end, setEnd] = useState(localToday);
  const [start, setStart] = useState(() => dateFromDay(dayNumber(localToday()) - 13));
  const [agent, setAgent] = useState('');
  const [project, setProject] = useState('');
  const [exportStatus, setExportStatus] = useState('');
  useEffect(() => {
    window.__onExportResult = (status) => setExportStatus(status === 'canceled' ? '' : status);
    return () => { delete window.__onExportResult; };
  }, []);

  const agents = useMemo(() => [...new Set(rows.map((row) => row.client))].sort(), [rows]);
  const projects = useMemo(() => [...new Set(rows.map((row) => row.project).filter(Boolean))].sort(), [rows]);
  const validRange = !!start && !!end && dayNumber(start) <= dayNumber(end);
  const span = validRange ? dayNumber(end) - dayNumber(start) + 1 : 0;
  const previousStart = validRange ? dateFromDay(dayNumber(start) - span) : '';
  const previousEnd = validRange ? dateFromDay(dayNumber(start) - 1) : '';
  const matching = useMemo(() => rows.filter((row) => (!agent || row.client === agent) && (!project || row.project === project)), [rows, agent, project]);
  const current = validRange ? matching.filter((row) => row.date >= start && row.date <= end) : [];
  const previous = validRange ? matching.filter((row) => row.date >= previousStart && row.date <= previousEnd) : [];
  const currentTotal = summary(current);
  const previousTotal = summary(previous);
  const diff = (now, before) => before > 0 ? `${now >= before ? '+' : ''}${(((now - before) / before) * 100).toFixed(1)}%` : (now > 0 ? (zh ? '上期无记录' : 'No prior data') : '—');
  const byDay = [...current.reduce((map, row) => {
    const old = map.get(row.date) || { date: row.date, tokens: 0, cost: 0, unpriced: 0 };
    old.tokens += row.agg.total;
    old.cost += row.agg.cost;
    old.unpriced += row.unpriced_messages;
    map.set(row.date, old);
    return map;
  }, new Map()).values()].sort((a, b) => b.date.localeCompare(a.date));
  const requestExport = (format) => {
    if (!validRange) return;
    setExportStatus('');
    window.ipc?.postMessage?.(JSON.stringify({ cmd: 'exportUsage', format, start, end, agent: agent || null, project: project || null }));
  };

  return <section className="analysis-panel" aria-label={zh ? '区间分析' : 'Range analysis'}>
    <div className="analysis-heading">
      <div><strong>{zh ? '区间分析' : 'Range analysis'}</strong></div>
      <div className="usage-export-actions">
        <button type="button" disabled={!validRange} onClick={() => requestExport('csv')}>CSV</button>
        <button type="button" disabled={!validRange} onClick={() => requestExport('json')}>JSON</button>
      </div>
    </div>
    <div className="analysis-controls">
      <label>{zh ? '开始' : 'From'}<input type="date" value={start} max={end || undefined} onChange={(event) => setStart(event.target.value)} /></label>
      <label>{zh ? '结束' : 'To'}<input type="date" value={end} min={start || undefined} onChange={(event) => setEnd(event.target.value)} /></label>
      <label>Agent<select value={agent} onChange={(event) => setAgent(event.target.value)}><option value="">{zh ? '全部 Agent' : 'All agents'}</option>{agents.map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
      <label>{zh ? '项目' : 'Project'}<select value={project} onChange={(event) => setProject(event.target.value)}><option value="">{zh ? '全部项目' : 'All projects'}</option>{projects.map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
    </div>
    {!validRange && <p className="settings-form-error" role="alert">{zh ? '日期无效' : 'Invalid dates'}</p>}
    <div className="analysis-summary">
      <div><span>Tokens</span><strong>{compact(currentTotal.tokens)}</strong><small>{diff(currentTotal.tokens, previousTotal.tokens)} · {zh ? '上期' : 'prior'} {compact(previousTotal.tokens)}</small></div>
      <div><span>{zh ? '已知成本' : 'Known cost'}{currentTotal.unpriced > 0 ? (zh ? ' · 部分汇总' : ' · partial') : ''}</span><strong>{usd(currentTotal.reported + currentTotal.estimated)}</strong><small>{diff(currentTotal.reported + currentTotal.estimated, previousTotal.reported + previousTotal.estimated)} · {zh ? '上期' : 'prior'} {usd(previousTotal.reported + previousTotal.estimated)}</small></div>
      <div><span>{zh ? '请求数' : 'Messages'}</span><strong>{compact(currentTotal.messages)}</strong><small>{diff(currentTotal.messages, previousTotal.messages)} · {zh ? '上期' : 'prior'} {compact(previousTotal.messages)}</small></div>
    </div>
    {exportStatus && <p role="status" className="analysis-provenance">{{ saved: zh ? '文件已保存' : 'File saved', error: zh ? '导出失败' : 'Export failed', unavailable: zh ? '暂无数据' : 'No data' }[exportStatus]}</p>}
    <details className="analysis-detail"><summary>{zh ? `逐日明细（${byDay.length} 天有记录）` : `Daily detail (${byDay.length} days with data)`}</summary>
      <div className="analysis-table" role="table">{byDay.length === 0 ? <p>{zh ? '所选条件下暂无记录' : 'No records for these filters'}</p> : byDay.map((day) => <div role="row" key={day.date}><span>{day.date}</span><span>{compact(day.tokens)} tokens</span><span>{usd(day.cost)}{day.unpriced > 0 ? ' *' : ''}</span></div>)}</div>
    </details>
  </section>;
}
