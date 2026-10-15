import { PROVIDERS } from './ui-data.js';
import { CURRENCIES } from './currency.js';

export const GUI_PREFERENCES_KEY = 'token-monitor.gui-preferences.v1';
export const THEME_ORDER = ['system', 'light', 'dark', 'cassette'];
export const DEFAULT_GUI_PREFERENCES = {
  lang: 0,
  theme: 'system',
  typeface: 'sarasa',
  density: 'comfortable',
  motion: 'system',
  currency: 'USD',
  notifications: 'on',
  quietHours: false,
  quotaRemainingView: true,
  closeBehavior: 'tray',
  globalShortcut: false,
  autostart: false,
  pinnedCards: [],
  hiddenCards: [],
  providerColors: {},
  clipboardClear: true,
  sidebarHidden: false,
};

export function readGuiPreferences(storage = globalThis.localStorage) {
  try {
    const stored = JSON.parse(storage.getItem(GUI_PREFERENCES_KEY) || '{}');
    return {
      lang: stored.lang === 1 ? 1 : 0,
      theme: THEME_ORDER.includes(stored.theme) ? stored.theme : DEFAULT_GUI_PREFERENCES.theme,
      typeface: ['sarasa', 'plex'].includes(stored.typeface) ? stored.typeface : DEFAULT_GUI_PREFERENCES.typeface,
      density: ['comfortable', 'compact'].includes(stored.density) ? stored.density : DEFAULT_GUI_PREFERENCES.density,
      motion: ['system', 'reduced'].includes(stored.motion) ? stored.motion : DEFAULT_GUI_PREFERENCES.motion,
      currency: CURRENCIES.includes(stored.currency) ? stored.currency : DEFAULT_GUI_PREFERENCES.currency,
      notifications: ['on', 'off'].includes(stored.notifications) ? stored.notifications : DEFAULT_GUI_PREFERENCES.notifications,
      quietHours: stored.quietHours === true,
      quotaRemainingView: typeof stored.quotaRemainingView === 'boolean' ? stored.quotaRemainingView : DEFAULT_GUI_PREFERENCES.quotaRemainingView,
      closeBehavior: ['tray', 'exit'].includes(stored.closeBehavior) ? stored.closeBehavior : 'tray',
      globalShortcut: stored.globalShortcut === true,
      autostart: stored.autostart === true,
      pinnedCards: Array.isArray(stored.pinnedCards) ? stored.pinnedCards.filter((item) => PROVIDERS.includes(item)) : [],
      hiddenCards: Array.isArray(stored.hiddenCards) ? stored.hiddenCards.filter((item) => PROVIDERS.includes(item)) : [],
      providerColors: Object.fromEntries(Object.entries(stored.providerColors || {}).filter(([key, value]) => PROVIDERS.includes(key) && /^#[0-9a-fA-F]{6}$/.test(value))),
      clipboardClear: stored.clipboardClear !== false,
      sidebarHidden: stored.sidebarHidden === true,
    };
  } catch {
    return DEFAULT_GUI_PREFERENCES;
  }
}

export function saveGuiPreferences(next, storage = globalThis.localStorage) {
  try { storage.setItem(GUI_PREFERENCES_KEY, JSON.stringify(next)); } catch { /* Preferences remain active for this session. */ }
  return next;
}
