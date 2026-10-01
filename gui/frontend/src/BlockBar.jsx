import React, { useRef } from 'react';
import { motion } from 'motion/react';
import { LAYOUT_SPRING, FADE } from './motion-tuning.js';
import { useMotionPreference } from './ui-motion.jsx';

const BAR_BLOCKS = 20;
// 大幅切换(口径翻转)才按左→右波浪 stagger;日常刷新 ±1 格即时弹簧,不拖泥带水
const WAVE_THRESHOLD = 3;
const WAVE_STEP = 0.014;

export default function BlockBar({ pct, color }) {
  const reduced = useMotionPreference();
  const clamped = Math.max(0, Math.min(100, pct));
  const filled = clamped > 0 ? Math.max(1, Math.round(clamped / 100 * BAR_BLOCKS)) : 0;
  const prev = useRef(filled);
  const wave = Math.abs(filled - prev.current) > WAVE_THRESHOLD;
  prev.current = filled;
  return (
    <div className="block-bar" style={{ '--bar-tone': color }} aria-hidden="true">
      {Array.from({ length: BAR_BLOCKS }, (_, i) => (
        <motion.span
          key={i}
          className={`block-bar-cell${i < filled ? ' is-filled' : ''}`}
          initial={false}
          animate={{ opacity: i < filled ? 1 : 0.15 }}
          transition={reduced ? FADE : { ...LAYOUT_SPRING, delay: wave ? i * WAVE_STEP : 0 }}
        />
      ))}
    </div>
  );
}
