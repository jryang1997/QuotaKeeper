const DAY_MS = 86_400_000;

function isFiniteNumber(value) {
  return typeof value === 'number' && Number.isFinite(value);
}

function clampPct(value) {
  if (!isFiniteNumber(value)) return null;
  return Math.min(100, Math.max(0, value));
}

export function classifyQuotaCard(card) {
  if (!card || card.kind !== 'Live') return 'unavailable';
  if (card.method === 'activity_vs_average') return 'activity';
  const limits = Array.isArray(card.limits) ? card.limits : [];
  return limits.some((limit) => isFiniteNumber(limit?.used_pct)) ? 'quota' : 'empty';
}

export function projectReset(resetDelayAtFetchMs, { observedAtMs, now, stale }) {
  if (!isFiniteNumber(resetDelayAtFetchMs)) return { state: 'none', remainingMs: null };
  if (stale || !isFiniteNumber(observedAtMs) || observedAtMs <= 0 || !isFiniteNumber(now)) {
    return { state: 'stale', remainingMs: null };
  }
  if (resetDelayAtFetchMs <= 0) return { state: 'awaiting', remainingMs: null };
  const remainingMs = resetDelayAtFetchMs - Math.max(0, now - observedAtMs);
  if (remainingMs <= 0) return { state: 'awaiting', remainingMs: null };
  return { state: 'countdown', remainingMs };
}

export function projectWindow(limit, { observedAtMs, now, stale, remainingView }) {
  const usedPct = clampPct(limit?.used_pct);
  const viewRemaining = remainingView !== false;
  const percent = usedPct == null ? null : (viewRemaining ? clampPct(100 - usedPct) : usedPct);
  const tone = usedPct == null ? null : usedPct >= 100 ? 'exhausted' : usedPct >= 80 ? 'warning' : 'ok';
  return {
    label: limit?.label ?? '',
    usedPct,
    percent,
    tone,
    reset: projectReset(limit?.resets_in_ms, { observedAtMs, now, stale }),
  };
}

function projectActivityWindow(limit) {
  const usedPct = clampPct(limit?.used_pct);
  return {
    label: limit?.label ?? '',
    usedPct,
    percent: usedPct,
    tone: 'activity',
    reset: { state: 'none', remainingMs: null },
  };
}

export function projectExpiry(endsAtMs, now) {
  if (!isFiniteNumber(endsAtMs) || !isFiniteNumber(now)) return null;
  if (endsAtMs <= now) return { state: 'expired', warning: false, days: 0, md: null };
  const days = Math.ceil((endsAtMs - now) / DAY_MS);
  const date = new Date(endsAtMs);
  const md = `${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
  return { state: 'active', warning: days <= 7, days, md };
}

export function formatObservedAt(ms) {
  if (!isFiniteNumber(ms) || ms <= 0) return null;
  const date = new Date(ms);
  const pad = (value) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function projectCard(card, context) {
  const kind = classifyQuotaCard(card);
  const limits = Array.isArray(card?.limits) ? card.limits : [];
  const windows = kind === 'quota'
    ? limits.filter((limit) => isFiniteNumber(limit?.used_pct)).map((limit) => projectWindow(limit, context))
    : kind === 'activity'
      ? limits.filter((limit) => isFiniteNumber(limit?.used_pct)).map((limit) => projectActivityWindow(limit))
      : [];
  const peakUsed = kind === 'quota' && windows.length
    ? windows.reduce((best, window) => Math.max(best, window.usedPct), 0)
    : null;
  return {
    provider: card?.provider ?? null,
    kind,
    countsAsQuota: kind === 'quota',
    windows,
    peakUsed,
  };
}

export function arrangeQuota({
  cards = [],
  hiddenCards = [],
  pinnedCards = [],
  observedAtMs = null,
  now = null,
  stale = false,
  remainingView = true,
} = {}) {
  const hidden = new Set(hiddenCards);
  const pinned = new Set(pinnedCards);
  const context = { observedAtMs, now, stale: Boolean(stale), remainingView };
  const quota = [];
  const empty = [];
  const activity = [];
  const folded = [];

  for (const card of cards) {
    const kind = classifyQuotaCard(card);
    const isHidden = hidden.has(card?.provider);
    if (kind === 'unavailable' || isHidden) {
      folded.push({
        provider: card?.provider,
        title: card?.title || card?.provider || '',
        kind,
        hidden: isHidden,
        hints: card?.hints || null,
      });
      continue;
    }
    const projected = { ...projectCard(card, context), card, pinned: pinned.has(card.provider) };
    if (kind === 'quota') quota.push(projected);
    else if (kind === 'activity') activity.push(projected);
    else empty.push(projected);
  }

  quota.sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.peakUsed - a.peakUsed);

  return {
    quota,
    empty,
    activity,
    folded,
    missingCount: folded.filter((item) => item.kind === 'unavailable').length,
    hiddenCount: folded.filter((item) => item.hidden && item.kind !== 'unavailable').length,
  };
}
