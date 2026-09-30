import { sendCommand } from './gui-bridge.js';
import React, { useEffect, useState } from 'react';
import { Text } from '@astryxdesign/core/Text';
import { PROVIDERS, PROVIDER_COLOR_DEFAULTS, STR, fmtCost } from './ui-data.js';
import { CURRENCIES, formatCurrency } from './currency.js';
import { SelectionGroup, SelectionHighlight } from './ui-motion.jsx';

function SettingsChoice({ label, value, onChange, options }) {
  return (
    <SelectionGroup><div className={`settings-choice${options.length > 3 ? ' settings-choice-many' : ''}`} role="group" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
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

function SettingsSection({ title, hint, children, className = '' }) {
  return (
    <section className={`settings-card ${className}`}>
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

export default function SettingsPage({ lang, preferences, currency, onPreferenceChange, onResetAppearance, onResetCards, onShowWelcome, monthlyValue, nativeError, goKeyStored, goKeyFeedback, onSaveGoKey, onDeleteGoKey, goatKeyStored, goatKeyFeedback, onSaveGoatKey, onDeleteGoatKey, deepseekKeyStored, deepseekKeyFeedback, onSaveDeepSeekKey, onDeleteDeepSeekKey }) {
  const t = (key) => STR[key][lang];
  const initialBenchmark = monthlyValue?.list_price_benchmark_usd ?? monthlyValue?.default_list_price_benchmark_usd ?? 40;
  const [benchmarkOverride, setBenchmarkOverride] = useState(null);
  const [planDraft, setPlanDraft] = useState(initialBenchmark.toFixed(2));
  const [planError, setPlanError] = useState('');
  const [saved, setSaved] = useState(false);
  const [goKeyDraft, setGoKeyDraft] = useState('');
  const [goKeyError, setGoKeyError] = useState('');
  const [goatKeyDraft, setGoatKeyDraft] = useState('');
  const [goatKeyError, setGoatKeyError] = useState('');
  const [deepseekKeyDraft, setDeepSeekKeyDraft] = useState('');
  const [deepseekKeyError, setDeepSeekKeyError] = useState('');
  const saveGoKey = (event) => {
    event.preventDefault();
    const key = goKeyDraft.trim();
    if (!key || /\s/.test(key)) { setGoKeyError(t('settings_go_key_invalid')); return; }
    onSaveGoKey(key);
    setGoKeyDraft('');
    setGoKeyError('');
  };
  const saveGoatKey = (event) => {
    event.preventDefault();
    const key = goatKeyDraft.trim();
    if (!key || /\s/.test(key)) { setGoatKeyError(t('settings_go_key_invalid')); return; }
    onSaveGoatKey(key);
    setGoatKeyDraft('');
    setGoatKeyError('');
  };
  const saveDeepSeekKey = (event) => {
    event.preventDefault();
    const key = deepseekKeyDraft.trim();
    if (!key || /\s/.test(key)) { setDeepSeekKeyError(t('settings_go_key_invalid')); return; }
    onSaveDeepSeekKey(key);
    setDeepSeekKeyDraft('');
    setDeepSeekKeyError('');
  };
  useEffect(() => {
    setBenchmarkOverride(null);
    setPlanDraft(initialBenchmark.toFixed(2));
  }, [initialBenchmark]);

  const benchmark = benchmarkOverride?.amount ?? initialBenchmark;
  const isCustom = benchmarkOverride?.isCustom ?? monthlyValue?.benchmark_is_custom ?? false;
  const saveBenchmark = (event) => {
    event.preventDefault();
    const amount = Number(planDraft);
    if (!Number.isFinite(amount) || amount < 0.01 || amount > 1_000_000) {
      setPlanError(t('settings_invalid_price'));
      setSaved(false);
      return;
    }
    sendCommand({ cmd: 'setPlanBenchmark', amount });
    setBenchmarkOverride({ amount, isCustom: true });
    setPlanError('');
    setSaved(true);
  };
  const resetBenchmark = () => {
    const amount = monthlyValue?.default_list_price_benchmark_usd ?? 40;
    sendCommand({ cmd: 'resetPlanBenchmark' });
    setBenchmarkOverride({ amount, isCustom: false });
    setPlanDraft(amount.toFixed(2));
    setPlanError('');
    setSaved(true);
  };

  return (
    <div className="settings-page">
      <header className="settings-intro">
        <Text type="supporting" className="settings-kicker">{t('settings_kicker')}</Text>
        <h1>{t('nav_settings')}</h1>
      </header>

      <div className="settings-grid">
        <SettingsSection title={t('settings_appearance')} className="settings-appearance">
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
              ]}
            />
          </SettingsRow>
          <div className="settings-theme-preview" aria-hidden="true">
            <span className="settings-preview-window"><i /><i /><i /><b /><b /><b /></span>
              <span className="settings-preview-copy"><strong>{t('settings_font_sample')}</strong><small>12,480 tokens · {formatCurrency(3.84, currency)}</small></span>
            <span className="settings-preview-spark" />
          </div>
        </SettingsSection>

        <div className="settings-column">
          <SettingsSection title={t('settings_go_key')} hint={t('settings_go_key_hint')} className="settings-go-key">
            <p className="settings-go-key-status">{goKeyStored ? t('settings_go_key_stored') : t('settings_go_key_missing')}</p>
            <form className="settings-go-key-form" onSubmit={saveGoKey}>
              <input type="password" autoComplete="off" spellCheck="false" value={goKeyDraft} onChange={(event) => { setGoKeyDraft(event.target.value); setGoKeyError(''); }} placeholder={t('settings_go_key_placeholder')} aria-label={t('settings_go_key_placeholder')} />
              <button className="settings-save" type="submit">{t('settings_save')}</button>
              <button className="settings-reset" type="button" disabled={!goKeyStored} onClick={onDeleteGoKey}>{t('settings_go_key_remove')}</button>
            </form>
            {(goKeyError || goKeyFeedback) && <p className="settings-form-error" role="status">{goKeyError || goKeyFeedback}</p>}
          </SettingsSection>
          <SettingsSection title={t('settings_goat_key')} hint={t('settings_goat_key_hint')} className="settings-go-key">
            <p className="settings-go-key-status">{goatKeyStored ? t('settings_go_key_stored') : (lang === 0 ? '尚未保存专用密钥；仍可读取环境变量或 cmdc 登录态。' : 'No dedicated key saved; an environment variable or cmdc login may still work.')}</p>
            <form className="settings-go-key-form" onSubmit={saveGoatKey}>
              <input type="password" autoComplete="off" spellCheck="false" value={goatKeyDraft} onChange={(event) => { setGoatKeyDraft(event.target.value); setGoatKeyError(''); }} placeholder="Command Code API key" aria-label={`${t('settings_goat_key')} API key`} />
              <button className="settings-save" type="submit">{t('settings_save')}</button>
              <button className="settings-reset" type="button" disabled={!goatKeyStored} onClick={onDeleteGoatKey}>{t('settings_go_key_remove')}</button>
            </form>
            {(goatKeyError || goatKeyFeedback) && <p className="settings-form-error" role="status">{goatKeyError || goatKeyFeedback}</p>}
          </SettingsSection>
          <SettingsSection title={lang === 0 ? 'DeepSeek 官方余额' : 'DeepSeek official balance'} hint={lang === 0 ? '使用 DeepSeek 官方 API key 查询账户充值与赠送余额；密钥保存在 Windows 凭据管理器。' : 'Use an official DeepSeek API key to read prepaid and granted balances. The key is stored in Windows Credential Manager.'} className="settings-go-key">
            <p className="settings-go-key-status">{deepseekKeyStored ? t('settings_go_key_stored') : (lang === 0 ? '尚未保存密钥；也可使用 DEEPSEEK_API_KEY 环境变量。' : 'No key saved; DEEPSEEK_API_KEY also works.')}</p>
            <form className="settings-go-key-form" onSubmit={saveDeepSeekKey}>
              <input type="password" autoComplete="off" spellCheck="false" value={deepseekKeyDraft} onChange={(event) => { setDeepSeekKeyDraft(event.target.value); setDeepSeekKeyError(''); }} placeholder="DeepSeek API key" aria-label="DeepSeek API key" />
              <button className="settings-save" type="submit">{t('settings_save')}</button>
              <button className="settings-reset" type="button" disabled={!deepseekKeyStored} onClick={onDeleteDeepSeekKey}>{t('settings_go_key_remove')}</button>
            </form>
            {(deepseekKeyError || deepseekKeyFeedback) && <p className="settings-form-error" role="status">{deepseekKeyError || deepseekKeyFeedback}</p>}
          </SettingsSection>
          <SettingsSection title={t('settings_type')} className="settings-type">
            <SettingsRow title={t('settings_font')}>
              <SettingsChoice
                label={t('settings_font')}
                value={preferences.typeface}
                onChange={(value) => onPreferenceChange('typeface', value)}
                options={[
                  { value: 'sarasa', label: t('font_sarasa') },
                  { value: 'maple', label: t('font_maple') },
                  { value: 'plex', label: t('font_plex') },
                  { value: 'misans', label: t('font_misans') },
                  { value: 'smiley', label: t('font_smiley') },
                  { value: 'pixel', label: t('font_pixel') },
                  { value: 'system', label: t('font_system') },
                ]}
              />
            </SettingsRow>
            <div className={`settings-font-sample${preferences.typeface === 'system' ? '' : ` is-${preferences.typeface}`}`}>
              <span className="settings-font-sample-label">TYPE SPECIMEN · 16</span>
              <div className="settings-font-sample-en">Token usage, at a glance</div>
              <div className="settings-font-sample-zh">令牌用量，一目了然</div>
              <div className="settings-font-sample-numbers">12,480 tokens · {formatCurrency(3.84, currency)}</div>
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
          </SettingsSection>

          <SettingsSection title={t('settings_plan')} hint={t('settings_plan_hint')} className="settings-plan">
            <SettingsRow title={t('settings_currency')} hint={t('settings_currency_hint')}>
              <select className="settings-currency-select" value={preferences.currency} onChange={(event) => onPreferenceChange('currency', event.target.value)} aria-label={t('settings_currency')}>
                {CURRENCIES.map((code) => <option key={code} value={code}>{code} · {new Intl.DisplayNames([lang === 0 ? 'zh-CN' : 'en'], { type: 'currency' }).of(code)}</option>)}
              </select>
              <p className="settings-currency-rate" role="status">{preferences.currency === 'USD' ? t('settings_currency_usd') : currency?.rate > 0 ? `${t('settings_currency_rate')} 1 USD ≈ ${formatCurrency(1, currency)} · ${currency.date || '—'}${currency.stale ? ` · ${t('settings_currency_stale')}` : ''}` : currency?.error ? t('settings_currency_error') : t('settings_currency_loading')}</p>
              <small className="settings-currency-source">{t('settings_currency_source')} <a href="https://frankfurter.dev/" target="_blank" rel="noreferrer">Frankfurter</a></small>
            </SettingsRow>
            <form className="settings-plan-form" onSubmit={saveBenchmark}>
              <label htmlFor="settings-plan-input">{t('settings_monthly')} (USD)</label>
              <div className="settings-plan-input-wrap">
                <span>$</span>
                <input
                  id="settings-plan-input"
                  type="number"
                  min="0.01"
                  max="1000000"
                  step="0.01"
                  required
                  value={planDraft}
                  onChange={(event) => { setPlanDraft(event.target.value); setSaved(false); setPlanError(''); }}
                  aria-label={`${t('settings_monthly')} (USD)`}
                />
                <span className="settings-plan-period">/ mo</span>
              </div>
              <div className="settings-plan-actions">
                <button className="settings-save" type="submit">{saved ? t('settings_saved') : t('settings_save')}</button>
                <button className="settings-reset" type="button" onClick={resetBenchmark} disabled={!isCustom}>{t('settings_default')}</button>
              </div>
              {planError && <p className="settings-form-error" role="alert">{planError}</p>}
            </form>
            <div className="settings-plan-note">
              <span className="settings-plan-note-dot" />
              {isCustom ? `${t('settings_monthly')}: ${fmtCost(benchmark)}` : `${t('settings_default')}: ${fmtCost(benchmark)}`}{preferences.currency !== 'USD' && currency?.rate > 0 ? ` ≈ ${formatCurrency(benchmark, currency)}` : ''}
            </div>
          </SettingsSection>

          <SettingsSection title={t('settings_desktop')} hint={t('settings_desktop_hint')} className="settings-desktop">
            <SettingsRow title={t('settings_close_behavior')}>
              <SettingsChoice label={t('settings_close_behavior')} value={preferences.closeBehavior} onChange={(value) => onPreferenceChange('closeBehavior', value)} options={[{ value: 'tray', label: t('settings_close_tray') }, { value: 'exit', label: t('settings_close_exit') }]} />
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
            <SettingsRow title={t('settings_pace_alerts')} hint={t('settings_pace_alerts_hint')}>
              <SettingsChoice label={t('settings_pace_alerts')} value={preferences.paceAlerts} onChange={(value) => onPreferenceChange('paceAlerts', value)} options={[{ value: true, label: t('settings_alerts_on') }, { value: false, label: t('settings_alerts_off') }]} />
            </SettingsRow>
            <SettingsRow title={t('settings_quiet_hours')} hint={t('settings_quiet_hours_hint')}>
              <SettingsChoice label={t('settings_quiet_hours')} value={preferences.quietHours} onChange={(value) => onPreferenceChange('quietHours', value)} options={[{ value: false, label: t('settings_alerts_off') }, { value: true, label: t('settings_alerts_on') }]} />
            </SettingsRow>
            <button type="button" className="settings-quit" onClick={() => sendCommand({ cmd: 'quit' })}>{t('settings_quit')}</button>
          </SettingsSection>

          <SettingsSection title={t('cards_custom')} hint={t('cards_custom_hint')} className="settings-cards">
            {PROVIDERS.map((provider) => <div className="settings-provider-row" key={provider}>
              <span>{provider}</span>
              <label><span>{lang === 0 ? '颜色' : 'Color'}</span><input type="color" aria-label={`${provider} ${lang === 0 ? '颜色' : 'color'}`} value={preferences.providerColors[provider] || PROVIDER_COLOR_DEFAULTS[provider]} onChange={(event) => onPreferenceChange('providerColors', { ...preferences.providerColors, [provider]: event.target.value })} /></label>
              <button type="button" onClick={() => onPreferenceChange('pinnedCards', preferences.pinnedCards.includes(provider) ? preferences.pinnedCards.filter((item) => item !== provider) : [...preferences.pinnedCards, provider])}>{preferences.pinnedCards.includes(provider) ? t('card_unpin') : t('card_pin')}</button>
              <button type="button" onClick={() => onPreferenceChange('hiddenCards', preferences.hiddenCards.includes(provider) ? preferences.hiddenCards.filter((item) => item !== provider) : [...preferences.hiddenCards, provider])}>{preferences.hiddenCards.includes(provider) ? t('card_show') : t('card_hide')}</button>
            </div>)}
            <div className="settings-card-footer"><button type="button" onClick={onResetCards}>{t('cards_restore')}</button><button type="button" onClick={onShowWelcome}>{t('welcome_reopen')}</button></div>
          </SettingsSection>

          <div className="settings-footer">
            <span>Token Monitor <span className="settings-footer-dot">·</span> v0.1</span>
            <button type="button" onClick={onResetAppearance}>{t('settings_reset_all')}</button>
          </div>
        </div>
      </div>
    </div>
  );
}
