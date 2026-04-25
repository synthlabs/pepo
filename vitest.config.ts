import { mergeConfig, defineConfig } from 'vitest/config';
import base from './utils/configs/vitest.base';

export default mergeConfig(
	base,
	defineConfig({
		test: {
			include: ['src/**/*.{test,spec}.ts', 'utils/js/**/*.{test,spec}.ts'],
		},
	}),
);
