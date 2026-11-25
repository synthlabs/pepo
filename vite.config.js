import { defineConfig, searchForWorkspaceRoot } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
	plugins: [sveltekit(), tailwindcss()],

	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	//
	// 1. prevent vite from obscuring rust errors
	clearScreen: false,
	// 2. tauri expects a fixed port, fail if that port is not available
	server: {
		port: 1420,
		strictPort: true,
		host: host || false,
		hmr: host
			? {
					protocol: 'ws',
					host,
					port: 1421
				}
			: undefined,
		watch: {
			// 3. tell vite to ignore watching non-frontend trees (Rust targets, local tooling, caches)
			ignored: [
				'**/src-tauri/**',
				'**/target/**',
				'**/bin/**',
				'**/.pnpm-store/**',
				'**/build/**',
				'**/archive/**',
				'**/internal/rust/**',
				'**/utils/rust/**',
				'**/utils/go/**',
				'**/utils/packaging/**',
				'**/utils/.tmp/**',
				'**/.venv/**'
			]
		},
		fs: {
			allow: [
				// search up for workspace root
				searchForWorkspaceRoot(process.cwd())
			]
		}
	}
}));
