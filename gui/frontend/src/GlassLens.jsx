import React, { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';

// 液态玻璃折射:药丸轮廓 SDF → 边缘带内沿法线外推,幅值随深度平方增长,
// 归一后 R=X / G=Y 通道编码(128=中性),喂给 feDisplacementMap。
// 静态贴图仅随元素尺寸重建;缩放幅度由 filter 的 scale 承担。
export function buildStadiumDisplacementMap(width, height, bezel = 14) {
  const w = Math.max(2, Math.ceil(width));
  const h = Math.max(2, Math.ceil(height));
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d');
  const image = ctx.createImageData(w, h);
  const data = image.data;
  for (let i = 0; i < w * h; i += 1) {
    const o = i * 4;
    data[o] = 128;
    data[o + 1] = 128;
    data[o + 3] = 255;
  }
  const r = Math.min(h / 2, w / 2);
  const spineA = r;
  const spineB = Math.max(r, w - r);
  let peak = 0;
  const lens = [];
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const px = Math.min(Math.max(x + 0.5, spineA), spineB);
      const dx = x + 0.5 - px;
      const dy = y + 0.5 - h / 2;
      const dist = Math.hypot(dx, dy) || 1e-6;
      const inside = r - dist; // 距边界深度,>0 在轮廓内
      if (inside <= 0 || inside > bezel) continue;
      const t = 1 - inside / bezel; // 0=内沿 1=边界
      const m = t ** 1.7;
      if (m > peak) peak = m;
      lens.push([y * w + x, dx / dist, dy / dist, m]);
    }
  }
  for (const [i, nx, ny, m] of lens) {
    const o = i * 4;
    data[o] = 128 + (nx * m / peak) * 127;
    data[o + 1] = 128 + (ny * m / peak) * 127;
  }
  ctx.putImageData(image, 0, 0);
  return { url: canvas.toDataURL(), width: w, height: h, scale: bezel * 0.72 };
}

// 渲染一枚 <filter> 并在贴图就绪/元素改尺寸时回报其 id。
// onValue 收到 filter id(不含 #);贴图未就绪前不回报,Dock 用 CSS 毛玻璃兜底。
export function LiquidGlassFilter({ targetRef, bezel = 14, onValue }) {
  const filterId = `glass-lens-${useId().replace(/[^a-zA-Z0-9_-]/g, '')}`;
  const [map, setMap] = useState(null);
  const onValueRef = useRef(onValue);
  onValueRef.current = onValue;

  useLayoutEffect(() => {
    const el = targetRef.current;
    if (!el) return undefined;
    let frame = 0;
    const rebuild = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const rect = el.getBoundingClientRect();
        if (rect.width < 8 || rect.height < 8) return;
        setMap(buildStadiumDisplacementMap(rect.width, rect.height, bezel));
      });
    };
    rebuild();
    const observer = new ResizeObserver(rebuild);
    observer.observe(el);
    return () => { cancelAnimationFrame(frame); observer.disconnect(); };
  }, [targetRef, bezel]);

  useEffect(() => {
    if (map) onValueRef.current(filterId);
  }, [map, filterId]);

  return (
    <svg className="svg-defs" aria-hidden="true" focusable="false">
      <filter id={filterId} filterUnits="userSpaceOnUse" colorInterpolationFilters="sRGB">
        {map && <feImage href={map.url} x="0" y="0" width={map.width} height={map.height} preserveAspectRatio="none" result="lens-map" />}
        {map && <feGaussianBlur in="lens-map" stdDeviation="1.1" result="lens-soft" />}
        {map && <feDisplacementMap in="SourceGraphic" in2="lens-soft" scale={map.scale} xChannelSelector="R" yChannelSelector="G" />}
      </filter>
    </svg>
  );
}
