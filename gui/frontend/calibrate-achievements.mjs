// 校准工具:读最后一次 push 缓存(或传入的 push JSON 路径),按生产 evaluate 打出每项价值/目标/进度。
// 用法:node calibrate-achievements.mjs [path/to/gui-last-push.json]
import { readFileSync } from 'node:fs';
import { ledgerFacts, evaluateAchievements } from './src/achievements.js';

const push = JSON.parse(readFileSync(process.argv[2] || 'C:/Users/Jerry/.tui-token-monitor/gui-last-push.json', 'utf8'));
const ledger = push.ledger;
const extras = {
  monthlyValue: push.monthly_value,
  counters: { palette: 0, share: 0, lang: 0, overlay: 0 },
  unlockedCount: 0,
};
const facts = ledgerFacts(ledger);
console.log('== FACTS ==');
for (const [key, value] of Object.entries(facts)) {
  console.log(`${key.padEnd(18)} ${typeof value === 'number' ? Math.round(value * 100) / 100 : value}`);
}
console.log('\n== ACHIEVEMENTS ==');
for (const item of evaluateAchievements(ledger, extras)) {
  const pct = String(Math.round(item.pct * 100)).padStart(3);
  const value = item.value >= 1000 ? item.value.toExponential(2) : Math.round(item.value * 100) / 100;
  console.log(`${item.unlocked ? 'UNLOCK' : `  ${pct}%`}  ${item.id.padEnd(16)} ${String(value).padEnd(12)} / ${item.target}`);
}
console.log('\nmonthly_value =', JSON.stringify(push.monthly_value));
