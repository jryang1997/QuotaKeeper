// Ctrl+K 快速跳转面板:一组命令(页面/设置分组/模块/服务商卡/动作),
// ↑↓ 选择、Enter 执行、Esc 关闭。键盘高频入口,动效只做短淡入与轻微缩放。
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { STR } from './ui-data.js';
import { filterCommands, moveIndex } from './command-palette.js';
import { CONTROL_SPRING, FADE } from './motion-tuning.js';
import { useMotionPreference } from './ui-motion.jsx';

const GROUP_ORDER = ['page', 'settings', 'module', 'provider', 'action'];
const GROUP_LABEL = {
  page: 'jump_group_page',
  settings: 'jump_group_settings',
  module: 'jump_group_module',
  provider: 'jump_group_provider',
  action: 'jump_group_action',
};
const GROUP_TAG = { page: 'PAGE', settings: 'SET', module: 'MOD', provider: 'PLAN', action: 'DO' };

export default function CommandPalette({ open, onClose, commands, lang }) {
  const t = (key) => STR[key][lang];
  const reduced = useMotionPreference();
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const inputRef = useRef(null);
  const listRef = useRef(null);

  const results = useMemo(() => filterCommands(commands, query), [commands, query]);
  const grouped = useMemo(() => {
    const buckets = new Map();
    for (const command of results) {
      if (!buckets.has(command.group)) buckets.set(command.group, []);
      buckets.get(command.group).push(command);
    }
    return GROUP_ORDER.filter((group) => buckets.has(group)).map((group) => [group, buckets.get(group)]);
  }, [results]);
  const flat = useMemo(() => grouped.flatMap(([, list]) => list), [grouped]);

  useEffect(() => {
    if (!open) return;
    setQuery('');
    setActive(0);
    const timer = window.setTimeout(() => inputRef.current?.focus(), 30);
    return () => window.clearTimeout(timer);
  }, [open]);

  useEffect(() => {
    if (active >= flat.length) setActive(0);
  }, [flat.length, active]);

  useEffect(() => {
    if (!open) return;
    const node = listRef.current?.querySelector('.jump-item.is-active');
    node?.scrollIntoView({ block: 'nearest' });
  }, [active, open]);

  const run = (command) => {
    if (!command) return;
    onClose();
    // 让面板先卸载再执行跳转,避免滚动被退出动画打断
    window.setTimeout(() => command.run(), 10);
  };

  const onKeyDown = (event) => {
    if (event.key === 'ArrowDown' || (event.key === 'n' && event.ctrlKey)) { event.preventDefault(); setActive((current) => moveIndex(current, 1, flat.length)); }
    else if (event.key === 'ArrowUp' || (event.key === 'p' && event.ctrlKey)) { event.preventDefault(); setActive((current) => moveIndex(current, -1, flat.length)); }
    else if (event.key === 'Enter') { event.preventDefault(); run(flat[active]); }
    else if (event.key === 'Escape') { event.preventDefault(); onClose(); }
  };

  let cursor = -1;
  return (
    <AnimatePresence>
      {open && <motion.div className="jump-backdrop" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={FADE} onClick={onClose}>
        <motion.section
          className="jump-panel"
          role="dialog"
          aria-modal="true"
          aria-label={t('jump_title')}
          initial={{ opacity: 0, transform: reduced ? 'none' : 'translateY(-8px) scale(0.985)' }}
          animate={{ opacity: 1, transform: 'translateY(0px) scale(1)' }}
          exit={{ opacity: 0, transform: reduced ? 'none' : 'translateY(-6px) scale(0.99)' }}
          transition={CONTROL_SPRING}
          onClick={(event) => event.stopPropagation()}
        >
          <div className="jump-head">
            <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.6-3.6" /></svg>
            <input
              ref={inputRef}
              value={query}
              onChange={(event) => { setQuery(event.target.value); setActive(0); }}
              onKeyDown={onKeyDown}
              placeholder={t('jump_placeholder')}
              aria-label={t('jump_title')}
              aria-controls="jump-list"
              aria-activedescendant={flat[active] ? `jump-${flat[active].id}` : undefined}
              autoComplete="off"
              spellCheck="false"
            />
            <kbd>ESC</kbd>
          </div>
          <div className="jump-list" id="jump-list" role="listbox" ref={listRef} aria-label={t('jump_title')}>
            {grouped.map(([group, list]) => (
              <div className="jump-group" key={group}>
                <span className="jump-group-label">{t(GROUP_LABEL[group])}</span>
                {list.map((command) => {
                  cursor += 1;
                  const index = cursor;
                  return (
                    <button
                      type="button"
                      key={command.id}
                      id={`jump-${command.id}`}
                      role="option"
                      aria-selected={index === active}
                      className={`jump-item${index === active ? ' is-active' : ''}`}
                      onMouseMove={() => setActive(index)}
                      onClick={() => run(command)}
                    >
                      <span className="jump-tag">{GROUP_TAG[group]}</span>
                      <span className="jump-label">{command.label}</span>
                      {command.hint && <span className="jump-hint">{command.hint}</span>}
                    </button>
                  );
                })}
              </div>
            ))}
            {flat.length === 0 && <p className="jump-empty">{t('jump_empty')}</p>}
          </div>
          <div className="jump-foot"><span>↑↓ {t('jump_move')}</span><span>↵ {t('jump_run')}</span><span>Ctrl+K</span></div>
        </motion.section>
      </motion.div>}
    </AnimatePresence>
  );
}
