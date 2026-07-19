import { svelte } from '@sveltejs/vite-plugin-svelte';
import { mergeConfig, defineConfig } from 'vitest/config';
import base from './utils/configs/vitest.base';

export default mergeConfig(
	base,
	defineConfig({
		plugins: [svelte()],
		resolve: {
			conditions: ['browser']
		},
		test: {
			include: ['src/**/*.{test,spec}.ts', 'utils/js/**/*.{test,spec}.ts']
		}
	})
);
