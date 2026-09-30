import React, { createContext, useContext, useId, useState, useSyncExternalStore } from 'react';
import { AnimatePresence, LayoutGroup, MotionConfig, motion, useIsPresent } from 'motion/react';
import { CONTROL_SPRING, FADE, LAYOUT_SPRING } from './motion-tuning.js';

const ReducedMotionContext = createContext(false);
let query;
const media = () => query ||= window.matchMedia('(prefers-reduced-motion: reduce)');
const subscribe = (notify) => {
  if (typeof window === 'undefined' || !window.matchMedia) return () => {};
  media().addEventListener('change', notify);
  return () => media().removeEventListener('change', notify);
};
const snapshot = () => typeof window !== 'undefined' && !!window.matchMedia && media().matches;
export function useMotionPreference(explicit = false) {
  const configured = useContext(ReducedMotionContext);
  const system = useSyncExternalStore(subscribe, snapshot, () => false);
  return explicit || configured || system;
}
export function MotionProvider({ reduceMotion, children }) {
  const reduced = useMotionPreference(reduceMotion);
  return <ReducedMotionContext.Provider value={reduced}><MotionConfig reducedMotion={reduced ? 'always' : 'never'} transition={LAYOUT_SPRING}>{children}</MotionConfig></ReducedMotionContext.Provider>;
}

export function SelectionGroup({ children }) {
  return <LayoutGroup id={useId()}>{children}</LayoutGroup>;
}
export function SelectionHighlight() {
  const reduced = useMotionPreference();
  return <motion.span className="choice-highlight" aria-hidden="true" layoutId={reduced ? undefined : 'selection'} transition={reduced ? { duration: 0 } : CONTROL_SPRING} />;
}
export function PopoverSurface({ children, className, ...props }) {
  const reduced = useMotionPreference();
  const present = useIsPresent();
  const hidden = { opacity: 0, scale: reduced ? 1 : 0.97, y: reduced ? 0 : -4 };
  return <motion.div {...props} className={className} inert={present ? undefined : true}
    initial={hidden} animate={{ opacity: 1, scale: 1, y: 0 }} exit={hidden}
    transition={{ ...CONTROL_SPRING, opacity: FADE }}>{children}</motion.div>;
}
function DisclosureBody({ children, id, reduced }) {
  const present = useIsPresent();
  return <motion.div id={id} className="motion-details-content" inert={present ? undefined : true}
    initial={{ height: reduced ? 'auto' : 0, opacity: 0 }} animate={{ height: 'auto', opacity: 1 }}
    exit={{ height: reduced ? 'auto' : 0, opacity: 0 }} transition={{ height: LAYOUT_SPRING, opacity: FADE }}>
    <div className="motion-details-inner">{children}</div>
  </motion.div>;
}
export function MotionDisclosure({ label, className, children }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  const reduced = useMotionPreference();
  return <div className={className}><button type="button" className="motion-details-trigger" aria-expanded={open} aria-controls={id} onClick={() => setOpen((value) => !value)}>
    <span aria-hidden="true">{open ? '▾' : '▸'}</span> {label}
  </button><AnimatePresence initial={false}>{open && <DisclosureBody key="body" id={id} reduced={reduced}>{children}</DisclosureBody>}</AnimatePresence></div>;
}
