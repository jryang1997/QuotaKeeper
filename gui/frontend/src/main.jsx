import React, { lazy, Suspense, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import '@astryxdesign/core/reset.css';
import '@astryxdesign/core/astryx.css';
import '@astryxdesign/theme-neutral/theme.css';
import './app.css';
import { PROVIDERS, SOURCE_PATHS, SOURCE_REPAIR, QUOTA_REPAIR, STR, fmtRel } from './ui-data.js';
import QuotaPage from './QuotaPage.jsx';
import UsagePage from './UsagePage.jsx';
import SettingsPage from './SettingsPage.jsx';
import { DEFAULT_GUI_PREFERENCES, THEME_ORDER, readGuiPreferences, saveGuiPreferences } from './gui-preferences.js';
import { registerNativeCallbacks, sendCommand } from './gui-bridge.js';
import PanelsPage from './PanelsPage.jsx';
import { Card } from '@astryxdesign/core/Card';
import { Text } from '@astryxdesign/core/Text';
import { Button } from '@astryxdesign/core/Button';
import { Spinner } from '@astryxdesign/core/Spinner';
import { Toast } from '@astryxdesign/core/Toast';

const PageTransition = lazy(() => import('./PageTransition.jsx'));

const NAV_ITEMS = ['quota', 'usage', 'heat', 'settings'];

function NavIcon({ page }) {
  if (page === 'quota') {
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 15a8 8 0 1 1 16 0" /><path d="m12 12 4-4" /><path d="M8 19h8" /></svg>;
  }
  if (page === 'usage') {
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 19V5" /><path d="M4 19h16" /><path d="M8 16v-4" /><path d="M12 16V8" /><path d="M16 16v-7" /></svg>;
  }
  if (page === 'settings') {
    return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3" /><path d="m19.4 15 .1.1a1.7 1.7 0 0 1-2.4 2.4l-.1-.1a1.7 1.7 0 0 0-2.9 1.2v.2a1.7 1.7 0 0 1-3.4 0v-.2a1.7 1.7 0 0 0-2.9-1.2l-.1.1a1.7 1.7 0 1 1-2.4-2.4l.1-.1a1.7 1.7 0 0 0-1.2-2.9H4a1.7 1.7 0 0 1 0-3.4h.2a1.7 1.7 0 0 0 1.2-2.9l-.1-.1a1.7 1.7 0 1 1 2.4-2.4l.1.1a1.7 1.7 0 0 0 2.9-1.2V2a1.7 1.7 0 0 1 3.4 0v.2a1.7 1.7 0 0 0 2.9 1.2l.1-.1a1.7 1.7 0 1 1 2.4 2.4l-.1.1a1.7 1.7 0 0 0 1.2 2.9h.2a1.7 1.7 0 0 1 0 3.4h-.2a1.7 1.7 0 0 0-1.2 2.9Z" /></svg>;
  }
  return <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="5" height="5" rx="1" /><rect x="10" y="4" width="5" height="5" rx="1" /><rect x="16" y="4" width="5" height="5" rx="1" /><rect x="4" y="10" width="5" height="5" rx="1" /><rect x="10" y="10" width="5" height="5" rx="1" /><rect x="16" y="10" width="5" height="5" rx="1" /><rect x="4" y="16" width="5" height="5" rx="1" /><rect x="10" y="16" width="5" height="5" rx="1" /><rect x="16" y="16" width="5" height="5" rx="1" /></svg>;
}

function PrimaryNav({ lang, page, onNavigate, variant }) {
  return (
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
            {active && <span className="nav-active-indicator" aria-hidden="true" />}
            <NavIcon page={key} />
            <span className="nav-label">{STR[`nav_${key}`][lang]}</span>
          </button>
        );
      })}
    </nav>
  );
}

function SourceWelcome({ data, lang, onDone }) {
  const t = (key) => STR[key][lang];
  const firstButton = useRef(null);
  useEffect(() => { firstButton.current?.focus(); }, []);
  const sources = data.ledger?.sources || [];
  const cards = data.cards || [];
  return <div className="welcome-backdrop"><section className="welcome-panel" role="dialog" aria-modal="true" aria-labelledby="welcome-title">
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
  </section></div>;
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
  const lang = preferences.lang;
  const [page, setPage] = useState('quota');
  const [data, setData] = useState(null);
  const [refreshing, setRefreshing] = useState(false);
  const [now, setNow] = useState(Date.now);
  const refreshRequestedAt = useRef(0);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 5_000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const unregister = registerNativeCallbacks({
      __onPush: (payload) => {
        setData(payload);
        if (!refreshRequestedAt.current || (payload.fetched_at_ms >= refreshRequestedAt.current && payload.quota_checked_at_ms >= refreshRequestedAt.current)) {
          refreshRequestedAt.current = 0;
          setRefreshing(false);
        }
      },
      __showSettings: () => setPage('settings'),
      __onNativeState: (state) => {
        setGoKeyStored(!!state.goKeyStored);
        setGoatKeyStored(!!state.goatKeyStored);
        setPreferences((current) => {
          const next = { ...current, autostart: !!state.autostart };
          saveGuiPreferences(next);
          return next;
        });
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
      const next = { ...DEFAULT_GUI_PREFERENCES, notifications: current.notifications, paceAlerts: current.paceAlerts, quietHours: current.quietHours, closeBehavior: current.closeBehavior, globalShortcut: current.globalShortcut, autostart: current.autostart, pinnedCards: current.pinnedCards, hiddenCards: current.hiddenCards, providerColors: current.providerColors };
      saveGuiPreferences(next);
      return next;
    });
  }, []);

  const resetCards = useCallback(() => {
    setPreferences((current) => {
      const next = { ...current, pinnedCards: [], hiddenCards: [], providerColors: {} };
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
      else if (event.key === '3') setPage('heat');
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
  const pageContent = page === 'settings' ? (
    <SettingsPage
      lang={lang}
      preferences={preferences}
      onPreferenceChange={updatePreference}
      onResetAppearance={resetAppearance}
      onResetCards={resetCards}
      onShowWelcome={() => setShowWelcome(true)}
      nativeError={nativeError}
      goKeyStored={goKeyStored}
      goKeyFeedback={goKeyFeedback}
      onSaveGoKey={(goApiKey) => { setGoKeyFeedback(''); sendCommand({ cmd: 'setGoApiKey', goApiKey }); }}
      onDeleteGoKey={() => { setGoKeyFeedback(''); sendCommand({ cmd: 'deleteGoApiKey' }); }}
      goatKeyStored={goatKeyStored}
      goatKeyFeedback={goatKeyFeedback}
      onSaveGoatKey={(goatApiKey) => { setGoatKeyFeedback(''); sendCommand({ cmd: 'setGoatApiKey', goatApiKey }); }}
      onDeleteGoatKey={() => { setGoatKeyFeedback(''); sendCommand({ cmd: 'deleteGoatApiKey' }); }}
      monthlyValue={data?.monthly_value}
    />
  ) : data ? (
    <>
      {page === 'quota' && (data.cards?.length ? <QuotaPage cards={data.cards} sources={data.ledger?.sources} ledger={data.ledger} goatUsage={data.goat_usage} quotaFetchedAtMs={data.quota_fetched_at_ms} quotaStale={quotaStale} now={now} lang={lang} preferences={preferences} onTogglePin={(provider) => toggleCard('pinnedCards', provider)} onToggleHidden={(provider) => toggleCard('hiddenCards', provider)} onOpenSettings={() => setPage('settings')} /> : <Card className="empty-card"><Text type="supporting">{t('empty_quota')}</Text></Card>)}
      {page === 'usage' && <UsagePage ledger={data.ledger} cards={data.cards} monthlyValue={data.monthly_value} lang={lang} typeface={preferences.typeface} onOpenSettings={() => setPage('settings')} />}
      {page === 'heat' && <PanelsPage ledger={data.ledger} lang={lang} />}
    </>
  ) : null;

  return (
    <div
      className="app"
      data-astryx-theme="neutral"
      data-language={lang === 0 ? 'zh' : 'en'}
      data-typeface={preferences.typeface}
      data-palette={preferences.theme}
      data-density={preferences.density}
      data-motion={preferences.motion}
    >
      <aside className="sidebar">
        <div className="sidebar-title">
          <span className="logo-block" />
          <Text weight="medium">{t('title')}</Text>
        </div>
        <PrimaryNav lang={lang} page={page} onNavigate={setPage} variant="desktop" />
        <div className="sidebar-foot">
          <div className="sidebar-status">
            {refreshing ? (
              <><Spinner size="sm" /> <span>{t('refreshing')}</span></>
            ) : (
              <span className="sidebar-age">
                {t('updated')} {age != null ? fmtRel(age * 1000) : '—'}
              </span>
            )}
          </div>
        </div>
      </aside>

      <main className="main">
        <div className="main-toolbar">
          <Text weight="medium" className="page-title">{t(page === 'quota' ? 'nav_quota' : page === 'usage' ? 'nav_usage' : page === 'heat' ? 'nav_heat' : 'nav_settings')}</Text>
          <div className="main-toolbar-spacer" />
          <button className="toolbar-theme" type="button" onClick={cycleTheme} title={`${t('theme_quick')} (P)`} aria-label={`${t('theme_quick')}: ${t(`theme_${preferences.theme}`)}`}>
            <span className={`toolbar-theme-swatch toolbar-theme-${preferences.theme}`} aria-hidden="true" />
            <span>{t(`theme_${preferences.theme}`)}</span>
          </button>
          <Button size="sm" onClick={refresh} disabled={refreshing}>
            {t('refresh')}
          </Button>
        </div>
        {page !== 'settings' && data && <div className="freshness-strip" role="status">
          <span className={quotaStale ? 'freshness-badge is-stale' : 'freshness-badge'} title={data.quota_fetched_at_ms ? new Date(data.quota_fetched_at_ms).toLocaleString() : undefined}><i />{t('freshness_quota')} · {quotaStale && `${t('freshness_stale')} · `}{quotaAge == null ? t('freshness_waiting') : `${t('freshness_last_success')} ${fmtRel(quotaAge)}`}</span>
          <span className={ledgerStale ? 'freshness-badge is-stale' : 'freshness-badge'} title={data.fetched_at_ms ? new Date(data.fetched_at_ms).toLocaleString() : undefined}><i />{t('freshness_local')} · {ledgerStale && `${t('freshness_stale')} · `}{ledgerAge == null ? t('freshness_waiting') : `${t('freshness_last_success')} ${fmtRel(ledgerAge)}`}</span>
        </div>}
        {page !== 'settings' && data?.quotaError && (
          <div className="banner-error">
            <Toast kind="error">{t('quota_error')}: {data.quotaError}</Toast>
          </div>
        )}
        {page !== 'settings' && !data ? <Card className="empty-card" role="status"><Spinner size="sm" /> <Text type="supporting">{t('loading')}</Text></Card> : (
          <Suspense fallback={<div className="page-content">{pageContent}</div>}>
            <PageTransition page={page} reduceMotion={preferences.motion === 'reduced'}>{pageContent}</PageTransition>
          </Suspense>
        )}
      </main>
      <PrimaryNav lang={lang} page={page} onNavigate={setPage} variant="mobile" />
      {showWelcome && data?.ledger && data.quota_checked_at_ms > 0 && <SourceWelcome data={data} lang={lang} onDone={dismissWelcome} />}
    </div>
  );
}

window.addEventListener('error', (e) => {
  document.title = `E ${String(e.message).slice(0, 90)} @${e.lineno}:${e.colno}`;
});
window.addEventListener('unhandledrejection', () => {
  document.title = 'E promise';
});

createRoot(document.getElementById('root')).render(<App />);
