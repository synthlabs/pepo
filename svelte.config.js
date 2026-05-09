import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { existsSync } from 'node:fs';

const internalEnabled = process.env.ENABLE_INTERNAL === '1' && existsSync('./internal/frontend/index.ts');
const internalEntry = internalEnabled ? './internal/frontend/index.ts' : './src/lib/internal/index.ts';
const internalDirectory = internalEnabled ? './internal/frontend' : './src/lib/internal';

/** @type {import('@sveltejs/kit').Config} */
const config = {
    // Consult https://svelte.dev/docs/kit/integrations
    // for more information about preprocessors
    preprocess: vitePreprocess(),

    kit: {
        // adapter-auto only supports some environments, see https://svelte.dev/docs/kit/adapter-auto for a list.
        // If your environment is not supported, or you settled on a specific environment, switch out the adapter.
        // See https://svelte.dev/docs/kit/adapters for more information about adapters.
        adapter: adapter({
            pages: 'build',
            assets: 'build',
            fallback: "index.html",
            precompress: false,
            strict: true
        }),
        alias: {
            '$utils': './utils/js',
            '$utils/*': './utils/js/*',
            '$internal': internalEntry,
            '$internal/*': `${internalDirectory}/*`,
        }
    }
};

export default config;
