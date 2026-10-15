export const EMPTY_USAGE_AGG = Object.freeze({
  input: 0, output: 0, cache_read: 0, cache_write: 0,
  reasoning: 0, messages: 0, cost: 0, cost_reported: 0, cost_estimated: 0, total: 0,
});

const AGG_KEYS = Object.keys(EMPTY_USAGE_AGG);

export function localDate(date = new Date()) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

export function dateOffset(date, days) {
  const [year, month, day] = date.split('-').map(Number);
  return localDate(new Date(year, month - 1, day + days));
}

export function usageBounds(range, today = localDate(), customStart = '', customEnd = '') {
  if (range === 'today') return { start: today, end: today };
  if (range === 'yesterday') return { start: dateOffset(today, -1), end: dateOffset(today, -1) };
  if (range === '30d') return { start: dateOffset(today, -29), end: today };
  if (range === 'month') return { start: `${today.slice(0, 7)}-01`, end: today };
  if (range === 'custom') return { start: customStart, end: customEnd };
  return { start: dateOffset(today, -6), end: today };
}

function addAgg(target, source) {
  for (const key of AGG_KEYS) target[key] += Number(source?.[key] || 0);
}

function freshAgg() { return { ...EMPTY_USAGE_AGG }; }

function byClientRows(clientMap) {
  return [...clientMap.values()].sort((a, b) => b.agg.total - a.agg.total || b.agg.messages - a.agg.messages || String(a.client).localeCompare(String(b.client)));
}

function matchesSelection(row, bounds, agent, model) {
  return row.date >= bounds.start && row.date <= bounds.end
    && (!agent || row.client === agent) && (!model || row.model === model);
}

function calendarStamp(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return NaN;
  const stamp = Date.parse(`${value}T00:00:00Z`);
  return Number.isFinite(stamp) && new Date(stamp).toISOString().slice(0, 10) === value ? stamp : NaN;
}

export function aggregateUsage(rows, bounds, agent = '', model = '') {
  const start = calendarStamp(bounds.start);
  const end = calendarStamp(bounds.end);
  const valid = Number.isFinite(start) && Number.isFinite(end) && start <= end;
  const total = freshAgg();
  const dayMap = new Map();
  const clientMap = new Map();
  const modelMap = new Map();
  let unpriced = 0;
  const empty = { valid: false, total, days: [], byClient: [], byModel: [], unpriced };
  if (!valid) return empty;
  const count = (end - start) / 86_400_000 + 1;
  if (!Number.isFinite(count) || count < 1 || count > 366) return empty;

  for (const row of rows || []) {
    if (!matchesSelection(row, bounds, agent, model)) continue;
    addAgg(total, row.agg);
    const rowUnpriced = Number(row.unpriced_messages || 0);
    unpriced += rowUnpriced;
    let day = dayMap.get(row.date);
    if (!day) {
      day = { date: row.date, agg: freshAgg(), agents: new Map(), unpriced: 0 };
      dayMap.set(row.date, day);
    }
    addAgg(day.agg, row.agg);
    day.unpriced += rowUnpriced;
    if (!day.agents.has(row.client)) day.agents.set(row.client, freshAgg());
    addAgg(day.agents.get(row.client), row.agg);
    let client = clientMap.get(row.client);
    if (!client) {
      client = { client: row.client, agg: freshAgg(), unpriced: 0 };
      clientMap.set(row.client, client);
    }
    addAgg(client.agg, row.agg);
    client.unpriced += rowUnpriced;
    const name = row.model ?? 'unknown';
    let modelRow = modelMap.get(name);
    if (!modelRow) {
      modelRow = { model: name, agg: freshAgg(), unpriced: 0, rawModels: new Set(), clients: new Set() };
      modelMap.set(name, modelRow);
    }
    addAgg(modelRow.agg, row.agg);
    modelRow.unpriced += rowUnpriced;
    modelRow.rawModels.add(row.raw_model ?? '');
    modelRow.clients.add(row.client);
  }

  const days = [];
  for (let index = 0; index < count; index++) {
    const date = dateOffset(bounds.start, index);
    days.push(dayMap.get(date) || { date, agg: freshAgg(), agents: new Map(), unpriced: 0 });
  }
  const byModel = [...modelMap.values()]
    .sort((a, b) => b.agg.total - a.agg.total || b.agg.messages - a.agg.messages || String(a.model).localeCompare(String(b.model)))
    .map((row) => ({ ...row, rawModels: [...row.rawModels].sort(), clients: [...row.clients].sort() }));
  return { valid: true, total, days, byClient: byClientRows(clientMap), byModel, unpriced };
}

export function aggregateIntraday(rows, date, agent = '', model = '') {
  const bounds = { start: date, end: date };
  const buckets = Array.from({ length: 12 }, (_, index) => {
    const hour = index * 2;
    const start = `${String(hour).padStart(2, '0')}:00`;
    const end = `${String(hour + 2).padStart(2, '0')}:00`;
    return { date: `${date} ${start}–${end}`, label: start, agg: freshAgg(), agents: new Map(), unpriced: 0 };
  });
  for (const row of rows || []) {
    if (!matchesSelection(row, bounds, agent, model) || !Number.isInteger(row.hour) || row.hour < 0 || row.hour > 23) continue;
    const bucket = buckets[Math.floor(row.hour / 2)];
    addAgg(bucket.agg, row.agg);
    bucket.unpriced += Number(row.unpriced_messages || 0);
    if (!bucket.agents.has(row.client)) bucket.agents.set(row.client, freshAgg());
    addAgg(bucket.agents.get(row.client), row.agg);
  }
  return buckets;
}

export function usageMetric(agg, metric, unpriced = 0) {
  if (metric === 'cost') return unpriced > 0 && unpriced >= Number(agg.messages || 0) ? null : agg.cost;
  return metric === 'messages' ? agg.messages : agg.total;
}

const COST_TOLERANCE = 1e-6;
const COST_KEYS = new Set(['cost', 'cost_reported', 'cost_estimated']);

export function seriesMatchesTotal(buckets, total, unpriced = 0) {
  const rolled = freshAgg();
  let rolledUnpriced = 0;
  for (const bucket of buckets || []) {
    addAgg(rolled, bucket?.agg);
    rolledUnpriced += Number(bucket?.unpriced || 0);
  }
  return rolledUnpriced === Number(unpriced || 0) && AGG_KEYS.every((key) => {
    const actual = rolled[key];
    const expected = Number(total?.[key] || 0);
    return COST_KEYS.has(key) ? Math.abs(actual - expected) <= COST_TOLERANCE : actual === expected;
  });
}

export function usageCoverage(sources, agent, summary) {
  if (!summary?.valid) return { state: 'invalid', failed: [] };
  if (!Array.isArray(sources) || sources.length === 0) return { state: 'unknown', failed: [] };
  const relevant = agent ? sources.filter((source) => source?.client === agent) : sources;
  if (relevant.length === 0) return { state: 'unknown', failed: [] };
  const failed = relevant.filter((source) => source?.error).map((source) => source.client);
  const recorded = Number(summary.total?.total || 0) !== 0 || Number(summary.total?.messages || 0) !== 0;
  if (failed.length === 0) return { state: recorded ? 'complete' : 'empty', failed };
  return { state: recorded ? 'partial' : 'unknown', failed };
}
