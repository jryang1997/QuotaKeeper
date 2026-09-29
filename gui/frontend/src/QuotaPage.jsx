import React, { useState } from 'react';
import { Card } from '@astryxdesign/core/Card';
import { Text } from '@astryxdesign/core/Text';
import { Tooltip } from '@astryxdesign/core/Tooltip';
import { PROVIDER_ACCENT, STATUS_COLOR, PROVIDER_LOGOS, SOURCE_PATHS, QUOTA_HINT, STR, fmtInt, fmtTokens, fmtCountdown } from './ui-data.js';
import BlockBar from './BlockBar.jsx';

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
  const max = Math.max(1, ...values);
  return (
    <div className="trend-bars" aria-hidden="true">
      {values.map((v, i) => (
        <span
          key={i}
          style={{
            height: `${Math.max(4, (v / max) * 100)}%`,
            background: color,
            opacity: i === values.length - 1 ? 1 : 0.45,
          }}
        />
      ))}
    </div>
  );
}

function UsageSummaryRow({ ledger, goat, lang }) {
  const t = (k) => STR[k][lang];
  const zh = lang === 0;
  const now = new Date();
  const monthPrefix = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
  const monthDays = (ledger?.daily_totals || []).filter((d) => d.date.startsWith(monthPrefix));
  const tokens = goat ? goat.tokens : ledger?.month.total ?? 0;
  const requests = goat ? goat.requests : ledger?.month.messages ?? 0;
  const caption = goat ? t('usage_sum_official') : t('usage_sum_local');
  return (
    <div className="usage-summary-row">
      <div className="usage-summary-card">
        <div className="usage-summary-head" style={{ color: 'var(--sum-purple)' }}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M12 8v8M8 12h8" /></svg>
          <span>{t('usage_sum_tokens')}</span>
        </div>
        <div className="usage-summary-number">{fmtTokens(tokens)}<span>{t('usage_unit_tokens')}</span></div>
        <TrendBars values={monthDays.map((d) => d.tokens)} color="var(--sum-purple)" />
        <div className="usage-summary-caption">{caption}</div>
      </div>
      <div className="usage-summary-card">
        <div className="usage-summary-head" style={{ color: 'var(--sum-orange)' }}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13l11-6.5z" /></svg>
          <span>{t('usage_sum_runs')}</span>
        </div>
        <div className="usage-summary-number">{fmtInt(requests)}<span>{t('usage_unit_runs')}</span></div>
        <TrendBars values={monthDays.map((d) => d.messages)} color="var(--sum-orange)" />
        <div className="usage-summary-caption">{caption}</div>
      </div>
    </div>
  );
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
function QuotaRing({ pct, tone }) {
  // 每段 15° 槽位:10° 实块 + 5° 间隙;平头(butt)让分块缝隙清晰可见
  const seg = 10;
  const gap = 15 - seg;
  const filled = Math.max(0, Math.min(RING_SEGMENTS, Math.round(pct / 100 * RING_SEGMENTS)));
  const dash = Array(filled).fill(`${seg} ${gap}`).join(' ');
  const track = Array(RING_SEGMENTS).fill(`${seg} ${gap}`).join(' ');
  return (
    <svg className="quota-ring" viewBox="0 0 116 116" aria-hidden="true">
      <circle className="quota-ring-track" cx="58" cy="58" r="48" pathLength="360" strokeDasharray={track} style={{ color: tone }} />
      {filled > 0 && <circle className="quota-ring-fill" cx="58" cy="58" r="48" pathLength="360" strokeDasharray={dash} style={{ color: tone }} />}
    </svg>
  );
}

function ExpiryChip({ endsInMs, lang }) {
  if (endsInMs == null) return null;
  // endsInMs 已是绝对时间戳(QuotaPage 减过 elapsed),不能再叠加 Date.now()
  const days = Math.max(0, Math.ceil((endsInMs - Date.now()) / 86_400_000));
  const date = new Date(endsInMs);
  const md = `${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
  return <span className={`quota-expiry${days <= 7 ? ' quota-expiry-warning' : ''}`}>{lang === 0 ? `${md} 到期` : `expires ${md}`}</span>;
}

function QuotaCards({ cards, lang, allowPace = false, pinnedCards = [], hiddenCards = [], providerColors = {}, monthByClient = [], goatUsage = null, onTogglePin, onToggleHidden }) {
  const t = (k) => STR[k][lang];
  const ordered = [...cards].sort((a, b) => Number(pinnedCards.includes(b.provider)) - Number(pinnedCards.includes(a.provider)));
  return (
    <div className="quota-grid">
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
              <Text weight="medium" className="quota-title">{c.title}</Text>
              <span className="quota-plan-type" title={c.kind === 'Live' && c.plan_type ? c.plan_type : undefined}>{c.kind === 'Live' ? c.plan_type || '' : ''}</span>
              {c.kind === 'Live' && <ExpiryChip endsInMs={c.subscription_ends_at_ms} lang={lang} />}
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
                  const pct = Math.round(l.used_pct);
                  return (
                  <div key={l.label} className="quota-limit">
                    <div className="quota-limit-row">
                      <span className="quota-limit-label">{l.label}</span>
                      <span className="quota-limit-pct" style={{ color: isActivity ? accent : STATUS_COLOR[l.status] }}>
                        {isActivity
                          ? (l.used_pct >= 100 ? t('activity_at_least_average') : `${pct}% ${lang === 0 ? 'vs' : 'of'} ${t('activity_vs_average')}`)
                          : l.used_pct >= 100 ? '100%' : `${pct}%`}
                      </span>
                    </div>
                    <BlockBar pct={l.used_pct} color={isActivity ? accent : STATUS_COLOR[l.status]} />
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
    </div>
  );
}

export default function QuotaPage({ cards, sources, ledger, goatUsage, quotaFetchedAtMs, quotaStale, now, lang, preferences, onTogglePin, onToggleHidden, onOpenSettings }) {
  const t = (key) => STR[key][lang];
  const [selectedSource, setSelectedSource] = useState(null);
  const sourceDetail = sources?.find((source) => source.client === selectedSource);
  const elapsed = quotaFetchedAtMs ? Math.max(0, now - quotaFetchedAtMs) : 0;
  const currentCards = cards.map((card) => card.kind === 'Live' ? {
    ...card,
    limits: card.limits.map((limit) => ({ ...limit, resets_in_ms: limit.resets_in_ms == null ? null : Math.max(0, limit.resets_in_ms - elapsed) })),
    subscription_ends_at_ms: card.subscription_ends_at_ms == null ? null : card.subscription_ends_at_ms - elapsed,
  } : card);
  const live = currentCards.filter((card) => card.kind === 'Live' && card.method !== 'activity_vs_average' && !preferences.hiddenCards.includes(card.provider));
  const limits = live.flatMap((card) => card.limits.map((limit) => ({ ...limit, provider: card.title.split(' ·')[0] })));
  const tightest = limits.reduce((best, current) => !best || current.used_pct > best.used_pct ? current : best, null);
  const nextReset = limits.filter((limit) => limit.resets_in_ms > 0).sort((a, b) => a.resets_in_ms - b.resets_in_ms)[0];
  const attention = limits.filter((limit) => limit.status !== 'ok').length;
  const pct = tightest ? Math.max(0, Math.min(100, tightest.used_pct)) : 0;
  return <div className="quota-page">
    <section className="quota-overview" aria-label={t('quota_overview')}>
      <div className="quota-overview-copy">
        <span className="eyebrow">// {t('quota_overview')}</span>
        <h1>{tightest ? `${tightest.provider} · ${tightest.label}` : t('quota_unavailable')}</h1>
        <p>{tightest ? `${attention ? `${attention} ${t('quota_attention')}` : t('quota_all_clear')} · ${nextReset ? `${t('quota_next_reset')} ${fmtCountdown(nextReset.resets_in_ms)}` : '—'}` : t('empty_quota')}</p>
      </div>
      <div className="quota-overview-meter" aria-label={tightest ? `${Math.round(pct)}% ${t('quota_used')}` : undefined}>
        <QuotaRing pct={pct} tone={STATUS_COLOR[tightest?.status] || 'var(--color-text-blue)'} />
        <strong>{tightest ? `${Math.round(pct)}%` : '—'}</strong>
        <span>{t('quota_used')}</span>
      </div>
    </section>
    <UsageSummaryRow ledger={ledger} goat={goatUsage} lang={lang} />
    <QuotaCards cards={currentCards} lang={lang} allowPace={!!quotaFetchedAtMs && !quotaStale} pinnedCards={preferences.pinnedCards} hiddenCards={preferences.hiddenCards} providerColors={preferences.providerColors} monthByClient={ledger?.month_by_client || []} goatUsage={goatUsage} onTogglePin={onTogglePin} onToggleHidden={onToggleHidden} />
    {preferences.hiddenCards.length > 0 && <button className="quota-hidden-link" type="button" onClick={onOpenSettings}>{lang === 0 ? `已隐藏 ${preferences.hiddenCards.length} 张卡片 · 在设置中恢复` : `${preferences.hiddenCards.length} hidden cards · restore in Settings`}</button>}
    {sources?.length > 0 && <section className="source-section" aria-label={t('source_health')}>
      <div className="source-heading"><h2>{t('source_health')}</h2></div>
      <div className="source-grid">{sources.map((source) => {
        const state = source.error ? 'error' : source.records > 0 ? 'ready' : 'idle';
        return <button type="button" className={`source-item${selectedSource === source.client ? ' is-selected' : ''}`} key={source.client} onClick={() => setSelectedSource((current) => current === source.client ? null : source.client)} aria-expanded={selectedSource === source.client}>
          <span className={`source-indicator source-indicator-${state}`} />
          <span className="source-name">{source.label}</span>
          <span className="source-count">{source.error ? t('source_error') : source.records > 0 ? `${fmtInt(source.records)} ${t('source_ready')}` : t('source_idle')}</span>
        </button>;
      })}</div>
      {sourceDetail && <div className="source-detail" role="status">
        <strong>{sourceDetail.label}</strong>
        <span>{t('source_location')} · <code>{SOURCE_PATHS[sourceDetail.client] || '—'}</code></span>
        <span>{t('source_diagnostic')} · {sourceDetail.error || (sourceDetail.records ? t('source_ok_detail') : t('source_no_records'))}</span>
      </div>}
    </section>}
  </div>;
}

/* ---------------- 用量页 ---------------- */

function tightestQuota(cards) {
  const limits = cards
    .filter((card) => card.kind === 'Live' && card.method !== 'activity_vs_average')
    .flatMap((card) => card.limits.map((limit) => ({ provider: card.title.split(' ·')[0], ...limit })));
  return limits.sort((a, b) => b.used_pct - a.used_pct)[0] || null;
}
