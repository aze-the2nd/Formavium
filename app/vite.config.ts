import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Im Browser (Entwicklung im Container) leitet Vite /api an die Dev-Bridge
// weiter. In der Tauri-Hülle läuft dieselbe Oberfläche über IPC.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    host: '0.0.0.0',
    port: 5173,
    strictPort: true,
    proxy: { '/api': 'http://127.0.0.1:8787' },
    // Windows-Bind-Mounts melden im Container keine Dateiänderungen.
    watch: { usePolling: true, interval: 300 },
  },
  build: { target: 'chrome110', outDir: 'dist', chunkSizeWarningLimit: 2000 },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
});
