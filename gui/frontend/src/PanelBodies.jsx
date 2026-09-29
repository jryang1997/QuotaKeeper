import React, { useState } from 'react';
import { Tooltip } from '@astryxdesign/core/Tooltip';
import { CLIENT_COLORS, fmtInt, fmtTokens, fmtCost } from './ui-data.js';
import BlockBar from './BlockBar.jsx';

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
          <span className="board-label" title={r.title || r.label}>{r.label}</span>
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
    .map((m) => ({
      key: m.model,
      label: m.model,
      v: m.agg.total,
      title: m.aliases?.length ? `${m.model} ← ${m.aliases.join(', ')}` : undefined,
    }));
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

export const PANEL_BODIES = {
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

export class PanelErrorBoundary extends React.Component {
  constructor(props) { super(props); this.state = { err: null }; }
  static getDerivedStateFromError(err) { return { err }; }
  render() {
    if (this.state.err) return <div className="panel-error">PANEL ERROR · {String(this.state.err?.message || this.state.err).slice(0, 160)}</div>;
    return this.props.children;
  }
}

export function panelMeta(id, ledger, lang) {
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
