import React from 'react';
import ModuleBoard from './ModuleBoard.jsx';
import { STR } from './ui-data.js';
import { PANEL_BODIES, PanelErrorBoundary, panelMeta } from './PanelBodies.jsx';
import { PANEL_CATALOG, DEFAULT_PANELS, PANELS_KEY } from './panel-layout.js';

export default function PanelsPage({ ledger, lang, currency }) {
  const catalog = PANEL_CATALOG.map((item) => ({ ...item, title: STR[`panel_${item.id}`][lang], description: STR[`catalog_${item.id}`][lang] }));
  return <ModuleBoard catalog={catalog} defaults={DEFAULT_PANELS} storageKey={PANELS_KEY} lang={lang} modular={false} meta={(id) => panelMeta(id, ledger, lang)} renderModule={(id) => {
    const Body = PANEL_BODIES[id];
    return <PanelErrorBoundary>{ledger && Body ? <Body ledger={ledger} lang={lang} currency={currency} /> : null}</PanelErrorBoundary>;
  }} />;
}
