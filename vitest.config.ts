import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [
		svelte(),
		{
			name: 'mock-sveltekit-navigation',
			resolveId(id) {
				if (id === '$app/state' || id === '$app/paths' || id === '$app/navigation') return id;
			}
		}
	],
	resolve: {
		alias: {
			$lib: fileURLToPath(new URL('./src/lib', import.meta.url)),
			$utils: fileURLToPath(new URL('./utils/js', import.meta.url)),
			'$env/static/public': fileURLToPath(
				new URL('./utils/configs/env-static-public-stub.ts', import.meta.url)
			)
		},
		conditions: ['browser']
	},
	test: {
		environment: 'jsdom',
		globals: false,
		setupFiles: ['./tests/support/unit/setup.ts'],
		include: ['tests/unit/**/*.test.ts'],
		exclude: [
			'**/node_modules/**',
			'**/dist/**',
			'**/build/**',
			'**/.svelte-kit/**',
			'**/target/**'
		],
		retry: 0,
		clearMocks: true,
		restoreMocks: true,
		unstubGlobals: true,
		unstubEnvs: true,
		reporters: ['default', 'json'],
		outputFile: { json: 'target/test-results/vitest.json' }
	}
});
