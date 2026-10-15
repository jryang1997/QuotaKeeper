import React, { useEffect, useState } from 'react';
import { sendCommand } from './gui-bridge.js';
import { STR } from './ui-data.js';
import { SelectionGroup, SelectionHighlight } from './ui-motion.jsx';

const EMPTY = { enabled: false, port: null, url: null };

const WARNING = [
  '开启后，同一局域网内的任何设备都能读取本机用量（仅剩余额度，不含密钥）。',
  'When enabled, any device on the local network can read this machine\'s usage (remaining quota only, no secrets).',
];

function text(pair, lang) {
  return pair[lang] ?? pair[1];
}

function nativeUrl(lan) {
  return typeof lan.url === 'string' && lan.url ? lan.url : '';
}

function statusText(lang, lan, url) {
  if (lan.enabled !== true) return STR.lan_stopped[lang];
  if (url) return STR.lan_running[lang];
  const port = Number.isInteger(lan.port) ? lan.port : null;
  return lang === 0
    ? `已开启，但没有局域网地址${port == null ? '' : `（端口 ${port}）`}`
    : `Enabled, but no LAN address${port == null ? '' : ` (port ${port})`}`;
}

export default function LanSettings({ lang, lan = EMPTY, onToggle }) {
  const status = lan && typeof lan === 'object' ? lan : EMPTY;
  const t = (key) => STR[key][lang];
  const enabled = status.enabled === true;
  const url = nativeUrl(status);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    sendCommand({ cmd: 'queryNativePreferences' });
  }, []);

  const copyUrl = () => {
    if (!url || !navigator.clipboard?.writeText) return;
    navigator.clipboard.writeText(url).then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    }).catch(() => {});
  };

  return (
    <div className="settings-row">
      <div className="settings-row-copy">
        <div className="settings-row-title">{t('lan_title')}</div>
        <div className="settings-row-hint">{text(WARNING, lang)}</div>
        <div className="settings-row-hint" role="status">
          {statusText(lang, status, url)}
          {url ? <> · <code>{url}</code></> : null}
        </div>
      </div>
      <div className="settings-row-control">
        <SelectionGroup>
          <div className="settings-choice" role="group" aria-label={t('lan_title')}>
            <button type="button" aria-pressed={!enabled} className={`settings-choice-option${!enabled ? ' is-selected' : ''}`} onClick={() => onToggle?.(false)}>
              {!enabled && <SelectionHighlight />}
              <span>{t('lan_off')}</span>
            </button>
            <button type="button" aria-pressed={enabled} className={`settings-choice-option${enabled ? ' is-selected' : ''}`} onClick={() => onToggle?.(true)}>
              {enabled && <SelectionHighlight />}
              <span>{t('lan_on')}</span>
            </button>
          </div>
        </SelectionGroup>
        {url ? <button type="button" className="settings-inline-action" onClick={copyUrl}>{copied ? t('lan_copied') : t('lan_copy')}</button> : null}
        <button type="button" className="settings-inline-action" onClick={() => sendCommand({ cmd: 'queryNativePreferences' })}>{lang === 0 ? '刷新状态' : 'Refresh status'}</button>
      </div>
    </div>
  );
}
