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

// Run with node scripts/check-chat-engine.mjs; BROWSER_BIN selects a Chromium browser.
const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.join(root, 'target/chat-engine-check');
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
export const page = $state({ url: new URL(location.href), params: { id: 'rooma' } });
export const resolve = value => value;
export const goto = async value => {
  history.pushState({}, '', value); page.url = new URL(value, location.origin);
  page.params = value.includes('/chat/') ? { id: value.split('/').at(-1) } : {};
};
export const isTauriMobile = false;
export class Channel { constructor(callback) { this.onmessage = callback; } }
export const invoke = async command => { throw new Error('Unexpected native invoke in chat engine fixture: ' + command); };
window.__TAURI_INTERNALS__ = {};
export const native = window.chatNative = { feeds: new Map(), closed: [], acknowledgements: [], metadataRequests: 0, delays: {}, resolutionErrors: {}, roomFailures: {} };
let sequence = 0;
const histories = new Map();
const channelSequences = new Map();
const info = channel => ({ broadcaster_id: channel, broadcaster_login: channel, broadcaster_name: channel,
  game_id: '', game_name: '', broadcaster_language: 'en', title: channel, description: '', tags: [], delay: 0, is_branded_content: false, profile_image_url: '' });
const message = (channel, text) => {
  const channel_sequence = channelSequences.get(channel) ?? 0;
  channelSequences.set(channel, channel_sequence + 1);
  return { ts: '2026-10-25T00:00:00Z', broadcaster_user_id: channel,
    broadcaster_user_login: channel, broadcaster_user_name: channel, chatter_user_id: 'user', chatter_user_name: 'User',
    message_id: String(++sequence), index: sequence, channel_sequence, text, fragments: [{ Text: { index: 0, text } }],
    message_type: 'text', badges: [], color: '#ffffff', translation: null };
};
for (const channel of ['rooma', 'roomb']) histories.set(channel, []);
for (const i of [1,2,3]) for (const channel of ['rooma', 'roomb']) histories.get(channel).push(message(channel, channel + '-history-' + i));
native.push = (channel, text) => {
  const msg = message(channel, text); const history = histories.get(channel) ?? [];
  history.push(msg); histories.set(channel, history.slice(-(native.settings?.obj.chat.message_limit ?? 500)));
  for (const feed of native.feeds.values()) if (feed.channel === channel) feed.onmessage({ cursor: sequence, event: { type: 'message', message: msg } });
};
native.translate = (channel, text) => {
  const msg = histories.get(channel).at(-1);
  const update = { message_id: msg.message_id, translation: { source_language: 'ru', target_language: 'en', translated_text: text } };
  msg.translation = update.translation;
  const cursor = ++sequence;
  for (const feed of native.feeds.values()) if (feed.channel === channel) feed.onmessage({ cursor, event: { type: 'translation', update } });
};
native.go = channel => goto(channel ? '/app/chat/' + channel : '/app/settings');
native.reset = () => {
  for (const feed of native.feeds.values()) feed.onmessage({ cursor: sequence, event: { type: 'reset', messages: structuredClone(histories.get(feed.channel) ?? []) } });
};
export class SyncedState {
  obj = $state({}); ready = $state(true);
  constructor(key, fallback) {
    this.obj = structuredClone(fallback ?? {});
    if (key === 'auth_state') this.obj = { phase: 'authorized', token: { user_id: '2', login: 'jerod', profile_image_url: '' } };
    if (key === 'channel_cache') this.obj = { channels: { rooma: { display_name: 'Room A', is_live: false }, roomb: { display_name: 'Room B', is_live: true } } };
    if (key === 'app_settings') native.settings = this;
  }
  async sync() {
    this.obj = structuredClone($state.snapshot(this.obj));
    if (this === native.settings) for (const [channel, history] of histories) histories.set(channel, history.slice(-this.obj.chat.message_limit));
    return true;
  }
}
export const commands = {
  async getChatFeedContext() { return { status: 'ok', data: { epoch: 'browser-page' } }; },
  async subscribeChat(channel, request, sink) {
    if (native.resolutionErrors[channel]) return { status: 'error', error: native.resolutionErrors[channel] };
    native.feeds.set(request, { channel, onmessage: sink.onmessage });
    sink.onmessage({ cursor: sequence, event: { type: 'snapshot', messages: structuredClone(histories.get(channel) ?? []) } });
    if (native.roomFailures[channel]) sink.onmessage({ cursor: sequence, event: { type: 'unavailable', error: native.roomFailures[channel] } });
    await new Promise(resolve => setTimeout(resolve, native.delays[channel] ?? 0));
    return { status: 'ok', data: { broadcaster_id: channel, broadcaster_login: channel, broadcaster_name: channel } };
  },
  async unsubscribeChat(request) { const feed = native.feeds.get(request); if (feed) native.closed.push(feed); native.feeds.delete(request); },
  async acknowledgeChat(request, cursor) { native.acknowledgements.push({request, cursor}); },
  async getChannelInfo(channel) { native.metadataRequests++; return { status: 'ok', data: info(channel) }; },
  async getFollowedChannels() { return { status:'ok', data: ['rooma','roomb'].map(login => ({ id: login, login, display_name: login, profile_image_url: '' })) }; },
  async getChatEngineStatus() { return { connected:true, joined:2, eligible:2, lingering:1, pending:0, limited:0, failures:[] }; },
  async searchEmotes() { return { status:'ok', data:[] }; },
  async logout() { return { status:'ok' }; }
};
export const listen = async () => () => {};
export default { debug() {}, info() {}, warn() {}, error() {} };
`
);
await writeFile(
	path.join(output, 'internal.js'),
	"export { default as InternalRoot } from './noop.svelte';"
);
await writeFile(
	path.join(output, 'inbound.js'),
	`export { default as ReportWizard } from './noop.svelte';
export { ErrorController, errorDetails, logHandledError } from '../../utils/js/inbound/index.ts';`
);
await writeFile(path.join(output, 'noop.svelte'), '');
await writeFile(
	path.join(output, 'fixture.svelte'),
	`
<script>
  import Layout from '../../src/routes/app/+layout.svelte';
  import Chat from '../../src/routes/app/chat/[id]/+page.svelte';
  import Settings from '../../src/routes/app/settings/+page.svelte';
  import { page } from './native.svelte.js';
  import { applyThemePreference } from '$lib/settings';
  import { getNormalizedAppSettings } from '$lib/stores/settings.svelte';
  const settings = $derived(getNormalizedAppSettings());
  $effect(() => applyThemePreference(settings.appearance.theme));
</script>
<Layout>{#if page.params.id}<Chat />{:else}<Settings />{/if}</Layout>
`
);
await writeFile(
	path.join(output, 'main.js'),
	"import { mount } from 'svelte'; import Fixture from './fixture.svelte'; import '../../src/app.css'; mount(Fixture, { target: document.body });"
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
			{ find: '@tauri-apps/api/core', replacement: mocks },
			{ find: '@tauri-apps/api/event', replacement: mocks },
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

	const readRows = (sessionId) =>
		evaluate(
			sessionId,
			`Array.from(document.querySelectorAll('[data-chat-message-index]')).map(row => ({
    index: row.dataset.chatMessageIndex,
    background: getComputedStyle(row).backgroundColor,
    shade: row.classList.contains('bg-content-primary') ? 'primary' : row.classList.contains('bg-content-secondary') ? 'secondary' : null
  }))`
		);
	const assertAlternating = (rows) => {
		assert(rows.length > 1, 'Expected multiple message rows');
		assert.equal(
			new Set(rows.map((row) => row.background)).size,
			2,
			'Expected two painted backgrounds'
		);
		for (let i = 1; i < rows.length; i++) {
			assert.notEqual(
				rows[i].background,
				rows[i - 1].background,
				'Adjacent messages must have different backgrounds'
			);
		}
	};
	const assertStable = (before, after) => {
		const previous = new Map(before.map((row) => [row.index, row]));
		let shared = 0;
		for (const row of after) {
			const old = previous.get(row.index);
			if (!old) continue;
			shared++;
			assert.equal(row.shade, old.shade, 'Retained messages must keep their shade');
			assert.equal(
				row.background,
				old.background,
				'Retained messages must keep their painted background'
			);
		}
		assert(shared > 0, 'Expected retained messages for stability comparison');
	};

	for (const { width, theme } of [1200, 375].flatMap((width) =>
		['dark', 'light'].map((theme) => ({ width, theme }))
	)) {
		const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
		const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
		await send('Runtime.enable', {}, sessionId);
		await send(
			'Emulation.setDeviceMetricsOverride',
			{ width, height: 800, deviceScaleFactor: 1, mobile: width < 768 },
			sessionId
		);
		await send('Page.navigate', { url: origin + '/app/chat/rooma' }, sessionId);
		await waitFor(sessionId, "document.querySelectorAll('[data-chat-message-index]').length === 3");
		await evaluate(
			sessionId,
			`chatNative.settings.obj.appearance.theme = '${theme}'; chatNative.settings.sync()`
		);
		await waitFor(sessionId, `document.documentElement.classList.contains('${theme}')`);
		const openingRows = await readRows(sessionId);
		assertAlternating(openingRows);
		assert.equal(openingRows[0].shade, 'primary');
		assert.equal(
			await evaluate(sessionId, "document.querySelectorAll('[data-chat-history-divider]').length"),
			1
		);
		await evaluate(
			sessionId,
			"chatNative.push('roomb','hidden-background'); chatNative.push('rooma','visible-new')"
		);
		await waitFor(sessionId, "document.querySelectorAll('[data-chat-message-index]').length === 4");
		assert.equal(
			await evaluate(sessionId, "document.body.textContent.includes('hidden-background')"),
			false
		);
		assert.equal(
			await evaluate(
				sessionId,
				"document.querySelector('[data-chat-history-divider]').nextElementSibling.textContent.includes('visible-new')"
			),
			true
		);
		await waitFor(sessionId, 'chatNative.acknowledgements.length > 0');
		assertStable(openingRows, await readRows(sessionId));
		assertAlternating(await readRows(sessionId));
		const beforeTranslation = await readRows(sessionId);
		await evaluate(
			sessionId,
			"chatNative.translate('rooma','translated-live'); chatNative.push('rooma','after-translation')"
		);
		await waitFor(
			sessionId,
			"document.querySelectorAll('[data-chat-message-index]').length === 5 && document.body.textContent.includes('translated-live')"
		);
		const liveRows = await readRows(sessionId);
		assertStable(beforeTranslation, liveRows);
		assertAlternating(liveRows);
		await evaluate(
			sessionId,
			'chatNative.settings.obj.chat.show_history_divider = false; chatNative.settings.sync()'
		);
		await waitFor(
			sessionId,
			"document.querySelectorAll('[data-chat-history-divider]').length === 0"
		);
		await evaluate(
			sessionId,
			'chatNative.settings.obj.chat.show_history_divider = true; chatNative.settings.sync(); chatNative.reset()'
		);
		await waitFor(
			sessionId,
			"document.querySelectorAll('[data-chat-history-divider]').length === 1"
		);
		assert.equal(
			await evaluate(
				sessionId,
				"document.querySelector('[data-chat-history-divider]').nextElementSibling.textContent.includes('visible-new')"
			),
			true
		);
		assertStable(liveRows, await readRows(sessionId));
		assertAlternating(await readRows(sessionId));
		await evaluate(
			sessionId,
			'chatNative.settings.obj.chat.alternate_backgrounds = false; chatNative.settings.sync()'
		);
		await waitFor(
			sessionId,
			"Array.from(document.querySelectorAll('[data-chat-message-index]')).every(row => !row.classList.contains('bg-content-primary') && !row.classList.contains('bg-content-secondary'))"
		);
		await evaluate(
			sessionId,
			'chatNative.settings.obj.chat.alternate_backgrounds = true; chatNative.settings.sync()'
		);
		await waitFor(
			sessionId,
			"!!document.querySelector('[data-chat-message-index].bg-content-primary')"
		);
		assertStable(liveRows, await readRows(sessionId));
		assertAlternating(await readRows(sessionId));
		await evaluate(sessionId, "chatNative.go('roomb')");
		await waitFor(sessionId, "document.body.textContent.includes('hidden-background')");
		assert.equal(await evaluate(sessionId, 'chatNative.feeds.size'), 1);
		assert.equal(await evaluate(sessionId, 'chatNative.metadataRequests'), 0);
		await evaluate(sessionId, 'chatNative.go(null)');
		await waitFor(sessionId, "!!document.querySelector('[data-settings-page]')");
		assert.equal(await evaluate(sessionId, 'chatNative.feeds.size'), 0);
		await waitFor(sessionId, "document.body.textContent.includes('2 joined')");

		await evaluate(sessionId, "chatNative.push('rooma','while-settings'); chatNative.go('rooma')");
		await waitFor(sessionId, "document.body.textContent.includes('while-settings')");
		assert.equal(
			await evaluate(
				sessionId,
				"document.querySelector('[data-chat-history-divider]').nextElementSibling"
			),
			null
		);
		await evaluate(sessionId, "chatNative.delays.roomb = 150; chatNative.go('roomb')");
		await delay(10);
		await evaluate(sessionId, "chatNative.go('rooma')");
		await delay(200);
		assert.equal(await evaluate(sessionId, 'chatNative.feeds.size'), 1);
		assert.equal(
			await evaluate(sessionId, "document.body.textContent.includes('while-settings')"),
			true
		);
		assert.deepEqual(exceptions.get(sessionId) ?? [], []);
		const screenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
		await writeFile(
			path.join(output, 'chat-' + width + '-' + theme + '.png'),
			Buffer.from(screenshot.data, 'base64')
		);
		let beforeTrim = await readRows(sessionId);
		assertStable(liveRows, beforeTrim);
		assertAlternating(beforeTrim);
		for (const limit of [5, 3, 1]) {
			await evaluate(
				sessionId,
				`chatNative.settings.obj.chat.message_limit = ${limit}; chatNative.settings.obj.chat.message_gc_interval_ms = 10; chatNative.settings.sync()`
			);
			await waitFor(
				sessionId,
				`document.querySelectorAll('[data-chat-message-index]').length === ${limit}`
			);
			const trimmed = await readRows(sessionId);
			assertStable(beforeTrim, trimmed);
			if (limit > 1) assertAlternating(trimmed);
			await evaluate(sessionId, 'chatNative.reset()');
			await delay(50);
			assert.deepEqual(await readRows(sessionId), trimmed);
			await evaluate(sessionId, "chatNative.go('roomb')");
			await waitFor(sessionId, "document.body.textContent.includes('hidden-background')");
			await evaluate(sessionId, "chatNative.go('rooma')");
			await waitFor(sessionId, "document.body.textContent.includes('while-settings')");
			assert.deepEqual(await readRows(sessionId), trimmed);
			beforeTrim = trimmed;
		}
		await evaluate(
			sessionId,
			"chatNative.translate('roomb','background-translation'); chatNative.push('rooma','limit-one-new')"
		);
		await waitFor(
			sessionId,
			"document.querySelectorAll('[data-chat-message-index]').length === 1 && document.body.textContent.includes('limit-one-new')"
		);
		assert.notEqual((await readRows(sessionId))[0].shade, beforeTrim[0].shade);
		assert.deepEqual(exceptions.get(sessionId) ?? [], []);
		await evaluate(
			sessionId,
			"chatNative.resolutionErrors.unknown = 'channel not found'; chatNative.go('unknown')"
		);
		await waitFor(sessionId, "document.body.textContent.includes('channel not found')");
		assert.equal(await evaluate(sessionId, 'chatNative.feeds.size'), 0);
		assert.equal(await evaluate(sessionId, "document.querySelectorAll('.bg-red-950').length"), 1);
		await evaluate(
			sessionId,
			"chatNative.roomFailures.rooma = '403 Forbidden'; chatNative.go('rooma')"
		);
		await waitFor(sessionId, "document.body.textContent.includes('403 Forbidden')");
		assert.equal(await evaluate(sessionId, 'chatNative.feeds.size'), 1);
		await evaluate(sessionId, 'chatNative.go(null)');
		await waitFor(sessionId, "!!document.querySelector('[data-settings-page]')");
		await evaluate(sessionId, "chatNative.go('rooma')");
		await waitFor(sessionId, "document.body.textContent.includes('403 Forbidden')");
		assert.equal(await evaluate(sessionId, "document.querySelectorAll('.bg-red-950').length"), 1);
		assert.deepEqual(exceptions.get(sessionId) ?? [], []);
		await send('Target.closeTarget', { targetId });
	}
	console.log(
		'Passed chat feeds, stable alternating rows, translations, divider/settings toggles, trimming, recovery, and room navigation in dark/light themes at desktop/mobile widths.'
	);
} finally {
	if (send) await send('Browser.close').catch(() => browser.kill());
	else browser.kill();
	socket?.close();
	await new Promise((resolve) => server.close(resolve));
}
