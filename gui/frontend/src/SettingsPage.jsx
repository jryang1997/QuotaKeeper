// 规则:颜色与全局偏好归设置,布局与显隐归所在页面;每个概念只有这一处编辑面。
import { sendCommand } from './gui-bridge.js';
import React, { useEffect, useMemo, useState } from 'react';
import { PROVIDERS, PROVIDER_COLOR_DEFAULTS, QUOTA_HINT, QUOTA_REPAIR, SOURCE_PATHS, SOURCE_REPAIR, STR, fmtInt } from './ui-data.js';
import { CURRENCIES, formatCurrency } from './currency.js';
import { SelectionGroup, SelectionHighlight } from './ui-motion.jsx';
import LanSettings from './LanSettings.jsx';

const GROUP_IDS = ['appearance', 'sources', 'desktop', 'about'];

function SettingsChoice({ label, value, onChange, options }) {
  return (
    <SelectionGroup><div className={`settings-choice${options.length > 3 ? ' settings-choice-many' : ''}`} role="group" aria-label={label}>
      {options.map((option) => (
        <button
          key={String(option.value)}
          type="button"
          aria-pressed={value === option.value}
          className={`settings-choice-option${value === option.value ? ' is-selected' : ''}`}
          onClick={() => onChange(option.value)}
        >
          {value === option.value && <SelectionHighlight />}
          {option.icon && <span className={`settings-choice-icon ${option.icon}`} aria-hidden="true" />}
          <span>{option.label}</span>
        </button>
      ))}
    </div>
    </SelectionGroup>
  );
}

function SettingsSection({ id, title, hint, children, className = '' }) {
  return (
    <section className={`settings-card ${className}`} id={id} data-settings-section={id}>
      <header className="settings-card-heading">
        <div>
          <h2>{title}</h2>
          {hint && <p>{hint}</p>}
        </div>
      </header>
      {children}
    </section>
  );
}

function SettingsRow({ title, hint, children, className = '' }) {
  return (
    <div className={`settings-row ${className}`}>
      <div className="settings-row-copy">
        <div className="settings-row-title">{title}</div>
        {hint && <div className="settings-row-hint">{hint}</div>}
      </div>
      <div className="settings-row-control">{children}</div>
    </div>
  );
}

// 目录跟随滚动高亮:section 进入视口上半区即激活。
function useSectionSpy() {
  const [active, setActive] = useState(GROUP_IDS[0]);
  useEffect(() => {
    if (typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver((entries) => {
      const visible = entries.filter((entry) => entry.isIntersecting)
        .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)[0];
      if (visible?.target?.id) setActive(visible.target.id);
    }, { rootMargin: '-84px 0px -58% 0px', threshold: 0 });
    for (const id of GROUP_IDS) {
      const node = document.getElementById(id);
      if (node) observer.observe(node);
    }
    return () => observer.disconnect();
  }, []);
  return active;
}

function scrollToSection(id) {
  const node = document.getElementById(id);
  if (!node) return;
  node.scrollIntoView({ behavior: 'smooth', block: 'start' });
}


// 一张服务商密钥行:状态 + 输入 + 保存/移除;三家用同一形状,不再各写一遍。
function ProviderKeyRow({ lang, provider, title, hint, placeholder, stored, feedback, onSave, onDelete, status, onOpenDetails }) {
  const t = (key) => STR[key][lang];
  const [draft, setDraft] = useState('');
  const [error, setError] = useState('');
  const submit = (event) => {
    event.preventDefault();
    const key = draft.trim();
    if (!key || /\s/.test(key)) { setError(t('settings_go_key_invalid')); return; }
    onSave(key);
    setDraft('');
    setError('');
  };
  return (
    <div className="settings-provider" data-provider={provider}>
      <div className="settings-provider-head">
        <div>
          <div className="settings-provider-title">
            <span className={`source-indicator source-indicator-${status === 'live' ? 'ready' : status === 'error' ? 'error' : 'idle'}`} />
            <strong>{title}</strong>
            {status && <span className="settings-provider-state">{status === 'live' ? t('settings_provider_live') : t('settings_provider_unreadable')}</span>}
          </div>
          {hint && <p className="settings-provider-hint">{hint}</p>}
        </div>
        <span className={`settings-provider-badge${stored ? ' is-on' : ''}`}>{stored ? t('settings_provider_ready') : t('settings_provider_missing')}</span>
      </div>
      <form className="settings-provider-form" onSubmit={submit}>
        <input type="password" autoComplete="off" spellCheck="false" value={draft} onChange={(event) => { setDraft(event.target.value); setError(''); }} placeholder={placeholder} aria-label={placeholder} />
        <button className="settings-save" type="submit">{t('settings_save')}</button>
        <button className="settings-reset" type="button" disabled={!stored} onClick={onDelete}>{t('settings_go_key_remove')}</button>
      </form>
      {(error || feedback) && <p className="settings-form-error" role="status">{error || feedback}</p>}
      {status === 'error' && onOpenDetails && <button type="button" className="settings-provider-repair" onClick={onOpenDetails}>{t('welcome_repair')}</button>}
    </div>
  );
}

export default function SettingsPage({
  lang, preferences, currency, nativeError, sources = [], cards = [],
  goKeyStored, goKeyFeedback, onSaveGoKey, onDeleteGoKey,
  goatKeyStored, goatKeyFeedback, onSaveGoatKey, onDeleteGoatKey,
  deepseekKeyStored, deepseekKeyFeedback, onSaveDeepSeekKey, onDeleteDeepSeekKey,
  onPreferenceChange, onResetAppearance, onShowWelcome, lan, onLanToggle,
}) {
  const t = (key) => STR[key][lang];
  const zh = lang === 0;
  const [openSource, setOpenSource] = useState('');
  const active = useSectionSpy();
  const cardByProvider = useMemo(() => Object.fromEntries((cards || []).map((card) => [card.provider, card])), [cards]);
  const otherProviders = PROVIDERS.filter((id) => !['opencode-go', 'goat', 'deepseek'].includes(id));

  const groups = [
    ['appearance', t('settings_group_appearance')],
    ['sources', t('settings_group_sources')],
    ['desktop', t('settings_group_desktop')],
    ['about', t('settings_group_about')],
  ];

  return (
    <div className="settings-page">
      <div className="settings-shell">
        <nav className="settings-toc" aria-label={t('nav_settings')}>
          {groups.map(([id, label]) => (
            <button key={id} type="button" className={active === id ? 'is-active' : ''} onClick={() => scrollToSection(id)}>
              <i aria-hidden="true" />{label}
            </button>
          ))}
        </nav>

        <div className="settings-body">
          <SettingsSection id="appearance" title={t('settings_group_appearance')} className="settings-appearance">
            <SettingsRow title={t('settings_language')}>
              <SettingsChoice
                label={t('settings_language')}
                value={lang}
                onChange={(value) => onPreferenceChange('lang', value)}
                options={[{ value: 0, label: '中文' }, { value: 1, label: 'English' }]}
              />
            </SettingsRow>
            <SettingsRow title={t('settings_theme')} hint={t('settings_theme_hint')}>
              <SettingsChoice
                label={t('settings_theme')}
                value={preferences.theme}
                onChange={(value) => onPreferenceChange('theme', value)}
                options={[
                  { value: 'system', label: t('theme_system'), icon: 'theme-icon-system' },
                  { value: 'light', label: t('theme_light'), icon: 'theme-icon-light' },
                  { value: 'dark', label: t('theme_dark'), icon: 'theme-icon-dark' },
                  { value: 'ink', label: t('theme_ink'), icon: 'theme-icon-ink' },
                  { value: 'sand', label: t('theme_sand'), icon: 'theme-icon-sand' },
                  { value: 'cassette', label: t('theme_cassette'), icon: 'theme-icon-cassette' },
                  { value: 'washi', label: t('theme_washi'), icon: 'theme-icon-washi' },
                  { value: 'palace', label: t('theme_palace'), icon: 'theme-icon-palace' },
                  { value: 'aurora', label: t('theme_aurora'), icon: 'theme-icon-aurora' },
                  { value: 'amber', label: t('theme_amber'), icon: 'theme-icon-amber' },
                  { value: 'mocha', label: t('theme_mocha'), icon: 'theme-icon-mocha' },
                ]}
              />
            </SettingsRow>
            <SettingsRow title={t('settings_font')}>
              <SettingsChoice
                label={t('settings_font')}
                value={preferences.typeface}
                onChange={(value) => onPreferenceChange('typeface', value)}
                options={[
                  { value: 'sarasa', label: t('font_sarasa') },
                  { value: 'plex', label: t('font_plex') },
                ]}
              />
            </SettingsRow>
            <div className="settings-theme-preview" aria-hidden="true">
              <span className="settings-preview-window"><i /><i /><i /><b /><b /><b /></span>
              <span className="settings-preview-copy"><strong>{t('settings_font_sample')}</strong><small>12,480 tokens · {formatCurrency(3.84, currency)}</small></span>
              <span className="settings-preview-spark" />
            </div>
            <SettingsRow title={t('settings_density')}>
              <SettingsChoice
                label={t('settings_density')}
                value={preferences.density}
                onChange={(value) => onPreferenceChange('density', value)}
                options={[
                  { value: 'comfortable', label: t('density_comfortable') },
                  { value: 'compact', label: t('density_compact') },
                ]}
              />
            </SettingsRow>
            <SettingsRow title={t('settings_motion')}>
              <SettingsChoice
                label={t('settings_motion')}
                value={preferences.motion}
                onChange={(value) => onPreferenceChange('motion', value)}
                options={[
                  { value: 'system', label: t('motion_system') },
                  { value: 'reduced', label: t('motion_reduce') },
                ]}
              />
            </SettingsRow>
            <SettingsRow title={t('settings_colors')} hint={t('settings_colors_hint')}>
              <div className="settings-colors">
                {PROVIDERS.map((provider) => (
                  <label key={provider} className="settings-color">
                    <input
                      type="color"
                      aria-label={`${provider} ${t('settings_colors')}`}
                      value={preferences.providerColors[provider] || PROVIDER_COLOR_DEFAULTS[provider]}
                      onChange={(event) => onPreferenceChange('providerColors', { ...preferences.providerColors, [provider]: event.target.value })}
                    />
                    <span>{provider}</span>
                  </label>
                ))}
              </div>
            </SettingsRow>
            <SettingsRow title={t('settings_currency')} hint={t('settings_currency_hint')}>
              <select className="settings-currency-select" value={preferences.currency} onChange={(event) => onPreferenceChange('currency', event.target.value)} aria-label={t('settings_currency')}>
                {CURRENCIES.map((code) => <option key={code} value={code}>{code} · {new Intl.DisplayNames([zh ? 'zh-CN' : 'en'], { type: 'currency' }).of(code)}</option>)}
              </select>
              <p className="settings-currency-rate" role="status">{preferences.currency === 'USD' ? t('settings_currency_usd') : currency?.rate > 0 ? `${t('settings_currency_rate')} 1 USD ≈ ${formatCurrency(1, currency)} · ${currency.date || '—'}${currency.stale ? ` · ${t('settings_currency_stale')}` : ''}` : currency?.error ? t('settings_currency_error') : t('settings_currency_loading')}</p>
              <small className="settings-currency-source">{t('settings_currency_source')} <a href="https://frankfurter.dev/" target="_blank" rel="noreferrer">Frankfurter</a></small>
            </SettingsRow>
          </SettingsSection>

          <SettingsSection id="sources" title={t('settings_group_sources')} hint={t('settings_providers_hint')}>
            <div className="settings-subsection">
              <div className="settings-subhead">
                <div><h3>{t('settings_providers_title')}</h3></div>
              </div>
              <ProviderKeyRow
                lang={lang}
                provider="opencode-go"
                title={t('settings_go_key')}
                hint={t('settings_go_key_hint')}
                placeholder={t('settings_go_key_placeholder')}
                stored={goKeyStored}
                feedback={goKeyFeedback}
                onSave={onSaveGoKey}
                onDelete={onDeleteGoKey}
                status={cardByProvider['opencode-go']?.kind === 'Live' ? 'live' : cardByProvider['opencode-go'] ? 'error' : ''}
              />
              <ProviderKeyRow
                lang={lang}
                provider="goat"
                title={t('settings_goat_key')}
                hint={t('settings_goat_key_hint')}
                placeholder="Command Code API key"
                stored={goatKeyStored}
                feedback={goatKeyFeedback}
                onSave={onSaveGoatKey}
                onDelete={onDeleteGoatKey}
                status={cardByProvider.goat?.kind === 'Live' ? 'live' : cardByProvider.goat ? 'error' : ''}
              />
              <ProviderKeyRow
                lang={lang}
                provider="deepseek"
                title={t('settings_deepseek_key')}
                hint={t('settings_deepseek_hint')}
                placeholder="DeepSeek API key"
                stored={deepseekKeyStored}
                feedback={deepseekKeyFeedback}
                onSave={onSaveDeepSeekKey}
                onDelete={onDeleteDeepSeekKey}
                status={cardByProvider.deepseek?.kind === 'Live' ? 'live' : ''}
              />
              <SettingsRow title={t('settings_clipboard')} hint={t('settings_clipboard_hint')}>
                <SettingsChoice
                  label={t('settings_clipboard')}
                  value={preferences.clipboardClear !== false}
                  onChange={(value) => onPreferenceChange('clipboardClear', value)}
                  options={[{ value: false, label: t('settings_alerts_off') }, { value: true, label: t('settings_alerts_on') }]}
                />
              </SettingsRow>
            </div>

            <div className="settings-subsection">
              <div className="settings-subhead">
                <div><h3>{t('settings_source_other')}</h3></div>
              </div>
              {otherProviders.map((provider) => {
                const card = cardByProvider[provider];
                const live = card?.kind === 'Live';
                return (
                  <div className="settings-source-row" key={provider}>
                    <span className={`source-indicator source-indicator-${live ? 'ready' : 'idle'}`} />
                    <strong>{provider}</strong>
                    <span className="settings-source-state">{live ? t('settings_provider_live') : (QUOTA_HINT[provider]?.[lang] || t('settings_provider_missing'))}</span>
                    {!live && <details><summary>{t('welcome_repair')}</summary><p>{QUOTA_REPAIR[provider]?.[lang]}</p></details>}
                  </div>
                );
              })}
            </div>

            <div className="settings-subsection">
              <div className="settings-subhead">
                <div><h3>{t('settings_sources_title')}</h3><p>{t('settings_sources_hint')}</p></div>
              </div>
              {sources.map((source) => {
                const state = source.error ? 'error' : source.records > 0 ? 'ready' : 'idle';
                const open = openSource === source.client;
                return (
                  <div className={`settings-source-row is-${state}`} key={source.client}>
                    <span className={`source-indicator source-indicator-${state}`} />
                    <strong>{source.label}</strong>
                    <span className="settings-source-state">
                      {source.error ? t('source_error') : source.records > 0 ? `${fmtInt(source.records)} ${t('source_ready')}` : t('source_no_records')}
                    </span>
                    <button type="button" className="settings-source-toggle" aria-expanded={open} onClick={() => setOpenSource(open ? '' : source.client)}>{t('source_diagnostic')}</button>
                    {open && <div className="settings-source-detail">
                      <span>{t('source_location')} · <code>{SOURCE_PATHS[source.client] || '—'}</code></span>
                      <p>{SOURCE_REPAIR[source.client]?.[lang]}</p>
                      {source.error && <p className="is-error">{source.error}</p>}
                    </div>}
                  </div>
                );
              })}
              {sources.length === 0 && <p className="settings-agent-path" role="status">{t('settings_sources_empty')}</p>}
            </div>

          </SettingsSection>

          <SettingsSection id="desktop" title={t('settings_group_desktop')} hint={t('settings_desktop_hint')}>
            <SettingsRow title={t('settings_close_behavior')}>
              <SettingsChoice label={t('settings_close_behavior')} value={preferences.closeBehavior} onChange={(value) => onPreferenceChange('closeBehavior', value)} options={[{ value: 'tray', label: t('settings_close_tray') }, { value: 'exit', label: t('settings_close_exit') }]} />
            </SettingsRow>
            <SettingsRow title={t('settings_overlay')} hint={t('settings_overlay_hint')}>
              <button type="button" className="settings-inline-action" onClick={() => sendCommand({ cmd: 'toggleOverlay' })}>{t('settings_overlay_toggle')}</button>
            </SettingsRow>
            <SettingsRow title={t('settings_shortcut')} hint={t('settings_shortcut_hint')}>
              <SettingsChoice label={t('settings_shortcut')} value={preferences.globalShortcut} onChange={(value) => onPreferenceChange('globalShortcut', value)} options={[{ value: false, label: t('settings_alerts_off') }, { value: true, label: t('settings_alerts_on') }]} />
            </SettingsRow>
            <SettingsRow title={t('settings_autostart')} hint={t('settings_autostart_hint')}>
              <SettingsChoice label={t('settings_autostart')} value={preferences.autostart} onChange={(value) => onPreferenceChange('autostart', value)} options={[{ value: false, label: t('settings_alerts_off') }, { value: true, label: t('settings_alerts_on') }]} />
            </SettingsRow>
            {nativeError && <p className="settings-form-error" role="alert">{nativeError}</p>}
            <SettingsRow title={t('settings_notifications')} hint={t('settings_notifications_hint')}>
              <SettingsChoice label={t('settings_notifications')} value={preferences.notifications} onChange={(value) => onPreferenceChange('notifications', value)} options={[{ value: 'on', label: t('settings_alerts_on') }, { value: 'off', label: t('settings_alerts_off') }]} />
            </SettingsRow>
            <SettingsRow title={t('settings_quiet_hours')} hint={t('settings_quiet_hours_hint')}>
              <SettingsChoice label={t('settings_quiet_hours')} value={preferences.quietHours} onChange={(value) => onPreferenceChange('quietHours', value)} options={[{ value: false, label: t('settings_alerts_off') }, { value: true, label: t('settings_alerts_on') }]} />
            </SettingsRow>
            <LanSettings lang={lang} lan={lan} onToggle={onLanToggle} />
          </SettingsSection>

          <SettingsSection id="about" title={t('settings_group_about')} hint={t('settings_about_hint')}>
            <SettingsRow title={t('settings_version')}>
              <span className="settings-version">QuotaKeeper · v0.1</span>
            </SettingsRow>
            <SettingsRow title={t('welcome_reopen')} hint={t('welcome_intro')}>
              <button type="button" className="settings-inline-action" onClick={onShowWelcome}>{t('welcome_reopen')}</button>
            </SettingsRow>
            <SettingsRow title={t('settings_reset_all')}>
              <button type="button" className="settings-inline-action" onClick={onResetAppearance}>{t('settings_reset_all')}</button>
            </SettingsRow>
            <SettingsRow title={t('settings_quit')}>
              <button type="button" className="settings-inline-action is-danger" onClick={() => sendCommand({ cmd: 'quit' })}>{t('settings_quit')}</button>
            </SettingsRow>
          </SettingsSection>
        </div>
      </div>
    </div>
  );
}
