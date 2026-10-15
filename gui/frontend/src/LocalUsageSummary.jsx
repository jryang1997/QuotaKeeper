import React from 'react';
import { STR, fmtTokens } from './ui-data.js';
import { usageCoverage } from './usage-data.js';

export default function LocalUsageSummary({ ledger, lang, stale, onOpenUsage }) {
  const t = (key) => STR[key][lang];
  const sources = ledger?.sources || [];
  const windows = ['today', 'week', 'month'].map((key) => ({
    key,
    total: ledger?.[key],
    state: usageCoverage(sources, '', { valid: true, total: ledger?.[key] }).state,
  }));
  const available = windows.some(({ state }) => state !== 'unknown');
  const partial = sources.some((source) => source.error);
  const note = !available ? t('local_usage_missing')
    : [partial && t('local_usage_partial'), stale && t('local_usage_previous')].filter(Boolean).join(' ');
  return <section className="local-usage-summary" aria-label={t('local_usage_title')}>
    <header>
      <div><span className="eyebrow">{t('local_usage_title')}</span><p>{t('local_usage_hint')}</p></div>
      <button type="button" onClick={onOpenUsage}>{t('local_usage_details')} <span aria-hidden="true">↗</span></button>
    </header>
    <div className="local-usage-values">
      {windows.map(({ key, total, state }) => <div key={key}>
        <span>{t(key)}</span>
        <strong>{state === 'unknown' ? '—' : fmtTokens(total?.total || 0, lang === 0)} <small>tokens</small></strong>
      </div>)}
    </div>
    {note && <p className="local-usage-note" role="status">{note}</p>}
  </section>;
}
