import React, { forwardRef } from 'react';
import { AnimatePresence, motion, useIsPresent } from 'motion/react';
import { useMotionPreference } from './ui-motion.jsx';
import { FADE, LAYOUT_SPRING } from './motion-tuning.js';

const PageSurface = forwardRef(function PageSurface({ children, reduced }, ref) {
  const present = useIsPresent();
  return <motion.div ref={ref} className="page-content" inert={!present ? true : undefined}
    initial={{ opacity: 0, y: reduced ? 0 : 6 }} animate={{ opacity: 1, y: 0 }}
    exit={{ opacity: 0, y: reduced ? 0 : 6 }}
    transition={{ y: LAYOUT_SPRING, opacity: FADE }} style={{ pointerEvents: present ? 'auto' : 'none' }}>
    {children}
  </motion.div>;
});

export default function PageTransition({ page, children, reduceMotion = false }) {
  const reduced = useMotionPreference(reduceMotion);
  return <div className="page-stage"><AnimatePresence mode="popLayout" initial={false}>
    <PageSurface key={page} reduced={reduced}>{children}</PageSurface>
  </AnimatePresence></div>;
}
