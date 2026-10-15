// Critically damped springs: retarget from the current presentation and velocity.
export const LAYOUT_SPRING = { type: 'spring', stiffness: 360, damping: 38, mass: 1, restDelta: 0.25, restSpeed: 2 };
export const CONTROL_SPRING = { type: 'spring', stiffness: 500, damping: 45, mass: 1 };
export const NUMBER_SPRING = { type: 'spring', stiffness: 190, damping: 28, mass: 1, restDelta: 0.001, restSpeed: 0.01 };
export const FADE = { duration: 0.14, ease: [0.2, 0, 0, 1] };

