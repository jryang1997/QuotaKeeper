import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// 相对路径 + 自定义协议:构建产物由 wry 经 app://local 提供
// overlay.html 是悬浮窗独立页(无框架,静态直出)
export default defineConfig({
  plugins: [react()],
  base: './',
  build: {
    outDir: 'dist',
    target: 'chrome120',
    rollupOptions: { input: { main: 'index.html', overlay: 'overlay.html' } },
  },
  server: { port: 5184, strictPort: true },
});
