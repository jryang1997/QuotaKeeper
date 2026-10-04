// 成就:账本事实 + 交互彩蛋 + meta 阶梯,全部纯函数评估,便于测试与跨页复用。
// unlocked 状态与首次解锁时间存在 localStorage;新解锁由壳层弹 toast。
// extras(可选):monthlyValue(钱)、counters(交互计数)、unlockedCount(非 meta 已解锁数)。

export const ACHIEVEMENTS_KEY = 'token-monitor.achievements.v1';

export const TIER_ORDER = ['bronze', 'silver', 'gold', 'legend', 'platinum'];

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
  const longestSession = (ledger?.by_session || []).reduce((best, row) => ((row.agg?.messages || 0) > (best?.agg?.messages || 0) ? row : best), null);
  const projects = ledger?.by_project || [];
  const projectTotal = sum(projects, (row) => row.agg?.total);
  const topProject = projects.reduce((best, row) => ((row.agg?.total || 0) > (best?.agg?.total || 0) ? row : best), null);
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
    lunchTokens: sum(weekHours, (row) => (row || []).slice(12, 14).reduce((total, value) => total + (Number(value) || 0), 0)),
    eveningTokens: sum(weekHours, (row) => (row || []).slice(18, 22).reduce((total, value) => total + (Number(value) || 0), 0)),
    hourCoverage: (ledger?.hourly || []).filter((value) => Number(value) > 0).length,
    sourceCount: (ledger?.sources || []).filter((source) => (source?.records || 0) > 0).length,
    models: models.length,
    projects: projects.length,
    topProjectShare: projectTotal > 0 ? (topProject?.agg?.total || 0) / projectTotal : 0,
    agents: (ledger?.month_by_client || []).filter((row) => (row.agg?.total || 0) > 0).length,
    topSessionTokens: topSession?.agg?.total || 0,
    topSessionMessages: longestSession?.agg?.messages || 0,
    reasoningMonth: Number(month.reasoning || 0),
    outputMonth: Number(month.output || 0),
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

// value(facts, extras) 给当前进度,target 是解锁线;ready 是额外门槛(如样本量)。
// meta: 'count' 按 extras.unlockedCount 计进度;'all' 追加「非 meta 全解锁」。
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
  { id: 'day-10m', tier: 'silver', glyph: 'sun', value: (f) => f.bestDayTokens, target: 100_000_000,
    title: ['单日一亿', '100M day'], hint: ['单日 token 达到 1 亿', 'Burn 100M tokens in one day'] },
  { id: 'day-100m', tier: 'legend', glyph: 'sun', value: (f) => f.bestDayTokens, target: 500_000_000,
    title: ['单日五亿', '500M day'], hint: ['单日 token 达到 5 亿', 'Burn 500M tokens in one day'] },
  { id: 'night-owl', tier: 'silver', glyph: 'moon', value: (f) => f.nightTokens, target: 50_000_000,
    title: ['夜猫子', 'Night owl'], hint: ['0–5 点累计 5,000 万 token', '50M tokens between 00:00 and 05:00'] },
  { id: 'early-bird', tier: 'silver', glyph: 'sunrise', value: (f) => f.earlyTokens, target: 100_000_000,
    title: ['早鸟', 'Early bird'], hint: ['5–9 点累计 1 亿 token', '100M tokens between 05:00 and 09:00'] },
  { id: 'weekend-warrior', tier: 'silver', glyph: 'star', value: (f) => f.weekendTokens, target: 1_000_000_000,
    title: ['周末战士', 'Weekend warrior'], hint: ['周末累计 10 亿 token', '1B tokens on weekends'] },
  { id: 'lunch-grinder', tier: 'silver', glyph: 'noodle', value: (f) => f.lunchTokens, target: 100_000_000,
    title: ['午休卷王', 'Lunch grinder'], hint: ['12–14 点累计 1 亿 token', '100M tokens over lunch (12:00–14:00)'] },
  { id: 'after-hours', tier: 'silver', glyph: 'sundown', value: (f) => f.eveningTokens, target: 200_000_000,
    title: ['下班冲刺', 'After hours'], hint: ['18–22 点累计 2 亿 token', '200M tokens from 18:00 to 22:00'] },
  { id: 'round-the-clock', tier: 'gold', glyph: 'clock', value: (f) => f.hourCoverage, target: 24,
    title: ['全天候', 'Round the clock'], hint: ['近 30 天每个小时都有记录', 'Every hour of the day logged in 30 days'] },
  { id: 'models-10', tier: 'silver', glyph: 'diamond', value: (f) => f.models, target: 10,
    title: ['模型收藏家', 'Model collector'], hint: ['用过 10 个不同模型', 'Use 10 different models'] },
  { id: 'agents-4', tier: 'gold', glyph: 'stack', value: (f) => f.agents, target: 4,
    title: ['Agent 全家桶', 'Full agent stack'], hint: ['本月 4 个不同 Agent 有记录', 'Record usage from 4 agents this month'] },
  { id: 'full-house', tier: 'legend', glyph: 'hex', value: (f) => f.sourceCount, target: 6,
    title: ['六边形战士', 'Full house'], hint: ['六类来源全部有记录', 'All six sources have records'] },
  { id: 'projects-10', tier: 'bronze', glyph: 'compass', value: (f) => f.projects, target: 10,
    title: ['十面手', 'Multi-project'], hint: ['在 10 个项目里留下用量', 'Log usage across 10 projects'] },
  { id: 'one-trick', tier: 'bronze', glyph: 'heart', value: (f) => f.topProjectShare * 100, target: 80,
    ready: (f) => f.projects >= 3,
    title: ['独宠', 'One trick'], hint: ['近 30 天单一项目占比 ≥ 80%(至少 3 个项目)', 'One project takes 80% of 30 days (3+ projects)'] },
  { id: 'socializer', tier: 'gold', glyph: 'trident', value: (f) => f.projects, target: 6,
    ready: (f) => f.topProjectShare < 0.4 && f.topProjectShare > 0,
    title: ['海王', 'Socializer'], hint: ['近 30 天 6 个以上项目,无一占比超 40%', '6+ projects in 30 days, none over 40%'] },
  { id: 'long-haul', tier: 'silver', glyph: 'chat', value: (f) => f.topSessionMessages, target: 1_000,
    title: ['千轮马拉松', 'Long haul'], hint: ['单个会话 1,000 条消息', '1,000 messages in one session'] },
  { id: 'cache-60', tier: 'gold', glyph: 'gauge', value: (f) => f.cacheHit * 100, target: 90,
    ready: (f) => f.promptTokens >= 1_000_000,
    title: ['缓存大师', 'Cache master'], hint: ['本月缓存命中率 ≥ 90%(需 100 万 prompt token 样本)', 'Hit 90% cache rate this month (1M prompt tokens sample)'] },
  { id: 'deep-thinker', tier: 'bronze', glyph: 'thought', value: (f) => f.reasoningMonth, target: 1_000_000,
    ready: (f) => f.reasoningMonth > f.outputMonth,
    title: ['想得多', 'Deep thinker'], hint: ['本月思考 token 超 100 万且反超输出', 'Reasoning over 1M and past output this month'] },
  { id: 'eloquent', tier: 'silver', glyph: 'quill', value: (f) => f.outputMonth, target: 5_000_000,
    title: ['出口成章', 'Eloquent'], hint: ['单月输出 500 万 token', '5M output tokens in one month'] },
  { id: 'marathon', tier: 'gold', glyph: 'rocket', value: (f) => f.topSessionTokens, target: 50_000_000,
    title: ['马拉松会话', 'Marathon session'], hint: ['单个会话用到 5,000 万 token', 'Spend 50M tokens in a single session'] },
  { id: 'speed-120', tier: 'gold', glyph: 'wave', value: (f) => f.fastestTokPerSec, target: 80,
    title: ['疾速', 'Full speed'], hint: ['实测某模型 ≥ 80 tok/s', 'Measured 80 tok/s on any model'] },
  { id: 'sonic', tier: 'legend', glyph: 'comet', value: (f) => f.fastestTokPerSec, target: 200,
    title: ['超音速', 'Sonic'], hint: ['实测某模型 ≥ 200 tok/s', 'Measured 200 tok/s on any model'] },
  { id: 'break-even', tier: 'bronze', glyph: 'balance', value: (f, x) => x.monthlyValue?.list_price_multiple || 0, target: 1,
    ready: (f, x) => (x.monthlyValue?.list_price_benchmark_usd || 0) > 0,
    title: ['回本', 'Break even'], hint: ['本月 API 等价追平套餐基准价', 'Month matches the plan benchmark in API value'] },
  { id: 'x10-value', tier: 'gold', glyph: 'coins', value: (f, x) => x.monthlyValue?.list_price_multiple || 0, target: 10,
    ready: (f, x) => (x.monthlyValue?.list_price_benchmark_usd || 0) > 0,
    title: ['十倍血赚', '10x value'], hint: ['API 等价达到套餐基准价 10 倍', 'Worth 10x the plan benchmark'] },
  { id: 'spend-1k', tier: 'legend', glyph: 'crown', value: (f) => f.cost, target: 1_000,
    title: ['千金一掷', 'Thousand-dollar club'], hint: ['本机估算累计价值达到 $1,000', 'Cross $1,000 in estimated value'] },
  { id: 'keyboard-flow', tier: 'bronze', glyph: 'keys', value: (f, x) => x.counters?.palette || 0, target: 50,
    title: ['键盘流', 'Keyboard flow'], hint: ['Ctrl+K 唤起快速跳转 50 次', 'Open quick jump with Ctrl+K 50 times'] },
  { id: 'card-sharer', tier: 'bronze', glyph: 'photo', value: (f, x) => x.counters?.share || 0, target: 5,
    title: ['晒卡狂魔', 'Card sharer'], hint: ['导出 5 张分享卡', 'Export 5 share cards'] },
  { id: 'polyglot', tier: 'bronze', glyph: 'globe', value: (f, x) => x.counters?.lang || 0, target: 10,
    title: ['国际化', 'Polyglot'], hint: ['中英切换 10 次', 'Flip the language 10 times'] },
  { id: 'desk-anchor', tier: 'bronze', glyph: 'pin', value: (f, x) => x.counters?.overlay || 0, target: 1,
    title: ['镇桌之宝', 'Desk anchor'], hint: ['呼出过三环悬浮窗', 'Summon the ring overlay'] },
  { id: 'collector', tier: 'silver', glyph: 'medal', meta: 'count', target: 10,
    title: ['收藏家', 'Collector'], hint: ['解锁 10 项成就', 'Unlock 10 achievements'] },
  { id: 'completionist', tier: 'gold', glyph: 'book', meta: 'count', target: 20,
    title: ['图鉴大师', 'Completionist'], hint: ['解锁 20 项成就', 'Unlock 20 achievements'] },
  { id: 'platinum', tier: 'platinum', glyph: 'trophy', meta: 'all', target: 0,
    title: ['白金殿堂', 'Platinum'], hint: ['解锁全部成就', 'Unlock every achievement'] },
];

// meta 成就不给非 meta 计数灌水,单独存 id 集合。
const META_IDS = new Set(ACHIEVEMENTS.filter((def) => def.meta).map((def) => def.id));
const NON_META_TOTAL = ACHIEVEMENTS.filter((def) => !def.meta).length;

export function unlockedNonMetaCount(state) {
  return Object.keys(state?.unlocked || {}).filter((id) => !META_IDS.has(id)).length;
}

export function evaluateAchievements(ledger, extras = {}) {
  const facts = ledgerFacts(ledger);
  return ACHIEVEMENTS.map((def) => {
    const value = Math.max(0, Number(def.meta ? (extras.unlockedCount || 0) : def.value(facts, extras)) || 0);
    const target = def.meta === 'all' ? NON_META_TOTAL : def.target;
    const ratio = target > 0 ? value / target : 0;
    const ready = def.ready ? !!def.ready(facts, extras) : true;
    return {
      id: def.id,
      tier: def.tier,
      glyph: def.glyph,
      title: def.title,
      hint: def.hint,
      value,
      target,
      pct: Math.max(0, Math.min(1, ratio)),
      unlocked: ready && value >= target,
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
