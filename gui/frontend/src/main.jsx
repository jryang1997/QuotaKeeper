import React, { lazy, Suspense, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import '@astryxdesign/core/reset.css';
import '@astryxdesign/core/astryx.css';
import '@astryxdesign/theme-neutral/theme.css';
import './app.css';
import './usage-dashboard.css';
import './ia.css';
import { PROVIDERS, SOURCE_PATHS, SOURCE_REPAIR, QUOTA_REPAIR, STR, fmtRel } from './ui-data.js';
import QuotaPage from './QuotaPage.jsx';
import UsageDashboard from './UsageDashboard.jsx';
import SettingsPage from './SettingsPage.jsx';
import { DEFAULT_GUI_PREFERENCES, THEME_ORDER, readGuiPreferences, saveGuiPreferences } from './gui-preferences.js';
import { fetchExchangeRate, readExchangeRates } from './currency.js';
import { registerNativeCallbacks, sendCommand } from './gui-bridge.js';
import SharePage from './SharePage.jsx';
import { AnimatePresence, LayoutGroup, motion, useIsPresent } from 'motion/react';
import { MotionProvider, PopoverSurface, useMotionPreference, SelectionGroup, SelectionHighlight } from './ui-motion.jsx';
import { CONTROL_SPRING, FADE } from './motion-tuning.js';
import { Card } from '@astryxdesign/core/Card';
import { Text } from '@astryxdesign/core/Text';
import { Button } from '@astryxdesign/core/Button';
import { Spinner } from '@astryxdesign/core/Spinner';
import { Toast } from '@astryxdesign/core/Toast';
import { AchievementToasts, AchievementsModal, useAchievements } from './Achievements.jsx';

const PageTransition = lazy(() => import('./PageTransition.jsx'));

const NAV_ITEMS = ['quota', 'usage', 'share', 'settings'];
const EDITABLE_PAGES = { quota: true, usage: true };

function NavIcon({ page }) {
  if (page === 'quota') {
    // gauge:下半弧 + 指针,指针指向右上(趋势向上)
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="m12 14 4-4" /><path d="M3.34 19a10 10 0 1 1 17.32 0" /></svg>;
  }
  if (page === 'usage') {
    // 带轴圆帽柱:轴一次画到位,三根柱节奏降序,读作「用量」
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 3v16a2 2 0 0 0 2 2h16" /><path d="M8 17v-4" /><path d="M13 17V8" /><path d="M18 17v-7" /></svg>;
  }
  if (page === 'settings') {
    // lucide 现代齿轮:外环由 8 组对称圆弧构成,光学中心与圆孔重合
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" /><circle cx="12" cy="12" r="3" /></svg>;
  }
  // 分享:三节点连线,读作「卡片 → 分享出去」
  return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="18" cy="5" r="3" /><circle cx="6" cy="12" r="3" /><circle cx="18" cy="19" r="3" /><path d="m8.6 10.5 6.8-4M8.6 13.5l6.8 4" /></svg>;
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

// 数据状态:正常时收进侧栏一枚极小的状态点,点开才看明细;异常才在页内提示。
function DataStatus({ lang, data, refreshing, age, quotaStale, ledgerStale, quotaAge, ledgerAge }) {
  const t = (key) => STR[key][lang];
  const [open, setOpen] = useState(false);
  const stale = quotaStale || ledgerStale;
  return (
    <div className="sidebar-status-wrap">
      <button type="button" className={`sidebar-status${stale ? ' is-stale' : ''}`} aria-expanded={open} onClick={() => setOpen((value) => !value)} title={t('freshness_open')}>
        {refreshing ? (
          <><Spinner size="sm" /> <span>{t('refreshing')}</span></>
        ) : (
          <><i className={`status-dot${stale ? ' is-stale' : ''}`} aria-hidden="true" /><span className="sidebar-age">{t('updated')} {age != null ? fmtRel(age * 1000) : '—'}</span></>
        )}
      </button>
      <AnimatePresence>{open && <PopoverSurface className="status-popover" role="status">
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

const WELCOME_DONE_KEY = 'token-monitor.welcome-done.v1';

function App() {
  const [preferences, setPreferences] = useState(readGuiPreferences);
  const [showWelcome, setShowWelcome] = useState(() => { try { return window.localStorage.getItem(WELCOME_DONE_KEY) !== '1'; } catch { return true; } });
  const [nativeError, setNativeError] = useState('');
  const [goKeyStored, setGoKeyStored] = useState(false);
  const [goKeyFeedback, setGoKeyFeedback] = useState('');
  const [goatKeyStored, setGoatKeyStored] = useState(false);
  const [goatKeyFeedback, setGoatKeyFeedback] = useState('');
  const [deepseekKeyStored, setDeepSeekKeyStored] = useState(false);
  const [deepseekKeyFeedback, setDeepSeekKeyFeedback] = useState('');
  const [agents, setAgents] = useState(null);
  const [agentsScanning, setAgentsScanning] = useState(false);
  const [agentUpdating, setAgentUpdating] = useState(null);
  const [agentFeedback, setAgentFeedback] = useState(null);
  const [planModels, setPlanModels] = useState({});
  const [shareStatus, setShareStatus] = useState(null);
  const [lan, setLan] = useState({ enabled: false, port: null, url: null });
  const [achOpen, setAchOpen] = useState(false);
  const [editing, setEditing] = useState({ quota: false, usage: false });
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
  const achievements = useAchievements(data?.ledger, lang);
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
      __onShareImageSaved: (status) => {
        setShareStatus(status);
        setTimeout(() => setShareStatus((current) => (current === status ? null : current)), 4000);
      },
      __onLanStatus: (status) => {
        setLan(status || { enabled: false, port: null, url: null });
      },
      __onAutostartResult: (success) => {
        if (!success) {
          setNativeError('Windows 启动项写入失败 / Failed to update Windows startup');
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
          setNativeError('Ctrl+Alt+U 已被其他应用占用 / Ctrl+Alt+U is already in use');
          setPreferences((current) => {
            const next = { ...current, globalShortcut: false };
            saveGuiPreferences(next);
            return next;
          });
        } else setNativeError('');
      },
      __onAgentsScanned: (payload) => {
        setAgents(Array.isArray(payload) ? payload : []);
        setAgentsScanning(false);
      },
      __onAgentUpdateResult: ({ results }) => {
        setAgentUpdating(null);
        setAgentFeedback({ ok: Array.isArray(results) && results.length > 0 && results.every(([, ok]) => ok), results });
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

  const scanAgents = useCallback(() => {
    setAgentsScanning(true);
    sendCommand({ cmd: 'scanAgents' });
  }, []);

  const updateAgent = useCallback((id) => {
    setAgentUpdating(id);
    setAgentFeedback(null);
    sendCommand({ cmd: 'runAgentUpdate', agent: id });
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
      const next = { ...DEFAULT_GUI_PREFERENCES, notifications: current.notifications, paceAlerts: current.paceAlerts, quietHours: current.quietHours, closeBehavior: current.closeBehavior, globalShortcut: current.globalShortcut, autostart: current.autostart, pinnedCards: current.pinnedCards, hiddenCards: current.hiddenCards, providerColors: current.providerColors };
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
    try { window.localStorage.setItem(WELCOME_DONE_KEY, '1'); } catch { /* session only */ }
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
    sendCommand({ cmd: 'setNotifications', enabled: preferences.notifications === 'on', paceAlerts: preferences.paceAlerts, quietHours: preferences.quietHours, lang: preferences.lang });
  }, [preferences.notifications, preferences.paceAlerts, preferences.quietHours, preferences.lang]);

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
    else document.documentElement.dataset.theme = ['dark', 'ink'].includes(preferences.theme) ? 'dark' : 'light';
    document.documentElement.lang = lang === 0 ? 'zh-CN' : 'en';
  }, [preferences.theme, lang]);

  useEffect(() => {
    const onKeyDown = (event) => {
      const target = event.target;
      if (event.altKey || event.ctrlKey || event.metaKey || target?.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName || '')) return;
      if (event.key === '1') setPage('quota');
      else if (event.key === '2') setPage('usage');
      else if (event.key === '3') setPage('share');
      else if (event.key === '4') setPage('settings');
      else if (event.key.toLowerCase() === 'r' && !refreshing) refresh();
      else if (event.key.toLowerCase() === 'p') cycleTheme();
      else return;
      event.preventDefault();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [refresh, refreshing, cycleTheme]);

  const t = useCallback((k) => STR[k][lang], [lang]);

  const age = data ? Math.floor((now - data.fetched_at_ms) / 1000) : null;
  const quotaAge = data?.quota_fetched_at_ms ? now - data.quota_fetched_at_ms : null;
  const ledgerAge = data?.fetched_at_ms ? now - data.fetched_at_ms : null;
  const quotaStale = data && (data.quotaError || quotaAge == null || quotaAge > 120_000);
  const ledgerStale = data && (ledgerAge == null || ledgerAge > 20_000);
  const currency = { code: preferences.currency, rate: exchangeRate?.rate, date: exchangeRate?.date, stale: exchangeRate?.stale, error: exchangeError };
  const setPageEditing = (key, value) => setEditing((current) => ({ ...current, [key]: value }));
  const pageContent = page === 'settings' ? (
    <SettingsPage
      lang={lang}
      preferences={preferences}
      currency={currency}
      monthlyValue={data?.monthly_value}
      sources={data?.ledger?.sources || []}
      cards={data?.cards || []}
      onPreferenceChange={updatePreference}
      onResetAppearance={resetAppearance}
      onShowWelcome={() => setShowWelcome(true)}
      onOpenShare={() => setPage('share')}
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
      agents={agents}
      agentsScanning={agentsScanning}
      agentUpdating={agentUpdating}
      agentFeedback={agentFeedback}
      onScanAgents={scanAgents}
      onUpdateAgent={updateAgent}
    />
  ) : data ? (
    <>
      {page === 'quota' && (data.cards?.length ? <QuotaPage
        cards={data.cards}
        ledger={data.ledger}
        goatUsage={data.goat_usage}
        deepseekBalance={data.deepseek_balance}
        quotaFetchedAtMs={data.quota_fetched_at_ms}
        quotaStale={quotaStale}
        now={now}
        lang={lang}
        preferences={preferences}
        planModels={planModels}
        editing={editing.quota}
        onToggleEditing={(value) => setPageEditing('quota', value)}
        onRestoreCard={restoreCard}
        onTogglePin={(provider) => toggleCard('pinnedCards', provider)}
        onToggleHidden={(provider) => toggleCard('hiddenCards', provider)}
        onOpenSettings={() => setPage('settings')}
        onPreferenceChange={updatePreference}
      /> : <Card className="empty-card"><Text type="supporting">{t('empty_quota')}</Text></Card>)}
      {page === 'usage' && <UsageDashboard
        ledger={data.ledger}
        monthlyValue={data.monthly_value}
        lang={lang}
        currency={{ code: preferences.currency, rate: exchangeRate?.rate }}
        onOpenSettings={() => setPage('settings')}
        onOpenAchievements={() => setAchOpen(true)}
        editing={editing.usage}
        onToggleEditing={(value) => setPageEditing('usage', value)}
      />}
      {page === 'share' && <SharePage ledger={data.ledger} monthlyValue={data.monthly_value} lang={lang} shareStatus={shareStatus} lan={lan} onLanToggle={(enabled) => sendCommand({ cmd: 'setLanDashboard', enabled })} />}
    </>
  ) : null;

  const stale = quotaStale || ledgerStale;
  return (
    <MotionProvider reduceMotion={reducedMotion}><div
      className="app"
      data-astryx-theme="neutral"
      data-language={lang === 0 ? 'zh' : 'en'}
      data-typeface={preferences.typeface}
      data-palette={preferences.theme}
      data-density={preferences.density}
      data-motion={reducedMotion ? 'reduced' : 'system'}
    >
      <aside className="sidebar">
        <div className="sidebar-title">
          <span className="logo-block" />
          <Text weight="medium">{t('title')}</Text>
        </div>
        <PrimaryNav lang={lang} page={page} onNavigate={setPage} variant="desktop" />
        <div className="sidebar-foot">
          <DataStatus lang={lang} data={data} refreshing={refreshing} age={age} quotaStale={quotaStale} ledgerStale={ledgerStale} quotaAge={quotaAge} ledgerAge={ledgerAge} />
        </div>
      </aside>

      <main ref={mainRef} className="main" onScroll={(event) => { scrollByPage.current[page] = event.currentTarget.scrollTop; }}>
        <div className="main-toolbar">
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
          {EDITABLE_PAGES[page] && <button type="button" className={`toolbar-edit${editing[page] ? ' is-on' : ''}`} aria-pressed={editing[page]} title={t('edit_layout_hint')} onClick={() => setPageEditing(page, !editing[page])}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 20h4l10-10-4-4L4 16z" /><path d="m13.5 6.5 4 4" /></svg>
            <span>{editing[page] ? t('edit_layout_done') : t('edit_layout')}</span>
          </button>}
          <button className="toolbar-theme" type="button" onClick={cycleTheme} title={`${t('theme_quick')} (P)`} aria-label={`${t('theme_quick')}: ${t(`theme_${preferences.theme}`)}`}>
            <span className={`toolbar-theme-swatch toolbar-theme-${preferences.theme}`} aria-hidden="true" />
            <span>{t(`theme_${preferences.theme}`)}</span>
          </button>
          <Button size="sm" className="toolbar-refresh" onClick={refresh} disabled={refreshing}>
            {t('refresh')}
          </Button>
        </div>
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
        {!data ? <Card className="empty-card" role="status"><Spinner size="sm" /> <Text type="supporting">{t('loading')}</Text></Card> : (
          <Suspense fallback={<div className="page-content">{pageContent}</div>}>
            <PageTransition page={page} reduceMotion={preferences.motion === 'reduced'}>{pageContent}</PageTransition>
          </Suspense>
        )}
      </main>
      <PrimaryNav lang={lang} page={page} onNavigate={setPage} variant="mobile" />
      <AnimatePresence>{showWelcome && data?.ledger && data.quota_checked_at_ms > 0 && <SourceWelcome key="welcome" data={data} lang={lang} onDone={dismissWelcome} />}</AnimatePresence>
      <AnimatePresence>{achOpen && <AchievementsModal key="achievements" ledger={data?.ledger} lang={lang} state={achievements.state} onClose={() => setAchOpen(false)} />}</AnimatePresence>
      <AchievementToasts items={achievements.toasts} lang={lang} onDismiss={achievements.dismiss} />
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
