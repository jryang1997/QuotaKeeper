// 快速跳转:命令表与过滤是纯函数(可测),面板只做渲染与键盘导航。
export function makeCommand({ id, group, label, hint = '', keywords = '', run }) {
  return { id, group, label, hint, keywords, run };
}

// 前缀命中 > 包含 > 关键词命中;同类内保持传入顺序(稳定性由 sort 的稳定性保证)。
export function filterCommands(commands, query) {
  const q = String(query || '').trim().toLowerCase();
  if (!q) return commands;
  return commands
    .map((command, index) => ({ command, index, score: scoreOf(command, q) }))
    .filter((entry) => entry.score > 0)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((entry) => entry.command);
}

function scoreOf(command, q) {
  const label = String(command.label || '').toLowerCase();
  if (label.startsWith(q)) return 3;
  if (label.includes(q)) return 2;
  if (String(command.keywords || '').toLowerCase().includes(q)) return 1;
  if (String(command.hint || '').toLowerCase().includes(q)) return 1;
  return 0;
}

export function moveIndex(current, delta, length) {
  if (length <= 0) return 0;
  return (current + delta + length) % length;
}
