import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	resolve: {
		alias: {
			$utils: fileURLToPath(new URL('../../../utils/js', import.meta.url)),
			'$env/static/public': fileURLToPath(
				new URL('../../../utils/configs/env-static-public-stub.ts', import.meta.url)
			)
		}
	},
	test: {
		environment: 'jsdom',
		globals: false,
		setupFiles: ['./tests/support/unit/setup.ts'],
		include: ['utils/js/**/*.test.ts'],
		exclude: [
			'**/node_modules/**',
			'**/dist/**',
			'**/build/**',
			'**/.svelte-kit/**',
			'**/target/**',
			'utils/js/translate/**'
		],
		retry: 0,
		clearMocks: true,
		restoreMocks: true,
		unstubGlobals: true,
		unstubEnvs: true,
		reporters: ['default', 'json'],
		outputFile: { json: 'target/test-results/vitest-shared.json' }
	}
});
