import React from 'react';
import { STR, fill, fmtInt, fmtRel } from './ui-data.js';
import { formatCurrency, formatTokenRate } from './currency.js';
import { snapshotFetchAge } from './pricing-data.js';

const RATE_BUCKETS = ['input', 'output', 'cache_read', 'cache_write'];

function priceTime(value, lang) {
  if (!Number.isInteger(value) || !Number.isFinite(new Date(value).getTime())) return STR.price_time_unknown[lang];
  return new Date(value).toLocaleString(lang === 0 ? 'zh-CN' : 'en-US', {
    year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit',
    hour12: false, timeZoneName: 'short',
  });
}

export function PricingSummary({ pricing, currency, lang }) {
  const t = (key) => STR[key][lang];
  const records = (n) => fill(t('price_records'), { n: fmtInt(n) });
  return <span className="usage-price-summary">
    {['reported', 'estimated'].map((kind) => <span key={kind} data-price-kind={kind}>
      {t(`price_${kind}`)} {formatCurrency(pricing[kind].records > 0 ? pricing[kind].amount : null, currency)} · {records(pricing[kind].records)}
    </span>)}
    <span data-price-kind="unpriced">{t('price_unpriced')} · {records(pricing.unpriced.records)}</span>
  </span>;
}

function PriceBasis({ group, currency, lang }) {
  const t = (key) => STR[key][lang];
  const { basis } = group;
  const estimate = basis.kind === 'estimated' ? basis : basis.attempted;
  return <details className="usage-price-basis" data-price-basis-id={group.id}>
    <summary>
      <span className="usage-price-basis-name"><span>{t(`price_${basis.kind}`)}</span>{estimate && <code>{estimate.matched_entry}</code>}</span>
      <span>{fill(t('price_records'), { n: fmtInt(group.records) })} · {formatCurrency(group.amount, currency)}</span>
    </summary>
    <div className="usage-price-basis-body">
      <p>{basis.kind === 'unpriced' ? t(`price_missing_${basis.reason}`) : t(`price_${basis.kind}_hint`)}</p>
      <dl className="usage-price-facts">
        <div><dt>{t('price_raw_names')}</dt><dd>{group.rawModels.map((name) => <code key={name}>{name || t('price_empty_name')}</code>)}</dd></div>
        <div><dt>{t('period_agent')}</dt><dd>{group.clients.join(lang === 0 ? '、' : ', ')}</dd></div>
        {estimate && <>
          <div><dt>{t(basis.kind === 'unpriced' ? 'price_attempted_entry' : 'price_adopted_entry')}</dt><dd><code>{estimate.matched_entry}</code></dd></div>
          <div><dt>{t('price_match')}</dt><dd>{t(`price_match_${estimate.match_method}`)}</dd></div>
        </>}
      </dl>
      {estimate && <>
        {estimate.family_fallback && <p className="usage-price-approx">{t('price_family')}</p>}
        {estimate.cache_fill && <p className="usage-price-approx">{t('price_cache_fill')}</p>}
        <p className="usage-price-rate-unit">{t('price_rate_unit')}</p>
        <dl className="usage-price-rates">{RATE_BUCKETS.map((bucket) => {
          const rate = estimate[bucket];
          return <div key={bucket} data-price-bucket={bucket}>
            <dt>{t(`price_bucket_${bucket}`)}</dt>
            <dd>{rate ? <>
              <span>{formatTokenRate(rate.usd_per_token, currency)}</span>
              <small>{t(`price_origin_${rate.origin}`)}</small>
              <code>{rate.entry}</code>
            </> : <span>{t('price_no_rate')}</span>}</dd>
          </div>;
        })}</dl>
        <p>{t('usage_reasoning_in_output')}</p>
      </>}
    </div>
  </details>;
}

export default function PricingDetails({ pricing, snapshot, currency, currencyFallback, lang }) {
  const t = (key) => STR[key][lang];
  const fetchAge = snapshotFetchAge(snapshot);
  return <details className="usage-surface usage-price-disclosure">
    <summary>{t('price_title')}</summary>
    <div className="usage-price-content">
      <p>{t('price_history_hint')}</p>
      <p>{t('price_catalogue_hint')}</p>
      {currencyFallback && <p>{t('price_currency_fallback')}</p>}
      <dl className="usage-price-facts usage-price-snapshot">
        <div><dt>{t('price_snapshot')}</dt><dd>{t(`price_source_${snapshot.source}`)}</dd></div>
        <div><dt>{t('price_cache_state')}</dt><dd>{t(`price_cache_${snapshot.cache_state}`)}</dd></div>
        <div><dt>{t('price_fetched')}</dt><dd>{priceTime(snapshot.fetched_at_ms, lang)}</dd></div>
        <div><dt>{t('price_fetch_age')}</dt><dd>{fetchAge === null ? t('price_age_unknown') : fmtRel(fetchAge)}</dd></div>
        <div><dt>{t('price_loaded')}</dt><dd>{priceTime(snapshot.loaded_at_ms, lang)}</dd></div>
        {snapshot.dataset_url && <div><dt>{t('price_dataset')}</dt><dd>{snapshot.dataset_url}</dd></div>}
      </dl>
      <p>{t('price_load_hint')}</p>
      <div className="usage-price-bases">{pricing.bases.map((group) => <PriceBasis key={group.id} group={group} currency={currency} lang={lang} />)}</div>
      {pricing.bases.length === 0 && <p>{t('usage_no_records')}</p>}
      <p>{t('price_export_hint')}</p>
    </div>
  </details>;
}