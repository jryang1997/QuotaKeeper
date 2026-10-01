import ModuleBoard from './ModuleBoard.jsx';
import React, { useState, useRef } from 'react';
import { Card } from '@astryxdesign/core/Card';
import { Text } from '@astryxdesign/core/Text';
import { Tooltip } from '@astryxdesign/core/Tooltip';
import { PROVIDER_ACCENT, STATUS_COLOR, PROVIDER_LOGOS, SOURCE_PATHS, QUOTA_HINT, STR, fmtInt, fmtTokens, fmtCountdown } from './ui-data.js';
import BlockBar from './BlockBar.jsx';
import { AnimatePresence, motion } from 'motion/react';
import AnimatedNumber from './AnimatedNumber.jsx';
import { useMotionPreference } from './ui-motion.jsx';
import { FADE, LAYOUT_SPRING } from './motion-tuning.js';

function quotaPace(limit) {
  const label = limit.label.toLowerCase();
  const now = new Date();
  let totalMs;
  if (/5\s*h|5\s*hour|5\s*小时/.test(label)) totalMs = 5 * 3600_000;
  else if (/7\s*d|7\s*day|week|周/.test(label)) totalMs = 7 * 86400_000;
  else if (/month|月/.test(label)) totalMs = new Date(now.getFullYear(), now.getMonth() + 1, 1).getTime() - new Date(now.getFullYear(), now.getMonth(), 1).getTime();
  else return null;
  const resetMs = limit.resets_in_ms;
  if (resetMs == null || resetMs <= 0 || resetMs >= totalMs || limit.used_pct < 5) return null;
  const elapsed = totalMs - resetMs;
  if (elapsed < totalMs * 0.1) return null;
  if (limit.used_pct >= 100) return { eta: 'exhausted', delta: limit.used_pct - elapsed / totalMs * 100 };
  const eta = elapsed * (100 - limit.used_pct) / limit.used_pct;
  if (!Number.isFinite(eta) || eta <= 0) return null;
  return { eta: eta < resetMs ? eta : 'sustainable', delta: limit.used_pct - elapsed / totalMs * 100 };
}

function TrendBars({ values, color }) {
  const reduced = useMotionPreference();
  const max = Math.max(1, ...values);
  return (
    <div className="trend-bars" aria-hidden="true">
      {values.map((v, i) => (
        <motion.span
          key={i}
          initial={false}
          animate={{ scaleY: Math.max(0.04, v / max) }}
          transition={reduced ? { duration: 0 } : LAYOUT_SPRING}
          style={{
            height: '100%', transformOrigin: 'bottom',
            background: color,
            opacity: i === values.length - 1 ? 1 : 0.45,
          }}
        />
      ))}
    </div>
  );
}

function UsageSummaryRow({ ledger, goat, lang, part }) {
  const t = (k) => STR[k][lang];
  const zh = lang === 0;
  const now = new Date();
  const monthPrefix = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
  const monthDays = (ledger?.daily_totals || []).filter((d) => d.date.startsWith(monthPrefix));
  const tokens = goat ? goat.tokens : ledger?.month.total ?? 0;
  const requests = goat ? goat.requests : ledger?.month.messages ?? 0;
  const caption = goat ? t('usage_sum_official') : t('usage_sum_local');
  const content = (
    <div className="usage-summary-row">
      <div className="usage-summary-card">
        <div className="usage-summary-head" style={{ color: 'var(--sum-purple)' }}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 8v8M8 12h8" /></svg>
          <span>{t('usage_sum_tokens')}</span>
        </div>
        <div className="usage-summary-number"><AnimatedNumber value={tokens} format={fmtTokens} /><span>{t('usage_unit_tokens')}</span></div>
        <TrendBars values={monthDays.map((d) => d.tokens)} color="var(--sum-purple)" />
        <div className="usage-summary-caption">{caption}</div>
      </div>
      <div className="usage-summary-card">
        <div className="usage-summary-head" style={{ color: 'var(--sum-orange)' }}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13l11-6.5z" /></svg>
          <span>{t('usage_sum_runs')}</span>
        </div>
        <div className="usage-summary-number"><AnimatedNumber value={requests} format={(value) => fmtInt(Math.round(value))} /><span>{t('usage_unit_runs')}</span></div>
        <TrendBars values={monthDays.map((d) => d.messages)} color="var(--sum-orange)" />
        <div className="usage-summary-caption">{caption}</div>
      </div>
    </div>
  );
  return part ? content.props.children[part === 'tokens' ? 0 : 1] : content;
}

function ProviderLogo({ provider }) {
  const src = PROVIDER_LOGOS[provider];
  if (!src) return <span className="quota-logo quota-logo-fallback" aria-hidden="true" />;
  return <img className="quota-logo" src={src} alt="" aria-hidden="true" draggable="false" />;
}

function paceChipText(pace, lang) {
  const zh = lang === 0;
  if (pace.eta === 'sustainable') return zh ? '可撑到重置' : 'lasts to reset';
  if (pace.eta === 'exhausted') return zh ? '已耗尽' : 'exhausted';
  const d = Math.abs(pace.delta).toFixed(0);
  const eta = `≈${fmtCountdown(pace.eta)}`;
  return zh
    ? `${pace.delta >= 0 ? '快' : '慢'} ${d}pt · ${eta} 后耗尽`
    : `${d}pt ${pace.delta >= 0 ? 'ahead' : 'behind'} · ${eta} to empty`;
}

const QUOTA_CLIENT_OF = { 'opencode-go': 'opencode', codex: 'codex' };

const RING_SEGMENTS = 24;

// hero 分块环:24 段,pathLength=360 使 dasharray 直接用角度(每段 15°)。
// 填充 = round(pct/100×24) 段状态色;空段 = 同色 15% 透明(与套餐卡分块条同语言)。
// 大幅翻转(已用↔剩余)按顺时针波浪 stagger,日常刷新 ±1 段即时弹簧。
function QuotaRing({ pct, tone }) {
  const reduced = useMotionPreference();
  // 每段 15° 槽位:10° 实块 + 5° 间隙;平头(butt)让分块缝隙清晰可见
  const seg = 10;
  const gap = 15 - seg;
  const filled = Math.max(0, Math.min(RING_SEGMENTS, Math.round(pct / 100 * RING_SEGMENTS)));
  const prev = useRef(filled);
  const wave = Math.abs(filled - prev.current) > 3;
  prev.current = filled;
  const track = Array(RING_SEGMENTS).fill(`${seg} ${gap}`).join(' ');
  return (
    <svg className="quota-ring" viewBox="0 0 116 116" aria-hidden="true">
      <circle className="quota-ring-track" cx="58" cy="58" r="48" pathLength="360" strokeDasharray={track} style={{ color: tone }} />
      {Array.from({ length: RING_SEGMENTS }, (_, index) => <motion.circle key={index} className="quota-ring-fill" cx="58" cy="58" r="48" pathLength="360" strokeDasharray={`${seg} ${360 - seg}`} strokeDashoffset={-index * 15} style={{ color: tone }} initial={false} animate={{ opacity: index < filled ? 1 : 0 }} transition={reduced ? FADE : { ...LAYOUT_SPRING, delay: wave ? index * 0.012 : 0 }} />)}
    </svg>
  );
}

function ExpiryChip({ endsAtMs, lang }) {
  if (endsAtMs == null) return null;
  const days = Math.max(0, Math.ceil((endsAtMs - Date.now()) / 86_400_000));
  const date = new Date(endsAtMs);
  const md = `${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
  return <span className={`quota-expiry${days <= 7 ? ' quota-expiry-warning' : ''}`}>{lang === 0 ? `${md} 到期` : `expires ${md}`}</span>;
}

function DeepSeekBalanceCard({ balance, lang, pinned, accent, onTogglePin, onToggleHidden, onOpenSettings }) {
  const zh = lang === 0;
  const money = (amount, currency) => new Intl.NumberFormat(zh ? 'zh-CN' : 'en-US', { style: 'currency', currency, maximumFractionDigits: 2 }).format(amount);
  const status = balance?.status || 'missing_key';
  return <Card className="quota-card deepseek-balance-card" role="group" aria-label={zh ? 'DeepSeek API 余额' : 'DeepSeek API balance'} style={{ '--deepseek-accent': accent }}>
    <div className="quota-head">
      <ProviderLogo provider="deepseek" />
      <Text weight="medium" className="quota-title">DeepSeek API</Text>
      <span className="quota-plan-type">{zh ? '官方余额' : 'Official balance'}</span>
      <div className="quota-card-actions">
        <button type="button" onClick={onTogglePin} aria-label={pinned ? (zh ? '取消置顶 DeepSeek' : 'Unpin DeepSeek') : (zh ? '置顶 DeepSeek' : 'Pin DeepSeek')}>{pinned ? '◆' : '◇'}</button>
        <button type="button" onClick={onToggleHidden} aria-label={zh ? '隐藏 DeepSeek' : 'Hide DeepSeek'}>×</button>
      </div>
    </div>
    {status === 'ready' ? <>
      <div className="deepseek-balance-status">{balance.is_available ? (zh ? 'API 可用' : 'API available') : (zh ? 'API 当前不可用' : 'API unavailable')}</div>
      {(balance.balances || []).map((row) => <div className="deepseek-balance-row" key={row.currency}>
        <div className="deepseek-balance-total"><span>{row.currency}</span><strong><AnimatedNumber value={row.total} format={(value) => money(value, row.currency)} /></strong></div>
        <div className="deepseek-balance-breakdown">{zh ? '充值' : 'Topped up'} {money(row.topped_up, row.currency)} · {zh ? '赠送' : 'Granted'} {money(row.granted, row.currency)}</div>
        <div className="deepseek-balance-eta">{row.eta_days != null
          ? (zh ? `按近 7 天余额变化，约可用 ${Math.max(0, Math.round(row.eta_days))} 天` : `About ${Math.max(0, Math.round(row.eta_days))} days at recent spend`)
          : (zh ? '余额变化样本不足，满 24 小时后估算' : 'Need 24 hours of balance history for an estimate')}</div>
      </div>)}
      <p className="deepseek-balance-note">{zh ? '估算可能受充值、赠送额度到期及其他 API 调用影响。' : 'Refills, expiring grants, and other API activity affect estimates.'}</p>
    </> : <div className="quota-missing"><Text type="supporting" className="quota-missing-hint">{status === 'error' ? balance.message : (zh ? '添加 DeepSeek 官方 API key 后显示余额。' : 'Add a DeepSeek API key to show your balance.')}</Text><button className="deepseek-settings-link" type="button" onClick={onOpenSettings}>{zh ? '前往设置' : 'Open settings'}</button></div>}
  </Card>;
}

function QuotaCards({ cards, showDeepseek = true, deepseekBalance, lang, allowPace = false, remainingView = false, pinnedCards = [], hiddenCards = [], providerColors = {}, monthByClient = [], goatUsage = null, onTogglePin, onToggleHidden, onOpenSettings }) {
  const t = (k) => STR[k][lang];
  const ordered = [...cards].sort((a, b) => Number(pinnedCards.includes(b.provider)) - Number(pinnedCards.includes(a.provider)));
  return (
    <div className="quota-grid">
      {showDeepseek && pinnedCards.includes('deepseek') && !hiddenCards.includes('deepseek') && <DeepSeekBalanceCard balance={deepseekBalance} lang={lang} pinned accent={providerColors.deepseek || PROVIDER_ACCENT.deepseek} onTogglePin={() => onTogglePin?.('deepseek')} onToggleHidden={() => onToggleHidden?.('deepseek')} onOpenSettings={onOpenSettings} />}
      {ordered.filter((c) => !hiddenCards.includes(c.provider)).map((c) => {
        const accent = providerColors[c.provider] || PROVIDER_ACCENT[c.provider] || 'var(--color-text-primary)';
        const isActivity = c.method === 'activity_vs_average';
        const monthlyTokens = c.kind !== 'Live' ? undefined
          : c.provider === 'goat'
            ? goatUsage?.tokens
            : (() => {
              const client = QUOTA_CLIENT_OF[c.provider];
              const row = client && monthByClient.find((r) => r.client === client);
              return row?.agg.total;
            })();
        return (
          <Card key={c.provider} className={`quota-card${isActivity ? ' quota-card-activity' : ''}`} role="group" aria-label={c.title}>
            <div className="quota-head">
              <ProviderLogo provider={c.provider} />
              <Text weight="medium" className="quota-title" title={c.title}>{c.title}</Text>
              <span className="quota-plan-type" title={c.kind === 'Live' && c.plan_type ? c.plan_type : undefined}>{c.kind === 'Live' ? c.plan_type || '' : ''}</span>
              {c.kind === 'Live' && <ExpiryChip endsAtMs={c.subscription_ends_at_ms} lang={lang} />}
              <div className="quota-card-actions">
                <button type="button" aria-label={`${pinnedCards.includes(c.provider) ? t('card_unpin') : t('card_pin')} ${c.title}`} title={pinnedCards.includes(c.provider) ? t('card_unpin') : t('card_pin')} onClick={() => onTogglePin?.(c.provider)}>{pinnedCards.includes(c.provider) ? '◆' : '◇'}</button>
                <button type="button" aria-label={`${t('card_hide')} ${c.title}`} title={t('card_hide')} onClick={() => onToggleHidden?.(c.provider)}>×</button>
              </div>
            </div>
            {c.kind === 'Live' ? (
              <div className="quota-limits">
                {c.limits.length === 0 && <Text type="supporting" className="quota-missing-hint">{lang === 0 ? '暂无额度窗口' : 'No quota windows'}</Text>}
                {c.limits.map((l) => {
                  const pace = allowPace && !isActivity ? quotaPace(l) : null;
                  // 剩余视图只翻转真实配额窗口;activity 卡口径不同,保持已用
                  const barPct = remainingView && !isActivity ? Math.max(0, Math.min(100, 100 - l.used_pct)) : l.used_pct;
                  const pct = Math.round(barPct);
                  return (
                  <div key={l.label} className="quota-limit">
                    <div className="quota-limit-row">
                      <span className="quota-limit-label">{l.label}</span>
                      <span className="quota-limit-pct" style={{ color: isActivity ? accent : STATUS_COLOR[l.status] }}>
                        {isActivity
                          ? (l.used_pct >= 100 ? t('activity_at_least_average') : `${pct}% ${lang === 0 ? 'vs' : 'of'} ${t('activity_vs_average')}`)
                          : <AnimatedNumber value={barPct} format={(value) => `${Math.round(value)}%`} />}
                      </span>
                    </div>
                    <BlockBar pct={barPct} color={isActivity ? accent : STATUS_COLOR[l.status]} />
                    <div className="quota-limit-sub">
                      <span>{isActivity ? t('activity_today') : l.resets_in_ms != null ? `${t('resets_in')} ${fmtCountdown(l.resets_in_ms)}` : ' '}</span>
                      {pace != null && <span className={`quota-pace${pace.eta !== 'sustainable' ? ' quota-pace-warning' : ''}`}>{paceChipText(pace, lang)}</span>}
                    </div>
                  </div>
                );})}
              </div>
            ) : (
              <div className="quota-missing">
                <Text type="supporting" className="quota-missing-hint">{QUOTA_HINT[c.provider]?.[lang] || c.hints?.[lang]}</Text>
              </div>
            )}
            {monthlyTokens != null && monthlyTokens > 0 && (
              <div className="quota-monthly">
                <span className="quota-monthly-label">{t('usage_card_monthly')}</span>
                <span className="quota-monthly-value">{fmtTokens(monthlyTokens)} {t('usage_unit_tokens')}</span>
              </div>
            )}
          </Card>
        );
      })}
      {showDeepseek && !pinnedCards.includes('deepseek') && !hiddenCards.includes('deepseek') && <DeepSeekBalanceCard balance={deepseekBalance} lang={lang} pinned={false} accent={providerColors.deepseek || PROVIDER_ACCENT.deepseek} onTogglePin={() => onTogglePin?.('deepseek')} onToggleHidden={() => onToggleHidden?.('deepseek')} onOpenSettings={onOpenSettings} />}
    </div>
  );
}

export default function QuotaPage({ cards, sources, ledger, goatUsage, deepseekBalance, quotaFetchedAtMs, quotaStale, now, lang, preferences, onTogglePin, onToggleHidden, onOpenSettings }) {
  const t = (key) => STR[key][lang];
  const [selectedSource, setSelectedSource] = useState(null);
  const sourceDetail = sources?.find((source) => source.client === selectedSource);
  const elapsed = quotaFetchedAtMs ? Math.max(0, now - quotaFetchedAtMs) : 0;
  const currentCards = cards.map((card) => card.kind === 'Live' ? {
    ...card,
    limits: card.limits.map((limit) => ({ ...limit, resets_in_ms: limit.resets_in_ms == null ? null : Math.max(0, limit.resets_in_ms - elapsed) })),
  } : card);
  const live = currentCards.filter((card) => card.kind === 'Live' && card.method !== 'activity_vs_average' && !preferences.hiddenCards.includes(card.provider));
  const limits = live.flatMap((card) => card.limits.map((limit) => ({ ...limit, provider: card.title.split(' ·')[0] })));
  const tightest = limits.reduce((best, current) => !best || current.used_pct > best.used_pct ? current : best, null);
  const nextReset = limits.filter((limit) => limit.resets_in_ms > 0).sort((a, b) => a.resets_in_ms - b.resets_in_ms)[0];
  const attention = limits.filter((limit) => limit.status !== 'ok').length;
  const pct = tightest ? Math.max(0, Math.min(100, tightest.used_pct)) : 0;
  // 已用/剩余口径:仅展示层翻转,状态色与重置/燃尽信息不动
  const remainingView = preferences.quotaRemainingView === true;
  const heroPct = remainingView ? 100 - pct : pct;
  const meterLabel = remainingView ? t('quota_remaining') : t('quota_used');
  const modules = [
    { id: 'overview', title: t('quota_overview'), w: 12, h: 2, minW: 6, minH: 2, content: (<section className="quota-overview" aria-label={t('quota_overview')}>
      <div className="quota-overview-copy">
        <span className="eyebrow">// {t('quota_overview')}</span>
        <h1>{tightest ? `${tightest.provider} · ${tightest.label}` : t('quota_unavailable')}</h1>
        <p>{tightest ? `${attention ? `${attention} ${t('quota_attention')}` : t('quota_all_clear')} · ${nextReset ? `${t('quota_next_reset')} ${fmtCountdown(nextReset.resets_in_ms)}` : '—'}` : t('empty_quota')}</p>
      </div>
      <div className="quota-overview-meter" aria-label={tightest ? `${Math.round(heroPct)}% ${meterLabel}` : undefined}>
        <QuotaRing pct={heroPct} tone={STATUS_COLOR[tightest?.status] || 'var(--color-text-blue)'} />
        <strong>{tightest ? <AnimatedNumber value={heroPct} format={(value) => `${Math.round(value)}%`} /> : '—'}</strong>
        <span>{meterLabel}</span>
      </div>
    </section>) },
    { id: 'tokens', title: t('usage_sum_tokens'), w: 6, h: 2, minW: 4, minH: 2, content: (<UsageSummaryRow part="tokens" ledger={ledger} goat={goatUsage} lang={lang} />) },
    { id: 'requests', title: t('usage_sum_runs'), w: 6, h: 2, minW: 4, minH: 2, content: (<UsageSummaryRow part="requests" ledger={ledger} goat={goatUsage} lang={lang} />) },
    ...[...currentCards].sort((a, b) => Number(preferences.pinnedCards.includes(b.provider)) - Number(preferences.pinnedCards.includes(a.provider))).map((card) => ({ id: card.provider, title: card.title, w: 6, h: 3, minW: 4, minH: 2, content: (<QuotaCards cards={[card]} showDeepseek={false} deepseekBalance={deepseekBalance} lang={lang} allowPace={!!quotaFetchedAtMs && !quotaStale} remainingView={remainingView} pinnedCards={preferences.pinnedCards} hiddenCards={[]} providerColors={preferences.providerColors} monthByClient={ledger?.month_by_client || []} goatUsage={goatUsage} onTogglePin={onTogglePin} onToggleHidden={onToggleHidden} onOpenSettings={onOpenSettings} />) })),
    { id: 'deepseek', title: 'DeepSeek API', w: 6, h: 3, minW: 4, minH: 2, content: (<QuotaCards cards={[]} deepseekBalance={deepseekBalance} lang={lang} allowPace={!!quotaFetchedAtMs && !quotaStale} remainingView={remainingView} pinnedCards={preferences.pinnedCards} hiddenCards={[]} providerColors={preferences.providerColors} monthByClient={ledger?.month_by_client || []} goatUsage={goatUsage} onTogglePin={onTogglePin} onToggleHidden={onToggleHidden} onOpenSettings={onOpenSettings} />) },
    { id: 'sources', title: t('source_health'), w: 12, h: 3, minW: 6, minH: 2, content: (<section className="source-section" aria-label={t('source_health')}>
      <div className="source-heading"><h2>{t('source_health')}</h2></div>
      <div className="source-grid">{sources.map((source) => {
        const state = source.error ? 'error' : source.records > 0 ? 'ready' : 'idle';
        return <button type="button" className={`source-item${selectedSource === source.client ? ' is-selected' : ''}`} key={source.client} onClick={() => setSelectedSource((current) => current === source.client ? null : source.client)} aria-expanded={selectedSource === source.client}>
          <span className={`source-indicator source-indicator-${state}`} />
          <span className="source-name">{source.label}</span>
          <span className="source-count">{source.error ? t('source_error') : source.records > 0 ? `${fmtInt(source.records)} ${t('source_ready')}` : t('source_idle')}</span>
        </button>;
      })}</div>
      <AnimatePresence>{sourceDetail && <motion.div key={sourceDetail.client} className="source-detail" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={FADE} role="status">
        <strong>{sourceDetail.label}</strong>
        <span>{t('source_location')} · <code>{SOURCE_PATHS[sourceDetail.client] || '—'}</code></span>
        <span>{t('source_diagnostic')} · {sourceDetail.error || (sourceDetail.records ? t('source_ok_detail') : t('source_no_records'))}</span>
      </motion.div>}</AnimatePresence>
    </section>) },
  ];
  let row = 0, col = 0;
  const defaultLayout = modules.filter((item) => !preferences.hiddenCards.includes(item.id)).map((item) => {
    if (col + item.w > 12) { row += 3; col = 0; }
    const panel = { id: item.id, x: col, y: row, w: item.w, h: item.h, locked: false };
    col += item.w;
    if (col === 12) { row += item.h; col = 0; }
    return panel;
  });
  return <div className="quota-page">
    <ModuleBoard storageKey="token-monitor.quota-layout.v1" lang={lang} catalog={modules} hiddenIds={preferences.hiddenCards} defaults={defaultLayout} renderModule={(id) => modules.find((item) => item.id === id)?.content} />
    {preferences.hiddenCards.length > 0 && <button className="quota-hidden-link" type="button" onClick={onOpenSettings}>{lang === 0 ? `已隐藏 ${preferences.hiddenCards.length} 张卡片 · 在设置中恢复` : `${preferences.hiddenCards.length} hidden cards · restore in Settings`}</button>}

  </div>;
}

/* ---------------- 用量页 ---------------- */

function tightestQuota(cards) {
  const limits = cards
    .filter((card) => card.kind === 'Live' && card.method !== 'activity_vs_average')
    .flatMap((card) => card.limits.map((limit) => ({ provider: card.title.split(' ·')[0], ...limit })));
  return limits.sort((a, b) => b.used_pct - a.used_pct)[0] || null;
}
