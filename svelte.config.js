import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/kit/vite';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	onwarn: (warning, handler) => {
		if (warning.code.startsWith('a11y-')) return
		handler(warning)
	},
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: "index.html",
			precompress: false,
			strict: true
		}),
		alias: {
			$lib: 'src/lib',
			$resources: 'src/resources',
			$static: 'static',
		}
	}
};

export default config;
