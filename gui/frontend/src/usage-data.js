export const EMPTY_USAGE_AGG = Object.freeze({
  input: 0, output: 0, cache_read: 0, cache_write: 0,
  reasoning: 0, messages: 0, cost: 0, total: 0,
});

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
  for (const key of Object.keys(EMPTY_USAGE_AGG)) target[key] += Number(source?.[key] || 0);
}

function freshAgg() { return { ...EMPTY_USAGE_AGG }; }

export function aggregateUsage(rows, bounds, agent = '') {
  const valid = /^\d{4}-\d{2}-\d{2}$/.test(bounds.start) && /^\d{4}-\d{2}-\d{2}$/.test(bounds.end) && bounds.start <= bounds.end;
  const total = freshAgg();
  const dayMap = new Map();
  let unpriced = 0;
  if (!valid) return { valid: false, total, days: [], unpriced };
  const count = Math.round((new Date(`${bounds.end}T12:00:00`) - new Date(`${bounds.start}T12:00:00`)) / 86_400_000) + 1;
  if (!Number.isFinite(count) || count < 1 || count > 366) return { valid: false, total, days: [], unpriced };

  for (const row of rows || []) {
    if (row.date < bounds.start || row.date > bounds.end || (agent && row.client !== agent)) continue;
    addAgg(total, row.agg);
    unpriced += Number(row.unpriced_messages || 0);
    let day = dayMap.get(row.date);
    if (!day) {
      day = { date: row.date, agg: freshAgg(), agents: new Map(), unpriced: 0 };
      dayMap.set(row.date, day);
    }
    addAgg(day.agg, row.agg);
    day.unpriced += Number(row.unpriced_messages || 0);
    if (!day.agents.has(row.client)) day.agents.set(row.client, freshAgg());
    addAgg(day.agents.get(row.client), row.agg);
  }

  const days = [];
  for (let index = 0; index < count; index++) {
    const date = dateOffset(bounds.start, index);
    days.push(dayMap.get(date) || { date, agg: freshAgg(), agents: new Map(), unpriced: 0 });
  }
  return { valid: true, total, days, unpriced };
}

export function aggregateIntraday(rows, date, agent = '') {
  const buckets = Array.from({ length: 12 }, (_, index) => {
    const hour = index * 2;
    const start = `${String(hour).padStart(2, '0')}:00`;
    const end = `${String(hour + 2).padStart(2, '0')}:00`;
    return { date: `${date} ${start}–${end}`, label: start, agg: freshAgg(), agents: new Map(), unpriced: 0 };
  });
  for (const row of rows || []) {
    if (row.date !== date || (agent && row.client !== agent) || !Number.isInteger(row.hour) || row.hour < 0 || row.hour > 23) continue;
    const bucket = buckets[Math.floor(row.hour / 2)];
    addAgg(bucket.agg, row.agg);
    bucket.unpriced += Number(row.unpriced_messages || 0);
    if (!bucket.agents.has(row.client)) bucket.agents.set(row.client, freshAgg());
    addAgg(bucket.agents.get(row.client), row.agg);
  }
  return buckets;
}

export function usageMetric(agg, metric) {
  return metric === 'cost' ? agg.cost : metric === 'messages' ? agg.messages : agg.total;
}

// 缓存命中率:input 口径不含缓存(采集侧已扣),命中占全部提示词的比例.
// 无输入(如纯输出)或无数据返回 null,由调用方显示占位.
export function cacheHitRate(agg) {
  const prompt = Number(agg?.input || 0) + Number(agg?.cache_read || 0);
  if (prompt <= 0) return null;
  return Number(agg.cache_read || 0) / prompt;
}

// 时长展示:亚秒给毫秒,10 秒内一位小数,再往上取整秒.
export function fmtDurationMs(ms) {
  if (ms == null || ms <= 0) return '—';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 10_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.round(ms / 1000)}s`;
}

// 速度榜行:tok/s = 计时段合计输出 / 计时总长;ttft 只对有 ttft 样本的调用取均值.
// 仅收有计时样本的模型,按 tok/s 降序(最快在前才有榜单的趣味).
export function speedBoard(models, limit = 6) {
  const timed = (models || [])
    .filter((m) => m.speed && m.speed.duration_ms_total > 0 && m.speed.timed_calls > 0)
    .map((m) => ({
      model: m.model,
      timedCalls: m.speed.timed_calls,
      ttftMs: m.speed.ttft_samples > 0 ? m.speed.ttft_ms_total / m.speed.ttft_samples : null,
      tokPerSec: m.speed.output_total / (m.speed.duration_ms_total / 1000),
    }))
    .sort((a, b) => b.tokPerSec - a.tokPerSec);
  const rest = (models || []).length - timed.length;
  return { rows: timed.slice(0, limit), unTimed: Math.max(0, rest) };
}
