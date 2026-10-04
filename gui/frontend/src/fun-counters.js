// 交互彩蛋计数:键盘跳转/晒卡/切语言/悬浮窗,纯前端行为,localStorage 持久。
// bump 后广播事件,壳层 hook 监听刷新,成就评估随之重跑。

export const FUN_COUNTERS_KEY = 'token-monitor.fun-counters.v1';
export const FUN_COUNTERS_EVENT = 'token-monitor.fun-counters';

export function readFunCounters(storage = globalThis.localStorage) {
  try {
    const raw = JSON.parse(storage.getItem(FUN_COUNTERS_KEY) || '{}');
    const counters = {};
    for (const [key, value] of Object.entries(raw || {})) {
      if (Number.isFinite(value)) counters[key] = value;
    }
    return counters;
  } catch {
    return {};
  }
}

export function bumpFunCounter(name, step = 1, storage = globalThis.localStorage) {
  const counters = readFunCounters(storage);
  counters[name] = (counters[name] || 0) + step;
  try { storage.setItem(FUN_COUNTERS_KEY, JSON.stringify(counters)); } catch { /* session only */ }
  if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent(FUN_COUNTERS_EVENT));
  return counters;
}
