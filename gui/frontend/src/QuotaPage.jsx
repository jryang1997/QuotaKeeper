import './quota-focus.css';
import React, { useMemo, useState } from 'react';
import { Text } from '@astryxdesign/core/Text';
import { PLAN_MODELS, PROVIDER_LOGOS, PROVIDER_COLOR_DEFAULTS, QUOTA_HINT, QUOTA_REPAIR, STATUS_COLOR, STR, fmtCountdown } from './ui-data.js';
import { arrangeQuota, formatObservedAt, projectExpiry } from './quota-data.js';
import BlockBar from './BlockBar.jsx';
import CardDetails from './CardDetails.jsx';
import { AnimatePresence } from 'motion/react';
import AnimatedNumber from './AnimatedNumber.jsx';
import { PopoverSurface } from './ui-motion.jsx';

function text(lang, zh, en) {
  return lang === 0 ? zh : en;
}

function ProviderLogo({ provider }) {
  const src = PROVIDER_LOGOS[provider];
  if (!src) return <span className="quota-logo quota-logo-fallback" aria-hidden="true" />;
  return <img className="quota-logo" src={src} alt="" aria-hidden="true" draggable="false" />;
}

function formatBalance(balance, lang) {
  if (!balance || !Number.isFinite(balance.amount)) return null;
  if (!/^[A-Z]{3}$/.test(balance.currency || '')) return `${balance.amount.toFixed(2)} ${balance.currency || ''}`.trim();
  return new Intl.NumberFormat(lang === 0 ? 'zh-CN' : 'en-US', { style: 'currency', currency: balance.currency, maximumFractionDigits: 2 }).format(balance.amount);
}

function ExpiryChip({ endsAtMs, now, lang }) {
  const expiry = projectExpiry(endsAtMs, now);
  if (!expiry) return null;
  if (expiry.state === 'expired') {
    return <span className="quota-expiry quota-expiry-expired">{text(lang, '已过期', 'Expired')}</span>;
  }
  return <span className={`quota-expiry${expiry.warning ? ' quota-expiry-warning' : ''}`}>{text(lang, `${expiry.md} 到期`, `expires ${expiry.md}`)}</span>;
}

function resetCopy(reset, lang) {
  if (reset.state === 'countdown') return `${STR.resets_in[lang]} ${fmtCountdown(reset.remainingMs)}`;
  if (reset.state === 'awaiting') return text(lang, '待刷新', 'Waiting for refresh');
  if (reset.state === 'stale') return text(lang, '旧读数', 'Old reading');
  return '';
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

function ProviderCard({ card, reading, lang, pinned, providerColor, now, planModels, clipboardClear, onPreferenceChange, onTogglePin, onToggleHidden }) {
  const t = (key) => STR[key][lang];
  const isActivity = reading.kind === 'activity';
  const accent = providerColor || PROVIDER_COLOR_DEFAULTS[card.provider] || 'var(--color-text-primary)';
  const [expanded, setExpanded] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const balanceText = card.kind === 'Live' && card.balance ? formatBalance(card.balance, lang) : null;
  const detailsAvailable = (PLAN_MODELS[card.provider]?.length || 0) > 0 || ['opencode-go', 'goat', 'glm', 'kimi', 'minimax'].includes(card.provider);
  const menuItems = [
    { key: 'pin', label: pinned ? t('card_unpin') : t('card_pin'), onSelect: () => { onTogglePin(card.provider); setMenuOpen(false); } },
    ...(detailsAvailable ? [{ key: 'details', label: expanded ? t('card_details_hide') : t('card_details'), onSelect: () => { setExpanded((value) => !value); setMenuOpen(false); } }] : []),
    { key: 'hide', label: t('card_hide'), tone: 'danger', onSelect: () => { onToggleHidden(card.provider); setMenuOpen(false); } },
  ];
  return (
    <div className={`quota-card${pinned ? ' is-pinned' : ''}${isActivity ? ' quota-card-activity' : ''}`} data-provider={card.provider} data-kind={reading.kind} style={{ '--card-accent': accent }}>
      <div className="quota-head">
        <ProviderLogo provider={card.provider} />
        <Text weight="medium" className="quota-title" title={card.title}>{card.title.split(' ·')[0]}</Text>
        <CardMenu lang={lang} open={menuOpen} onToggle={() => setMenuOpen((value) => !value)} items={menuItems} />
        {(card.plan_type || card.subscription_ends_at_ms) && <div className="quota-account-meta">
          {card.plan_type && card.plan_type.toLowerCase() !== card.title.toLowerCase() && <span className="quota-plan-type" title={card.plan_type}>{card.plan_type}</span>}
          <ExpiryChip endsAtMs={card.subscription_ends_at_ms} now={now} lang={lang} />
        </div>}
      </div>
      {reading.kind === 'empty' && <p className="quota-no-window">{text(lang, '没有额度窗口', 'No quota window')}</p>}
      {reading.windows.length > 0 && <div className="quota-limits">
        {reading.windows.map((slot, index) => {
          const reset = resetCopy(slot.reset, lang);
          const resetTitle = slot.reset.state === 'awaiting'
            ? text(lang, '已过预定重置时刻，额度数字未改', 'Scheduled reset time has passed; the quota number is unchanged')
            : undefined;
          return (
            <div key={`${slot.label}-${index}`} className="quota-limit" data-reset={slot.reset.state}>
              <div className="quota-limit-row">
                <span className="quota-limit-label">{slot.label}</span>
                <span className={`quota-limit-pct is-${slot.tone}`}>
                  {isActivity
                    ? (slot.usedPct >= 100 ? t('activity_at_least_average') : `${Math.round(slot.percent)}% ${text(lang, 'vs', 'of')} ${t('activity_vs_average')}`)
                    : <AnimatedNumber value={slot.percent} format={(value) => `${Math.round(value)}%`} />}
                </span>
              </div>
              <BlockBar pct={slot.percent} color={isActivity ? accent : STATUS_COLOR[slot.tone]} />
              <div className="quota-limit-sub">
                <span className={slot.reset.state === 'awaiting' ? 'quota-reset-awaiting' : slot.reset.state === 'stale' ? 'quota-reset-stale' : undefined} title={resetTitle}>
                  {isActivity ? t('activity_today') : reset || ' '}
                </span>
              </div>
            </div>
          );
        })}
      </div>}
      {balanceText && <div className="quota-footnotes"><div className="quota-footnote"><span>{t('quota_balance')}</span><strong>{balanceText}</strong></div></div>}
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
  return <div className={`quota-card deepseek-balance-card${pinned ? ' is-pinned' : ''}`} data-provider="deepseek" data-kind="balance" style={{ '--card-accent': accent }}>
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

function repairText(item, lang) {
  if (item.provider === 'deepseek') return text(lang, '添加 DeepSeek 官方 API key 后显示余额。', 'Add a DeepSeek API key to show your balance.');
  return QUOTA_REPAIR[item.provider]?.[lang] || item.hints?.[lang] || QUOTA_HINT[item.provider]?.[lang] || '';
}

function FoldedProviders({ items, lang, missingCount, hiddenCount, onRestoreCard, onOpenSettings }) {
  if (items.length === 0) return null;
  return (
    <details className="quota-folded">
      <summary>
        <span className={`quota-folded-count${missingCount > 0 ? ' is-failure' : ''}`}>{missingCount} {STR.missing[lang]}</span>
        {hiddenCount > 0 && <span>{text(lang, `${hiddenCount} 个已隐藏`, `${hiddenCount} hidden`)}</span>}
      </summary>
      <ul className="quota-folded-list">
        {items.map((item) => {
          const repair = item.kind === 'unavailable' || item.provider === 'deepseek' ? repairText(item, lang) : '';
          return (
            <li key={item.provider} data-provider={item.provider}>
              <div className="quota-folded-row">
                <ProviderLogo provider={item.provider} />
                <strong>{item.title}</strong>
                <span className="quota-folded-kind">{item.kind === 'unavailable' ? STR.missing[lang] : text(lang, '已隐藏', 'Hidden')}</span>
                {item.hidden && <button type="button" onClick={() => onRestoreCard(item.provider)}>{STR.card_show[lang]}</button>}
                {item.provider === 'deepseek' && <button type="button" className="quota-folded-settings" onClick={onOpenSettings}>{text(lang, '前往设置', 'Open settings')}</button>}
              </div>
              {repair && <p className="quota-repair"><span>{STR.welcome_repair[lang]}</span>{repair}</p>}
            </li>
          );
        })}
      </ul>
    </details>
  );
}

export default function QuotaPage({
  cards,
  deepseekBalance,
  quotaFetchedAtMs,
  quotaStale,
  now,
  lang,
  preferences,
  planModels = {},
  onTogglePin,
  onToggleHidden,
  onRestoreCard,
  onOpenSettings,
  onPreferenceChange,
}) {
  const hiddenCards = preferences?.hiddenCards ?? [];
  const pinnedCards = preferences?.pinnedCards ?? [];
  const remainingView = preferences?.quotaRemainingView !== false;
  const observedAtMs = quotaFetchedAtMs > 0 ? quotaFetchedAtMs : null;
  const observedLabel = formatObservedAt(observedAtMs);
  const arranged = useMemo(() => arrangeQuota({
    cards,
    hiddenCards,
    pinnedCards,
    observedAtMs,
    now,
    stale: Boolean(quotaStale),
    remainingView,
  }), [cards, hiddenCards, pinnedCards, observedAtMs, now, quotaStale, remainingView]);
  const deepseekHidden = hiddenCards.includes('deepseek');
  const deepseekMissing = !deepseekBalance || deepseekBalance.status === 'missing_key';
  const folded = deepseekHidden || deepseekMissing
    ? [...arranged.folded, { provider: 'deepseek', title: 'DeepSeek API', kind: deepseekMissing ? 'unavailable' : 'balance', hidden: deepseekHidden, hints: null }]
    : arranged.folded;
  const missingCount = arranged.missingCount + (deepseekMissing ? 1 : 0);
  const hiddenCount = arranged.hiddenCount + (deepseekHidden && !deepseekMissing ? 1 : 0);
  const cardProps = {
    lang,
    now,
    planModels,
    clipboardClear: preferences?.clipboardClear !== false,
    onPreferenceChange,
    onTogglePin,
    onToggleHidden,
  };
  const showDeepseek = !deepseekHidden && !deepseekMissing;
  const nothingLive = arranged.quota.length === 0 && arranged.empty.length === 0;

  return <div className="quota-page quota-focus">
    <header className={`quota-observed${quotaStale ? ' is-stale' : ''}`}>
      <span className="eyebrow">{STR.updated[lang]}</span>
      {observedLabel
        ? <time dateTime={new Date(observedAtMs).toISOString()}>{observedLabel}</time>
        : <span>{STR.freshness_waiting[lang]}</span>}
      {arranged.quota.length > 0 && <span className="quota-view-label">{remainingView ? STR.quota_remaining[lang] : STR.quota_used[lang]}</span>}
      {quotaStale && observedLabel && <span className="quota-stale-label">{text(lang, '旧读数', 'Old reading')}</span>}
    </header>
    {nothingLive && <p className="quota-none">{STR.quota_unavailable[lang]}</p>}
    {arranged.quota.length > 0 && <section aria-label={STR.quota_overview[lang]}>
      <div className="quota-grid">
        {arranged.quota.map((item) => <ProviderCard key={item.provider} card={item.card} reading={item} pinned={item.pinned} providerColor={preferences?.providerColors?.[item.provider]} {...cardProps} />)}
      </div>
    </section>}
    {arranged.empty.length > 0 && <section className="quota-empty-live" aria-label={text(lang, '没有额度窗口', 'No quota window')}>
      <div className="quota-grid">
        {arranged.empty.map((item) => <ProviderCard key={item.provider} card={item.card} reading={item} pinned={item.pinned} providerColor={preferences?.providerColors?.[item.provider]} {...cardProps} />)}
      </div>
    </section>}
    {showDeepseek && <section className="quota-balance" aria-label={STR.quota_balance[lang]}>
      <h2 className="eyebrow">{STR.quota_balance[lang]}</h2>
      <div className="quota-grid">
        <DeepSeekCard
          balance={deepseekBalance}
          lang={lang}
          pinned={pinnedCards.includes('deepseek')}
          accent={preferences?.providerColors?.deepseek || PROVIDER_COLOR_DEFAULTS.deepseek}
          onTogglePin={onTogglePin}
          onToggleHidden={onToggleHidden}
          onOpenSettings={onOpenSettings}
        />
      </div>
    </section>}
    {arranged.activity.length > 0 && <section className="quota-activity" aria-label={STR.activity_today[lang]}>
      <h2 className="eyebrow">{STR.activity_today[lang]}</h2>
      <p className="quota-section-note">{text(lang, '相对 7 日均值，不是订阅剩余额度。', 'Compared with a 7-day average. Not remaining subscription quota.')}</p>
      <div className="quota-grid">
        {arranged.activity.map((item) => <ProviderCard key={item.provider} card={item.card} reading={item} pinned={item.pinned} providerColor={preferences?.providerColors?.[item.provider]} {...cardProps} />)}
      </div>
    </section>}
    <FoldedProviders
      items={folded}
      lang={lang}
      missingCount={missingCount}
      hiddenCount={hiddenCount}
      onRestoreCard={onRestoreCard}
      onOpenSettings={onOpenSettings}
    />
  </div>;
}
