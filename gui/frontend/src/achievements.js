// 成就:全部由本机账本推导(不掺服务商配额),纯函数评估,便于测试与跨页复用。
// unlocked 状态与首次解锁时间存在 localStorage;新解锁由壳层弹 toast。

export const ACHIEVEMENTS_KEY = 'token-monitor.achievements.v1';

export const TIER_ORDER = ['bronze', 'silver', 'gold', 'legend'];

const sum = (list, pick) => (list || []).reduce((total, item) => total + (Number(pick(item)) || 0), 0);

// 从 LedgerReport 抽出成就需要的标量事实;缺字段一律退回 0,不抛错。
export function ledgerFacts(ledger) {
  const days = ledger?.daily_totals || [];
  const activeDays = days.filter((day) => day.tokens > 0);
  let bestStreak = 0;
  let run = 0;
  for (const day of days) {
    run = day.tokens > 0 ? run + 1 : 0;
    bestStreak = Math.max(bestStreak, run);
  }
  let currentStreak = 0;
  for (let index = days.length - 1; index >= 0 && days[index].tokens > 0; index--) currentStreak++;
  const weekdays = ledger?.weekday || [];
  const weekHours = ledger?.weekday_hour || [];
  const models = ledger?.by_model || [];
  const month = ledger?.month || {};
  const prompt = Number(month.input || 0) + Number(month.cache_read || 0);
  const fastest = speedTop(models);
  const bestDay = days.reduce((best, day) => (day.tokens > (best?.tokens || 0) ? day : best), null);
  const busiestDay = days.reduce((best, day) => (day.messages > (best?.messages || 0) ? day : best), null);
  const topSession = (ledger?.by_session || []).reduce((best, row) => ((row.agg?.total || 0) > (best?.agg?.total || 0) ? row : best), null);
  return {
    tokens: sum(days, (day) => day.tokens),
    messages: sum(days, (day) => day.messages),
    cost: sum(days, (day) => day.cost),
    activeDays: activeDays.length,
    bestStreak,
    currentStreak,
    bestDayTokens: bestDay?.tokens || 0,
    busiestDayMessages: busiestDay?.messages || 0,
    weekendTokens: (weekdays[5] || 0) + (weekdays[6] || 0),
    nightTokens: sum(weekHours, (row) => (row || []).slice(0, 5).reduce((total, value) => total + (Number(value) || 0), 0)),
    earlyTokens: sum(weekHours, (row) => (row || []).slice(5, 9).reduce((total, value) => total + (Number(value) || 0), 0)),
    models: models.length,
    projects: (ledger?.by_project || []).length,
    agents: (ledger?.month_by_client || []).filter((row) => (row.agg?.total || 0) > 0).length,
    topSessionTokens: topSession?.agg?.total || 0,
    cacheHit: prompt > 0 ? Number(month.cache_read || 0) / prompt : 0,
    promptTokens: prompt,
    fastestTokPerSec: fastest,
  };
}

function speedTop(models) {
  let fastest = 0;
  for (const model of models) {
    const speed = model?.speed;
    if (!speed || !(speed.duration_ms_total > 0) || !(speed.timed_calls > 0)) continue;
    fastest = Math.max(fastest, Number(speed.output_total || 0) / (speed.duration_ms_total / 1000));
  }
  return fastest;
}

// 每个成就:value(facts) 给当前进度,target 是解锁线;ready 是额外门槛(如样本量)。
export const ACHIEVEMENTS = [
  { id: 'first-light', tier: 'bronze', glyph: 'spark', value: (f) => f.messages, target: 1,
    title: ['初次点亮', 'First light'], hint: ['记录到第一次调用', 'Log your first call'] },
  { id: 'calls-1k', tier: 'bronze', glyph: 'bolt', value: (f) => f.messages, target: 1_000,
    title: ['一千次对话', '1,000 calls'], hint: ['累计请求数达到 1,000', 'Reach 1,000 requests'] },
  { id: 'calls-10k', tier: 'gold', glyph: 'bolt', value: (f) => f.messages, target: 10_000,
    title: ['一万次对话', '10,000 calls'], hint: ['累计请求数达到 10,000', 'Reach 10,000 requests'] },
  { id: 'tokens-100m', tier: 'bronze', glyph: 'layers', value: (f) => f.tokens, target: 100_000_000,
    title: ['一亿达成', '100M tokens'], hint: ['累计 1 亿 token', 'Reach 100M tokens in total'] },
  { id: 'tokens-1b', tier: 'silver', glyph: 'layers', value: (f) => f.tokens, target: 1_000_000_000,
    title: ['十亿俱乐部', 'Billion club'], hint: ['累计 10 亿 token', 'Reach 1B tokens in total'] },
  { id: 'tokens-10b', tier: 'legend', glyph: 'layers', value: (f) => f.tokens, target: 10_000_000_000,
    title: ['百亿殿堂', 'Ten-billion hall'], hint: ['累计 100 亿 token', 'Reach 10B tokens in total'] },
  { id: 'streak-3', tier: 'bronze', glyph: 'flame', value: (f) => f.bestStreak, target: 3,
    title: ['三日不辍', '3-day streak'], hint: ['连续 3 天留下记录', 'Stay active 3 days in a row'] },
  { id: 'streak-7', tier: 'silver', glyph: 'flame', value: (f) => f.bestStreak, target: 7,
    title: ['一周不断', '7-day streak'], hint: ['连续 7 天留下记录', 'Stay active 7 days in a row'] },
  { id: 'streak-30', tier: 'gold', glyph: 'flame', value: (f) => f.bestStreak, target: 30,
    title: ['满月连击', '30-day streak'], hint: ['连续 30 天留下记录', 'Stay active 30 days in a row'] },
  { id: 'streak-100', tier: 'legend', glyph: 'flame', value: (f) => f.bestStreak, target: 100,
    title: ['百日筑基', '100-day streak'], hint: ['连续 100 天留下记录', 'Stay active 100 days in a row'] },
  { id: 'day-10m', tier: 'silver', glyph: 'sun', value: (f) => f.bestDayTokens, target: 10_000_000,
    title: ['单日千万', '10M day'], hint: ['单日 token 达到 1,000 万', 'Burn 10M tokens in one day'] },
  { id: 'day-100m', tier: 'legend', glyph: 'sun', value: (f) => f.bestDayTokens, target: 100_000_000,
    title: ['单日一亿', '100M day'], hint: ['单日 token 达到 1 亿', 'Burn 100M tokens in one day'] },
  { id: 'night-owl', tier: 'silver', glyph: 'moon', value: (f) => f.nightTokens, target: 5_000_000,
    title: ['夜猫子', 'Night owl'], hint: ['0–5 点累计 500 万 token', '5M tokens between 00:00 and 05:00'] },
  { id: 'early-bird', tier: 'silver', glyph: 'sunrise', value: (f) => f.earlyTokens, target: 5_000_000,
    title: ['早鸟', 'Early bird'], hint: ['5–9 点累计 500 万 token', '5M tokens between 05:00 and 09:00'] },
  { id: 'weekend-warrior', tier: 'silver', glyph: 'star', value: (f) => f.weekendTokens, target: 20_000_000,
    title: ['周末战士', 'Weekend warrior'], hint: ['周末累计 2,000 万 token', '20M tokens on weekends'] },
  { id: 'models-10', tier: 'silver', glyph: 'diamond', value: (f) => f.models, target: 10,
    title: ['模型收藏家', 'Model collector'], hint: ['用过 10 个不同模型', 'Use 10 different models'] },
  { id: 'agents-4', tier: 'gold', glyph: 'stack', value: (f) => f.agents, target: 4,
    title: ['Agent 全家桶', 'Full agent stack'], hint: ['本月 4 个不同 Agent 有记录', 'Record usage from 4 agents this month'] },
  { id: 'projects-10', tier: 'bronze', glyph: 'compass', value: (f) => f.projects, target: 10,
    title: ['十面手', 'Multi-project'], hint: ['在 10 个项目里留下用量', 'Log usage across 10 projects'] },
  { id: 'cache-60', tier: 'gold', glyph: 'gauge', value: (f) => f.cacheHit * 100, target: 60,
    ready: (f) => f.promptTokens >= 1_000_000,
    title: ['缓存大师', 'Cache master'], hint: ['本月缓存命中率 ≥ 60%(需 100 万 prompt token 样本)', 'Hit 60% cache rate this month (1M prompt tokens sample)'] },
  { id: 'marathon', tier: 'gold', glyph: 'rocket', value: (f) => f.topSessionTokens, target: 5_000_000,
    title: ['马拉松会话', 'Marathon session'], hint: ['单个会话用到 500 万 token', 'Spend 5M tokens in a single session'] },
  { id: 'speed-120', tier: 'gold', glyph: 'wave', value: (f) => f.fastestTokPerSec, target: 120,
    title: ['疾速', 'Full speed'], hint: ['实测某模型 ≥ 120 tok/s', 'Measured 120 tok/s on any model'] },
  { id: 'spend-1k', tier: 'legend', glyph: 'crown', value: (f) => f.cost, target: 1_000,
    title: ['千金一掷', 'Thousand-dollar club'], hint: ['本机估算累计价值达到 $1,000', 'Cross $1,000 in estimated value'] },
];

export function evaluateAchievements(ledger) {
  const facts = ledgerFacts(ledger);
  return ACHIEVEMENTS.map((def) => {
    const value = Math.max(0, Number(def.value(facts)) || 0);
    const ratio = def.target > 0 ? value / def.target : 0;
    const ready = def.ready ? !!def.ready(facts) : true;
    return {
      id: def.id,
      tier: def.tier,
      glyph: def.glyph,
      title: def.title,
      hint: def.hint,
      value,
      target: def.target,
      pct: Math.max(0, Math.min(1, ratio)),
      unlocked: ready && value >= def.target,
    };
  });
}

export function readAchievementState(storage = globalThis.localStorage) {
  try {
    const raw = JSON.parse(storage.getItem(ACHIEVEMENTS_KEY) || '{}');
    const unlocked = raw && typeof raw.unlocked === 'object' && raw.unlocked ? raw.unlocked : {};
    return { unlocked: Object.fromEntries(Object.entries(unlocked).filter(([, at]) => Number.isFinite(at))) };
  } catch {
    return { unlocked: {} };
  }
}

export function saveAchievementState(state, storage = globalThis.localStorage) {
  try { storage.setItem(ACHIEVEMENTS_KEY, JSON.stringify(state)); } catch { /* session only */ }
  return state;
}

// 与已存状态比对:返回新解锁集合 + 合并后的状态(首次解锁时间保留)。
export function applyUnlocks(list, state, now = Date.now()) {
  const unlocked = { ...state.unlocked };
  const fresh = [];
  for (const item of list) {
    if (item.unlocked && !unlocked[item.id]) {
      unlocked[item.id] = now;
      fresh.push(item.id);
    }
  }
  return { state: { unlocked }, fresh };
}
