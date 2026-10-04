// 分享页:两个"把数据带出去"的形态——桌面分享卡(PNG)与局域网看板(手机/平板)。
// 编辑器工具条在画布外;画布内只保留模块本身,编辑所见即导出所得。
import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import ModuleBoard from './ModuleBoard.jsx';
import { STR, fmtInt, fmtTokens, fmtCost } from './ui-data.js';
import { sendCommand } from './gui-bridge.js';
import { bumpFunCounter, readFunCounters } from './fun-counters.js';
import { AchievementGlyph } from './Achievements.jsx';
import { evaluateAchievements, readAchievementState, unlockedNonMetaCount, TIER_ORDER } from './achievements.js';
import { snapdom } from '@zumer/snapdom';
import qrcode from 'qrcode-generator';

const PALETTE_KEY = 'token-monitor.share-palette.v1';
const SHARE_LAYOUT_KEY = 'token-monitor.share-layout.v1';

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

// 局域网看板:开关 + 地址 + 二维码;数据只读、不含密钥。
function LanBoard({ lang, lan, onToggle }) {
  const t = (key) => STR[key][lang];
  const [copied, setCopied] = useState(false);
  const [showQr, setShowQr] = useState(false);
  const copyUrl = () => {
    if (!lan.url) return;
    navigator.clipboard.writeText(lan.url).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };
  // 白底容器保证扫码对比度;scalable 让 SVG 自适应容器
  const qrSvg = showQr && lan.url ? (() => {
    const qr = qrcode(0, 'M');
    qr.addData(lan.url);
    qr.make();
    return qr.createSvgTag({ cellSize: 4, margin: 1, scalable: true });
  })() : null;
  return (
    <section className="lan-board">
      <header className="lan-board-head">
        <div>
          <span className="eyebrow">// {t('lan_title')}</span>
          <h2>{t('lan_title')}</h2>
          <p>{t('lan_hint')}</p>
        </div>
        <div className="lan-board-switch" role="group" aria-label={t('lan_title')}>
          <button type="button" className={!lan.enabled ? 'is-selected' : ''} onClick={() => { onToggle(false); setShowQr(false); }}>{t('lan_off')}</button>
          <button type="button" className={lan.enabled ? 'is-selected' : ''} onClick={() => onToggle(true)}>{t('lan_on')}</button>
        </div>
      </header>
      {lan.enabled && (
        <div className="lan-board-body">
          <div className="lan-board-status" role="status">
            <span className={`lan-dot${lan.url ? ' is-on' : ''}`} />
            <code>{lan.url || t('lan_stopped')}</code>
            {lan.url && <button type="button" onClick={() => setShowQr((open) => !open)} aria-expanded={showQr}>{t('lan_qr')}</button>}
            {lan.url && <button type="button" onClick={copyUrl}>{copied ? t('lan_copied') : t('lan_copy')}</button>}
          </div>
          {qrSvg && <div className="lan-qr" title={lan.url} dangerouslySetInnerHTML={{ __html: qrSvg }} />}
          <small className="lan-security">{t('lan_security')}</small>
        </div>
      )}
    </section>
  );
}

// 模块主体都是「mono 眉题 + 大数字/分块条」的同族语言,导出后与编辑器部件分离。
export default function SharePage({ ledger, monthlyValue, lang, shareStatus, lan, onLanToggle }) {
  const t = (key) => STR[key][lang];
  const zh = lang === 0;
  const canvasRef = useRef(null);
  const viewportRef = useRef(null);
  const [tab, setTab] = useState('card');
  const [exporting, setExporting] = useState(false);
  const [scale, setScale] = useState(1);
  const [boardKey, setBoardKey] = useState(0);
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
  }, [tab]);

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

  // 勋章行:稀有度优先(档位高在前),同级取最近解锁,最多 3 枚;与壳层成就同一套评估。
  const badges = useMemo(() => {
    if (!ledger) return [];
    const state = readAchievementState();
    const extras = { monthlyValue, counters: readFunCounters(), unlockedCount: unlockedNonMetaCount(state) };
    const tierRank = (tier) => TIER_ORDER.indexOf(tier);
    return evaluateAchievements(ledger, extras)
      .filter((item) => item.unlocked)
      .sort((a, b) => tierRank(b.tier) - tierRank(a.tier) || (state.unlocked[b.id] || 0) - (state.unlocked[a.id] || 0))
      .slice(0, 3);
  }, [ledger, monthlyValue]);

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
          <strong className="share-number">{hasToday ? fmtTokens(L.today.total, zh) : '—'}</strong>
          <div className="share-hero-sub">
            <span>{fmtInt(L.today.messages)} {zh ? '次请求' : 'requests'}</span>
            {L.burn_rate != null && L.burn_rate > 0 && <span>{zh ? '燃烧' : 'burn'} {fmtTokens(Math.round(L.burn_rate), zh)}/min</span>}
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
              <i key={hour} className={hour === peakHour && value > 0 ? 'is-peak' : ''} style={{ height: `${Math.max(3, (value / hourMax) * 100)}%` }} title={`${hour}:00`} />
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
              <span className="share-rank-value">{fmtTokens(row.agg.total, zh)}</span>
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
              <span className="share-chip" key={row.client}>{row.label || row.client}<b>{fmtTokens(row.agg.total, zh)}</b></span>
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
            {days.map((day) => <i key={day.date} style={{ height: `${Math.max(3, (day.tokens / dayMax) * 100)}%` }} title={`${day.date} · ${fmtTokens(day.tokens, zh)}`} />)}
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
            <span>{zh ? '本月' : 'Month'} {fmtTokens(L.month.total, zh)}</span>
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
    {
      id: 'achievements', title: t('share_mod_achievements'), w: 12, h: 2, minW: 6, minH: 2,
      content: (
        <div className="share-block">
          <span className="share-eyebrow">// {zh ? '成就' : 'ACHIEVEMENTS'}</span>
          <div className="share-ach-row">
            {badges.map((item) => (
              <span className={`share-ach is-${item.tier}`} key={item.id}>
                <span className="share-ach-glyph"><AchievementGlyph glyph={item.glyph} /></span>
                <b>{item.title[lang]}</b>
              </span>
            ))}
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
    if (module.id === 'achievements') return badges.length > 0;
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

  const resetLayout = () => {
    try { localStorage.removeItem(SHARE_LAYOUT_KEY); } catch { /* session only */ }
    setBoardKey((value) => value + 1);
  };

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
      bumpFunCounter('share');
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
      <div className="share-tabs" role="tablist" aria-label={t('nav_share')}>
        <button type="button" role="tab" aria-selected={tab === 'card'} className={tab === 'card' ? 'is-selected' : ''} onClick={() => setTab('card')}>{t('share_tab_card')}</button>
        <button type="button" role="tab" aria-selected={tab === 'lan'} className={tab === 'lan' ? 'is-selected' : ''} onClick={() => setTab('lan')}>{t('share_tab_lan')}</button>
      </div>
      {tab === 'card' ? <>
        <div className="share-toolbar">
          <div className="share-palettes" role="group" aria-label={t('share_palette')}>
            <button type="button" className={palette === 'deep' ? 'is-selected' : ''} onClick={() => setPalette('deep')}>{t('share_palette_deep')}</button>
            <button type="button" className={palette === 'paper' ? 'is-selected' : ''} onClick={() => setPalette('paper')}>{t('share_palette_paper')}</button>
          </div>
          <span className="share-hint">{t('share_hint')}</span>
          <button type="button" className="share-tool" onClick={resetLayout}>{t('panels_reset')}</button>
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
              <ModuleBoard key={boardKey} storageKey={SHARE_LAYOUT_KEY} lang={lang} catalog={withData} defaults={defaults} hideTools renderModule={(id) => withData.find((module) => module.id === id)?.content} />
              <footer className="share-foot">{t('share_footer')}</footer>
            </div>
          </div>
        </div>
      </> : <LanBoard lang={lang} lan={lan} onToggle={onLanToggle} />}
    </div>
  );
}
