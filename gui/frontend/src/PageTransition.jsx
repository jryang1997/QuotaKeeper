import { AnimatePresence, LazyMotion, MotionConfig, domAnimation, m } from 'motion/react';
import { useEffect, useState } from 'react';

export default function PageTransition({ page, children, reduceMotion = false }) {
  const [systemReducedMotion, setSystemReducedMotion] = useState(() =>
    typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches,
  );

  useEffect(() => {
    const preference = window.matchMedia('(prefers-reduced-motion: reduce)');
    const updatePreference = (event) => setSystemReducedMotion(event.matches);
    preference.addEventListener('change', updatePreference);
    return () => preference.removeEventListener('change', updatePreference);
  }, []);

  const reducedMotion = reduceMotion || systemReducedMotion;
  const initial = reducedMotion ? { opacity: 0, y: 0 } : { opacity: 0, y: 7 };
  const exit = reducedMotion ? { opacity: 0, y: 0 } : { opacity: 0, y: -3 };

  return (
    <MotionConfig reducedMotion="user">
      <LazyMotion features={domAnimation} strict>
        <AnimatePresence mode="wait" initial={false}>
          <m.div
            key={page}
            className="page-content"
            initial={initial}
            animate={{ opacity: 1, y: 0 }}
            exit={exit}
            transition={{ duration: reducedMotion ? 0.14 : 0.2, ease: [0.22, 1, 0.36, 1] }}
          >
            {children}
          </m.div>
        </AnimatePresence>
      </LazyMotion>
    </MotionConfig>
  );
}
