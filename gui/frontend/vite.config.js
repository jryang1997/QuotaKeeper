import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// 相对路径 + 自定义协议:构建产物由 wry 经 app://local 提供
export default defineConfig({
  plugins: [react()],
  base: './',
  build: { outDir: 'dist', target: 'chrome120' },
  server: { port: 5184, strictPort: true },
});
