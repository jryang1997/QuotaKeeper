// 成就墙:面板(用量页模块) + 全屏弹窗 + 解锁 toast。
// 数据来自 achievements.js 的纯函数评估;本文件只做渲染与动效。
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { STR, fmtInt } from './ui-data.js';
import { applyUnlocks, evaluateAchievements, readAchievementState, saveAchievementState, unlockedNonMetaCount } from './achievements.js';
import { FUN_COUNTERS_EVENT, readFunCounters } from './fun-counters.js';
import { CONTROL_SPRING, FADE, LAYOUT_SPRING } from './motion-tuning.js';
import { useMotionPreference } from './ui-motion.jsx';

// 统一 24×24 描边图形;填充件用 currentColor,线条件走 stroke。
const GLYPHS = {
  spark: <path d="M12 2.6 13.9 10.1 21.4 12 13.9 13.9 12 21.4 10.1 13.9 2.6 12 10.1 10.1Z" fill="currentColor" stroke="none" />,
  bolt: <path d="M13.4 2 5 13.4h5.4L9 22l8.6-11.6h-5.4z" fill="currentColor" stroke="none" />,
  layers: <><path d="M12 2.8 21 8l-9 5.2L3 8Z" fill="currentColor" stroke="none" opacity="0.9" /><path d="m3.6 12.4 8.4 4.9 8.4-4.9" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" /><path d="m3.6 16.3 8.4 4.9 8.4-4.9" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" opacity="0.55" /></>,
  flame: <path d="M12 2.5c2.8 3.7 5.2 6.3 5.2 10.2a5.2 5.2 0 0 1-10.4 0c0-2.1 1-3.7 2.6-5.3.4 1.6 1.4 2.2 2.5 2.7-.6-2.4-1.4-5.1.1-7.6Z" fill="currentColor" stroke="none" />,
  sun: <><circle cx="12" cy="12" r="4.2" fill="currentColor" stroke="none" /><path d="M12 2.4v2.2M12 19.4v2.2M2.4 12h2.2M19.4 12h2.2M5.2 5.2l1.6 1.6M17.2 17.2l1.6 1.6M18.8 5.2l-1.6 1.6M6.8 17.2l-1.6 1.6" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></>,
  sunrise: <><path d="M4 18h16" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /><path d="M7.4 14.2a4.6 4.6 0 0 1 9.2 0Z" fill="currentColor" stroke="none" /><path d="M12 3.6v2.4M4.4 7l1.7 1.7M19.6 7l-1.7 1.7" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></>,
  moon: <path d="M20.4 14.6A8.8 8.8 0 1 1 9.4 3.6a7.2 7.2 0 0 0 11 11Z" fill="currentColor" stroke="none" />,
  star: <path d="M12 2.8 14.9 9l6.6.6-5 4.4 1.5 6.5L12 17l-6 3.5L7.5 14l-5-4.4L9.1 9Z" fill="currentColor" stroke="none" />,
  diamond: <><path d="M12 2.4 20.6 12 12 21.6 3.4 12Z" fill="currentColor" stroke="none" opacity="0.9" /><path d="M7.2 12h9.6M12 6.4 12 17.6" stroke="var(--ach-glyph-cut, rgba(0,0,0,0.35))" strokeWidth="1.2" /></>,
  stack: <><rect x="3.4" y="13.4" width="17.2" height="6.4" rx="1.8" fill="currentColor" stroke="none" opacity="0.55" /><rect x="3.4" y="8" width="17.2" height="6.4" rx="1.8" fill="currentColor" stroke="none" opacity="0.78" /><rect x="3.4" y="2.6" width="17.2" height="6.4" rx="1.8" fill="currentColor" stroke="none" /></>,
  compass: <><circle cx="12" cy="12" r="8.6" stroke="currentColor" strokeWidth="1.6" fill="none" /><path d="m15.4 8.6-2 4.8-4.8 2 2-4.8Z" fill="currentColor" stroke="none" /></>,
  gauge: <><path d="M4.2 17.6a9 9 0 1 1 15.6 0" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" /><path d="M12 14.4 16.2 9" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" /><circle cx="12" cy="14.8" r="1.8" fill="currentColor" stroke="none" /></>,
  rocket: <><path d="M12 2.6c3.1 2.6 4.7 6.1 4.7 9.7L12 16.9l-4.7-4.6c0-3.6 1.6-7.1 4.7-9.7Z" fill="currentColor" stroke="none" /><path d="M9.6 16.4 7.4 21l3.1-1.5L12 21l1.5-1.5L16.6 21l-2.2-4.6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" fill="none" /></>,
  wave: <path d="M3 7.6l3.6 4.4L3 16.4M9.4 7.6l3.6 4.4-3.6 4.4M15.8 7.6l3.6 4.4-3.6 4.4" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round" />,
  crown: <path d="M3.4 18.6h17.2l1.6-9.8-5.4 3.6L12 5.6 7.2 12.4 1.8 8.8Z" fill="currentColor" stroke="none" />,
  trident: <><path d="M12 2.8v18.4" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" /><path d="M6.4 4.6v3.6a5.6 5.6 0 0 0 4.5 5.5" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" /><path d="M17.6 4.6v3.6a5.6 5.6 0 0 1-4.5 5.5" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" /></>,
  heart: <path d="M12 20.6C6.8 16.8 3.6 13.6 3.6 9.9c0-2.8 2.1-4.9 4.8-4.9 1.5 0 2.8.7 3.6 1.9.8-1.2 2.1-1.9 3.6-1.9 2.7 0 4.8 2.1 4.8 4.9 0 3.7-3.2 6.9-8.4 10.7Z" fill="currentColor" stroke="none" />,
  chat: <><path d="M4.6 4.4h10.2a2.4 2.4 0 0 1 2.4 2.4v4.6a2.4 2.4 0 0 1-2.4 2.4H9.6l-3.6 3v-3H4.6a2.4 2.4 0 0 1-2.4-2.4V6.8a2.4 2.4 0 0 1 2.4-2.4Z" fill="currentColor" stroke="none" /><path d="M17.8 16.6h.2a2.2 2.2 0 0 0 2.2-2.2V9.6" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" opacity="0.55" /></>,
  hex: <><path d="M12 2.4 20.2 7.2v9.6L12 21.6 3.8 16.8V7.2Z" fill="currentColor" stroke="none" opacity="0.9" /><circle cx="12" cy="12" r="3" fill="var(--ach-glyph-cut, rgba(0,0,0,0.35))" stroke="none" /></>,
  clock: <><circle cx="12" cy="12" r="8.4" stroke="currentColor" strokeWidth="1.8" fill="none" /><path d="M12 7.2V12l3.2 2" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round" /></>,
  noodle: <><path d="M3.8 12.4h16.4a8.2 8.2 0 0 1-5.4 7.7v1.3a1 1 0 0 1-1 1h-3.6a1 1 0 0 1-1-1v-1.3a8.2 8.2 0 0 1-5.4-7.7Z" fill="currentColor" stroke="none" /><path d="M8.6 3.4v5M12 3.4v5M15.4 3.4v5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></>,
  sundown: <><path d="M6.8 16.2a5.2 5.2 0 0 1 10.4 0Z" fill="currentColor" stroke="none" /><path d="M3 19.6h18" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /><path d="M12 2.8v6.2M9.3 6.3 12 9l2.7-2.7" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" strokeLinejoin="round" /></>,
  thought: <><path d="M7.2 15.9a4 4 0 0 1-.6-7.96 5.4 5.4 0 0 1 10.5-1.1 3.3 3.3 0 0 1-.5 9.06Z" fill="currentColor" stroke="none" /><circle cx="8.6" cy="19" r="1.4" fill="currentColor" stroke="none" /><circle cx="5.2" cy="21.4" r="1" fill="currentColor" stroke="none" opacity="0.7" /></>,
  quill: <><path d="M12 2.6c3.9 2.4 6.5 5.4 6.5 9.1 0 4.6-3.1 7.5-6.5 9.7-3.4-2.2-6.5-5.1-6.5-9.7 0-3.7 2.6-6.7 6.5-9.1Z" fill="currentColor" stroke="none" /><path d="M12 8.6v3.9" stroke="var(--ach-glyph-cut, rgba(0,0,0,0.35))" strokeWidth="1.4" strokeLinecap="round" /><circle cx="12" cy="14.8" r="1.8" fill="var(--ach-glyph-cut, rgba(0,0,0,0.35))" stroke="none" /></>,
  comet: <><circle cx="16.8" cy="7.2" r="3.6" fill="currentColor" stroke="none" /><path d="M11.9 10.6 4.2 18.3M13.6 13.4l-6 6M9 8.8l-4.8 4.8" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" /></>,
  balance: <><path d="M12 4.6v15.8M8.4 20.6h7.2M4 6.8h16" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" /><circle cx="12" cy="4" r="1.4" fill="currentColor" stroke="none" /><path d="M4 6.8 1.9 11.9M4 6.8l2.1 5.1M20 6.8l-2.1 5.1M20 6.8l2.1 5.1" stroke="currentColor" strokeWidth="1.4" fill="none" strokeLinecap="round" /><path d="M1.7 11.9a2.3 2.3 0 0 0 4.6 0ZM17.7 11.9a2.3 2.3 0 0 0 4.6 0Z" fill="currentColor" stroke="none" /></>,
  coins: <><ellipse cx="12" cy="5.6" rx="6.4" ry="2.5" stroke="currentColor" strokeWidth="1.6" fill="none" /><path d="M5.6 5.6v6.2c0 1.4 2.9 2.5 6.4 2.5s6.4-1.1 6.4-2.5V5.6" stroke="currentColor" strokeWidth="1.6" fill="none" /><path d="M5.6 11.8V18c0 1.4 2.9 2.5 6.4 2.5s6.4-1.1 6.4-2.5v-6.2" stroke="currentColor" strokeWidth="1.6" fill="none" /></>,
  keys: <><rect x="2.6" y="6" width="18.8" height="12" rx="2.4" stroke="currentColor" strokeWidth="1.6" fill="none" /><circle cx="7" cy="10.2" r="1" fill="currentColor" stroke="none" /><circle cx="10.3" cy="10.2" r="1" fill="currentColor" stroke="none" /><circle cx="13.7" cy="10.2" r="1" fill="currentColor" stroke="none" /><circle cx="17" cy="10.2" r="1" fill="currentColor" stroke="none" /><path d="M8.6 14.6h6.8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></>,
  photo: <><rect x="3" y="4.6" width="18" height="14.8" rx="2.6" stroke="currentColor" strokeWidth="1.6" fill="none" /><path d="m6.6 16.4 4-4.8 3 3.4 2.1-2.3 2.9 3.7" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" strokeLinejoin="round" /><circle cx="15.6" cy="9.2" r="1.5" fill="currentColor" stroke="none" /></>,
  globe: <><circle cx="12" cy="12" r="8.6" stroke="currentColor" strokeWidth="1.6" fill="none" /><ellipse cx="12" cy="12" rx="3.9" ry="8.6" stroke="currentColor" strokeWidth="1.4" fill="none" opacity="0.75" /><path d="M3.4 12h17.2" stroke="currentColor" strokeWidth="1.6" /></>,
  pin: <><path d="M12 17.2v3.6" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" /><path d="M9 10.8a2 2 0 0 1-1.1 1.8l-1.8.9A2 2 0 0 0 5 15.3v.7a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.7a2 2 0 0 0-1.1-1.8l-1.8-.9a2 2 0 0 1-1.1-1.8V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1Z" fill="currentColor" stroke="none" /></>,
  medal: <><circle cx="12" cy="14.8" r="5.2" stroke="currentColor" strokeWidth="1.8" fill="none" /><circle cx="12" cy="14.8" r="1.6" fill="currentColor" stroke="none" /><path d="M8.4 3 11 10M15.6 3 13 10" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" /></>,
  book: <><path d="M12 6.2C10.1 4.7 7.7 4 4.3 4c-.6 0-1.1.5-1.1 1.1v12c0 .6.5 1.1 1.1 1.1 3.4 0 5.8.7 7.7 2.2 1.9-1.5 4.3-2.2 7.7-2.2.6 0 1.1-.5 1.1-1.1v-12C20.8 4.5 20.3 4 19.7 4c-3.4 0-5.8.7-7.7 2.2Z" fill="currentColor" stroke="none" /><path d="M12 6.2v13.9" stroke="var(--ach-glyph-cut, rgba(0,0,0,0.35))" strokeWidth="1.4" /></>,
  trophy: <><path d="M6.8 3.6h10.4v5.6a5.2 5.2 0 0 1-10.4 0Z" fill="currentColor" stroke="none" /><path d="M6.8 5H4a3.2 3.2 0 0 0 3.2 3.2M17.2 5H20a3.2 3.2 0 0 1-3.2 3.2" stroke="currentColor" strokeWidth="1.6" fill="none" strokeLinecap="round" /><path d="M12 14.4v2.8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /><path d="M9.6 20.4h4.8l-.6-3.2h-3.6Z" fill="currentColor" stroke="none" /></>,
};

export function AchievementGlyph({ glyph, className = '' }) {
  return <svg className={`ach-glyph ${className}`} viewBox="0 0 24 24" aria-hidden="true">{GLYPHS[glyph] || GLYPHS.spark}</svg>;
}

function progressText(item, lang) {
  return item.value >= item.target ? '' : `${fmtInt(Math.round(item.value))} / ${fmtInt(item.target)}`;
}

export function AchievementsPanel({ items: itemsProp, ledger, lang, onOpen }) {
  const t = (key) => STR[key][lang];
  // 壳层已带 extras 评估;独立挂载(测试)时退回纯账本评估。
  const items = useMemo(() => itemsProp || evaluateAchievements(ledger), [itemsProp, ledger]);
  const unlocked = items.filter((item) => item.unlocked);
  const next = items.filter((item) => !item.unlocked).sort((a, b) => b.pct - a.pct).slice(0, Math.max(0, 6 - unlocked.length));
  const wall = [...unlocked, ...next].slice(0, 12);
  return (
    <div className="ach-panel">
      <div className="ach-panel-head">
        <span className="ach-count"><b>{unlocked.length}</b> / {items.length} {t('ach_unlocked')}</span>
        <button type="button" className="ach-open" onClick={onOpen}>{t('ach_view_all')} →</button>
      </div>
      <div className="ach-wall">
        {wall.map((item) => (
          <div className={`ach-tile is-${item.tier}${item.unlocked ? ' is-unlocked' : ' is-locked'}`} key={item.id}>
            <span className="ach-tile-glyph"><AchievementGlyph glyph={item.glyph} /></span>
            <span className="ach-tile-copy">
              <b>{item.title[lang]}</b>
              <small>{item.unlocked ? t('ach_done') : progressText(item, lang) || item.hint[lang]}</small>
            </span>
            {!item.unlocked && <i className="ach-tile-track"><motion.i initial={false} animate={{ transform: `scaleX(${item.pct})` }} transition={LAYOUT_SPRING} /></i>}
          </div>
        ))}
      </div>
    </div>
  );
}

export function AchievementsModal({ items, lang, state, onClose }) {
  const t = (key) => STR[key][lang];
  const unlocked = items.filter((item) => item.unlocked).length;
  const dateText = (id) => {
    const at = state?.unlocked?.[id];
    if (!at) return '';
    const date = new Date(at);
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
  };
  useEffect(() => {
    const onKey = (event) => { if (event.key === 'Escape') onClose(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);
  return (
    <motion.div className="ach-backdrop" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={FADE} onClick={onClose}>
      <motion.section
        className="ach-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t('ach_title')}
        initial={{ opacity: 0, transform: 'translateY(10px) scale(0.985)' }}
        animate={{ opacity: 1, transform: 'translateY(0px) scale(1)' }}
        exit={{ opacity: 0, transform: 'translateY(10px) scale(0.985)' }}
        transition={CONTROL_SPRING}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="ach-modal-head">
          <div>
            <span className="eyebrow">// {t('ach_title')}</span>
            <h2>{unlocked} <span>/ {items.length}</span></h2>
            <p>{t('ach_modal_hint')}</p>
          </div>
          <button type="button" className="ach-close" onClick={onClose} aria-label={t('ach_close')}>×</button>
        </header>
        <div className="ach-grid">
          {items.map((item) => (
            <div className={`ach-card is-${item.tier}${item.unlocked ? ' is-unlocked' : ' is-locked'}`} key={item.id}>
              <span className="ach-card-glyph"><AchievementGlyph glyph={item.glyph} /></span>
              <div className="ach-card-copy">
                <b>{item.title[lang]}</b>
                <p>{item.hint[lang]}</p>
                {item.unlocked ? <small className="ach-card-date">{t('ach_done')}{dateText(item.id) ? ` · ${dateText(item.id)}` : ''}</small> : (
                  <span className="ach-card-progress"><i><motion.i initial={false} animate={{ transform: `scaleX(${item.pct})` }} transition={LAYOUT_SPRING} /></i><em>{Math.round(item.pct * 100)}%</em></span>
                )}
              </div>
            </div>
          ))}
        </div>
      </motion.section>
    </motion.div>
  );
}

export function AchievementToasts({ items, lang, onDismiss }) {
  const reduced = useMotionPreference();
  return <div className="ach-toasts" role="status" aria-live="polite">
    <AnimatePresence>
      {items.map((toast) => (
        <motion.div
          key={toast.key}
          className={`ach-toast is-${toast.tier}`}
          initial={{ opacity: 0, transform: reduced ? 'none' : 'translateY(14px) scale(0.96)' }}
          animate={{ opacity: 1, transform: 'translateY(0px) scale(1)' }}
          exit={{ opacity: 0, transform: reduced ? 'none' : 'translateY(10px) scale(0.98)' }}
          transition={reduced ? FADE : { ...CONTROL_SPRING, bounce: 0.22 }}
          onClick={() => onDismiss(toast.key)}
        >
          <motion.span
            className="ach-toast-glyph"
            initial={{ transform: reduced ? 'none' : 'scale(0.55)', opacity: 0 }}
            animate={{ transform: 'scale(1)', opacity: 1 }}
            transition={reduced ? FADE : { type: 'spring', bounce: 0.34, duration: 0.6 }}
          ><AchievementGlyph glyph={toast.glyph} /></motion.span>
          <span className="ach-toast-copy">
            <em>{toast.multi ? STR.ach_multi[lang] : STR.ach_new[lang]}</em>
            <b>{toast.label}</b>
          </span>
        </motion.div>
      ))}
    </AnimatePresence>
  </div>;
}

// 壳层 hook:评估账本(extras 携带钱/计数/解锁数)→ 与存档比对 → 新解锁进队列(>3 个合并成一条,避免首跑刷屏)。
export function useAchievements(ledger, lang = 0, extrasInput = {}) {
  const [state, setState] = useState(readAchievementState);
  const [counters, setCounters] = useState(readFunCounters);
  const [toasts, setToasts] = useState([]);
  const counter = useRef(0);
  useEffect(() => {
    const onBump = () => setCounters(readFunCounters());
    window.addEventListener(FUN_COUNTERS_EVENT, onBump);
    return () => window.removeEventListener(FUN_COUNTERS_EVENT, onBump);
  }, []);
  const extras = useMemo(
    () => ({ monthlyValue: extrasInput.monthlyValue, counters, unlockedCount: unlockedNonMetaCount(state) }),
    [extrasInput.monthlyValue, counters, state],
  );
  const items = useMemo(() => evaluateAchievements(ledger, extras), [ledger, extras]);
  useEffect(() => {
    if (!ledger) return;
    setState((current) => {
      const result = applyUnlocks(items, current);
      if (result.fresh.length === 0) return current;
      saveAchievementState(result.state);
      if (result.fresh.length > 3) {
        counter.current += 1;
        const key = `multi-${counter.current}`;
        setToasts((list) => [...list.slice(-2), { key, multi: true, label: STR.ach_multi_label[lang], tier: 'gold', glyph: 'crown' }]);
      } else {
        for (const id of result.fresh) {
          const item = items.find((entry) => entry.id === id);
          if (!item) continue;
          counter.current += 1;
          setToasts((list) => [...list.slice(-2), { key: `${id}-${counter.current}`, label: item.title[lang], tier: item.tier, glyph: item.glyph }]);
        }
      }
      return result.state;
    });
  }, [items, ledger, lang]);
  useEffect(() => {
    if (toasts.length === 0) return;
    const timer = window.setTimeout(() => setToasts((list) => list.slice(1)), 6500);
    return () => window.clearTimeout(timer);
  }, [toasts]);
  const dismiss = (key) => setToasts((list) => list.filter((toast) => toast.key !== key));
  return { state, items, toasts, dismiss };
}
