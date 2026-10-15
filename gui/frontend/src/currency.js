export const CURRENCIES = ['USD', 'CNY', 'EUR', 'GBP', 'JPY', 'CAD', 'AUD', 'HKD', 'SGD', 'KRW', 'INR', 'BRL'];
const CACHE_KEY = 'token-monitor.exchange-rates.v1';
const MAX_AGE = 24 * 60 * 60 * 1000;

export function readExchangeRates(storage = globalThis.localStorage) {
  try {
    const rates = JSON.parse(storage.getItem(CACHE_KEY) || '{}');
    return typeof rates === 'object' && rates !== null ? rates : {};
  } catch { return {}; }
}

export async function fetchExchangeRate(code, storage = globalThis.localStorage, fetcher = globalThis.fetch) {
  if (code === 'USD') return { rate: 1, date: '', fetchedAt: Date.now() };
  const cached = readExchangeRates(storage)[code];
  if (cached?.rate > 0 && Date.now() - cached.fetchedAt < MAX_AGE) return cached;
  try {
    const response = await fetcher(`https://api.frankfurter.dev/v2/rate/usd/${code.toLowerCase()}`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const data = await response.json();
    if (data.base?.toUpperCase() !== 'USD' || data.quote?.toUpperCase() !== code || !Number.isFinite(data.rate) || data.rate <= 0) throw new Error('Invalid exchange rate');
    const rate = { rate: data.rate, date: data.date, fetchedAt: Date.now() };
    try { storage.setItem(CACHE_KEY, JSON.stringify({ ...readExchangeRates(storage), [code]: rate })); } catch { /* usable for this session */ }
    return rate;
  } catch (error) {
    if (cached?.rate > 0) return { ...cached, stale: true };
    throw error;
  }
}

export function formatCurrency(usd, currency = { code: 'USD', rate: 1 }) {
  if (usd == null) return '—';
  const { code = 'USD' } = currency;
  const rate = currency.rate ?? (code === 'USD' ? 1 : null);
  if (!Number.isFinite(rate) || rate <= 0) return '—';
  const amount = usd * rate;
  const digits = amount >= 100 ? 0 : amount >= 1 ? 2 : amount > 0 ? 4 : 2;
  return new Intl.NumberFormat(undefined, { style: 'currency', currency: code, minimumFractionDigits: digits, maximumFractionDigits: digits }).format(amount);
}

export function pricingCurrency(currency = {}) {
  const { code = 'USD', rate } = currency;
  return code !== 'USD' && Number.isFinite(rate) && rate > 0 ? { code, rate } : { code: 'USD', rate: 1 };
}

export function formatTokenRate(usdPerToken, currency) {
  if (!Number.isFinite(usdPerToken) || usdPerToken < 0) return '—';
  const { code, rate } = pricingCurrency(currency);
  const amount = usdPerToken * 1_000_000 * rate;
  const notation = amount > 0 && amount < 0.0001 ? 'scientific' : 'standard';
  return `${code} ${new Intl.NumberFormat(undefined, { maximumSignificantDigits: 8, notation }).format(amount)}`;
}
