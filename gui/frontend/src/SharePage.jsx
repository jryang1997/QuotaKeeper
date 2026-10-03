// 分享卡编辑器:1080 宽画布上复用 ModuleBoard 自由拖拽/缩放数据模块,
// snapdom 把画布光栅化成 PNG 经 IPC 存盘。画布自带品牌头,导出时隐藏编辑器部件。
import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import ModuleBoard from './ModuleBoard.jsx';
import { STR, fmtInt, fmtTokens, fmtCost } from './ui-data.js';
import { sendCommand } from './gui-bridge.js';
import { snapdom } from '@zumer/snapdom';

const PALETTE_KEY = 'token-monitor.share-palette.v1';

function todayStamp(lang) {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, '0');
  const d = String(now.getDate()).padStart(2, '0');
  return lang === 0 ? `${y}.${m}.${d}` : `${y}-${m}-${d}`;
}

function peakHourOf(hourly) {
  let best = 0;
  hourly.forEach((value, hour) => {
    if (value > (hourly[best] || 0)) best = hour;
  });
  return best;
}

// 模块主体都是「mono 眉题 + 大数字/分块条」的同族语言,导出后与编辑器部件分离。
export default function SharePage({ ledger, monthlyValue, lang, shareStatus }) {
  const t = (key) => STR[key][lang];
  const zh = lang === 0;
  const canvasRef = useRef(null);
  const viewportRef = useRef(null);
  const [exporting, setExporting] = useState(false);
  const [scale, setScale] = useState(1);
  const [palette, setPalette] = useState(() => {
    try { return localStorage.getItem(PALETTE_KEY) === 'paper' ? 'paper' : 'deep'; } catch { return 'deep'; }
  });

  useEffect(() => {
    try { localStorage.setItem(PALETTE_KEY, palette); } catch { /* session only */ }
  }, [palette]);

  useLayoutEffect(() => {
    const fit = () => {
      const available = (viewportRef.current?.clientWidth || 800) - 8;
      setScale(Math.min(1, Math.max(0.34, available / 1080)));
    };
    fit();
    const observer = new ResizeObserver(fit);
    if (viewportRef.current) observer.observe(viewportRef.current);
    return () => observer.disconnect();
  }, []);

  const data = useMemo(() => {
    const L = ledger;
    if (!L) return null;
    const hourly = L.hourly || Array.from({ length: 24 }, () => 0);
    const models = [...(L.by_model || [])].sort((a, b) => b.agg.total - a.agg.total).slice(0, 5);
    const agents = [...(L.by_agent || [])].sort((a, b) => b.agg.total - a.agg.total).slice(0, 4);
    const days = (L.daily_totals || []).slice(-14);
    const peakHour = peakHourOf(hourly);
    return { L, hourly, models, agents, days, peakHour };
  }, [ledger]);

  if (!data) return null;
  const { L, hourly, models, agents, days, peakHour } = data;
  const hourMax = Math.max(1, ...hourly);
  const modelMax = Math.max(1, ...models.map((row) => row.agg.total));
  const dayMax = Math.max(1, ...days.map((day) => day.tokens));
  const hasToday = L.today.total > 0 || L.today.messages > 0;

  const catalog = [
    {
      id: 'hero', title: t('share_mod_hero'), w: 12, h: 3, minW: 6, minH: 2,
      content: (
        <div className="share-block share-hero">
          <span className="share-eyebrow">// {zh ? '今日 TOKEN' : "TODAY'S TOKENS"}</span>
          <strong className="share-number">{hasToday ? fmtTokens(L.today.total) : '—'}</strong>
          <div className="share-hero-sub">
            <span>{fmtInt(L.today.messages)} {zh ? '次请求' : 'requests'}</span>
            {L.burn_rate != null && L.burn_rate > 0 && <span>{zh ? '燃烧' : 'burn'} {fmtTokens(Math.round(L.burn_rate))}/min</span>}
          </div>
        </div>
      ),
    },
    {
      id: 'clock', title: t('share_mod_clock'), w: 6, h: 4, minW: 4, minH: 3,
      content: (
        <div className="share-block">
          <span className="share-eyebrow">// {zh ? '活跃时段 · 24H' : 'ACTIVE HOURS · 24H'}</span>
          <div className="share-hour-bars">
            {hourly.map((value, hour) => (
              <i key={hour} className={hour === data.peakHour && value > 0 ? 'is-peak' : ''} style={{ height: `${Math.max(3, (value / hourMax) * 100)}%` }} title={`${hour}:00`} />
            ))}
          </div>
          <div className="share-hour-scale"><span>0</span><span>6</span><span>12</span><span>18</span><span>23</span></div>
          <div className="share-caption">{zh ? `最活跃 ${String(peakHour).padStart(2, '0')}:00 前后` : `Peaks around ${String(peakHour).padStart(2, '0')}:00`}</div>
        </div>
      ),
    },
    {
      id: 'models', title: t('share_mod_models'), w: 6, h: 4, minW: 4, minH: 3,
      content: (
        <div className="share-block">
          <span className="share-eyebrow">// {zh ? '最常用模型' : 'TOP MODELS'}</span>
          {models.length === 0 && <div className="share-caption">{t('share_empty')}</div>}
          {models.map((row, index) => (
            <div className="share-rank" key={row.model}>
              <span className="share-rank-name" title={row.model}>{index + 1} · {row.model}</span>
              <div className="share-rank-bar"><i style={{ width: `${Math.max(4, (row.agg.total / modelMax) * 100)}%` }} /></div>
              <span className="share-rank-value">{fmtTokens(row.agg.total)}</span>
            </div>
          ))}
        </div>
      ),
    },
    {
      id: 'agents', title: t('share_mod_agents'), w: 6, h: 3, minW: 4, minH: 2,
      content: (
        <div className="share-block">
          <span className="share-eyebrow">// {zh ? 'CODING AGENT' : 'BY AGENT'}</span>
          <div className="share-chips">
            {agents.map((row) => (
              <span className="share-chip" key={row.client}>{row.label || row.client}<b>{fmtTokens(row.agg.total)}</b></span>
            ))}
            {agents.length === 0 && <div className="share-caption">{t('share_empty')}</div>}
          </div>
        </div>
      ),
    },
    {
      id: 'days', title: t('share_mod_days'), w: 6, h: 3, minW: 4, minH: 2,
      content: (
        <div className="share-block">
          <span className="share-eyebrow">// {zh ? '近 14 天' : 'LAST 14 DAYS'}</span>
          <div className="share-day-bars">
            {days.map((day) => <i key={day.date} style={{ height: `${Math.max(3, (day.tokens / dayMax) * 100)}%` }} title={`${day.date} · ${fmtTokens(day.tokens)}`} />)}
          </div>
        </div>
      ),
    },
    {
      id: 'value', title: t('share_mod_value'), w: 6, h: 2, minW: 4, minH: 2,
      content: (
        <div className="share-block">
          <span className="share-eyebrow">// {zh ? '本月 API 等价价值' : 'MONTHLY VALUE'}</span>
          <strong className="share-number share-number-sm">{fmtCost(monthlyValue?.month_api_equivalent_usd ?? L.month.cost())}</strong>
          {monthlyValue?.list_price_multiple > 0 && (
            <div className="share-caption">{zh ? '约等于包月额度的' : `≈ ${monthlyValue.list_price_multiple.toFixed(1)}× of a monthly plan`} {monthlyValue.list_price_multiple.toFixed(1)}×</div>
          )}
        </div>
      ),
    },
    {
      id: 'summary', title: t('share_mod_summary'), w: 6, h: 2, minW: 4, minH: 2,
      content: (
        <div className="share-block">
          <span className="share-eyebrow">// {zh ? '今日之最' : 'HIGHLIGHTS'}</span>
          <div className="share-summary">
            <span>{zh ? '最活跃' : 'Peak'} {String(peakHour).padStart(2, '0')}:00</span>
            <span>{zh ? '本月' : 'Month'} {fmtTokens(L.month.total)}</span>
            {(L.weekday || []).length > 0 && (() => {
              const names = zh ? ['一', '二', '三', '四', '五', '六', '日'] : ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
              let best = 0;
              L.weekday.forEach((value, index) => { if (value > L.weekday[best]) best = index; });
              return <span>{zh ? '最勤' : 'Top day'} {names[best]}</span>;
            })()}
          </div>
        </div>
      ),
    },
  ];
  const withData = catalog.filter((module) => {
    if (module.id === 'hero') return hasToday;
    if (module.id === 'models') return models.length > 0;
    if (module.id === 'agents') return agents.length > 0;
    if (module.id === 'days') return days.length > 1;
    return true;
  });
  let row = 0;
  let col = 0;
  let rowMaxH = 0;
  const defaults = withData.map((module) => {
    if (col + module.w > 12) {
      col = 0;
      row += rowMaxH;
      rowMaxH = 0;
    }
    const panel = { id: module.id, x: col, y: row, w: module.w, h: module.h, locked: false };
    col += module.w;
    rowMaxH = Math.max(rowMaxH, module.h);
    return panel;
  });

  const exportPng = async () => {
    const canvas = canvasRef.current;
    if (!canvas || exporting) return;
    setExporting(true);
    try {
      canvas.classList.add('is-exporting');
      await new Promise((resolve) => setTimeout(resolve, 80));
      const image = await snapdom.toPng(canvas, { scale: 1 });
      // 诊断钩子:留存最近一次导出的 dataURL(内存即弃),验证与排障用
      window.__lastShareDataUrl = image.src;
      sendCommand({ cmd: 'saveShareImage', image: image.src });
    } catch (error) {
      // 诊断钩子:CDP 验证与用户排障都靠它定位 snapdom 失败原因
      window.__shareExportError = String(error?.stack || error);
      window.__onShareImageSaved?.('error');
    } finally {
      canvas.classList.remove('is-exporting');
      setExporting(false);
    }
  };

  return (
    <div className="share-page">
      <div className="share-toolbar">
        <div className="share-palettes" role="group" aria-label={t('share_palette')}>
          <button type="button" className={palette === 'deep' ? 'is-selected' : ''} onClick={() => setPalette('deep')}>{t('share_palette_deep')}</button>
          <button type="button" className={palette === 'paper' ? 'is-selected' : ''} onClick={() => setPalette('paper')}>{t('share_palette_paper')}</button>
        </div>
        <span className="share-hint">{t('share_hint')}</span>
        <button type="button" className="share-export" onClick={exportPng} disabled={exporting}>{exporting ? t('share_exporting') : t('share_export')}</button>
      </div>
      {shareStatus && <div className={`share-status is-${shareStatus}`} role="status">{t(`share_${shareStatus}`)}</div>}
      <div className="share-viewport" ref={viewportRef}>
        <div style={{ height: (canvasRef.current?.offsetHeight || 0) * scale, minHeight: 200 }}>
          <div className={`share-canvas is-${palette}`} ref={canvasRef} style={{ transform: `scale(${scale})` }}>
            <header className="share-brand">
              <span className="share-brand-mark" />
              <span>TOKEN MONITOR · {todayStamp(lang)}</span>
            </header>
            <ModuleBoard storageKey="token-monitor.share-layout.v1" lang={lang} catalog={withData} defaults={defaults} renderModule={(id) => withData.find((module) => module.id === id)?.content} />
            <footer className="share-foot">{zh ? '由 Token Monitor 生成' : 'Made with Token Monitor'}</footer>
          </div>
        </div>
      </div>
    </div>
  );
}
