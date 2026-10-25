#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { parseArgs } from 'node:util';
import ts from 'typescript';

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function createFixture({
	MessageHistoryScheduler,
	captureScrollSnapshot,
	getPinnedBatchScrollSnapshot,
	restoreScrollAfterRender,
	scrollToBottom,
	settings
}) {
	const feed = document.createElement('div');
	feed.style.cssText =
		"position:absolute;top:30px;left:10px;height:200px;width:750px;overflow-x:hidden;overflow-y:auto;overflow-anchor:none;background:#080e19;color:white;font:14px/20px 'Segoe UI',sans-serif";
	const content = document.createElement('div');
	feed.append(content);
	document.body.append(feed);
	const icon =
		'data:image/svg+xml,' +
		encodeURIComponent(
			'<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28"><rect width="28" height="28" fill="#9363ec"/><circle cx="14" cy="14" r="8" fill="#fdba24"/></svg>'
		);
	const makeRow = (index) => {
		const row = document.createElement('div');
		row.dataset.chatMessageIndex = String(index);
		row.style.cssText =
			'box-sizing:border-box;display:block;width:100%;padding:4px 8px;font-size:14px;line-height:20px;background:' +
			(index % 2 ? '#0e1829' : '#080e19');
		const line = document.createElement('div');
		line.style.cssText = 'min-width:0;text-wrap:wrap;overflow-wrap:anywhere';
		const stamp = document.createElement('span');
		stamp.style.cssText = 'font-size:12px;line-height:16px;color:#64748b;white-space:nowrap';
		stamp.textContent = '4:15 PM';
		line.append(stamp, ' ');
		const badges = document.createElement('span');
		badges.style.cssText =
			'display:inline-flex;align-items:center;gap:4px;vertical-align:middle;white-space:nowrap';
		if (index % 3) {
			const button = document.createElement('button');
			button.style.cssText =
				'font:inherit;padding:0;margin:0;border:0;background:transparent;color:inherit';
			const image = document.createElement('img');
			image.src = icon;
			image.style.cssText = 'display:inline;vertical-align:middle;max-width:none;height:20px';
			button.append(image);
			badges.append(button);
		}
		line.append(badges, ' ');
		const username = document.createElement('span');
		username.style.cssText = 'white-space:nowrap;font-weight:700;color:#70c7ff';
		username.textContent = 'User' + index + ': ';
		line.append(username, 'fixed text to inspect vertical movement ');
		if (index % 3 === 2) {
			const emote = document.createElement('span');
			emote.style.cssText = 'display:inline-block;vertical-align:middle;line-height:1';
			const image = document.createElement('img');
			image.src = icon;
			image.style.cssText = 'display:block;max-width:none;height:28px;min-width:28px';
			emote.append(image);
			line.append(emote);
		}
		row.append(line);
		return row;
	};

	const messages = [];
	const batches = [];
	const gcPasses = [];
	const state = { ...settings, paused: false, interacting: false };
	const selector = '[data-chat-message-index]';
	let initialized = false;
	let resuming = false;
	let unread = 0;
	const snapshot = () =>
		initialized && !resuming
			? getPinnedBatchScrollSnapshot(null, feed, selector, !state.paused)
			: null;
	const restore = (saved) => {
		if (saved) restoreScrollAfterRender(feed, saved, selector);
	};
	const history = new MessageHistoryScheduler({
		getState: () => ({ ...state, message_count: messages.length }),
		append: (batch) => {
			const saved = snapshot();
			messages.push(...batch);
			const fragment = document.createDocumentFragment();
			for (const index of batch) fragment.append(makeRow(index));
			content.append(fragment);
			batches.push(batch.length);
			restore(saved);
		},
		trim: (count) => {
			const saved = snapshot();
			messages.splice(0, count);
			for (let i = 0; i < count; i++) content.firstElementChild.remove();
			gcPasses.push(count);
			restore(saved);
		}
	});
	for (let i = 0; i < settings.message_limit; i++) history.enqueue(i);
	await Promise.all([...content.querySelectorAll('img')].map((image) => image.decode()));
	await document.fonts.ready;
	feed.scrollTop = 8000;
	state.paused = true;
	history.update();
	initialized = true;
	batches.length = 0;
	const initial = captureScrollSnapshot(feed, selector);
	const anchor = [...content.children].find(
		(row) => row.dataset.chatMessageIndex === initial.anchor.key
	);
	const initialTop = anchor.getBoundingClientRect().top - feed.getBoundingClientRect().top;
	const metadata = () => ({
		count: messages.length,
		first: messages[0],
		last: messages.at(-1),
		unread,
		batches: [...batches],
		gcPasses: [...gcPasses],
		retained: anchor.isConnected,
		drift: anchor.isConnected
			? anchor.getBoundingClientRect().top - feed.getBoundingClientRect().top - initialTop
			: null
	});
	window.fixture = {
		metadata,
		enqueue: (batch) => {
			for (const message of batch) {
				if (state.paused) unread++;
				history.enqueue(message);
			}
			return metadata();
		},
		resume: () => {
			state.paused = false;
			resuming = true;
			try {
				history.resume();
			} finally {
				resuming = false;
			}
			scrollToBottom(feed);
			unread = 0;
			return metadata();
		},
		dispose: () => history.dispose()
	};

	let baseline;
	let width;
	let height;
	window.compareScreenshot = async (png) => {
		const bytes = Uint8Array.from(atob(png), (character) => character.charCodeAt(0));
		const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
		const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
		const context = canvas.getContext('2d');
		context.drawImage(bitmap, 0, 0);
		const pixels = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
		bitmap.close();
		if (!baseline) {
			baseline = pixels;
			width = canvas.width;
			height = canvas.height;
			return { changedPixels: 0 };
		}
		if (canvas.width !== width || canvas.height !== height)
			throw new Error('Screenshot dimensions changed');
		let changedPixels = 0;
		for (let i = 0; i < pixels.length; i += 4) {
			if (
				pixels[i] !== baseline[i] ||
				pixels[i + 1] !== baseline[i + 1] ||
				pixels[i + 2] !== baseline[i + 2]
			)
				changedPixels++;
		}
		return { changedPixels };
	};
	const box = feed.getBoundingClientRect();
	return {
		count: messages.length,
		dpr: devicePixelRatio,
		clip: {
			x: box.left + 8,
			y: box.top + 35,
			width: Math.min(300, innerWidth - box.left - 24),
			height: 120,
			scale: 1
		}
	};
}

async function connectDebugger(url) {
	const socket = new WebSocket(url);
	await new Promise((resolve, reject) => {
		socket.addEventListener('open', resolve, { once: true });
		socket.addEventListener('error', reject, { once: true });
	});
	let nextId = 0;
	const pending = new Map();
	socket.addEventListener('message', (event) => {
		const message = JSON.parse(event.data);
		const request = pending.get(message.id);
		if (!request) return;
		pending.delete(message.id);
		clearTimeout(request.timer);
		if (message.error) request.reject(new Error(JSON.stringify(message.error)));
		else request.resolve(message.result);
	});
	const command = (method, params = {}) =>
		new Promise((resolve, reject) => {
			const id = ++nextId;
			const timer = setTimeout(() => {
				pending.delete(id);
				reject(new Error('CDP timeout: ' + method));
			}, 10000);
			pending.set(id, { resolve, reject, timer });
			socket.send(JSON.stringify({ id, method, params }));
		});
	const evaluate = async (expression) => {
		const result = await command('Runtime.evaluate', {
			expression,
			returnByValue: true,
			awaitPromise: true
		});
		if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
		return result.result.value;
	};
	return {
		command,
		evaluate,
		close: () => {
			for (const request of pending.values()) {
				clearTimeout(request.timer);
				request.reject(new Error('Debugger closed'));
			}
			pending.clear();
			socket.close();
		}
	};
}

function removeTemporaryProfile(profile, root) {
	const resolved = realpathSync(profile);
	if (path.dirname(resolved) !== root || !path.basename(resolved).startsWith('pepo-chat-scroll-')) {
		throw new Error('Refusing to remove unexpected browser profile: ' + resolved);
	}
	rmSync(resolved, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}

async function checkScale(browserPath, scale, html) {
	const root = realpathSync(tmpdir());
	const profile = mkdtempSync(path.join(root, 'pepo-chat-scroll-'));
	let browser;
	let debuggerClient;
	try {
		browser = spawn(
			browserPath,
			[
				'--headless',
				'--disable-gpu',
				'--no-first-run',
				'--no-default-browser-check',
				'--disable-background-networking',
				'--disable-component-update',
				'--disable-sync',
				'--remote-debugging-port=0',
				'--force-device-scale-factor=' + scale,
				'--window-size=800,600',
				'--user-data-dir=' + profile,
				'about:blank'
			],
			{ windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] }
		);
		let stderr = '';
		let launchError;
		browser.on('error', (error) => {
			launchError = error;
		});
		browser.stderr.on('data', (chunk) => {
			stderr += chunk;
		});
		let port;
		for (let i = 0; i < 100; i++) {
			if (launchError) throw launchError;
			if (browser.exitCode !== null) throw new Error('Browser exited: ' + stderr.slice(-2000));
			const portFile = path.join(profile, 'DevToolsActivePort');
			try {
				port = readFileSync(portFile, 'utf8').split('\n')[0];
			} catch (error) {
				if (error.code !== 'ENOENT') throw error;
			}
			if (port) break;
			await delay(50);
		}
		if (!port) throw new Error('Browser debugging port did not initialize: ' + stderr.slice(-2000));
		const targets = await (await fetch('http://127.0.0.1:' + port + '/json/list')).json();
		const target = targets.find((item) => item.type === 'page');
		debuggerClient = await connectDebugger(target.webSocketDebuggerUrl);
		const { command, evaluate } = debuggerClient;
		await command('Page.navigate', {
			url: 'data:text/html;base64,' + Buffer.from(html).toString('base64')
		});
		for (let i = 0; i < 100 && !(await evaluate('!!window.fixtureReady')); i++) await delay(20);
		const initial = await evaluate('window.fixtureReady');
		if (!initial) throw new Error('Fixture did not initialize');
		if (Math.abs(initial.dpr - scale) > 0.001) throw new Error('Unexpected DPR: ' + initial.dpr);
		const capture = async () => {
			const shot = await command('Page.captureScreenshot', {
				format: 'png',
				fromSurface: true,
				captureBeyondViewport: false,
				clip: initial.clip
			});
			return evaluate('window.compareScreenshot(' + JSON.stringify(shot.data) + ')');
		};
		const waitFor = async (predicate) => {
			for (let i = 0; i < 250; i++) {
				const value = await evaluate('window.fixture.metadata()');
				if (predicate(value)) return value;
				await delay(20);
			}
			throw new Error('Timed out waiting for history update');
		};
		const assert = (condition, message) => {
			if (!condition) throw new Error('DPR ' + scale + ': ' + message);
		};
		await capture();
		let next = initial.count;
		for (let batch = 0; batch < 8; batch++) {
			const arrivals = Array.from({ length: 25 }, () => next++);
			const queued = await evaluate('window.fixture.enqueue(' + JSON.stringify(arrivals) + ')');
			assert(
				queued.count === initial.count + batch * 25,
				'paused arrivals appended before the batch deadline'
			);
			const visible = await waitFor((value) => value.count === next);
			assert(visible.retained && Math.abs(visible.drift) < 0.001, 'paused anchor moved');
			assert(visible.first === 0 && visible.gcPasses.length === 0, 'paused history was collected');
			assert(visible.unread === (batch + 1) * 25, 'unread arrivals were lost');
			const pixels = await capture();
			assert(
				pixels.changedPixels === 0,
				'retained message pixels changed: ' + pixels.changedPixels
			);
		}
		const queued = [next++, next++];
		await evaluate('window.fixture.enqueue(' + JSON.stringify(queued) + ')');
		const resumed = await evaluate('window.fixture.resume()');
		assert(
			resumed.count === initial.count &&
				resumed.first === next - initial.count &&
				resumed.last === next - 1,
			'resume lost or reordered history'
		);
		assert(
			resumed.gcPasses.length === 1 && resumed.gcPasses[0] === 202 && resumed.unread === 0,
			'resume did not perform one catch-up cleanup'
		);
		await delay(300);
		assert(
			(await evaluate('window.fixture.metadata()')).count === initial.count,
			'pending timer published messages twice'
		);
		const live = await evaluate('window.fixture.enqueue([' + next++ + '])');
		assert(
			live.count === initial.count + 1 && live.last === next - 1,
			'live following did not append immediately'
		);
		const collected = await waitFor((value) => value.gcPasses.length === 2);
		assert(
			collected.count === initial.count && collected.gcPasses[1] === 1,
			'live GC was not batched'
		);
		console.log(
			JSON.stringify({
				dpr: scale,
				pausedBatches: 8,
				pausedMessages: 200,
				changedPixels: 0,
				resumeGcPasses: 1,
				liveAppendImmediate: true,
				liveGcBatched: true
			})
		);
		await evaluate('window.fixture.dispose()');
	} finally {
		if (debuggerClient) {
			await debuggerClient.command('Browser.close').catch(() => {});
			debuggerClient.close();
		}
		if (browser && browser.exitCode === null) {
			await Promise.race([new Promise((resolve) => browser.once('exit', resolve)), delay(2000)]);
			if (browser.exitCode === null) browser.kill();
		}
		removeTemporaryProfile(profile, root);
	}
}

try {
	const { values } = parseArgs({ options: { browser: { type: 'string' } } });
	if (!values.browser)
		throw new Error('Usage: node scripts/check-chat-scroll.mjs --browser <browser executable>');
	const helpers = ['autoscroll.ts', 'message-history.ts'].map((file) =>
		readFileSync(new URL('../src/lib/chat/' + file, import.meta.url), 'utf8')
	);
	helpers.push(readFileSync(new URL('../src/lib/settings.ts', import.meta.url), 'utf8'));
	const compiled = helpers
		.map(
			(source) =>
				ts.transpileModule(source, {
					compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 }
				}).outputText
		)
		.join('\n');
	const html =
		'<!doctype html><meta charset="utf-8"><style>body{margin:0;background:#000}</style><script type="module">' +
		compiled +
		'\nwindow.fixtureReady=await (' +
		createFixture.toString() +
		')({MessageHistoryScheduler,captureScrollSnapshot,getPinnedBatchScrollSnapshot,restoreScrollAfterRender,scrollToBottom,settings:DEFAULT_APP_SETTINGS.chat});</script>';
	for (const scale of [1, 1.25, 1.5, 2]) await checkScale(values.browser, scale, html);
} catch (error) {
	console.error(error.stack);
	process.exitCode = 1;
}
