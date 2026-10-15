import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import '@astryxdesign/core/reset.css';
import '@astryxdesign/core/astryx.css';
import '@astryxdesign/theme-neutral/theme.css';
import './app.css';
import './usage-dashboard.css';
import './ia.css';
import './glass.css';
import './themes.css';
import './focus.css';
import './cassette.css';
import { SOURCE_PATHS, SOURCE_REPAIR, QUOTA_REPAIR, STR, fmtRel } from './ui-data.js';
import QuotaPage from './QuotaPage.jsx';
import UsageDashboard from './UsageDashboard.jsx';
import LocalUsageSummary from './LocalUsageSummary.jsx';
import SettingsPage from './SettingsPage.jsx';
import { DEFAULT_GUI_PREFERENCES, THEME_ORDER, readGuiPreferences, saveGuiPreferences } from './gui-preferences.js';
import { fetchExchangeRate, readExchangeRates } from './currency.js';
import { registerNativeCallbacks, sendCommand } from './gui-bridge.js';
import { AnimatePresence, LayoutGroup, motion, useIsPresent } from 'motion/react';
import { MotionProvider, PopoverSurface, useMotionPreference, SelectionGroup, SelectionHighlight } from './ui-motion.jsx';
import { CONTROL_SPRING, FADE } from './motion-tuning.js';
import { Card } from '@astryxdesign/core/Card';
import { Text } from '@astryxdesign/core/Text';
import { Button } from '@astryxdesign/core/Button';
import { Spinner } from '@astryxdesign/core/Spinner';
import { Toast } from '@astryxdesign/core/Toast';
import CommandPalette from './CommandPalette.jsx';
import { makeCommand } from './command-palette.js';
import { LiquidGlassFilter } from './GlassLens.jsx';

const NAV_ITEMS = ['quota', 'usage', 'settings'];
const GROUP_LABELS = [['appearance', 'settings_group_appearance'], ['sources', 'settings_group_sources'], ['desktop', 'settings_group_desktop'], ['about', 'settings_group_about']];

function NavIcon({ page }) {
  if (page === 'quota') {
    // gauge:下半弧 + 指针,指针指向右上(趋势向上)
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="m12 14 4-4" /><path d="M3.34 19a10 10 0 1 1 17.32 0" /></svg>;
  }
  if (page === 'usage') {
    // 带轴圆帽柱:轴一次画到位,三根柱节奏降序,读作「用量」
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 3v16a2 2 0 0 0 2 2h16" /><path d="M8 17v-4" /><path d="M13 17V8" /><path d="M18 17v-7" /></svg>;
  }
  return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" /><circle cx="12" cy="12" r="3" /></svg>;
}

function PrimaryNav({ lang, page, onNavigate, variant }) {
  const reduced = useMotionPreference();
  return (
    <LayoutGroup id={`navigation-${variant}`}>
      <nav className={`primary-nav primary-nav-${variant}`} aria-label={lang === 0 ? '主导航' : 'Main navigation'}>
        {NAV_ITEMS.map((key) => {
          const active = page === key;
          return (
            <button
              key={key}
              className={`nav-item${active ? ' nav-item-active' : ''}`}
              onClick={() => onNavigate(key)}
              aria-current={active ? 'page' : undefined}
            >
              {active && <motion.span layoutId={reduced ? undefined : 'active-navigation'} className="nav-active-indicator" aria-hidden="true" transition={reduced ? { duration: 0 } : CONTROL_SPRING} />}
              <NavIcon page={key} />
              <span className="nav-label">{STR[`nav_${key}`][lang]}</span>
            </button>
          );
        })}
      </nav>
    </LayoutGroup>
  );
}

// 数据状态:Dock 左端一枚极小的点,点开才看明细;异常才在页内提示。
function DataStatus({ lang, data, refreshing, quotaStale, ledgerStale, quotaAge, ledgerAge }) {
  const t = (key) => STR[key][lang];
  const [open, setOpen] = useState(false);
  const wrapRef = useRef(null);
  const stale = quotaStale || ledgerStale;
  useEffect(() => {
    if (!open) return undefined;
    // Apple popover 规矩:点外部即收起
    const onPointerDown = (event) => {
      if (!wrapRef.current?.contains(event.target)) setOpen(false);
    };
    document.addEventListener('pointerdown', onPointerDown);
    return () => document.removeEventListener('pointerdown', onPointerDown);
  }, [open]);
  return (
    <div ref={wrapRef} className="dock-status-wrap">
      <button type="button" className="dock-status-btn" aria-expanded={open} onClick={() => setOpen((value) => !value)} title={t('freshness_open')} aria-label={t('freshness_open')}>
        {refreshing ? <Spinner size="sm" /> : <i className={`status-dot${stale ? ' is-stale' : ''}`} aria-hidden="true" />}
      </button>
      <AnimatePresence>{open && <PopoverSurface className="status-popover glass-panel" role="status">
        <span className="status-popover-title">{t('freshness_title')}</span>
        <div className={`status-row${quotaStale ? ' is-stale' : ''}`}>
          <i /><span>{t('freshness_quota')}</span>
          <strong>{quotaStale ? t('freshness_stale') : quotaAge == null ? t('freshness_waiting') : `${fmtRel(quotaAge)}`}</strong>
        </div>
        <div className={`status-row${ledgerStale ? ' is-stale' : ''}`}>
          <i /><span>{t('freshness_local')}</span>
          <strong>{ledgerStale ? t('freshness_stale') : ledgerAge == null ? t('freshness_waiting') : `${fmtRel(ledgerAge)}`}</strong>
        </div>
        {data?.quotaError && <p className="status-popover-error">{data.quotaError}</p>}
      </PopoverSurface>}</AnimatePresence>
    </div>
  );
}

// 底部液态玻璃 Dock:主因导航,桌面与窄窗同一套;折射滤镜只挂这一枚。
function DockNav({ lang, page, onNavigate, status }) {
  const reduced = useMotionPreference();
  const dockRef = useRef(null);
  const [lens, setLens] = useState('');
  return (
    <>
      <LiquidGlassFilter targetRef={dockRef} onValue={setLens} />
      <LayoutGroup id="navigation-dock">
        <nav
          ref={dockRef}
          className="dock"
          aria-label={lang === 0 ? '主导航' : 'Main navigation'}
          style={lens ? { backdropFilter: `url(#${lens}) blur(22px) saturate(140%)`, WebkitBackdropFilter: `url(#${lens}) blur(22px) saturate(140%)` } : undefined}
        >
          {status && <DataStatus lang={lang} {...status} />}
          <i className="dock-status-divider" aria-hidden="true" />
          {NAV_ITEMS.map((key) => {
            const active = page === key;
            return (
              <button
                key={key}
                className={`dock-item${active ? ' dock-item-active' : ''}`}
                onClick={() => onNavigate(key)}
                aria-current={active ? 'page' : undefined}
              >
                {active && <motion.span layoutId={reduced ? undefined : 'dock-active'} className="dock-active-pill" aria-hidden="true" transition={reduced ? { duration: 0 } : CONTROL_SPRING} />}
                <NavIcon page={key} />
                <span className="dock-label">{STR[`nav_${key}`][lang]}</span>
              </button>
            );
          })}
        </nav>
      </LayoutGroup>
    </>
  );
}

function SourceWelcome({ data, lang, onDone }) {
  const t = (key) => STR[key][lang];
  const firstButton = useRef(null);
  useEffect(() => { firstButton.current?.focus(); }, []);
  const sources = data.ledger?.sources || [];
  const cards = data.cards || [];
  const reduced = useMotionPreference();
  const present = useIsPresent();
  return <motion.div className="welcome-backdrop" inert={present ? undefined : true} style={{ pointerEvents: present ? 'auto' : 'none' }} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={FADE}><motion.section className="welcome-panel" initial={{ scale: reduced ? 1 : 0.98, y: reduced ? 0 : 8 }} animate={{ scale: 1, y: 0 }} exit={{ scale: reduced ? 1 : 0.98, y: reduced ? 0 : 8 }} transition={CONTROL_SPRING} role="dialog" aria-modal="true" aria-labelledby="welcome-title">
    <div className="welcome-head"><div><span className="eyebrow">TOKEN MONITOR</span><h1 id="welcome-title">{t('welcome_title')}</h1><p>{t('welcome_intro')}</p></div><button ref={firstButton} type="button" onClick={onDone} aria-label={t('welcome_done')}>×</button></div>
    <h2>{t('freshness_quota')}</h2>
    <div className="welcome-source-list">{cards.map((card) => <div className="welcome-source" key={card.provider}>
      <div><span className={`source-indicator source-indicator-${card.kind === 'Live' ? 'ready' : 'error'}`} /><strong>{card.title}</strong><span>{card.kind === 'Live' ? t('source_ready') : t('missing')}</span></div>
      <a href={`#repair-quota-${card.provider}`} className="welcome-repair-link" onClick={(event) => { event.preventDefault(); event.currentTarget.nextElementSibling.open = true; }}>{t('welcome_repair')}</a>
      <details id={`repair-quota-${card.provider}`}><summary>{lang === 0 ? '修复步骤' : 'Repair steps'}</summary><p>{QUOTA_REPAIR[card.provider]?.[lang] || card.hints?.[lang]}</p></details>
    </div>)}</div>
    <h2>{t('freshness_local')}</h2>
    <div className="welcome-source-list">{sources.map((source) => <div className="welcome-source" key={source.client}>
      <div><span className={`source-indicator source-indicator-${source.error ? 'error' : source.records > 0 ? 'ready' : 'idle'}`} /><strong>{source.label}</strong><span>{source.error ? t('source_error') : source.records ? `${source.records} ${t('source_ready')}` : t('source_idle')}</span></div>
      <small>{SOURCE_PATHS[source.client]}</small>
      <a href={`#repair-local-${source.client}`} className="welcome-repair-link" onClick={(event) => { event.preventDefault(); event.currentTarget.nextElementSibling.open = true; }}>{t('welcome_repair')}</a>
      <details id={`repair-local-${source.client}`}><summary>{lang === 0 ? '修复步骤' : 'Repair steps'}</summary><p>{SOURCE_REPAIR[source.client]?.[lang]}</p>{source.error && <p>{source.error}</p>}</details>
    </div>)}</div>
    <button className="welcome-done" type="button" onClick={onDone}>{t('welcome_done')}</button>
  </motion.section></motion.div>;
}



function App() {
  const [preferences, setPreferences] = useState(readGuiPreferences);
  const [showWelcome, setShowWelcome] = useState(false);
  const [nativeError, setNativeError] = useState('');
  const [goKeyStored, setGoKeyStored] = useState(false);
  const [goKeyFeedback, setGoKeyFeedback] = useState('');
  const [goatKeyStored, setGoatKeyStored] = useState(false);
  const [goatKeyFeedback, setGoatKeyFeedback] = useState('');
  const [deepseekKeyStored, setDeepSeekKeyStored] = useState(false);
  const [deepseekKeyFeedback, setDeepSeekKeyFeedback] = useState('');
  const [planModels, setPlanModels] = useState({});
  const [lan, setLan] = useState({ enabled: false, port: null, url: null });
  const [jumpOpen, setJumpOpen] = useState(false);
  const lang = preferences.lang;
  const reducedMotion = useMotionPreference(preferences.motion === 'reduced');
  const [page, setPage] = useState('quota');
  const [data, setData] = useState(null);
  const [staleCache, setStaleCache] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [now, setNow] = useState(Date.now);
  const [exchangeRate, setExchangeRate] = useState(() => readExchangeRates()[preferences.currency] || null);
  const [exchangeError, setExchangeError] = useState(false);
  const refreshRequestedAt = useRef(0);
  const mainRef = useRef(null);
  const scrollByPage = useRef({});
  useLayoutEffect(() => {
    if (mainRef.current) mainRef.current.scrollTop = scrollByPage.current[page] || 0;
  }, [page]);

  useEffect(() => {
    let active = true;
    const code = preferences.currency;
    setExchangeRate(code === 'USD' ? { rate: 1, date: '' } : readExchangeRates()[code] || null);
    setExchangeError(false);
    fetchExchangeRate(code).then((rate) => {
      if (active) { setExchangeRate(rate); setExchangeError(false); }
    }).catch(() => { if (active) setExchangeError(true); });
    return () => { active = false; };
  }, [preferences.currency]);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 5_000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const unregister = registerNativeCallbacks({
      __onPush: (payload) => {
        setData(payload);
        setStaleCache(payload.stale === true);
        if (!refreshRequestedAt.current || (payload.fetched_at_ms >= refreshRequestedAt.current && payload.quota_checked_at_ms >= refreshRequestedAt.current)) {
          refreshRequestedAt.current = 0;
          setRefreshing(false);
        }
      },
      __showSettings: () => setPage('settings'),
      __onNativeState: (state) => {
        setGoKeyStored(!!state.goKeyStored);
        setGoatKeyStored(!!state.goatKeyStored);
        setDeepSeekKeyStored(!!state.deepseekKeyStored);
        setPlanModels(state.planModels || {});
        setLan(state.lan || { enabled: false, port: null, url: null });
        setPreferences((current) => {
          const next = { ...current, autostart: !!state.autostart };
          saveGuiPreferences(next);
          return next;
        });
      },
      __onProviderKey: (payload) => {
        // key 只经事件进入会话内存,由 CardDetails 缓存;不落 React 持久态
        window.dispatchEvent(new CustomEvent('provider-key', { detail: payload }));
      },
      __onPlanModelsSaved: ({ ok }) => {
        if (ok) sendCommand({ cmd: 'queryNativePreferences' });
      },
      __onLanStatus: (status) => {
        setLan(status || { enabled: false, port: null, url: null });
      },
      __onAutostartResult: (success) => {
        if (!success) {
          setNativeError(STR.startup_write_failed[readGuiPreferences().lang]);
          sendCommand({ cmd: 'queryNativePreferences' });
        } else setNativeError('');
      },
      __onGoKeyResult: ({ success, stored }) => {
        setGoKeyStored(!!stored);
        setGoKeyFeedback(success ? '' : STR.settings_go_key_failed[readGuiPreferences().lang]);
      },
      __onGoatKeyResult: ({ success, stored }) => {
        setGoatKeyStored(!!stored);
        setGoatKeyFeedback(success ? '' : STR.settings_go_key_failed[readGuiPreferences().lang]);
      },
      __onDeepSeekKeyResult: ({ success, stored }) => {
        setDeepSeekKeyStored(!!stored);
        setDeepSeekKeyFeedback(success ? '' : STR.settings_go_key_failed[readGuiPreferences().lang]);
      },
      __onShortcutStatus: (available) => {
        if (!available) {
          setNativeError(STR.shortcut_conflict[readGuiPreferences().lang]);
          setPreferences((current) => {
            const next = { ...current, globalShortcut: false };
            saveGuiPreferences(next);
            return next;
          });
        } else setNativeError('');
      },
    });
    sendCommand({ cmd: 'queryNativePreferences' });
    return unregister;
  }, []);

  const refresh = useCallback(() => {
    refreshRequestedAt.current = Date.now();
    setRefreshing(true);
    // wry 在 WebView2 上注入 window.ipc.postMessage(见 wry webview2/mod.rs 注入脚本)
    sendCommand({ cmd: 'refresh' });
  }, []);


  const updatePreference = useCallback((key, value) => {
    if (key === 'autostart') sendCommand({ cmd: 'setAutostart', autostart: value });
    setPreferences((current) => {
      const next = { ...current, [key]: value };
      saveGuiPreferences(next);
      return next;
    });
  }, []);

  const resetAppearance = useCallback(() => {
    setPreferences((current) => {
      const next = { ...DEFAULT_GUI_PREFERENCES, notifications: current.notifications, quietHours: current.quietHours, closeBehavior: current.closeBehavior, globalShortcut: current.globalShortcut, autostart: current.autostart, pinnedCards: current.pinnedCards, hiddenCards: current.hiddenCards, providerColors: current.providerColors, sidebarHidden: current.sidebarHidden };
      saveGuiPreferences(next);
      return next;
    });
  }, []);

  const restoreCard = useCallback((provider) => {
    setPreferences((current) => {
      const next = { ...current, hiddenCards: current.hiddenCards.filter((item) => item !== provider) };
      saveGuiPreferences(next);
      return next;
    });
  }, []);

  const dismissWelcome = useCallback(() => {
    setShowWelcome(false);
  }, []);

  const toggleCard = useCallback((key, provider) => {
    setPreferences((current) => {
      const values = current[key].includes(provider) ? current[key].filter((item) => item !== provider) : [...current[key], provider];
      const next = { ...current, [key]: values };
      saveGuiPreferences(next);
      return next;
    });
  }, []);

  useEffect(() => {
    sendCommand({ cmd: 'setNotifications', enabled: preferences.notifications === 'on', quietHours: preferences.quietHours, lang: preferences.lang });
  }, [preferences.notifications, preferences.quietHours, preferences.lang]);

  useEffect(() => {
    sendCommand({ cmd: 'setCloseBehavior', closeBehavior: preferences.closeBehavior });
  }, [preferences.closeBehavior]);

  useEffect(() => {
    sendCommand({ cmd: 'setGlobalShortcut', globalShortcut: preferences.globalShortcut });
  }, [preferences.globalShortcut]);

  const cycleTheme = useCallback(() => {
    const index = THEME_ORDER.indexOf(preferences.theme);
    updatePreference('theme', THEME_ORDER[(index + 1) % THEME_ORDER.length]);
  }, [preferences.theme, updatePreference]);

  useLayoutEffect(() => {
    if (preferences.theme === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = ['dark', 'ink', 'cassette', 'palace', 'aurora', 'amber', 'mocha'].includes(preferences.theme) ? 'dark' : 'light';
    document.documentElement.lang = lang === 0 ? 'zh-CN' : 'en';
  }, [preferences.theme, lang]);

  useEffect(() => {
    const onKeyDown = (event) => {
      const target = event.target;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setJumpOpen((value) => !value);
        return;
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'b') {
        event.preventDefault();
        setPreferences((current) => {
          const next = { ...current, sidebarHidden: !current.sidebarHidden };
          saveGuiPreferences(next);
          return next;
        });
        return;
      }
      if (event.altKey || event.ctrlKey || event.metaKey || target?.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName || '')) return;
      if (event.key === '1') setPage('quota');
      else if (event.key === '2') setPage('usage');
      else if (event.key === '3') setPage('settings');
      else if (event.key.toLowerCase() === 'r' && !refreshing) refresh();
      else if (event.key.toLowerCase() === 'p') cycleTheme();
      else return;
      event.preventDefault();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [refresh, refreshing, cycleTheme]);

  const t = useCallback((k) => STR[k][lang], [lang]);

  const jumpTo = useCallback((pageKey, selector) => {
    setPage(pageKey);
    window.requestAnimationFrame(() => {
      const node = document.querySelector(selector);
      if (!node) return;
      for (let parent = node.parentElement; parent; parent = parent.parentElement) {
        if (parent.tagName === 'DETAILS') parent.open = true;
      }
      node.scrollIntoView({ behavior: 'instant', block: 'center' });
      node.focus({ preventScroll: true });
    });
  }, []);

  const commands = useMemo(() => {
    const list = [];
    for (const key of NAV_ITEMS) list.push(makeCommand({ id: `page-${key}`, group: 'page', label: t(`nav_${key}`), hint: `${NAV_ITEMS.indexOf(key) + 1}`, keywords: key, run: () => setPage(key) }));
    for (const [id, key] of GROUP_LABELS) list.push(makeCommand({ id: `settings-${id}`, group: 'settings', label: t(key), run: () => jumpTo('settings', `#${id}`) }));
    for (const card of (data?.cards || []).filter((card) => !preferences.hiddenCards.includes(card.provider))) list.push(makeCommand({ id: `provider-${card.provider}`, group: 'provider', label: card.title.split(' ·')[0], hint: t('nav_quota'), keywords: card.provider, run: () => jumpTo('quota', `[data-provider="${card.provider}"]`) }));
    if (data?.deepseek_balance) list.push(makeCommand({ id: 'provider-deepseek', group: 'provider', label: 'DeepSeek API', hint: t('nav_quota'), keywords: 'deepseek', run: () => jumpTo('quota', '[data-provider="deepseek"]') }));
    list.push(makeCommand({ id: 'action-refresh', group: 'action', label: t('action_refresh'), keywords: 'refresh r', run: refresh }));
    list.push(makeCommand({ id: 'action-sidebar', group: 'action', label: preferences.sidebarHidden ? t('sidebar_show') : t('sidebar_hide'), keywords: 'sidebar toggle b', run: () => updatePreference('sidebarHidden', !preferences.sidebarHidden) }));
    list.push(makeCommand({ id: 'action-theme', group: 'action', label: t('action_theme'), keywords: 'theme p', run: cycleTheme }));
    list.push(makeCommand({ id: 'action-overlay', group: 'action', label: t('action_overlay'), keywords: 'overlay', run: () => sendCommand({ cmd: 'toggleOverlay' }) }));
    return list;
  }, [t, data, jumpTo, refresh, cycleTheme, preferences.sidebarHidden, preferences.hiddenCards, updatePreference]);

  const quotaAge = data?.quota_fetched_at_ms ? now - data.quota_fetched_at_ms : null;  const ledgerAge = data?.fetched_at_ms ? now - data.fetched_at_ms : null;
  const quotaStale = Boolean(data && (staleCache || data.quotaError || quotaAge == null || quotaAge > 120_000));
  const ledgerStale = Boolean(data && (staleCache || ledgerAge == null || ledgerAge > 20_000));
  const currency = { code: preferences.currency, rate: exchangeRate?.rate, date: exchangeRate?.date, stale: exchangeRate?.stale, error: exchangeError };
  const pageContent = page === 'settings' ? (
    <SettingsPage
      lang={lang}
      preferences={preferences}
      currency={currency}
      sources={data?.ledger?.sources || []}
      cards={data?.cards || []}
      onPreferenceChange={updatePreference}
      onResetAppearance={resetAppearance}
      onShowWelcome={() => setShowWelcome(true)}
      lan={lan}
      onLanToggle={(enabled) => sendCommand({ cmd: 'setLanDashboard', enabled })}
      nativeError={nativeError}
      goKeyStored={goKeyStored}
      goKeyFeedback={goKeyFeedback}
      onSaveGoKey={(goApiKey) => { setGoKeyFeedback(''); sendCommand({ cmd: 'setGoApiKey', goApiKey }); }}
      onDeleteGoKey={() => { setGoKeyFeedback(''); sendCommand({ cmd: 'deleteGoApiKey' }); }}
      goatKeyStored={goatKeyStored}
      goatKeyFeedback={goatKeyFeedback}
      onSaveGoatKey={(goatApiKey) => { setGoatKeyFeedback(''); sendCommand({ cmd: 'setGoatApiKey', goatApiKey }); }}
      onDeleteGoatKey={() => { setGoatKeyFeedback(''); sendCommand({ cmd: 'deleteGoatApiKey' }); }}
      deepseekKeyStored={deepseekKeyStored}
      deepseekKeyFeedback={deepseekKeyFeedback}
      onSaveDeepSeekKey={(deepseekApiKey) => { setDeepSeekKeyFeedback(''); sendCommand({ cmd: 'setDeepSeekApiKey', deepseekApiKey }); }}
      onDeleteDeepSeekKey={() => { setDeepSeekKeyFeedback(''); sendCommand({ cmd: 'deleteDeepSeekApiKey' }); }}
    />
  ) : data ? (
    <>
      {page === 'quota' && <LocalUsageSummary ledger={data.ledger} lang={lang} stale={ledgerStale} onOpenUsage={() => setPage('usage')} />}
      {page === 'quota' && <QuotaPage
        cards={data.cards}
        deepseekBalance={data.deepseek_balance}
        quotaFetchedAtMs={data.quota_fetched_at_ms}
        quotaStale={quotaStale}
        now={now}
        lang={lang}
        preferences={preferences}
        planModels={planModels}
        onRestoreCard={restoreCard}
        onTogglePin={(provider) => toggleCard('pinnedCards', provider)}
        onToggleHidden={(provider) => toggleCard('hiddenCards', provider)}
        onOpenSettings={() => setPage('settings')}
        onPreferenceChange={updatePreference}
      />}
      {page === 'usage' && <UsageDashboard
        ledger={data.ledger}
        lang={lang}
        currency={{ code: preferences.currency, rate: exchangeRate?.rate }}
        onOpenSettings={() => setPage('settings')}
      />}
    </>
  ) : null;

  return (
    <MotionProvider reduceMotion={reducedMotion}><div
      className="app"
      data-astryx-theme="neutral"
      data-language={lang === 0 ? 'zh' : 'en'}
      data-typeface={preferences.typeface}
      data-palette={preferences.theme}
      data-density={preferences.density}
      data-motion={reducedMotion ? 'reduced' : 'system'}
      data-sidebar={preferences.sidebarHidden ? 'hidden' : 'shown'}
    >
      <aside className="sidebar" inert={preferences.sidebarHidden || undefined}>
        <div className="sidebar-content">
          <div className="sidebar-title">
            <span className="logo-block" />
            <Text weight="medium">{t('title')}</Text>
          </div>
          <PrimaryNav lang={lang} page={page} onNavigate={setPage} variant="desktop" />
        </div>
      </aside>

      <main ref={mainRef} className="main" onScroll={(event) => { scrollByPage.current[page] = event.currentTarget.scrollTop; }}>
        <div className="main-toolbar">
          <button type="button" className="toolbar-sidebar" aria-pressed={!preferences.sidebarHidden} title={preferences.sidebarHidden ? t('sidebar_show') : t('sidebar_hide')} aria-label={preferences.sidebarHidden ? t('sidebar_show') : t('sidebar_hide')} onClick={() => updatePreference('sidebarHidden', !preferences.sidebarHidden)}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="3" /><path d="M9 3v18" /></svg>
          </button>
          <Text weight="medium" className="page-title">{t(`nav_${page}`)}</Text>
          <div className="main-toolbar-spacer" />
          {page === 'quota' && <div className="quota-mode-seg">
            <SelectionGroup>
              <div className="usage-segment" role="group" aria-label={lang === 0 ? '额度口径' : 'Quota metric'}>
                <button type="button" className={!preferences.quotaRemainingView ? 'is-selected' : ''} aria-pressed={!preferences.quotaRemainingView} onClick={() => updatePreference('quotaRemainingView', false)}>
                  {!preferences.quotaRemainingView && <SelectionHighlight />}<span className="segment-label">{t('quota_used')}</span>
                </button>
                <button type="button" className={preferences.quotaRemainingView ? 'is-selected' : ''} aria-pressed={!!preferences.quotaRemainingView} onClick={() => updatePreference('quotaRemainingView', true)}>
                  {preferences.quotaRemainingView && <SelectionHighlight />}<span className="segment-label">{t('quota_remaining')}</span>
                </button>
              </div>
            </SelectionGroup>
          </div>}
          <button type="button" className="toolbar-jump" onClick={() => setJumpOpen(true)} title={t('jump_open')} aria-label={t('jump_open')}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.6-3.6" /></svg>
            <kbd>Ctrl K</kbd>
          </button>
          <button className="toolbar-theme" type="button" onClick={cycleTheme} title={`${t('theme_quick')} (P)`} aria-label={`${t('theme_quick')}: ${t(`theme_${preferences.theme}`)}`}>
            <span className={`toolbar-theme-swatch toolbar-theme-${preferences.theme}`} aria-hidden="true" />
            <span>{t(`theme_${preferences.theme}`)}</span>
          </button>
          <Button size="sm" className="toolbar-refresh" onClick={refresh} disabled={refreshing}>
            {t('refresh')}
          </Button>
        </div>
        {data && page !== 'settings' && <div className="data-freshness" role="status">
          <span className={quotaStale ? 'is-stale' : ''}>{lang === 0 ? '订阅额度' : 'Quotas'} · {quotaAge == null ? t('freshness_waiting') : `${fmtRel(quotaAge)}${lang === 0 ? '前读取' : ' ago'}`}{quotaStale && ` · ${t('freshness_stale')}`}</span>
          <span className={ledgerStale ? 'is-stale' : ''}>{lang === 0 ? '本机记录' : 'Local records'} · {ledgerAge == null ? t('freshness_waiting') : `${fmtRel(ledgerAge)}${lang === 0 ? '前读取' : ' ago'}`}{ledgerStale && ` · ${t('freshness_stale')}`}</span>
        </div>}
        {staleCache && (
          <div className="banner-cache" role="status">
            <Spinner size="sm" /> <Text type="supporting">{t('cache_replay')}</Text>
          </div>
        )}
        {page !== 'settings' && data?.quotaError && (
          <div className="banner-error">
            <Toast kind="error">{t('quota_error')}: {data.quotaError}</Toast>
          </div>
        )}
        {!data && page !== 'settings' ? <Card className="empty-card" role="status"><Spinner size="sm" /> <Text type="supporting">{t('loading')}</Text></Card> : (
          <div className="page-content">{pageContent}</div>
        )}
      </main>
      <DockNav
        lang={lang}
        page={page}
        onNavigate={setPage}
        status={{ data, refreshing, quotaStale, ledgerStale, quotaAge, ledgerAge }}
      />
      <AnimatePresence>{showWelcome && data?.ledger && data.quota_checked_at_ms > 0 && <SourceWelcome key="welcome" data={data} lang={lang} onDone={dismissWelcome} />}</AnimatePresence>
      <CommandPalette open={jumpOpen} onClose={() => setJumpOpen(false)} commands={commands} lang={lang} />
    </div>
    </MotionProvider>
  );
}

window.addEventListener('error', (e) => {
  document.title = `E ${String(e.message).slice(0, 90)} @${e.lineno}:${e.colno}`;
});
window.addEventListener('unhandledrejection', () => {
  document.title = 'E promise';
});

createRoot(document.getElementById('root')).render(<App />);
