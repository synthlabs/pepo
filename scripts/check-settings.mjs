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
export const isTauriMobile = false;
export const native = window.settingsNative = { delay: 0, fail: false, reject: false, writes: [], states: {} };
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
  import { page } from './native.svelte.js';
  import { applyThemePreference } from '$lib/settings';
  import { getNormalizedAppSettings } from '$lib/stores/settings.svelte';
  const settings = $derived(getNormalizedAppSettings());
  $effect(() => applyThemePreference(settings.appearance.theme));
</script>
<Layout>
  {#if page.url.pathname === '/app/settings'}<SettingsRoute />
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
		assert.fail(
			`Timed out waiting for ${expression}; exceptions: ${JSON.stringify(exceptions.get(sessionId))}`
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
	];
	for (const test of cases) {
		const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
		const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
		await send('Runtime.enable', {}, sessionId);
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
			await evaluate(sessionId, `!!document.querySelector('#setting-chat-translation_layout')`),
			true
		);
		await search(sessionId, '');
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
