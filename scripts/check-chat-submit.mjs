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

// Run with node scripts/check-chat-submit.mjs; BROWSER_BIN selects Chromium.
const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.join(root, 'target/chat-submit-check');
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
export const page = $state({ url: new URL(location.href), params: { id: 'maya' } });
export const native = window.chatNative = {
    calls: [], requests: [], logs: [], debug: [], reports: [], pauses: [], joins: [], searches: [], captured: 0,
    failReport: false, failCapabilities: false,
    emote: { id: '25', name: 'Kappa', tier: '', emote_type: 'globals', emote_set_id: '0', format: ['static'],
        scale: ['1.0'], theme_mode: ['light', 'dark'], url: 'data:image/gif;base64,R0lGODlhAQABAAAAACw=',
        provider: 'Twitch', scope: 'Global' },
    logError(message) { return error(message); },
    navigate(id) { page.params = { id }; },
    resolve(index, result) { this.requests[index].resolve(result); },
    reject(index, message) { this.requests[index].reject(new Error(message)); }
};
export class SyncedState {
    obj = $state({}); ready = true; initialized = Promise.resolve();
    constructor(key, fallback) {
        this.obj = structuredClone(key === 'auth_state'
            ? { phase: 'authorized', token: { user_id: '2', login: 'jerod', profile_image_url: '' } }
            : fallback ?? {});
    }
    async sync() { return true; }
}
export const commands = {
    getChatFeedContext: async () => ({ status: 'ok', data: { epoch: 'submit-page' } }),
    subscribeChat: async (channel, request, sink) => {
        native.joins.push(channel);
        sink.onmessage({ cursor: 0, event: { type: 'snapshot', messages: [] } });
        return { status: 'ok', data: { broadcaster_id: channel + '-id', broadcaster_login: channel, broadcaster_name: channel } };
    },
    unsubscribeChat: async () => {},
    acknowledgeChat: async () => {},
    searchEmotes: async (query) => {
        native.searches.push(query);
        return { status: 'ok', data: String(query).toLowerCase().startsWith('ka') ? [native.emote] : [] };
    },
    sendChatMessage: (broadcasterId, message) => {
        native.calls.push({ broadcasterId, message });
        return new Promise((resolve, reject) => native.requests.push({ resolve, reject }));
    }
};
export class Channel { constructor(callback) { this.onmessage = callback; } }
const listeners = new Map();
export const listen = async (event, callback) => {
    const callbacks = listeners.get(event) ?? new Set(); callbacks.add(callback); listeners.set(event, callbacks);
    return () => callbacks.delete(callback);
};
export async function error(message, options) {
    native.logs.push({ message, options });
    if (options?.keyValues?.['inbound.handled'] !== 'true')
        for (const callback of listeners.get('inbound://error-captured') ?? [])
            callback({ payload: { id: String(++native.captured), error: { kind: 'error', message, target: 'webview', timestamp: '0' } } });
}
export async function invoke(command, args) {
    switch (command) {
        case 'plugin:inbound|capabilities':
            if (native.failCapabilities) throw new Error('capabilities failure');
            return { has_config: false };
        case 'plugin:inbound|preview_system': return { os: 'browser', arch: 'test', locale: 'en-US' };
        case 'plugin:inbound|preview_build': return { app_version: 'test', app_commit: 'test', build_time: 'test' };
        case 'plugin:inbound|preview_log_tail': return { exists: true, byte_count: 0, preview_text: 'test logs', gzipped_size: 0 };
        case 'plugin:inbound|preview_config': return null;
        case 'plugin:inbound|captured_errors': return [];
        case 'plugin:inbound|preview_error': return { kind: 'error', message: 'wrong latest native error', target: 'other', timestamp: '0' };
        case 'plugin:inbound|submit':
            native.reports.push(args.input);
            if (native.failReport) throw new Error('report submission failure');
            return;
        default: throw new Error('Unexpected invoke: ' + command);
    }
}
export const PUBLIC_UPDATE_INSTALL_MODE = '';
export const checkForAppUpdates = async () => {};
export default {
    debug(...values) { native.debug.push(values); }, info() {},
    error(...values) { void error(values.join(' ')); }
};
`
);
await writeFile(
	path.join(output, 'fixture.svelte'),
	`
<script>
    import Root from '../../src/routes/+layout.svelte';
    import Chat from '../../src/routes/app/chat/[id]/+page.svelte';
    import { page, native } from './native.svelte.js';
    import { appErrors } from '$lib/errors/controller';
    import { getNormalizedAppSettings, updateAppSettings } from '$lib/stores/settings.svelte';
    const pause = appErrors.pause.bind(appErrors);
    const resume = appErrors.resume.bind(appErrors);
    appErrors.pause = (id, reason) => { native.pauses.push(['pause', reason]); pause(id, reason); };
    appErrors.resume = (id, reason) => { native.pauses.push(['resume', reason]); resume(id, reason); };
    native.errors = appErrors;
    native.settings = getNormalizedAppSettings;
    native.updateSettings = updateAppSettings;
</script>
<Root><main class="flex h-screen w-full">{#key page.params.id}<Chat />{/key}</main></Root>
`
);
await writeFile(
	path.join(output, 'main.js'),
	`import { mount } from 'svelte'; import Fixture from './fixture.svelte'; mount(Fixture, { target: document.body });`
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
			...[
				'$app/state',
				'$utils/log',
				'$utils/updater',
				'$env/static/public',
				'tauri-svelte-synced-store',
				'@tauri-apps/api/event',
				'@tauri-apps/api/core',
				'@tauri-apps/plugin-log'
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
	const waitFor = async (sessionId, expression, timeoutMs = 5000) => {
		const deadline = Date.now() + timeoutMs;
		while (Date.now() < deadline) {
			if (await evaluate(sessionId, expression)) return;
			await delay(50);
		}
		assert.fail(
			`Timed out waiting for ${expression}; exceptions: ${JSON.stringify(exceptions.get(sessionId))}`
		);
	};

	const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
	const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
	await send('Runtime.enable', {}, sessionId);
	await send('Page.navigate', { url: origin }, sessionId);
	const run = (expression) => evaluate(sessionId, expression);
	const wait = (expression, timeoutMs) => waitFor(sessionId, expression, timeoutMs);
	const input = 'document.querySelector(\'[aria-label="Chat message"]\')';
	const form = `${input}.form`;
	const notice = '[data-error-notice]';
	const counter = '[data-chat-counter]';
	const inline = '[data-chat-input-error]';
	const clickText = async (selector, text) => {
		await run(
			`Array.from(document.querySelectorAll(${JSON.stringify(selector)})).find(node => node.textContent.trim() === ${JSON.stringify(text)}).click()`
		);
		await delay(40);
	};
	const type = async (text) => {
		await run(
			`(() => { const input = ${input}; input.focus(); input.value = ${JSON.stringify(text)}; input.dispatchEvent(new Event('input', { bubbles: true })); })()`
		);
	};
	const submit = async (text) => {
		const index = await run('chatNative.calls.length');
		await type(text);
		await run(`${form}.requestSubmit()`);
		await wait(`chatNative.calls.length === ${index + 1}`);
		assert.equal(
			await run(`new Promise(resolve => requestAnimationFrame(() => resolve(${input}.value)))`),
			'',
			'clears before backend settles'
		);
		return index;
	};
	const settle = async (index, error) => {
		await run(
			`chatNative.resolve(${index}, ${JSON.stringify(error ? { status: 'error', error } : { status: 'ok', data: null })})`
		);
		await wait(`${form}.getAttribute('aria-busy') === 'false'`);
	};
	const failure = (kind, message = `${kind} guidance`, details = 'private diagnostics') => ({
		kind,
		message,
		details
	});
	await wait(`${input} && chatNative.joins.length === 1`);
	let index = await submit('first message');
	assert.deepEqual(await run('chatNative.calls[0]'), {
		broadcasterId: 'maya-id',
		message: 'first message'
	});
	await type('next draft');
	await run(`${form}.requestSubmit()`);
	assert.equal(await run('chatNative.calls.length'), 1);
	assert.equal(await run(`${input}.value`), 'next draft');
	await wait(
		`document.querySelector('${inline}')?.textContent.includes('previous message is still sending')`
	);
	await settle(index);
	assert.equal(await run(`${input}.value`), 'next draft', 'success preserves subsequent typing');
	console.log('PASS immediate clear, blocked submit, success preserves draft');

	for (const kind of [
		'authentication',
		'permission_denied',
		'rate_limited',
		'rejected',
		'connection',
		'timeout'
	]) {
		index = await submit(`${kind} message`);
		await type('new draft');
		const debugBefore = await run('chatNative.debug.length');
		await settle(index, failure(kind));
		await wait(`document.querySelector('${inline}')?.textContent.includes('${kind} guidance')`);
		assert.equal(await run(`${input}.value`), 'new draft');
		assert.equal(
			await run('chatNative.debug.length'),
			debugBefore,
			'returned error never logs success'
		);
		assert.equal(await run(`document.querySelectorAll('${notice}').length`), 0);
	}
	console.log('PASS all actionable categories and returned-result handling');

	await wait(`!document.querySelector('${inline}')`, 8000);
	index = await submit('timer replacement');
	await type('blocked draft');
	await run(`${form}.requestSubmit()`);
	await wait(
		`document.querySelector('${inline}')?.textContent.includes('previous message is still sending')`
	);
	// A second blocked submit must restart the five-second timer; nothing observable marks the gap.
	await delay(2000);
	const replacedAt = await run(`(${form}.requestSubmit(), performance.now())`);
	await settle(index);
	await wait(`!document.querySelector('${inline}')`, 8000);
	const removedAt = await run('performance.now()');
	assert(
		removedAt - replacedAt >= 4500,
		`inline bar expired ${Math.round(removedAt - replacedAt)}ms after the replacing submit`
	);
	console.log('PASS inline timer replacement and expiry');

	index = await submit('old view pending');
	await run(`chatNative.navigate('other')`);
	await wait(
		`chatNative.joins.at(-1) === 'other' && ${form}.getAttribute('aria-busy') === 'false'`
	);
	const other = await submit('new view send');
	const logsBeforeLate = await run('chatNative.logs.length');
	await run(
		`chatNative.resolve(${index}, { status: 'error', error: ${JSON.stringify(failure('authentication'))} })`
	);
	await wait(`chatNative.logs.length > ${logsBeforeLate}`);
	assert.equal(
		await run(`!!document.querySelector('${inline}')`),
		false,
		'no late actionable feedback in another view'
	);
	await settle(other);
	assert.equal(await run(`${input}.value`), '', 'failed text is not restored');
	console.log('PASS per-view guard and late actionable failure suppression');

	index = await submit('late technical failure');
	await run(`chatNative.navigate('third')`);
	await wait(`chatNative.joins.at(-1) === 'third'`);
	await run(
		`chatNative.resolve(${index}, { status: 'error', error: ${JSON.stringify(failure('unexpected', 'not user guidance', 'late technical details'))} })`
	);
	await wait(`document.querySelector('${notice}')?.textContent.includes('#other')`);
	assert.equal(await run(`!!document.querySelector('${inline}')`), false);
	assert.equal(
		await run(`document.querySelector('${notice}').textContent.includes('late technical details')`),
		false
	);
	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);
	console.log('PASS late technical report offer without raw details');

	for (let repeat = 0; repeat < 2; repeat++) {
		index = await submit(`technical failure ${repeat}`);
		await settle(
			index,
			failure('unexpected', 'not user guidance', `selected technical details ${repeat}`)
		);
	}
	await wait(`document.querySelectorAll('${notice}').length === 2`);
	const box = await run(
		`(() => { const box = document.querySelector('${notice}').getBoundingClientRect(); return { x: box.x + 8, y: box.y + 8 }; })()`
	);
	await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...box }, sessionId);
	await wait(`chatNative.pauses.some(event => event[0] === 'pause' && event[1] === 'hover')`);
	await run(`document.querySelector('${notice} button').focus()`);
	await wait(`chatNative.pauses.some(event => event[0] === 'pause' && event[1] === 'focus')`);
	await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 }, sessionId);
	await wait(`chatNative.pauses.some(event => event[0] === 'resume' && event[1] === 'hover')`);
	await clickText(`${notice} button`, 'Report this');
	await wait(`!!document.querySelector('[data-inbound-wizard]')`);
	index = await submit('another failure during report');
	await settle(index, failure('unexpected', 'not user guidance', 'newest technical details'));
	assert.equal(
		await run(`document.querySelectorAll('${notice}').length`),
		0,
		'offers pause during reporting'
	);
	await wait(`!document.querySelector('[data-inbound-action="next"]')?.disabled`);
	for (let step = 0; step < 10; step++) {
		if (await run(`!!document.querySelector('[data-inbound-action="send"]')`)) break;
		await wait(`document.querySelector('[data-inbound-action="next"]')?.disabled === false`);
		await clickText('[data-inbound-wizard] button', 'Next');
	}
	await run('chatNative.failReport = true');
	await clickText('[data-inbound-wizard] button', 'Send');
	await wait(`!!document.querySelector('[data-inbound-action="retry"]')`);
	await run('chatNative.failReport = false');
	await clickText('[data-inbound-wizard] button', 'Try again');
	await wait(`!!document.querySelector('[data-inbound-action="send"]')`);
	await clickText('[data-inbound-wizard] button', 'Send');
	await wait(`!!document.querySelector('[data-inbound-action="close"]')`);
	const report = await run('chatNative.reports.at(-1)');
	assert.match(report.error.message, /selected technical details/);
	assert.doesNotMatch(report.error.message, /newest technical details|wrong latest native error/);
	const closedAt = await run(
		`(Array.from(document.querySelectorAll('[data-inbound-wizard] button')).find(node => node.textContent.trim() === 'Done').click(), performance.now())`
	);
	await wait(`!!document.querySelector('${notice}')`);
	await type('focus outside notification');
	await wait(`chatNative.pauses.some(event => event[0] === 'resume' && event[1] === 'focus')`);
	await wait(`!document.querySelector('${notice}')`, 15000);
	const expiredAt = await run('performance.now()');
	assert(
		expiredAt - closedAt >= 8000,
		`offers expired ${Math.round(expiredAt - closedAt)}ms after the report closed`
	);
	console.log(
		'PASS distinct cards, hover/focus pauses, frozen report snapshot, report retry and ten-second expiry'
	);

	index = await submit('thrown invocation');
	await type('draft survives exception');
	await run(`chatNative.reject(${index}, 'invocation exploded')`);
	await wait(
		`${form}.getAttribute('aria-busy') === 'false' && !!document.querySelector('${notice}')`
	);
	assert.equal(await run(`${input}.value`), 'draft survives exception');
	assert.equal(await run(`!!document.querySelector('${inline}')`), false);
	assert.equal(
		await run(
			`Array.from(document.querySelectorAll('button')).some(button => button.textContent.trim() === 'Report this')`
		),
		true
	);
	assert.equal(
		await run(
			'chatNative.logs.every(log => log.options?.keyValues?.["inbound.handled"] === "true")'
		),
		true
	);
	assert.equal(
		await run('chatNative.logs.some(log => log.message.includes("report submission failure"))'),
		true
	);
	assert.equal(
		await run('chatNative.logs.some(log => log.message.includes("invocation exploded"))'),
		true
	);
	assert.equal(await run(`document.querySelectorAll('${notice}').length`), 1, 'one card');
	assert.deepEqual(exceptions.get(sessionId) ?? [], [], 'no uncaught browser errors');
	console.log('PASS thrown invocation, repeatable report offers and handled logging');

	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);
	index = await submit('string rejection');
	await run(
		`chatNative.resolve(${index}, { status: 'error', error: 'invalid args broadcasterId for command send_chat_message: missing required key broadcasterId' })`
	);
	await wait(
		`${form}.getAttribute('aria-busy') === 'false' && !!document.querySelector('${notice}')`
	);
	assert.equal(await run(`!!document.querySelector('${inline}')`), false, 'no inline guidance');
	assert.equal(
		await run(`document.querySelector('${notice}').textContent.includes('#third')`),
		true,
		'card names the channel'
	);
	assert.equal(
		await run(
			'chatNative.logs.some(log => log.message.includes("missing required key broadcasterId"))'
		),
		true
	);
	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);
	console.log('PASS string invoke rejection becomes a report offer');

	assert.equal(await run(`${input}.maxLength`), 500);
	await type('a'.repeat(399));
	assert.equal(await run(`!!document.querySelector('${counter}')`), false, 'counter hidden');
	await type('a'.repeat(400));
	await wait(`document.querySelector('${counter}')?.textContent === '400/500'`);
	assert.equal(
		await run(`document.querySelector('${counter}').classList.contains('text-destructive')`),
		false
	);
	await type('a'.repeat(500));
	await wait(`document.querySelector('${counter}')?.classList.contains('text-destructive')`);
	await type('a'.repeat(496) + ' :ka');
	await wait(`!!document.querySelector('img[alt="Kappa"]')`);
	const keydown = `${input}.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }))`;
	const callsBeforeEmote = await run('chatNative.calls.length');
	await run(keydown);
	await wait(
		`document.querySelector('${inline}')?.textContent.includes('cannot exceed 500 characters')`
	);
	assert.equal(await run(`${input}.value`), 'a'.repeat(496) + ' :ka', 'refused insertion');
	await wait(`!document.querySelector('img[alt="Kappa"]')`);
	// A refused insertion keeps the query, so the next search needs a new one.
	await type('a'.repeat(10) + ' :kap');
	await wait(`!!document.querySelector('img[alt="Kappa"]')`);
	await run(keydown);
	await wait(`${input}.value === ${JSON.stringify('a'.repeat(10) + ' Kappa ')}`);
	assert.equal(await run('chatNative.calls.length'), callsBeforeEmote, 'Enter picked, not sent');
	await type('');
	console.log('PASS 500-character limit, counter and refused emote insertion');

	await run(`chatNative.logError('native failure 42')`);
	await wait(
		`document.querySelector('${notice}')?.textContent.includes('Error detected') && document.querySelector('${notice}').textContent.includes('native failure 42')`
	);
	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);
	await run(`setTimeout(() => { throw new Error('window boom'); }, 0)`);
	await wait(`document.querySelector('${notice}')?.textContent.includes('window boom')`);
	assert.equal(
		await run(
			'chatNative.logs.some(log => log.message.includes("window boom") && log.options?.keyValues?.["inbound.handled"] === "true")'
		),
		true,
		'window error logged as handled'
	);
	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);
	await run('chatNative.updateSettings({ diagnostics: { capture_window_errors: false } })');
	await wait('chatNative.settings().diagnostics.capture_window_errors === false');
	await run(`setTimeout(() => { throw new Error('window silent'); }, 0)`);
	await run(`chatNative.logError('native after toggle')`);
	await wait(`document.querySelector('${notice}')?.textContent.includes('native after toggle')`);
	assert.equal(await run(`document.body.textContent.includes('window silent')`), false);
	assert.equal(await run(`document.querySelectorAll('${notice}').length`), 1);
	assert.deepEqual(
		(exceptions.get(sessionId) ?? []).filter((text) => !/window (boom|silent)/.test(text)),
		[],
		'no unexpected browser errors'
	);
	await type('a'.repeat(450));
	console.log('PASS native and window errors share the host; window capture follows the setting');
	const screenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
	await writeFile(path.join(output, 'chat-submit.png'), Buffer.from(screenshot.data, 'base64'));
	console.log('Passed chat submission browser checks. Artifacts:', output);
} finally {
	if (send) await send('Browser.close').catch(() => browser.kill());
	else browser.kill();
	socket?.close();
	await new Promise((resolve) => server.close(resolve));
}
