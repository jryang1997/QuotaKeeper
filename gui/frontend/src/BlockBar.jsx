import React from 'react';

const BAR_BLOCKS = 20;

export default function BlockBar({ pct, color }) {
  const clamped = Math.max(0, Math.min(100, pct));
  const filled = clamped > 0 ? Math.max(1, Math.round(clamped / 100 * BAR_BLOCKS)) : 0;
  return (
    <div className="block-bar" style={{ '--bar-tone': color }} aria-hidden="true">
      {Array.from({ length: BAR_BLOCKS }, (_, i) => (
        <span key={i} className={`block-bar-cell${i < filled ? ' is-filled' : ''}`} />
      ))}
    </div>
  );
}
