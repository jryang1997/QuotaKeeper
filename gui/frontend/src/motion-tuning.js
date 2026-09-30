// Critically damped springs: retarget from the current presentation and velocity.
export const LAYOUT_SPRING = { type: 'spring', stiffness: 360, damping: 38, mass: 1, restDelta: 0.25, restSpeed: 2 };
export const CONTROL_SPRING = { type: 'spring', stiffness: 500, damping: 45, mass: 1 };
export const NUMBER_SPRING = { type: 'spring', stiffness: 190, damping: 28, mass: 1, restDelta: 0.001, restSpeed: 0.01 };
export const FADE = { duration: 0.14, ease: [0.2, 0, 0, 1] };

export function rubberband(value, min, max, dimension = 240) {
  const bounded = Math.max(min, Math.min(max, value));
  const excess = value - bounded;
  return bounded + (excess * dimension * 0.55) / (dimension + 0.55 * Math.abs(excess));
}

export function releaseVelocity(samples, now) {
  const recent = samples.filter((sample) => now - sample.t <= 100);
  if (recent.length < 2) return { x: 0, y: 0 };
  const first = recent[0];
  const last = recent.at(-1);
  const elapsed = last.t - first.t;
  if (elapsed < 8 || now - last.t > 60) return { x: 0, y: 0 };
  return { x: (last.x - first.x) * 1000 / elapsed, y: (last.y - first.y) * 1000 / elapsed };
}
export const project = (velocity, decelerationRate = 0.99) => (velocity / 1000) * decelerationRate / (1 - decelerationRate);

export function panelRect(panel, gridWidth, columns = 12, gap = 14, rowHeight = 110) {
  const stepX = (gridWidth + gap) / columns;
  const stepY = rowHeight + gap;
  return { x: panel.x * stepX, y: panel.y * stepY, width: panel.w * stepX - gap, height: panel.h * stepY - gap };
}
