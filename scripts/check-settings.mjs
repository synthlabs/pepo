#!/usr/bin/env node
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

// Run with node scripts/check-settings.mjs; BROWSER_BIN selects a Chromium browser.
const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.join(root, 'target/settings-check');
const browserPath = [
	process.env.BROWSER_BIN,
	'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
	'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
	'/usr/bin/chromium',
	'/usr/bin/chromium-browser',
	'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
].find((candidate) => candidate && existsSync(candidate));
assert(browserPath, 'Set BROWSER_BIN to an installed Chromium browser');
await mkdir(output, { recursive: true });

await writeFile(
	path.join(output, 'native.svelte.js'),
	`
const params = new URLSearchParams(location.search);
export const page = $state({ url: new URL(location.href), params: location.pathname.includes('/chat/') ? { id: 'maya' } : {} });
export const resolve = value => value;
export const goto = async value => {
  history.pushState({}, '', value); page.url = new URL(value, location.origin);
  page.params = value.includes('/chat/') ? { id: 'maya' } : {};
};
document.addEventListener('click', event => {
  const anchor = event.target instanceof Element ? event.target.closest('a[href]') : null;
  if (!anchor || event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
  if (anchor.origin !== location.origin || (anchor.pathname !== '/app' && !anchor.pathname.startsWith('/app/'))) return;
  event.preventDefault();
  void goto(anchor.pathname + anchor.search);
});
window.addEventListener('popstate', () => {
  page.url = new URL(location.href);
  page.params = location.pathname.includes('/chat/') ? { id: 'maya' } : {};
});
export const isTauriMobile = false;
export const native = window.settingsNative = { delay: 0, fail: false, reject: false, writes: [], states: {}, errors: [] };
export const appErrors = { capture: (...args) => native.errors.push(args) };
export class SyncedState {
  obj = $state({}); ready = $state(false); initialized;
  constructor(key, fallback) {
    let value = structuredClone(fallback ?? {});
    if (key === 'auth_state') value = { phase: 'authorized', token: { user_id: '2', login: 'jerod', profile_image_url: '' } };
    if (key === 'channel_cache') value = { channels: { maya: { display_name: 'Maya', is_live: false } } };
    if (key === 'app_settings') {
      if (!sessionStorage.getItem('settings-check-started')) { localStorage.removeItem('settings-check'); sessionStorage.setItem('settings-check-started', 'true'); }
      value = JSON.parse(localStorage.getItem('settings-check') ?? 'null') ?? value;
      if (params.has('open')) value.layout.sidebar_open = params.get('open') !== 'false';
      if (params.has('theme')) value.appearance.theme = params.get('theme');
      native.states[key] = this;
    }
    this.obj = value;
    this.initialized = new Promise(resolve => setTimeout(() => { this.obj = structuredClone(value); this.ready = true; resolve(); }, params.has('delayed') ? 500 : 0));
  }
  async sync() {
    const value = $state.snapshot(this.obj); native.writes.push(value);
    const fail = native.fail, reject = native.reject;
    await new Promise(resolve => setTimeout(resolve, native.delay));
    if (reject) throw new Error('Simulated native failure');
    if (fail) return false;
    localStorage.setItem('settings-check', JSON.stringify(value)); this.obj = structuredClone(value); return true;
  }
}
export const commands = {
  getFollowedChannels: async () => ({ status: 'ok', data: [{ id: '1', login: 'maya', display_name: 'Maya', profile_image_url: '' }] }),
  getChannelInfo: async () => ({ status: 'ok', data: { broadcaster_id: '1', broadcaster_login: 'maya', broadcaster_name: 'Maya' } }),
  logout: async () => ({ status: 'ok' })
};
export default { debug() {}, info() {}, error() {} };
`
);
await writeFile(
	path.join(output, 'internal.js'),
	`export { default as InternalRoot } from './noop.svelte';`
);
await writeFile(path.join(output, 'noop.svelte'), '');
await writeFile(
	path.join(output, 'inbound.js'),
	`export { default as ReportWizard } from './noop.svelte';`
);
await writeFile(
	path.join(output, 'fixture.svelte'),
	`
<script>
  import Layout from '../../src/routes/app/+layout.svelte';
  import SettingsRoute from '../../src/routes/app/settings/+page.svelte';
  import HomeRoute from '../../src/routes/app/+page.svelte';
  import { page } from './native.svelte.js';
  import { applyThemePreference } from '$lib/settings';
  import { getNormalizedAppSettings } from '$lib/stores/settings.svelte';
  const settings = $derived(getNormalizedAppSettings());
  $effect(() => applyThemePreference(settings.appearance.theme));
</script>
<Layout>
  {#if page.url.pathname === '/app/settings'}<SettingsRoute />
  {:else if page.url.pathname === '/app'}<HomeRoute />
  {:else}<div class="p-4" data-chat-fixture>Chat navigation fixture</div>{/if}
</Layout>
`
);
await writeFile(
	path.join(output, 'main.js'),
	`import { mount } from 'svelte'; import Fixture from './fixture.svelte'; import '../../src/app.css'; mount(Fixture, { target: document.body });`
);
await writeFile(
	path.join(output, 'index.html'),
	'<!doctype html><html><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><script type="module" src="/main.js"></script></body></html>'
);
const mocks = path.join(output, 'native.svelte.js');
await build({
	configFile: false,
	root: output,
	base: '/',
	plugins: [svelte({ configFile: false }), tailwindcss()],
	resolve: {
		alias: [
			{
				find: /^@tauri-apps\/api\/(core|event)$/,
				replacement: path.join(root, 'scripts/fixtures/translation-native.mjs')
			},
			{ find: '$lib/errors/controller', replacement: mocks },
			{ find: /^\$lib\/bindings(?:\.ts)?$/, replacement: mocks },
			{ find: /^\$lib\/tauri(?:\.ts)?$/, replacement: mocks },
			...[
				'$app/state',
				'$app/navigation',
				'$app/paths',
				'$utils/log',
				'tauri-svelte-synced-store'
			].map((find) => ({ find, replacement: mocks })),
			{ find: '$internal', replacement: path.join(output, 'internal.js') },
			{ find: '$utils/inbound', replacement: path.join(output, 'inbound.js') },
			{ find: '$lib', replacement: path.join(root, 'src/lib') },
			{ find: '$utils', replacement: path.join(root, 'utils/js') }
		]
	},
	build: { outDir: path.join(output, 'dist'), emptyOutDir: true }
});
const dist = path.join(output, 'dist');
const server = createServer(async (req, res) => {
	try {
		const url = new URL(req.url, 'http://localhost');
		if (url.pathname === '/pepo.png') {
			res.setHeader('Content-Type', 'image/png');
			res.end(await readFile(path.join(root, 'static/pepo.png')));
			return;
		}
		const filename = path.resolve(
			dist,
			'.' + (url.pathname.startsWith('/assets/') ? url.pathname : '/index.html')
		);
		assert(filename.startsWith(dist + path.sep));
		res.setHeader(
			'Content-Type',
			{ '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css' }[
				path.extname(filename)
			] ?? 'application/octet-stream'
		);
		res.end(await readFile(filename));
	} catch {
		res.writeHead(404).end();
	}
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = spawn(
	browserPath,
	[
		'--headless',
		'--disable-gpu',
		'--no-first-run',
		'--remote-debugging-port=0',
		`--user-data-dir=${path.join(output, 'browser-profile')}`,
		'about:blank'
	],
	{ windowsHide: true }
);
let socket;
let send;
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
try {
	const endpoint = await new Promise((resolve, reject) => {
		let logs = '';
		const timer = setTimeout(() => reject(new Error('Browser startup timed out')), 15000);
		browser.stderr.on('data', (chunk) => {
			logs += chunk;
			const match = logs.match(/DevTools listening on (ws:\/\/\S+)/);
			if (match) {
				clearTimeout(timer);
				resolve(match[1]);
			}
		});
		browser.on('error', (error) => {
			clearTimeout(timer);
			reject(error);
		});
		browser.on('exit', (code) => {
			clearTimeout(timer);
			reject(new Error(`Browser exited ${code}: ${logs}`));
		});
	});
	socket = new WebSocket(endpoint);
	await new Promise((resolve, reject) => {
		socket.onopen = resolve;
		socket.onerror = reject;
	});
	let sequence = 0;
	const pending = new Map();
	const exceptions = new Map();
	socket.onmessage = ({ data }) => {
		const message = JSON.parse(data);
		if (message.method === 'Runtime.exceptionThrown') {
			const errors = exceptions.get(message.sessionId) ?? [];
			errors.push(
				message.params.exceptionDetails.exception?.description ??
					message.params.exceptionDetails.text
			);
			exceptions.set(message.sessionId, errors);
		}
		const request = pending.get(message.id);
		if (!request) return;
		clearTimeout(request.timer);
		pending.delete(message.id);
		if (message.error) request.reject(new Error(JSON.stringify(message.error)));
		else request.resolve(message.result);
	};
	send = (method, params = {}, sessionId) =>
		new Promise((resolve, reject) => {
			const id = ++sequence;
			const timer = setTimeout(() => {
				pending.delete(id);
				reject(new Error(`Timed out: ${method}`));
			}, 10000);
			pending.set(id, { resolve, reject, timer });
			socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
		});
	const evaluate = async (sessionId, expression) => {
		const result = await send(
			'Runtime.evaluate',
			{ expression, awaitPromise: true, returnByValue: true },
			sessionId
		);
		assert(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
		return result.result?.value;
	};
	const waitFor = async (sessionId, expression) => {
		for (let attempt = 0; attempt < 100; attempt++) {
			if (await evaluate(sessionId, expression)) return;
			await delay(50);
		}
		const screenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
		await writeFile(path.join(output, 'failure.png'), Buffer.from(screenshot.data, 'base64'));
		await writeFile(
			path.join(output, 'failure.html'),
			await evaluate(sessionId, 'document.documentElement.outerHTML')
		);
		assert.fail(
			`Timed out waiting for ${expression}; exceptions: ${JSON.stringify(exceptions.get(sessionId))}; translation: ${JSON.stringify(await evaluate(sessionId, `({ calls: window.translationNative?.calls, errors: window.settingsNative?.errors, dialog: document.querySelector('[role="alertdialog"]')?.textContent, focus: document.activeElement?.outerHTML })`))}`
		);
	};
	const click = async (sessionId, selector) => {
		await evaluate(sessionId, `document.querySelector(${JSON.stringify(selector)}).click()`);
		await delay(30);
	};
	const search = async (sessionId, value) => {
		await evaluate(
			sessionId,
			`(() => { const input = document.querySelector('[aria-label="Search settings"]'); input.value = ${JSON.stringify(value)}; input.dispatchEvent(new Event('input', { bubbles: true })); })()`
		);
		await delay(30);
	};
	const cases = [
		{ name: 'expanded-dark', width: 1200, open: true, theme: 'dark' },
		{ name: 'collapsed-dark', width: 1200, open: false, theme: 'dark' },
		{ name: 'expanded-light', width: 1200, open: true, theme: 'light' },
		{ name: 'collapsed-light', width: 1200, open: false, theme: 'light' },
		{ name: 'narrow-expanded', width: 820, open: true, theme: 'dark' },
		{ name: 'mobile-dark', width: 375, open: true, theme: 'dark' },
		{ name: 'mobile-light', width: 320, open: false, theme: 'light' }
	].filter((test) => !process.argv[2] || test.name === process.argv[2]);
	assert(cases.length, 'Unknown settings browser scenario: ' + process.argv[2]);
	for (const test of cases) {
		const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
		const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
		await send('Runtime.enable', {}, sessionId);
		await send('Page.bringToFront', {}, sessionId);
		await send(
			'Emulation.setDeviceMetricsOverride',
			{ width: test.width, height: 800, deviceScaleFactor: 1, mobile: test.width < 768 },
			sessionId
		);
		const params = new URLSearchParams({ open: String(test.open), theme: test.theme });
		await send('Page.navigate', { url: `${origin}/app/settings?${params}` }, sessionId);
		await waitFor(
			sessionId,
			`document.querySelector('#setting-chat-show_badges')?.disabled === false`
		);
		await delay(250);
		const layout = await evaluate(
			sessionId,
			`(() => {
      const shell = document.querySelector('[data-settings-page]'), nav = document.querySelector('[aria-label="Settings categories"]');
      return { overflow: document.documentElement.scrollWidth > innerWidth, shellWidth: shell.clientWidth,
        direction: getComputedStyle(nav).flexDirection, dark: document.documentElement.classList.contains('dark'),
        sidebar: document.querySelector('[data-sidebar="panel"]')?.getBoundingClientRect().width,
        rows: document.querySelectorAll('[data-setting-row]').length };
    })()`
		);
		assert.equal(layout.overflow, false, test.name + ' page overflow');
		assert.equal(layout.rows, 6, test.name + ' Chat essentials');
		assert.equal(
			layout.direction,
			layout.shellWidth < 640 ? 'row' : 'column',
			test.name + ' content-width navigation'
		);
		assert.equal(layout.dark, test.theme === 'dark', test.name + ' theme');
		if (test.width >= 768)
			assert.equal(layout.sidebar, test.open ? 256 : 48, test.name + ' retained sidebar width');
		await search(sessionId, 'connector');
		assert.equal(
			await evaluate(sessionId, `!!document.querySelector('#setting-translation-layout')`),
			true
		);
		await search(sessionId, '');
		assert.equal(
			await evaluate(sessionId, `window.translationNative.calls.includes('status')`),
			false,
			'disabled startup does not connect model management'
		);
		await click(sessionId, '[aria-label="Settings categories"] button:nth-child(3)');
		assert.deepEqual(
			await evaluate(
				sessionId,
				`Array.from(document.querySelectorAll('[data-setting-row]'), row => row.dataset.settingRow)`
			),
			['translation.enabled', 'translation.target_language', 'translation.layout']
		);
		assert.equal(
			await evaluate(sessionId, `!!document.querySelector('[data-model-manager]')`),
			false
		);
		assert.equal(
			await evaluate(sessionId, `window.translationNative.calls.includes('status')`),
			false
		);
		assert.equal(
			await evaluate(sessionId, `document.documentElement.scrollWidth > innerWidth`),
			false
		);
		const capture = async (state) => {
			const screenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
			await writeFile(
				path.join(output, `translation-${state}-${test.name}.png`),
				Buffer.from(screenshot.data, 'base64')
			);
		};
		await capture('basic');
		if (test.name === 'expanded-dark') {
			await evaluate(sessionId, `window.translationNative.emit({ phase: 'unavailable' })`);
			await click(sessionId, '#setting-translation-enabled');
			await waitFor(sessionId, `!!document.querySelector('[data-translation-setup-error]')`);
			assert.equal(
				await evaluate(
					sessionId,
					`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked')`
				),
				'false'
			);
			await capture('error');
			await evaluate(sessionId, `window.translationNative.emit({ phase: 'ready' })`);
		}
		await click(sessionId, '#setting-translation-enabled');
		await waitFor(sessionId, `!!document.querySelector('[data-reviewed-model]')`);
		assert.equal(
			await evaluate(
				sessionId,
				`document.getElementById(document.querySelector('[role="alertdialog"]').getAttribute('aria-describedby')).textContent.replace(/\\s+/g, ' ').trim()`
			),
			'Pepo needs a small language model to translate messages into English.'
		);
		assert.equal(
			await evaluate(sessionId, `window.translationNative.calls.includes('start_install')`),
			false
		);
		assert.equal(
			await evaluate(
				sessionId,
				`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked')`
			),
			'false'
		);
		assert.equal(
			await evaluate(
				sessionId,
				`document.querySelector('[role="alertdialog"]').contains(document.activeElement)`
			),
			true
		);
		await capture('consent');
		await evaluate(
			sessionId,
			`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Cancel').click()`
		);
		await waitFor(sessionId, `!document.querySelector('[role="alertdialog"]')`);
		assert.equal(
			await evaluate(sessionId, `window.translationNative.calls.includes('start_install')`),
			false
		);
		await click(sessionId, '#setting-translation-enabled');
		await waitFor(sessionId, `!!document.querySelector('[data-reviewed-model]')`);
		await evaluate(
			sessionId,
			`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Download & enable').click()`
		);
		await waitFor(sessionId, `!!document.querySelector('[data-translation-setup]')`);
		assert.equal(
			await evaluate(
				sessionId,
				`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked')`
			),
			'false'
		);
		await capture('setup');
		if (test.name === 'expanded-dark') {
			await click(sessionId, '[data-translation-cancel]');
			await waitFor(sessionId, `!document.querySelector('[data-translation-setup]')`);
			assert.equal(
				await evaluate(
					sessionId,
					`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked')`
				),
				'false'
			);
			await click(sessionId, '#setting-translation-enabled');
			await waitFor(sessionId, `!!document.querySelector('[data-reviewed-model]')`);
			await evaluate(
				sessionId,
				`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Download & enable').click()`
			);
			await waitFor(sessionId, `!!document.querySelector('[data-translation-setup]')`);
		}
		await evaluate(sessionId, `window.translationNative.finish()`);
		await waitFor(
			sessionId,
			`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked') === 'true'`
		);
		if (test.name === 'expanded-dark') {
			await evaluate(
				sessionId,
				`(() => { const select = document.querySelector('#setting-translation-target_language'); select.value = 'rus'; select.dispatchEvent(new Event('change', { bubbles: true })); })()`
			);
			await waitFor(sessionId, `!!document.querySelector('[data-reviewed-model]')`);
			assert.equal(
				await evaluate(
					sessionId,
					`document.getElementById(document.querySelector('[role="alertdialog"]').getAttribute('aria-describedby')).textContent.replace(/\\s+/g, ' ').trim()`
				),
				'Pepo needs a small language model to translate messages into Russian.'
			);
			await evaluate(
				sessionId,
				`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Cancel').click()`
			);
			await waitFor(sessionId, `!document.querySelector('[role="alertdialog"]')`);
			assert.equal(
				await evaluate(
					sessionId,
					`document.querySelector('#setting-translation-target_language').value`
				),
				'eng'
			);
		}
		await capture('enabled');
		await click(sessionId, '#setting-translation-enabled');
		await waitFor(sessionId, `window.settingsNative.writes.at(-1)?.translation.enabled === false`);
		await evaluate(
			sessionId,
			`
			window.preparationFlashes = [];
			window.preparationObserver = new MutationObserver(records => {
				for (const record of records) {
					for (const node of record.addedNodes) {
						if (node.textContent?.includes('Preparing translation…')) window.preparationFlashes.push(node.textContent);
					}
				}
			});
			window.preparationObserver.observe(document.body, { childList: true, subtree: true });
		`
		);
		for (let attempt = 0; attempt < 3; attempt++) {
			if (attempt === 1) {
				await click(sessionId, '[aria-label="Settings categories"] button:first-child');
				await click(sessionId, '[aria-label="Settings categories"] button:nth-child(3)');
			}
			await click(sessionId, '#setting-translation-enabled');
			await waitFor(
				sessionId,
				`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked') === 'true'`
			);
			await click(sessionId, '#setting-translation-enabled');
			await waitFor(
				sessionId,
				`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked') === 'false'`
			);
		}
		await delay(350);
		assert.deepEqual(
			await evaluate(sessionId, `window.preparationFlashes`),
			[],
			test.name + ' preparation flash'
		);
		await evaluate(sessionId, `window.preparationObserver.disconnect()`);
		await search(sessionId, 'models');
		await click(sessionId, '[data-open-translation-models]');
		await waitFor(sessionId, `document.querySelectorAll('[data-model-row]').length === 2`);
		for (const field of [
			'translate_history',
			'detection_policy',
			'queue_policy',
			'queue_capacity',
			'max_in_flight'
		]) {
			assert.equal(
				await evaluate(sessionId, `!!document.querySelector('#setting-translation-${field}')`),
				true
			);
		}
		await evaluate(
			sessionId,
			`(() => {
			const input = document.querySelector('#setting-translation-queue_capacity');
			input.value = '257'; input.dispatchEvent(new Event('input', { bubbles: true }));
			input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
		})()`
		);
		await waitFor(
			sessionId,
			`document.querySelector('#setting-translation-queue_capacity').getAttribute('aria-invalid') === 'true'`
		);
		await evaluate(
			sessionId,
			`(() => {
			const input = document.querySelector('#setting-translation-queue_capacity');
			input.value = '32'; input.dispatchEvent(new Event('input', { bubbles: true }));
			input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
			const select = document.querySelector('#setting-translation-detection_policy');
			select.value = 'best_guess'; select.dispatchEvent(new Event('change', { bubbles: true }));
		})()`
		);
		await waitFor(
			sessionId,
			`JSON.parse(localStorage.getItem('settings-check')).translation.queue_capacity === 32 && JSON.parse(localStorage.getItem('settings-check')).translation.detection_policy === 'best_guess'`
		);
		assert.equal(
			await evaluate(sessionId, `!!document.querySelector('#setting-translation-layout')`),
			false
		);
		assert.equal(
			await evaluate(sessionId, `document.activeElement.id`),
			'translation-models',
			'model search focuses its heading'
		);
		assert.equal(
			await evaluate(sessionId, `document.documentElement.scrollWidth > innerWidth`),
			false,
			test.name + ' model list overflow'
		);
		assert.equal(
			await evaluate(
				sessionId,
				`Array.from(document.querySelectorAll('[aria-label="Settings categories"] button')).every(button => { const r = button.getBoundingClientRect(); return r.left >= 0 && r.right <= innerWidth; })`
			),
			true,
			test.name + ' every category remains visible'
		);
		assert.equal(
			await evaluate(sessionId, `!!document.querySelector('#setting-translation-enabled')`),
			false
		);
		const modelsScreenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
		await writeFile(
			path.join(output, `translation-${test.name}.png`),
			Buffer.from(modelsScreenshot.data, 'base64')
		);
		await capture('advanced');
		await evaluate(sessionId, `document.querySelector('[data-settings-content]').scrollTop = 0`);
		await capture('processing');
		await click(sessionId, '[data-model-row="eng:rus"] [data-model-action="install"]');
		await waitFor(sessionId, `!!document.querySelector('[data-reviewed-model]')`);
		assert.equal(
			await evaluate(
				sessionId,
				`document.querySelector('[data-model-installation]')?.textContent.includes('English → Russian') ?? false`
			),
			false,
			'download requires consent'
		);
		assert.equal(
			await evaluate(
				sessionId,
				`document.querySelector('[role="alertdialog"]').contains(document.activeElement)`
			),
			true,
			'confirmation receives focus'
		);
		await evaluate(
			sessionId,
			`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Download').focus()`
		);
		await send(
			'Input.dispatchKeyEvent',
			{
				type: 'keyDown',
				key: 'Enter',
				code: 'Enter',
				windowsVirtualKeyCode: 13,
				nativeVirtualKeyCode: 13,
				text: '\r',
				unmodifiedText: '\r'
			},
			sessionId
		);
		await send(
			'Input.dispatchKeyEvent',
			{
				type: 'keyUp',
				key: 'Enter',
				code: 'Enter',
				windowsVirtualKeyCode: 13,
				nativeVirtualKeyCode: 13
			},
			sessionId
		);
		await waitFor(sessionId, `!!document.querySelector('[data-model-installation]')`);
		assert.equal(
			await evaluate(sessionId, `document.querySelector('[data-model-progress-details]').open`),
			false
		);
		assert.equal(await evaluate(sessionId, `document.querySelector('progress').value`), 25);
		await click(sessionId, '[aria-label="Settings categories"] button:first-child');
		assert.equal(
			await evaluate(sessionId, `!!document.querySelector('[data-model-installation]')`),
			false,
			'progress stays in Translation settings'
		);
		await search(sessionId, 'models');
		await click(sessionId, '[data-open-translation-models]');
		await waitFor(sessionId, `!!document.querySelector('[data-model-installation]')`);
		await evaluate(sessionId, `window.translationNative.finish()`);
		await waitFor(sessionId, `document.querySelector('[data-model-notice="installed"]') !== null`);
		assert.equal(
			await evaluate(sessionId, `window.translationNative.calls.includes('verify_model')`),
			false,
			'opening settings does not verify models'
		);
		await click(sessionId, '[aria-label="Settings categories"] button:first-child');
		if (test.name === 'expanded-dark') {
			await evaluate(sessionId, `window.settingsNative.delay = 180`);
			await click(sessionId, '#setting-chat-show_badges');
			assert.match(
				await evaluate(sessionId, `document.querySelector('[data-settings-status]').ariaLabel`),
				/Saving/
			);
			await waitFor(
				sessionId,
				`document.querySelector('[data-settings-status]').ariaLabel === 'Settings saved'`
			);
			await evaluate(sessionId, `window.settingsNative.fail = true`);
			await click(sessionId, '#setting-chat-show_badges');
			await waitFor(
				sessionId,
				`document.querySelector('[data-settings-status]').ariaLabel.includes('Save failed')`
			);
			assert.equal(
				await evaluate(
					sessionId,
					`document.querySelector('#setting-chat-show_badges').getAttribute('aria-checked')`
				),
				'false'
			);
			await evaluate(sessionId, `document.querySelector('[data-settings-status]').focus()`);
			await waitFor(sessionId, `!!document.querySelector('[data-settings-status-tooltip]')`);
			await evaluate(
				sessionId,
				`window.settingsNative.fail = false; window.settingsNative.delay = 0`
			);
			await click(sessionId, '[aria-label="Reset settings"] > button');
			await waitFor(sessionId, `!!document.querySelector('[role="alertdialog"]')`);
			await evaluate(
				sessionId,
				`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Cancel').click()`
			);
			await search(sessionId, 'emote size');
			await evaluate(
				sessionId,
				`(() => { const input = document.querySelector('#setting-emotes-inline_emote_px'); input.value = '36'; input.dispatchEvent(new Event('input', { bubbles: true })); input.focus(); })()`
			);
			await send(
				'Input.dispatchKeyEvent',
				{ type: 'keyDown', key: 'Enter', code: 'Enter' },
				sessionId
			);
			await send(
				'Input.dispatchKeyEvent',
				{ type: 'keyUp', key: 'Enter', code: 'Enter' },
				sessionId
			);
			await waitFor(
				sessionId,
				`window.settingsNative.writes.at(-1)?.emotes.inline_emote_px === 36`
			);
			await waitFor(
				sessionId,
				`document.querySelector('[data-settings-status]').ariaLabel === 'Settings saved'`
			);
			await send('Page.reload', {}, sessionId);
			await waitFor(
				sessionId,
				`document.querySelector('#setting-chat-show_badges')?.disabled === false`
			);
			await search(sessionId, 'emote size');
			assert.equal(
				await evaluate(
					sessionId,
					`document.querySelector('#setting-emotes-inline_emote_px').value`
				),
				'36',
				'saved mock-native value survives reload'
			);
			await click(sessionId, '[aria-label="Settings categories"] button:nth-child(2)');
			await click(sessionId, '[aria-controls="settings-more-emotes"]');
			await click(sessionId, '[aria-label="Move BTTV up"]');
			await waitFor(
				sessionId,
				`document.querySelector('[data-provider-priority]')?.dataset.providerPriority === 'bttv'`
			);
			const ordered = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
			await writeFile(path.join(output, 'emotes-more.png'), Buffer.from(ordered.data, 'base64'));
			await search(sessionId, '');
			await click(sessionId, '[aria-label="Settings categories"] button:first-child');
		}
		if (test.name === 'collapsed-dark' || test.name === 'expanded-light') {
			await send('Page.navigate', { url: `${origin}/app/chat/maya?${params}` }, sessionId);
			await waitFor(sessionId, `!!document.querySelector('[data-chat-fixture]')`);
			await evaluate(
				sessionId,
				`document.querySelector('[data-sidebar="footer"] [data-sidebar="menu-button"]').focus()`
			);
			await send(
				'Input.dispatchKeyEvent',
				{ type: 'keyDown', key: 'Enter', code: 'Enter' },
				sessionId
			);
			await send(
				'Input.dispatchKeyEvent',
				{ type: 'keyUp', key: 'Enter', code: 'Enter' },
				sessionId
			);
			await waitFor(
				sessionId,
				`Array.from(document.querySelectorAll('[role="menuitem"]')).some(item => item.textContent.trim() === 'Settings')`
			);
			await evaluate(
				sessionId,
				`Array.from(document.querySelectorAll('[role="menuitem"]')).find(item => item.textContent.trim() === 'Settings').focus()`
			);
			await send(
				'Input.dispatchKeyEvent',
				{ type: 'keyDown', key: 'Enter', code: 'Enter' },
				sessionId
			);
			await send(
				'Input.dispatchKeyEvent',
				{ type: 'keyUp', key: 'Enter', code: 'Enter' },
				sessionId
			);
			await waitFor(
				sessionId,
				`!!document.querySelector('[data-settings-page]') && document.querySelector('h1')?.textContent === 'Settings'`
			);
			assert.equal(
				await evaluate(sessionId, 'location.pathname'),
				'/app/settings',
				'profile menu navigation'
			);
		}
		assert.deepEqual(exceptions.get(sessionId) ?? [], [], test.name + ' uncaught browser errors');
		const screenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
		await writeFile(path.join(output, test.name + '.png'), Buffer.from(screenshot.data, 'base64'));
		if (test.width >= 768) {
			const inputWidth = await evaluate(
				sessionId,
				`document.querySelector('[data-sidebar-search] input')?.getBoundingClientRect().width`
			);
			for (const source of ['settings', 'chat']) {
				if (source === 'chat') {
					await click(sessionId, '[data-sidebar="content"] a[href="/app/chat/maya"]');
					await waitFor(sessionId, `!!document.querySelector('[data-chat-fixture]')`);
				}
				await waitFor(sessionId, `!!document.querySelector('[data-sidebar-home]')`);
				await evaluate(
					sessionId,
					`document.querySelector('[data-sidebar-search] [data-popover-trigger]')?.click()`
				);
				await waitFor(
					sessionId,
					`!!document.querySelector('[data-sidebar-search] input, #search-floating')`
				);
				await evaluate(
					sessionId,
					`(() => {
          window.homeSidebar = document.querySelector('[data-sidebar="sidebar"]');
          const input = document.querySelector('[data-sidebar-search] input, #search-floating');
          input.value = 'maya'; input.dispatchEvent(new Event('input', { bubbles: true })); input.focus();
        })()`
				);
				if (test.open) {
					await waitFor(sessionId, `!document.querySelector('[data-sidebar-home]')`);
					await evaluate(
						sessionId,
						`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
					);
				}
				await waitFor(sessionId, `!!document.querySelector('[data-sidebar-home]')`);
				await delay(250);
				const keyboard = source === 'chat' ? test.open : !test.open;
				if (keyboard) {
					if (!test.open) {
						for (const type of ['keyDown', 'keyUp']) {
							await send(
								'Input.dispatchKeyEvent',
								{ type, key: 'Escape', code: 'Escape' },
								sessionId
							);
						}
						await waitFor(sessionId, `!document.querySelector('#search-floating')`);
					}
					await evaluate(sessionId, `document.querySelector('[data-sidebar-home]').focus()`);
					await waitFor(
						sessionId,
						`document.activeElement === document.querySelector('[data-sidebar-home]')`
					);
					for (const type of ['keyDown', 'keyUp']) {
						await send('Input.dispatchKeyEvent', { type, key: 'Enter', code: 'Enter' }, sessionId);
					}
				} else {
					const center = await evaluate(
						sessionId,
						`(() => {
            const r = document.querySelector('[data-sidebar-home]').getBoundingClientRect();
            return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
          })()`
					);
					await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...center }, sessionId);
					for (const type of ['mousePressed', 'mouseReleased']) {
						await send(
							'Input.dispatchMouseEvent',
							{ type, ...center, button: 'left', clickCount: 1 },
							sessionId
						);
					}
				}
				await waitFor(
					sessionId,
					`location.pathname === '/app' && !!document.querySelector('img[alt="The project logo"]')`
				);
				assert.equal(
					await evaluate(sessionId, `!!document.querySelector('[data-sidebar-home]')`),
					false,
					test.name + ' Home hidden on home page'
				);
				assert.equal(
					await evaluate(
						sessionId,
						`window.homeSidebar === document.querySelector('[data-sidebar="sidebar"]')`
					),
					true,
					test.name + ' navigation preserves sidebar instance'
				);
				await delay(250);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[data-sidebar="header"]').getBoundingClientRect().height`
					),
					test.open ? 48 : 96,
					test.name + ' no empty Home row'
				);
				if (test.open) {
					assert.equal(
						await evaluate(
							sessionId,
							`document.querySelector('[data-sidebar-search] input').getBoundingClientRect().width`
						),
						inputWidth + 40,
						test.name + ' no empty Home column'
					);
				} else {
					await evaluate(
						sessionId,
						`document.querySelector('[data-sidebar-search] [data-popover-trigger]').click()`
					);
					await waitFor(sessionId, `!!document.querySelector('#search-floating')`);
				}
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[data-sidebar-search] input, #search-floating').value`
					),
					'',
					test.name + ' ' + source + ' to Home clears search'
				);
				if (!test.open) {
					for (const type of ['keyDown', 'keyUp']) {
						await send(
							'Input.dispatchKeyEvent',
							{ type, key: 'Escape', code: 'Escape' },
							sessionId
						);
					}
					await waitFor(sessionId, `!document.querySelector('#search-floating')`);
				}
				await waitFor(
					sessionId,
					`document.querySelector('img[alt="The project logo"]').naturalWidth > 0`
				);
				const home = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
				await writeFile(
					path.join(output, test.name + '-home-from-' + source + '.png'),
					Buffer.from(home.data, 'base64')
				);
				await evaluate(sessionId, 'history.back()');
				await waitFor(
					sessionId,
					`location.pathname === '/app/${source === 'settings' ? 'settings' : 'chat/maya'}' && !!document.querySelector('[data-sidebar-home]')`
				);
			}
			await send('Page.navigate', { url: `${origin}/app?${params}` }, sessionId);
			await waitFor(
				sessionId,
				`!!document.querySelector('img[alt="The project logo"]') && !!document.querySelector('[data-sidebar-search]')`
			);
			assert.equal(
				await evaluate(sessionId, `!!document.querySelector('[data-sidebar-home]')`),
				false,
				test.name + ' Home hidden on direct entry'
			);
			assert.deepEqual(
				exceptions.get(sessionId) ?? [],
				[],
				test.name + ' navigation browser errors'
			);
		}
		console.log('PASS', test.name, JSON.stringify(layout));
		await send('Target.closeTarget', { targetId });
	}
	console.log(`Passed ${cases.length} settings browser scenarios. Screenshots: ${output}`);
} finally {
	if (send) await send('Browser.close').catch(() => browser.kill());
	else browser.kill();
	socket?.close();
	await new Promise((resolve) => server.close(resolve));
}
