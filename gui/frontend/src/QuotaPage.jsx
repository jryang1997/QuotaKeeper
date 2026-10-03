// 配额页:一条"额度带"(全部服务商,最紧张在前) + 每家有配额/余额的完整卡片。
// 口径:本页只讲服务商额度;本机用量以"本机本月"前缀标注,不混作一谈。
import ModuleBoard from './ModuleBoard.jsx';
import React, { useMemo, useState } from 'react';
import { Text } from '@astryxdesign/core/Text';
import { PLAN_MODELS, PROVIDER_LOGOS, PROVIDER_COLOR_DEFAULTS, QUOTA_HINT, STR, fmtInt, fmtTokens, fmtCountdown } from './ui-data.js';
import BlockBar from './BlockBar.jsx';
import CardDetails from './CardDetails.jsx';
import { AnimatePresence } from 'motion/react';
import AnimatedNumber from './AnimatedNumber.jsx';
import { PopoverSurface } from './ui-motion.jsx';

const PROVIDER_STATUS_COLOR = { ok: 'var(--color-success)', warning: 'var(--color-warning)', exhausted: 'var(--color-error)' };

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

function statusLabel(status, lang) {
  if (status === 'exhausted') return STR.status_exhausted[lang];
  if (status === 'warning') return STR.status_warning[lang];
  return STR.status_ok[lang];
}

function ProviderLogo({ provider }) {
  const src = PROVIDER_LOGOS[provider];
  if (!src) return <span className="quota-logo quota-logo-fallback" aria-hidden="true" />;
  return <img className="quota-logo" src={src} alt="" aria-hidden="true" draggable="false" />;
}

function ExpiryChip({ endsAtMs, lang }) {
  if (endsAtMs == null) return null;
  const days = Math.max(0, Math.ceil((endsAtMs - Date.now()) / 86_400_000));
  const date = new Date(endsAtMs);
  const md = `${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
  return <span className={`quota-expiry${days <= 7 ? ' quota-expiry-warning' : ''}`}>{lang === 0 ? `${md} 到期` : `expires ${md}`}</span>;
}

// 额度带:一行读一家;最紧张窗口、重置倒计时、提前耗尽预测同屏。
// 全部服务商都在这一屏,不再是"只看最紧张的那一家"。
function QuotaStrip({ cards, lang, hiddenCards, pinnedCards, now, onJump, onManage }) {
  const t = (key) => STR[key][lang];
  const rows = useMemo(() => cards
    .filter((card) => card.kind === 'Live' && !hiddenCards.includes(card.provider))
    .map((card) => {
      const limits = card.limits || [];
      const tightest = limits.reduce((best, limit) => (!best || limit.used_pct > best.used_pct ? limit : best), null);
      const nextReset = limits.filter((limit) => limit.resets_in_ms > 0).sort((a, b) => a.resets_in_ms - b.resets_in_ms)[0];
      const pace = tightest ? quotaPace(tightest) : null;
      const elapsed = card.fetchedElapsedMs || 0;
      return {
        provider: card.provider,
        title: card.title,
        kind: card.method === 'activity_vs_average' ? 'activity' : 'quota',
        used: tightest ? tightest.used_pct : 0,
        label: tightest?.label || (lang === 0 ? '暂无窗口' : 'no windows'),
        status: tightest?.status || 'ok',
        resetMs: nextReset ? Math.max(0, nextReset.resets_in_ms - elapsed) : null,
        pace,
        attention: limits.filter((limit) => limit.status !== 'ok').length,
      };
    })
    .sort((a, b) => b.used - a.used), [cards, hiddenCards, now, lang]);
  const missing = cards.filter((card) => card.kind !== 'Live' && !hiddenCards.includes(card.provider));
  const hiddenCount = hiddenCards.length;
  if (rows.length === 0) {
    return <section className="quota-strip" aria-label={t('quota_overview')}>
      <div className="quota-strip-empty"><Text type="supporting">{t('empty_quota')}</Text></div>
      {hiddenCount > 0 && <button type="button" className="quota-strip-manage" onClick={onManage}>{t('quota_hidden_link').replace('{n}', hiddenCount)}</button>}
    </section>;
  }
  const worst = rows[0];
  return (
    <section className="quota-strip" aria-label={t('quota_overview')}>
      <header className="quota-strip-head">
        <span className="eyebrow">// {t('quota_overview')}</span>
        <span className="quota-strip-summary">
          {rows.some((row) => row.attention > 0)
            ? `${rows.reduce((sum, row) => sum + row.attention, 0)} ${t('quota_attention')}`
            : t('quota_all_clear')}
        </span>
        {hiddenCount > 0 && <button type="button" className="quota-strip-manage" onClick={onManage}>{t('quota_hidden_link').replace('{n}', hiddenCount)}</button>}
      </header>
      <div className="quota-strip-list">
        {rows.map((row) => (
          <button type="button" className={`quota-strip-row is-${row.status}${row.provider === worst.provider ? ' is-worst' : ''}`} key={row.provider} onClick={() => onJump(row.provider)}>
            <span className="quota-strip-provider">
              <ProviderLogo provider={row.provider} />
              <span className="quota-strip-name">{row.title.split(' ·')[0]}</span>
              {pinnedCards.includes(row.provider) && <i className="quota-strip-pin" title={t('card_unpin')}>◆</i>}
            </span>
            <span className="quota-strip-window">{row.label}</span>
            <span className="quota-strip-bar"><BlockBar pct={row.kind === 'activity' ? Math.min(row.used, 100) : row.used} color={row.kind === 'activity' ? PROVIDER_COLOR_DEFAULTS[row.provider] : PROVIDER_STATUS_COLOR[row.status]} /></span>
            <span className="quota-strip-pct" style={{ color: row.kind === 'activity' ? undefined : PROVIDER_STATUS_COLOR[row.status] }}>
              {row.kind === 'activity' ? `${Math.round(row.used)}% ${lang === 0 ? 'vs' : 'of'} ${t('activity_vs_average')}` : <AnimatedNumber value={row.used} format={(value) => `${Math.round(value)}%`} />}
            </span>
            <span className="quota-strip-reset">{row.resetMs != null ? `${t('resets_in')} ${fmtCountdown(row.resetMs)}` : ''}</span>
            <span className={`quota-strip-pace${row.pace && row.pace.eta !== 'sustainable' ? ' is-warn' : ''}`}>
              {row.kind === 'quota' && row.pace ? (row.pace.eta === 'sustainable' ? '✓' : paceChipText(row.pace, lang)) : ''}
            </span>
          </button>
        ))}
        {missing.map((card) => (
          <button type="button" className="quota-strip-row is-missing" key={card.provider} onClick={() => onJump(card.provider)}>
            <span className="quota-strip-provider">
              <ProviderLogo provider={card.provider} />
              <span className="quota-strip-name">{card.title.split(' ·')[0]}</span>
            </span>
            <span className="quota-strip-window">{card.plan_type || ''}</span>
            <span className="quota-strip-bar" />
            <span className="quota-strip-pct">{t('missing')}</span>
            <span className="quota-strip-reset" />
            <span className="quota-strip-pace is-hint">{QUOTA_HINT[card.provider]?.[lang] || ''}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

function CardMenu({ open, onToggle, items, lang }) {
  return (
    <div className="quota-menu-wrap">
      <button type="button" className="quota-menu-btn" aria-expanded={open} aria-haspopup="menu" aria-label={STR.card_menu[lang]} title={STR.card_menu[lang]} onClick={onToggle}>⋯</button>
      <AnimatePresence>{open && <PopoverSurface className="quota-menu" role="menu">
        {items.map((item) => <button key={item.key} type="button" role="menuitem" onClick={item.onSelect} className={item.tone ? `is-${item.tone}` : ''}>{item.label}</button>)}
      </PopoverSurface>}</AnimatePresence>
    </div>
  );
}

function ProviderCard({ card, lang, allowPace, remainingView, pinned, providerColor, monthlyTokens, goatUsage, planModels, clipboardClear, onPreferenceChange, onTogglePin, onToggleHidden }) {
  const t = (key) => STR[key][lang];
  const isActivity = card.method === 'activity_vs_average';
  const accent = providerColor || PROVIDER_COLOR_DEFAULTS[card.provider] || 'var(--color-text-primary)';
  const [expanded, setExpanded] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const official = card.provider === 'goat' && goatUsage ? goatUsage : null;
  const detailsAvailable = (PLAN_MODELS[card.provider]?.length || 0) > 0 || ['opencode-go', 'goat', 'glm', 'kimi'].includes(card.provider);
  const menuItems = [
    { key: 'pin', label: pinned ? t('card_unpin') : t('card_pin'), onSelect: () => { onTogglePin(card.provider); setMenuOpen(false); } },
    ...(detailsAvailable ? [{ key: 'details', label: expanded ? t('card_details_hide') : t('card_details'), onSelect: () => { setExpanded((value) => !value); setMenuOpen(false); } }] : []),
    { key: 'hide', label: t('card_hide'), tone: 'danger', onSelect: () => { onToggleHidden(card.provider); setMenuOpen(false); } },
  ];
  return (
    <div className={`quota-card${pinned ? ' is-pinned' : ''}${isActivity ? ' quota-card-activity' : ''}`} data-provider={card.provider} style={{ '--card-accent': accent }}>
      <div className="quota-head">
        <ProviderLogo provider={card.provider} />
        <Text weight="medium" className="quota-title" title={card.title}>{card.title}</Text>
        {card.kind === 'Live' && card.plan_type && card.plan_type.toLowerCase() !== card.title.toLowerCase() && <span className="quota-plan-type" title={card.plan_type}>{card.plan_type}</span>}
        {card.kind === 'Live' && <ExpiryChip endsAtMs={card.subscription_ends_at_ms} lang={lang} />}
        <CardMenu lang={lang} open={menuOpen} onToggle={() => setMenuOpen((value) => !value)} items={menuItems} />
      </div>
      {card.kind === 'Live' ? (
        <div className="quota-limits">
          {card.limits.length === 0 && <Text type="supporting" className="quota-missing-hint">{lang === 0 ? '暂无额度窗口' : 'No quota windows'}</Text>}
          {card.limits.map((limit) => {
            const pace = allowPace && !isActivity ? quotaPace(limit) : null;
            const barPct = remainingView && !isActivity ? Math.max(0, Math.min(100, 100 - limit.used_pct)) : limit.used_pct;
            return (
              <div key={limit.label} className="quota-limit">
                <div className="quota-limit-row">
                  <span className="quota-limit-label">{limit.label}</span>
                  <span className="quota-limit-pct" style={{ color: isActivity ? accent : PROVIDER_STATUS_COLOR[limit.status] }}>
                    {isActivity
                      ? (limit.used_pct >= 100 ? t('activity_at_least_average') : `${Math.round(barPct)}% ${lang === 0 ? 'vs' : 'of'} ${t('activity_vs_average')}`)
                      : <AnimatedNumber value={barPct} format={(value) => `${Math.round(value)}%`} />}
                  </span>
                </div>
                <BlockBar pct={barPct} color={isActivity ? accent : PROVIDER_STATUS_COLOR[limit.status]} />
                <div className="quota-limit-sub">
                  <span>{isActivity ? t('activity_today') : limit.resets_in_ms != null ? `${t('resets_in')} ${fmtCountdown(limit.resets_in_ms)}` : ' '}</span>
                  {pace != null && <span className={`quota-pace${pace.eta !== 'sustainable' ? ' quota-pace-warning' : ''}`}>{paceChipText(pace, lang)}</span>}
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="quota-missing">
          <Text type="supporting" className="quota-missing-hint">{QUOTA_HINT[card.provider]?.[lang] || card.hints?.[lang]}</Text>
        </div>
      )}
      {(monthlyTokens > 0 || official) && <div className="quota-footnotes">
        {official && <div className="quota-footnote"><span>{t('quota_official_period')}</span><strong>{fmtTokens(official.tokens, lang === 0)} {t('usage_unit_tokens')} · {fmtInt(official.requests)} {t('quota_requests')}</strong></div>}
        {monthlyTokens > 0 && <div className="quota-footnote"><span>{t('quota_local_month')}</span><strong>{fmtTokens(monthlyTokens, lang === 0)} {t('usage_unit_tokens')}</strong></div>}
      </div>}
      <AnimatePresence>{expanded && <CardDetails key="details" provider={card.provider} lang={lang} imported={planModels[card.provider]} clipboardClear={clipboardClear} onPreferenceChange={onPreferenceChange} />}</AnimatePresence>
    </div>
  );
}

function DeepSeekCard({ balance, lang, pinned, accent, onTogglePin, onToggleHidden, onOpenSettings }) {
  const t = (key) => STR[key][lang];
  const zh = lang === 0;
  const [menuOpen, setMenuOpen] = useState(false);
  const money = (amount, currency) => new Intl.NumberFormat(zh ? 'zh-CN' : 'en-US', { style: 'currency', currency, maximumFractionDigits: 2 }).format(amount);
  const status = balance?.status || 'missing_key';
  const menuItems = [
    { key: 'pin', label: pinned ? t('card_unpin') : t('card_pin'), onSelect: () => { onTogglePin('deepseek'); setMenuOpen(false); } },
    { key: 'hide', label: t('card_hide'), tone: 'danger', onSelect: () => { onToggleHidden('deepseek'); setMenuOpen(false); } },
  ];
  return <div className={`quota-card deepseek-balance-card${pinned ? ' is-pinned' : ''}`} style={{ '--card-accent': accent }}>
    <div className="quota-head">
      <ProviderLogo provider="deepseek" />
      <Text weight="medium" className="quota-title">DeepSeek API</Text>
      <span className="quota-plan-type">{zh ? '官方余额' : 'Official balance'}</span>
      <CardMenu lang={lang} open={menuOpen} onToggle={() => setMenuOpen((value) => !value)} items={menuItems} />
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
  </div>;
}

export default function QuotaPage({ cards, ledger, goatUsage, deepseekBalance, quotaFetchedAtMs, quotaStale, now, lang, preferences, planModels = {}, onTogglePin, onToggleHidden, onOpenSettings, onPreferenceChange, editing, onToggleEditing, onRestoreCard }) {
  const t = (key) => STR[key][lang];
  const elapsed = quotaFetchedAtMs ? Math.max(0, now - quotaFetchedAtMs) : 0;
  const currentCards = useMemo(() => cards.map((card) => card.kind === 'Live' ? {
    ...card,
    limits: card.limits.map((limit) => ({ ...limit, resets_in_ms: limit.resets_in_ms == null ? null : Math.max(0, limit.resets_in_ms - elapsed) })),
    fetchedElapsedMs: elapsed,
  } : card), [cards, elapsed]);
  const allowPace = !!quotaFetchedAtMs && !quotaStale;
  const remainingView = preferences.quotaRemainingView === true;
  const hidden = preferences.hiddenCards;
  const pinnedCards = preferences.pinnedCards;

  const jumpTo = (provider) => {
    const node = document.querySelector(`[data-provider="${provider}"]`);
    if (!node) return;
    node.scrollIntoView({ behavior: 'smooth', block: 'center' });
    node.classList.add('is-flash');
    window.setTimeout(() => node.classList.remove('is-flash'), 1200);
  };

  const monthlyOf = (provider) => {
    if (provider === 'goat') return goatUsage?.tokens;
    const client = { 'opencode-go': 'opencode', codex: 'codex' }[provider];
    const row = client && (ledger?.month_by_client || []).find((item) => item.client === client);
    return row?.agg.total;
  };

  const visible = currentCards.filter((card) => !hidden.includes(card.provider));
  const ordered = [...visible].sort((a, b) => Number(pinnedCards.includes(b.provider)) - Number(pinnedCards.includes(a.provider)));
  const modules = [
    ...ordered.map((card) => ({
      id: card.provider,
      title: card.title,
      w: 6, h: 3, minW: 4, minH: 2, bare: true,
      content: <ProviderCard
        card={card}
        lang={lang}
        allowPace={allowPace}
        remainingView={remainingView}
        pinned={pinnedCards.includes(card.provider)}
        providerColor={preferences.providerColors?.[card.provider]}
        monthlyTokens={card.kind === 'Live' ? monthlyOf(card.provider) : undefined}
        goatUsage={goatUsage}
        planModels={planModels}
        clipboardClear={preferences.clipboardClear !== false}
        onPreferenceChange={onPreferenceChange}
        onTogglePin={onTogglePin}
        onToggleHidden={onToggleHidden}
      />,
    })),
    ...(!hidden.includes('deepseek') ? [{
      id: 'deepseek',
      title: 'DeepSeek API',
      w: 6, h: 3, minW: 4, minH: 2, bare: true,
      content: <DeepSeekCard
        balance={deepseekBalance}
        lang={lang}
        pinned={pinnedCards.includes('deepseek')}
        accent={preferences.providerColors?.deepseek || PROVIDER_COLOR_DEFAULTS.deepseek}
        onTogglePin={onTogglePin}
        onToggleHidden={onToggleHidden}
        onOpenSettings={onOpenSettings}
      />,
    }] : []),
  ];
  let row = 0;
  let col = 0;
  const defaultLayout = modules.map((item) => {
    if (col + item.w > 12) { row += item.h; col = 0; }
    const panel = { id: item.id, x: col, y: row, w: item.w, h: item.h, locked: false };
    col += item.w;
    if (col === 12) { row += item.h; col = 0; }
    return panel;
  });

  return <div className="quota-page">
    <QuotaStrip
      cards={currentCards}
      lang={lang}
      hiddenCards={hidden}
      pinnedCards={pinnedCards}
      now={now}
      onJump={jumpTo}
      onManage={() => onToggleEditing?.(true)}
    />
    <ModuleBoard
      catalog={modules}
      defaults={defaultLayout}
      storageKey="token-monitor.quota-layout.v2"
      lang={lang}
      editing={editing}
      hiddenIds={hidden}
      onRestoreHidden={onRestoreCard}
      renderModule={(id) => modules.find((item) => item.id === id)?.content}
    />
  </div>;
}
