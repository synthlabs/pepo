import { createServer as createHttpServer } from 'node:http';
import { realpathSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { fileURLToPath } from 'node:url';
import { svelte, vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { createServer as createViteServer } from 'vite';

const { values } = parseArgs({
	options: {
		host: { type: 'string', default: '127.0.0.1' },
		port: { type: 'string', default: '0' }
	}
});

const root = fileURLToPath(new URL('../../..', import.meta.url));
const supportDir = path.join(root, 'tests/support/browser');
const harnessesDir = path.join(supportDir, 'harnesses');
const internalNoop = path.join(supportDir, 'internal-noop.ts');
const inboundNoop = path.join(supportDir, 'inbound-noop.ts');
const inboundReportNoop = path.join(supportDir, 'inbound-report-noop.ts');
const translationNative = path.join(supportDir, 'translation-native.ts');
const baseAliases = [
	{ find: '$lib', replacement: path.join(root, 'src/lib') },
	{ find: '$utils', replacement: path.join(root, 'utils/js') }
];

function harnessAliases(name) {
	const dir = path.join(harnessesDir, name);
	const native = path.join(dir, 'native.svelte.ts');
	switch (name) {
		case 'chat-engine':
			return [
				{ find: '@tauri-apps/api/core', replacement: native },
				{ find: '@tauri-apps/api/event', replacement: native },
				{ find: /^\$lib\/bindings(?:\.ts)?$/, replacement: native },
				{ find: /^\$lib\/tauri(?:\.ts)?$/, replacement: native },
				...[
					'$app/state',
					'$app/navigation',
					'$app/paths',
					'$utils/log',
					'tauri-svelte-synced-store'
				].map((find) => ({ find, replacement: native })),
				{ find: '$internal', replacement: internalNoop },
				{ find: '$utils/inbound', replacement: inboundNoop },
				...baseAliases
			];
		case 'chat-scroll':
			return [...baseAliases];
		case 'chat-scroll-route':
			return [
				{ find: /^\$lib\/bindings(?:\.ts)?$/, replacement: native },
				...[
					'$app/state',
					'$utils/log',
					'$lib/stores/auth.svelte',
					'$lib/stores/settings.svelte',
					'@tauri-apps/api/event',
					'@tauri-apps/api/core'
				].map((find) => ({ find, replacement: native })),
				...baseAliases
			];
		case 'chat-submit':
			return [
				{ find: /^\$lib\/bindings(?:\.ts)?$/, replacement: native },
				...[
					'$app/state',
					'$utils/log',
					'$utils/updater',
					'$env/static/public',
					'tauri-svelte-synced-store',
					'@tauri-apps/api/event',
					'@tauri-apps/api/core',
					'@tauri-apps/plugin-log'
				].map((find) => ({ find, replacement: native })),
				...baseAliases
			];
		case 'network-ui':
			return [
				{ find: /^\$lib\/bindings(?:\.ts)?$/, replacement: native },
				{ find: /^\$lib\/tauri(?:\.ts)?$/, replacement: native },
				{ find: '$internal', replacement: internalNoop },
				...['$app/state', '$app/paths', '$app/navigation', 'tauri-svelte-synced-store'].map(
					(find) => ({
						find,
						replacement: native
					})
				),
				{ find: '$lib/stores/settings.svelte', replacement: path.join(dir, 'settings.svelte.ts') },
				{ find: '$utils/log', replacement: native },
				...['@tauri-apps/api/event', '@tauri-apps/api/core', '@tauri-apps/plugin-log'].map(
					(find) => ({
						find,
						replacement: native
					})
				),
				...baseAliases
			];
		case 'settings':
			return [
				{ find: /^@tauri-apps\/api\/(core|event)$/, replacement: translationNative },
				{ find: '$lib/errors/controller', replacement: native },
				{ find: /^\$lib\/bindings(?:\.ts)?$/, replacement: native },
				{ find: /^\$lib\/tauri(?:\.ts)?$/, replacement: native },
				...[
					'$app/state',
					'$app/navigation',
					'$app/paths',
					'$utils/log',
					'tauri-svelte-synced-store'
				].map((find) => ({ find, replacement: native })),
				{ find: '$internal', replacement: internalNoop },
				{ find: '$utils/inbound', replacement: inboundReportNoop },
				...baseAliases
			];
		case 'sidebar-header':
			return [
				{ find: '$lib/errors/controller', replacement: native },
				{ find: /^\$lib\/bindings(?:\.ts)?$/, replacement: native },
				...[
					'$app/state',
					'$app/navigation',
					'$app/paths',
					'$utils/log',
					'tauri-svelte-synced-store'
				].map((find) => ({ find, replacement: native })),
				...baseAliases
			];
		case 'user-card':
			return [
				...[
					'$lib/bindings',
					'$lib/stores/settings.svelte',
					'$lib/errors/controller',
					'@tauri-apps/api/window',
					'@tauri-apps/plugin-opener'
				].map((find) => ({ find, replacement: native })),
				...baseAliases
			];
		default:
			throw new Error(`Unknown browser harness: ${name}`);
	}
}

const HARNESSES = new Set([
	'chat-engine',
	'chat-scroll',
	'chat-scroll-route',
	'chat-submit',
	'network-ui',
	'settings',
	'sidebar-header',
	'user-card'
]);

const viteServers = new Map();
const allowDirs = [root, realpathSync(path.join(root, 'node_modules'))];

async function getVite(name) {
	if (!HARNESSES.has(name)) throw new Error(`Unknown harness: ${name}`);
	let promise = viteServers.get(name);
	if (!promise) {
		const dir = path.join(harnessesDir, name);
		promise = createViteServer({
			configFile: false,
			root: dir,
			base: `/__harness/${name}/`,
			publicDir: false,
			cacheDir: path.join(root, 'target/test-results/vite-cache', name),
			logLevel: 'warn',
			plugins: [svelte({ configFile: false, preprocess: vitePreprocess() }), tailwindcss()],
			optimizeDeps: {
				exclude: [
					'tauri-svelte-synced-store',
					'@tauri-apps/api/core',
					'@tauri-apps/api/event',
					'@tauri-apps/api/window',
					'@tauri-apps/plugin-opener',
					'@tauri-apps/plugin-log'
				]
			},
			resolve: {
				alias: harnessAliases(name)
			},
			server: {
				middlewareMode: true,
				hmr: false,
				ws: false,
				watch: null,
				fs: { allow: allowDirs }
			}
		});
		viteServers.set(name, promise);
	}
	return promise;
}

const viteClientStub = `
const sheetsMap = new Map();
export class ErrorOverlay {}
export function updateStyle(id, content) {
	let style = sheetsMap.get(id);
	if (!style) {
		style = document.createElement('style');
		style.setAttribute('type', 'text/css');
		style.setAttribute('data-vite-dev-id', id);
		document.head.appendChild(style);
		sheetsMap.set(id, style);
	}
	if (style.textContent !== content) style.textContent = content;
}
export function removeStyle(id) {
	const style = sheetsMap.get(id);
	if (style) {
		style.remove();
		sheetsMap.delete(id);
	}
}
export function createHotContext() {
	return {
		data: {},
		accept() {},
		acceptExports() {},
		dispose() {},
		prune() {},
		decline() {},
		invalidate() {},
		on() {},
		off() {},
		send() {}
	};
}
export function injectQuery(url, queryToInject) {
	if (url[0] !== '.' && url[0] !== '/') return url;
	const pathname = url.replace(/[?#].*$/, '');
	const { search, hash } = new URL(url, 'http://vite.dev');
	return pathname + '?' + queryToInject + (search ? '&' + search.slice(1) : '') + (hash || '');
}
`;

const server = createHttpServer(async (req, res) => {
	try {
		const url = new URL(req.url ?? '/', 'http://127.0.0.1');
		if (url.pathname === '/pepo.png') {
			res.setHeader('Content-Type', 'image/png');
			res.end(await readFile(path.join(root, 'static/pepo.png')));
			return;
		}
		const assetMatch = url.pathname.match(/^\/__harness\/([^/]+)(\/.*)$/);
		if (assetMatch && HARNESSES.has(assetMatch[1])) {
			if (assetMatch[2] === '/@vite/client') {
				res.setHeader('Content-Type', 'application/javascript; charset=utf-8');
				res.end(viteClientStub);
				return;
			}
			const vite = await getVite(assetMatch[1]);
			vite.middlewares(req, res, () => {
				res.statusCode = 404;
				res.end('Not found');
			});
			return;
		}
		let harness = url.searchParams.get('harness');
		const explicitHarness = Boolean(harness);
		if (!harness && req.headers.cookie) {
			const match = req.headers.cookie.match(/(?:^|;\s*)pepo_harness=([^;]+)/);
			if (match) harness = decodeURIComponent(match[1]);
		}
		if (!harness && req.headers.referer) {
			try {
				harness = new URL(req.headers.referer).searchParams.get('harness');
			} catch {
				harness = null;
			}
		}
		if (harness && HARNESSES.has(harness)) {
			await getVite(harness);
			const htmlPath = path.join(harnessesDir, harness, 'index.html');
			const rawHtml = await readFile(htmlPath, 'utf8');
			const transformed = rawHtml.replace('src="/main.ts"', `src="/__harness/${harness}/main.ts"`);
			if (explicitHarness) {
				res.setHeader('Set-Cookie', `pepo_harness=${encodeURIComponent(harness)}; Path=/`);
			}
			res.setHeader('Content-Type', 'text/html; charset=utf-8');
			res.end(transformed);
			return;
		}
		res.statusCode = 404;
		res.end('Unknown harness');
	} catch (error) {
		console.error(error);
		res.statusCode = 500;
		res.end(String(error));
	}
});

await new Promise((resolve, reject) => {
	server.once('error', reject);
	server.listen(Number(values.port), values.host, resolve);
});

const address = server.address();
if (!address || typeof address === 'string') {
	server.close();
	throw new Error('Browser test server did not bind a TCP port');
}

console.log(`Pepo browser port ${address.port}`);

const shutdown = () => {
	void Promise.all([
		new Promise((resolve) => server.close(resolve)),
		...[...viteServers.values()].map((promise) =>
			promise.then((vite) => vite.close()).catch(() => {})
		)
	]).finally(() => process.exit(0));
};

for (const signal of ['SIGINT', 'SIGTERM']) {
	process.once(signal, shutdown);
}
