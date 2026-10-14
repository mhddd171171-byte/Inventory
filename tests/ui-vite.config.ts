import { mergeConfig } from 'vite';
import appConfig from '../vite.config';

// Keep test optimization independent of app builds and Vitest. Prebundle dynamic
// report dependencies so Vite does not reload the fixture halfway through a flow.
export default mergeConfig(appConfig, {
  cacheDir: 'node_modules/.vite-ui',
  optimizeDeps: {
    entries: ['tests/ui-fixture.html'],
    noDiscovery: true,
    include: ['@tauri-apps/api/mocks', '@tauri-apps/api/core', 'react', 'react-dom/client', 'lucide-react', 'jspdf', 'jspdf-autotable', 'exceljs'],
  },
});
