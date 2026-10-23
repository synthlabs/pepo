import { svelte } from '@sveltejs/vite-plugin-svelte';
import { mergeConfig, defineConfig } from 'vitest/config';
import base from './utils/configs/vitest.base';
import { fileURLToPath } from 'node:url';

export default mergeConfig(
	base,
	defineConfig({
		plugins: [svelte()],
		resolve: {
			alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) },
			conditions: ['browser']
		},
		test: {
			include: ['src/**/*.{test,spec}.ts', 'utils/js/**/*.{test,spec}.ts']
		}
	})
);
