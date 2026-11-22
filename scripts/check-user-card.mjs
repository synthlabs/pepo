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

const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.join(root, 'target/user-card-check');
const browserPath = [
	process.env.BROWSER_BIN,
	'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
	'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
	'/usr/bin/chromium',
	'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
].find((candidate) => candidate && existsSync(candidate));
assert(browserPath, 'Set BROWSER_BIN to an installed Chromium browser');
await mkdir(output, { recursive: true });
await writeFile(
	path.join(output, 'native.svelte.js'),
	`
const callbacks = new Set();
let preferences = $state({ chat: { user_card_close_on_unfocus: true } });
let current = { user: { user_id: '1', login: 'maya', display_name: '마야', color: null },
  profile_image_url: null, channel_id: 'room', channel_login: 'room', badges: null,
  profile_status: 'ready', error: null, pinned: false, chrome: new URL(location.href).searchParams.get('chrome') ?? 'compact', revision: 1 };
const native = window.cardNative = { ready: false, heights: [], closes: 0, retries: 0, urls: [], errors: [],
  update(patch) { current = { ...current, ...patch, revision: current.revision + 1 }; for (const callback of callbacks) callback({ payload: current }); },
  policy(value) { preferences.chat.user_card_close_on_unfocus = value; }
};
const ok = (data) => ({ status: 'ok', data });
export const commands = {
  async getUserCardState() { return ok(current); },
  async readyUserCard(height) { native.heights.push(height); native.ready = true; return ok(null); },
  async setUserCardPinned(pinned) { native.update({ pinned }); return ok(null); },
  async refreshUserCard() { native.retries++; native.update({ profile_status: 'loading', error: null }); return ok(null); }
};
export const getCurrentWindow = () => ({ label: 'user-card-1',
  async listen(event, callback) { callbacks.add(callback); return () => callbacks.delete(callback); },
  async close() { native.closes++; }
});
export const appSettings = { initialized: Promise.resolve() };
export const getNormalizedAppSettings = () => preferences;
export const openUrl = async (url) => { native.urls.push(url); };
export const appErrors = { capture(error) { native.errors.push(String(error)); } };
`
);
await writeFile(
	path.join(output, 'main.js'),
	`import { mount } from 'svelte'; import Route from '../../src/routes/user-card/+page.svelte'; import '../../src/app.css'; mount(Route, { target: document.body });`
);
await writeFile(
	path.join(output, 'index.html'),
	'<!doctype html><html><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><script type="module" src="/main.js"></script></body></html>'
);
const mocks = path.join(output, 'native.svelte.js');
await build({
	configFile: false,
	root: output,
	plugins: [svelte({ configFile: false }), tailwindcss()],
	resolve: {
		alias: [
			...[
				'$lib/bindings',
				'$lib/stores/settings.svelte',
				'$lib/errors/controller',
				'@tauri-apps/api/window',
				'@tauri-apps/plugin-opener'
			].map((find) => ({ find, replacement: mocks })),
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
		browser.on('error', reject);
		browser.on('exit', (code) => reject(new Error(`Browser exited ${code}: ${logs}`)));
	});
	socket = new WebSocket(endpoint);
	await new Promise((resolve, reject) => {
		socket.onopen = resolve;
		socket.onerror = reject;
	});
	let sequence = 0;
	const pending = new Map();
	const exceptions = [];
	socket.onmessage = ({ data }) => {
		const message = JSON.parse(data);
		if (message.method === 'Runtime.exceptionThrown')
			exceptions.push(message.params.exceptionDetails);
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
			await delay(30);
		}
		assert.fail(`Timed out: ${expression}`);
	};
	const resize = (sessionId, width, height) =>
		send(
			'Emulation.setDeviceMetricsOverride',
			{ width, height, deviceScaleFactor: 1, mobile: false },
			sessionId
		);
	const key = async (sessionId, key, code, windowsVirtualKeyCode, modifiers = 0, text) => {
		await send(
			'Input.dispatchKeyEvent',
			{ type: 'rawKeyDown', key, code, windowsVirtualKeyCode, modifiers },
			sessionId
		);
		if (text)
			await send(
				'Input.dispatchKeyEvent',
				{ type: 'char', text, key, code, windowsVirtualKeyCode, modifiers },
				sessionId
			);
		await send(
			'Input.dispatchKeyEvent',
			{ type: 'keyUp', key, code, windowsVirtualKeyCode, modifiers },
			sessionId
		);
	};
	let count = 0;
	for (const theme of ['dark', 'light'])
		for (const chrome of ['compact', 'native'])
			for (const width of [320, 360, 420]) {
				const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
				const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
				await send('Runtime.enable', {}, sessionId);
				await send('Page.enable', {}, sessionId);
				await send(
					'Emulation.setDeviceMetricsOverride',
					{ width, height: 240, deviceScaleFactor: 1, mobile: false },
					sessionId
				);
				await send('Page.navigate', { url: `${origin}?chrome=${chrome}` }, sessionId);
				await waitFor(
					sessionId,
					'window.cardNative?.ready && !!document.querySelector("[data-user-card]")'
				);
				await evaluate(
					sessionId,
					`document.documentElement.classList.toggle('dark', ${theme === 'dark'}); cardNative.update({ chrome: ${JSON.stringify(chrome)} });`
				);
				await waitFor(sessionId, `!!document.querySelector('[aria-label="Pin user card"]')`);
				const initialHeight = await evaluate(sessionId, 'cardNative.heights[0]');
				assert(
					initialHeight >= 120 && initialHeight < 160,
					'initial height fits the empty-badge card'
				);
				await resize(sessionId, width, initialHeight);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[aria-label="Window controls"]').getBoundingClientRect().height`
					),
					28
				);
				assert.equal(
					await evaluate(sessionId, `getComputedStyle(document.querySelector('header')).cursor`),
					'move'
				);
				assert.equal(
					await evaluate(
						sessionId,
						`getComputedStyle(document.querySelector('[aria-label="Pin user card"]')).cursor`
					),
					'pointer'
				);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[data-user-card-channel]').textContent.trim()`
					),
					'#room'
				);
				assert.equal(
					await evaluate(sessionId, `!!document.querySelector('[aria-label="Channel badges"]')`),
					false
				);
				assert.equal(
					await evaluate(sessionId, `document.body.textContent.includes('Badges not observed')`),
					false
				);
				await evaluate(sessionId, 'cardNative.update({ badges: [] })');
				assert.equal(
					await evaluate(sessionId, `document.body.textContent.includes('No badges observed')`),
					false
				);

				assert.deepEqual(
					await evaluate(
						sessionId,
						`Array.from(document.querySelectorAll('header button'), button => button.getAttribute('aria-label'))`
					),
					['Open in Twitch', 'Pin user card', ...(chrome === 'compact' ? ['Close user card'] : [])]
				);
				assert.equal(
					await evaluate(sessionId, `!!document.querySelector('[data-user-card-footer]')`),
					false
				);
				for (const [label, tooltip, preceding, modifiers] of [
					['Open in Twitch', 'Open in Twitch', 'Pin user card', 8],
					['Pin user card', 'Keep this card open', 'Open in Twitch', 0]
				]) {
					const selector = `[aria-label="${label}"]`;
					await evaluate(
						sessionId,
						`window.dispatchEvent(new Event('blur')); document.activeElement.blur(); document.querySelector('${selector}').focus()`
					);
					await delay(200);
					assert.equal(
						await evaluate(sessionId, `!!document.querySelector('[role="tooltip"]')`),
						false,
						`${label}: automatic focus does not open tooltip`
					);
					await evaluate(
						sessionId,
						`window.dispatchEvent(new Event('blur')); window.dispatchEvent(new Event('focus')); document.activeElement.dispatchEvent(new FocusEvent('focus'))`
					);
					await delay(200);
					assert.equal(
						await evaluate(sessionId, `!!document.querySelector('[role="tooltip"]')`),
						false,
						`${label}: restored focus does not open tooltip`
					);
					await evaluate(
						sessionId,
						`document.querySelector('[aria-label="${preceding}"]').focus()`
					);
					await key(sessionId, 'Tab', 'Tab', 9, modifiers);
					await waitFor(
						sessionId,
						`document.querySelector('[role="tooltip"]')?.textContent === ${JSON.stringify(tooltip)}`
					);
					assert.equal(
						await evaluate(sessionId, `document.activeElement.getAttribute('aria-label')`),
						label
					);
					await evaluate(
						sessionId,
						`window.dispatchEvent(new Event('blur')); document.activeElement.blur()`
					);
					await waitFor(sessionId, `!document.querySelector('[role="tooltip"]')`);
					const point = await evaluate(
						sessionId,
						`(() => { const rect = document.querySelector('${selector}').getBoundingClientRect(); return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }; })()`
					);
					await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point }, sessionId);
					await waitFor(
						sessionId,
						`document.querySelector('[role="tooltip"]')?.textContent === ${JSON.stringify(tooltip)}`
					);
					await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 5, y: 80 }, sessionId);
					await evaluate(sessionId, `window.dispatchEvent(new Event('blur'))`);
					await waitFor(sessionId, `!document.querySelector('[role="tooltip"]')`);
				}
				await evaluate(
					sessionId,
					`document.querySelector('[aria-label="Open in Twitch"]').focus()`
				);
				await key(sessionId, 'Tab', 'Tab', 9);
				await waitFor(sessionId, `!!document.querySelector('[role="tooltip"]')`);
				await key(sessionId, 'Enter', 'Enter', 13, 0, '\r');
				await waitFor(
					sessionId,
					`document.querySelector('[aria-label="Unpin user card"]')?.getAttribute('aria-pressed') === 'true'`
				);
				await key(sessionId, ' ', 'Space', 32, 0, ' ');
				await waitFor(sessionId, `!!document.querySelector('[aria-label="Pin user card"]')`);
				await key(sessionId, 'Enter', 'Enter', 13, 0, '\r');
				await waitFor(sessionId, `!!document.querySelector('[aria-label="Unpin user card"]')`);
				await evaluate(
					sessionId,
					`window.dispatchEvent(new Event('blur')); document.activeElement.blur()`
				);
				await waitFor(sessionId, `!document.querySelector('[role="tooltip"]')`);

				const detailsTop = await evaluate(
					sessionId,
					`document.querySelector('[data-user-card-details]').getBoundingClientRect().top`
				);
				await resize(sessionId, width, 400);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[data-user-card-details]').getBoundingClientRect().top`
					),
					detailsTop,
					'growing the window leaves details below the controls'
				);
				await resize(sessionId, width, 120);
				assert.equal(
					await evaluate(
						sessionId,
						`(() => { const node = document.querySelector('[data-user-card-scroll]'); node.scrollTop = node.scrollHeight; return node.scrollHeight > node.clientHeight; })()`
					),
					true
				);
				assert.equal(
					await evaluate(sessionId, `document.querySelector('header').getBoundingClientRect().top`),
					0,
					'window controls remain visible while scrolling'
				);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[aria-label="Open in Twitch"]').getBoundingClientRect().bottom <= 28`
					),
					true,
					'Twitch button stays visible above the scrolling details'
				);
				await resize(sessionId, width, initialHeight);
				await evaluate(
					sessionId,
					`document.querySelector('[data-user-card-scroll]').scrollTop = 0`
				);
				const compactScreenshot = await send(
					'Page.captureScreenshot',
					{ format: 'png' },
					sessionId
				);
				await writeFile(
					path.join(output, `${theme}-${chrome}-${width}-empty.png`),
					Buffer.from(compactScreenshot.data, 'base64')
				);
				await evaluate(sessionId, `cardNative.update({ profile_status: 'unavailable' });`);
				await waitFor(sessionId, `document.body.textContent.includes('unavailable')`);
				await evaluate(
					sessionId,
					`Array.from(document.querySelectorAll('button')).find(button => button.textContent === 'Retry').click()`
				);
				assert.equal(await evaluate(sessionId, 'cardNative.retries'), 1);
				await evaluate(
					sessionId,
					`cardNative.update({ profile_status: 'ready', user: { user_id: '1', login: 'maya', display_name: '마야 — A longer display name that wraps', color: null }, badges: ['broadcaster', 'moderator', 'vip', 'subscriber', 'partner'].map(set_id => ({ set_id, id: '1', info: '', badge: { set_id: '', id: '', image_url_1x: '', image_url_2x: '', image_url_4x: '', title: '', description: '' } })) });`
				);
				await delay(50);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[aria-label="Channel badges"]').children.length`
					),
					5
				);
				assert.deepEqual(
					await evaluate(sessionId, 'cardNative.heights'),
					[initialHeight],
					'profile and badge updates do not refit an open window'
				);
				assert.equal(await evaluate(sessionId, 'innerHeight'), initialHeight);
				assert.equal(
					await evaluate(sessionId, 'document.documentElement.scrollWidth <= innerWidth'),
					true,
					'no horizontal overflow'
				);
				assert.equal(
					await evaluate(sessionId, `!!document.querySelector('[aria-label="Close user card"]')`),
					chrome === 'compact'
				);
				const screenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
				await writeFile(
					path.join(output, `${theme}-${chrome}-${width}.png`),
					Buffer.from(screenshot.data, 'base64')
				);
				await evaluate(sessionId, 'cardNative.policy(false)');
				await waitFor(sessionId, `!document.querySelector('[aria-label="Unpin user card"]')`);
				assert.deepEqual(
					await evaluate(
						sessionId,
						`Array.from(document.querySelectorAll('header button'), button => button.getAttribute('aria-label'))`
					),
					['Open in Twitch', ...(chrome === 'compact' ? ['Close user card'] : [])]
				);
				await evaluate(
					sessionId,
					`document.querySelector('[aria-label="Open in Twitch"]').click(); document.querySelector('[aria-label="Open in Twitch"]').focus()`
				);
				await key(sessionId, 'Enter', 'Enter', 13, 0, '\r');
				await key(sessionId, ' ', 'Space', 32, 0, ' ');
				assert.deepEqual(await evaluate(sessionId, 'cardNative.urls'), [
					'https://www.twitch.tv/maya',
					'https://www.twitch.tv/maya',
					'https://www.twitch.tv/maya'
				]);
				await send(
					'Input.dispatchKeyEvent',
					{ type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 },
					sessionId
				);
				await waitFor(sessionId, 'cardNative.closes === 1');
				assert.deepEqual(await evaluate(sessionId, 'cardNative.errors'), []);
				await send('Target.closeTarget', { targetId });
				count++;
			}
	assert.deepEqual(exceptions, []);
	console.log(`Passed ${count} user-card route browser scenarios. Screenshots: ${output}`);
} finally {
	if (send) await send('Browser.close').catch(() => browser.kill());
	else browser.kill();
	socket?.close();
	await new Promise((resolve) => server.close(resolve));
}
