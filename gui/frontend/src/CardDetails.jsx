// 套餐卡展开详情:官方可用模型清单(复制/导入) + API key 打码(悬停显示/点击复制)。
// key 只存在内存 Map,不进 localStorage/日志;悬停前也不读凭据管理器之外的东西。
import React, { useEffect, useRef, useState } from 'react';
import { PLAN_MODELS, PLAN_MODELS_VERIFIED, STR } from './ui-data.js';
import { sendCommand } from './gui-bridge.js';
import { motion } from 'motion/react';

// 会话内 key 缓存:仅在渲染进程内存,页面刷新即清
const keyCache = new Map();

// WebView2 里 clipboard API 需要用户手势;execCommand 兜底覆盖边缘场景
function writeClipboard(text) {
  return navigator.clipboard.writeText(text).catch(() => {
    const area = document.createElement('textarea');
    area.value = text;
    area.style.position = 'fixed';
    area.style.opacity = '0';
    document.body.appendChild(area);
    area.select();
    document.execCommand('copy');
    area.remove();
  });
}

// 应用存有凭据的套餐(opencode-go/goat/glm/kimi);其余走登录态,无 key 可展示
const KEY_PROVIDERS = new Set(['opencode-go', 'goat', 'glm', 'kimi']);

export function maskKey(key) {
  if (!key) return '';
  if (key.length <= 10) return '•'.repeat(key.length);
  return `${key.slice(0, 4)}${'•'.repeat(Math.min(24, key.length - 8))}${key.slice(-4)}`;
}

export default function CardDetails({ provider, lang, imported, clipboardClear, onPreferenceChange }) {
  const t = (key) => STR[key][lang];
  const models = PLAN_MODELS[provider] || [];
  const hasKey = KEY_PROVIDERS.has(provider);
  const [key, setKey] = useState(() => keyCache.get(provider) ?? undefined); // undefined=查询中, null=确认未存
  const [revealed, setRevealed] = useState(false);
  const [copied, setCopied] = useState('');
  const flash = useRef(null);
  const importedList = imported || [];
  const alreadyImported = models.length > 0 && models.every((model) => importedList.includes(model));

  useEffect(() => {
    if (hasKey && keyCache.get(provider) == null) {
      sendCommand({ cmd: 'revealProviderKey', provider });
    }
    const onKey = (event) => {
      const { provider: slug, key: value } = event.detail || {};
      if (slug !== provider) return;
      if (value) keyCache.set(slug, value);
      setKey(value ?? null);
    };
    window.addEventListener('provider-key', onKey);
    return () => window.removeEventListener('provider-key', onKey);
  }, [provider, hasKey]);

  const showCopied = (tag) => {
    clearTimeout(flash.current);
    setCopied(tag);
    flash.current = setTimeout(() => setCopied(''), 2500);
  };

  const copyModels = () => {
    writeClipboard(models.join('\n')).then(() => showCopied('models'));
  };
  const importModels = () => {
    sendCommand({ cmd: 'importPlanModels', provider, models });
  };
  const copyKey = () => {
    if (!key) return;
    writeClipboard(key).then(() => showCopied('key'));
    if (clipboardClear) {
      setTimeout(() => {
        navigator.clipboard.writeText('').catch(() => {});
      }, 30_000);
    }
  };

  if (!models.length && !hasKey) return null;
  return (
    <motion.div className="card-details" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.18 }}>
      {models.length > 0 && <div className="card-details-block">
        <div className="card-details-head">
          <span>{t('models_title')}</span>
          <small>{t('models_source')} {PLAN_MODELS_VERIFIED}</small>
        </div>
        <div className="model-chips">
          {models.map((model) => <code key={model} className={importedList.includes(model) ? 'is-imported' : ''}>{model}</code>)}
        </div>
        <div className="card-details-actions">
          <button type="button" onClick={copyModels}>{copied === 'models' ? t('models_copied') : t('models_copy')}</button>
          <button type="button" onClick={importModels} disabled={alreadyImported}>{alreadyImported ? t('models_imported') : t('models_import')}</button>
        </div>
      </div>}
      {hasKey && <div className="card-details-block">
        <div className="card-details-head">
          <span>{t('key_stored')}</span>
          <label className="key-clear-toggle">
            <input type="checkbox" checked={clipboardClear} onChange={(event) => onPreferenceChange?.('clipboardClear', event.target.checked)} />
            <span>{t('key_clip_clear')}</span>
          </label>
        </div>
        {key ? (
          <button
            type="button"
            className={`key-value${revealed ? ' is-revealed' : ''}`}
            title={t('key_mask_hint')}
            onMouseEnter={() => setRevealed(true)}
            onMouseLeave={() => setRevealed(false)}
            onClick={copyKey}
          >
            {revealed ? key : maskKey(key)}
          </button>
        ) : key === null && <span className="key-missing">{t('key_missing')}</span>}
        {copied === 'key' && <span className="key-copied" role="status">{t('key_copy_done')}</span>}
      </div>}
    </motion.div>
  );
}
