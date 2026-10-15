export function aggregatePricing(rows, priceBases) {
  const catalogue = new Map(priceBases.map(({ id, basis }) => [id, basis]));
  const reported = { records: 0, amount: 0 };
  const estimated = { records: 0, amount: 0 };
  const unpriced = { records: 0 };
  const dispositions = { reported, estimated, unpriced };
  if (catalogue.size !== priceBases.length || rows.some((row) => !Object.hasOwn(dispositions, catalogue.get(row.price_basis_id)?.kind))) {
    return { valid: false, reported, estimated, unpriced, amount: null, bases: [] };
  }
  const groups = new Map();
  for (const row of rows) {
    const basis = catalogue.get(row.price_basis_id);
    const { kind } = basis;
    const amount = kind === 'reported' ? row.agg.cost_reported : row.agg.cost_estimated;
    dispositions[kind].records += row.agg.messages;
    if (kind !== 'unpriced') dispositions[kind].amount += amount;
    let group = groups.get(row.price_basis_id);
    if (!group) {
      group = { id: row.price_basis_id, basis, records: 0, amount: kind === 'unpriced' ? null : 0, models: new Set(), rawModels: new Set(), clients: new Set() };
      groups.set(row.price_basis_id, group);
    }
    group.records += row.agg.messages;
    if (kind !== 'unpriced') group.amount += amount;
    group.models.add(row.model);
    group.rawModels.add(row.raw_model);
    group.clients.add(row.client);
  }
  const bases = [...groups.values()].sort((a, b) => a.id - b.id).map((group) => ({
    ...group, models: [...group.models].sort(), rawModels: [...group.rawModels].sort(), clients: [...group.clients].sort(),
  }));
  const amount = unpriced.records > 0 && reported.records + estimated.records === 0
    ? null : reported.amount + estimated.amount;
  return { valid: true, reported, estimated, unpriced, amount, bases };
}

export function snapshotFetchAge(snapshot) {
  const { loaded_at_ms: loaded, fetched_at_ms: fetched, cache_state: state } = snapshot;
  if (state === 'unknown_age' || ![loaded, fetched].every((value) => Number.isInteger(value) && Number.isFinite(new Date(value).getTime()))) return null;
  return loaded >= fetched ? loaded - fetched : null;
}