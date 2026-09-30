import React, { useEffect, useState } from 'react';
import { animate, useMotionValue } from 'motion/react';
import { NUMBER_SPRING } from './motion-tuning.js';
import { useMotionPreference } from './ui-motion.jsx';

export default function AnimatedNumber({ value, format, className = '', reduceMotion = false }) {
  const numeric = Number.isFinite(value) ? value : 0;
  const current = useMotionValue(numeric);
  const [display, setDisplay] = useState(numeric);
  const reduced = useMotionPreference(reduceMotion);
  useEffect(() => current.on('change', setDisplay), [current]);
  useEffect(() => {
    if (reduced) { current.set(numeric); setDisplay(numeric); return; }
    const animation = animate(current, numeric, NUMBER_SPRING);
    return () => animation.stop();
  }, [numeric, reduced, current]);
  return <span className={`animated-number ${className}`} aria-label={format(numeric)}><span aria-hidden="true">{format(reduced ? numeric : display)}</span></span>;
}
