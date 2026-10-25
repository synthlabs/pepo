#!/usr/bin/env node
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { build } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { checkScale } from './check-chat-scroll.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.join(root, 'target/chat-scroll-route-check');
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function createRouteFixture(native, tick) {
	while (!native.handlers.has('chat_message:fixture'))
		await new Promise((resolve) => setTimeout(resolve, 5));
	const feed = document.querySelector('[aria-label="Chat messages"]');
	const rows = () => [...feed.querySelectorAll('[data-chat-message-index]')];
	const spacer = () =>
		feed.querySelector('[data-chat-history-spacer]').getBoundingClientRect().height;
	const frames = async () => {
		await tick();
		for (let i = 0; i < 3; i++) await new Promise(requestAnimationFrame);
	};
	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = 28;
	const context = canvas.getContext('2d');
	context.fillStyle = '#9363ec';
	context.fillRect(0, 0, 28, 28);
	context.fillStyle = '#fdba24';
	context.fillRect(6, 6, 16, 16);
	const icon = canvas.toDataURL();
	let next = 0;
	const makeMessage = () => {
		const index = next++;
		const badge = {
			set_id: 'fixture',
			id: '1',
			title: 'Badge',
			description: 'Fixture badge',
			image_url_1x: icon,
			image_url_2x: icon,
			image_url_4x: icon
		};
		return {
			index,
			ts: '2026-10-25T16:15:00Z',
			broadcaster_user_id: '1',
			broadcaster_user_name: 'Fixture',
			broadcaster_user_login: 'fixture',
			chatter_user_id: String(index),
			chatter_user_name: 'User' + index,
			message_id: String(index),
			text: 'fixed text to inspect vertical movement',
			message_type: 'text',
			color: '#70c7ff',
			translation: null,
			badges: index % 3 ? [{ set_id: 'fixture', id: '1', info: '', badge }] : [],
			fragments: [
				{ Text: { text: 'fixed text to inspect vertical movement ' } },
				...(index % 3 === 2
					? [
							{
								Emote: {
									emote: {
										id: '1',
										name: 'Icon',
										tier: '',
										provider: 'twitch',
										url: icon,
										format: [],
										scale: [],
										theme_mode: [],
										emote_type: '',
										emote_set_id: ''
									}
								}
							}
						]
					: [])
			]
		};
	};
	const emit = (count) => {
		for (let i = 0; i < count; i++)
			native.handlers.get('chat_message:fixture')({ payload: makeMessage() });
	};
	const waitCount = async (count) => {
		for (let i = 0; i < 2000; i++) {
			await tick();
			if (rows().length === count) return;
			await new Promise((resolve) => setTimeout(resolve, 5));
		}
		throw new Error('Timed out waiting for ' + count + ' rows; got ' + rows().length);
	};
	const metadata = () => ({
		count: rows().length,
		first: rows()[0]?.dataset.chatMessageIndex,
		last: rows().at(-1)?.dataset.chatMessageIndex,
		spacer: spacer(),
		distance: feed.scrollHeight - feed.clientHeight - feed.scrollTop,
		paused: !!document.querySelector('button[aria-label^="Jump to"]')
	});
	emit(500);
	await waitCount(500);
	await Promise.all([...feed.querySelectorAll('img')].map((image) => image.decode()));
	await document.fonts.ready;
	await frames();
	native.settings.chat.message_gc_interval_ms = 1;
	const tail = rows().at(-1);
	let baseline;
	const box = feed.getBoundingClientRect();
	const clip = { x: box.left + 8, y: box.top + 35, width: 300, height: 120, scale: 1 };
	window.routeFixture = {
		metadata,
		emit: async (count) => {
			emit(count);
			await tick();
			return metadata();
		},
		cleanup: async (count) => {
			const geometry = () => ({
				top: tail.getBoundingClientRect().top,
				scrollTop: feed.scrollTop,
				scrollHeight: feed.scrollHeight,
				clientHeight: feed.clientHeight,
				viewportHeight: feed.getBoundingClientRect().height,
				listHeight: feed.firstElementChild.getBoundingClientRect().height,
				spacer: spacer()
			});
			const beforeGeometry = geometry();
			const samples = [];
			const before = tail.getBoundingClientRect().top;
			let movement = 0,
				active = true;
			const sample = () => {
				if (samples.length < 10) samples.push(geometry());
				movement = Math.max(movement, Math.abs(tail.getBoundingClientRect().top - before));
				if (active) requestAnimationFrame(sample);
			};
			requestAnimationFrame(sample);
			native.settings.chat.message_limit -= count;
			await waitCount(native.settings.chat.message_limit);
			await frames();
			active = false;
			movement = Math.max(movement, Math.abs(tail.getBoundingClientRect().top - before));
			return {
				...metadata(),
				movement,
				...(movement > 0.001 ? { beforeGeometry, samples, afterGeometry: geometry() } : {})
			};
		},
		setChat: async (update) => {
			Object.assign(native.settings.chat, update);
			await tick();
		},
		pause: async () => {
			feed.dispatchEvent(new WheelEvent('wheel', { deltaY: -100, bubbles: true }));
			feed.scrollTop -= 100;
			await new Promise((resolve) => setTimeout(resolve, 300));
			await frames();
			return metadata();
		},
		anchor: () => {
			const row = rows().find(
				(row) => row.getBoundingClientRect().bottom > feed.getBoundingClientRect().top
			);
			return { key: row.dataset.chatMessageIndex, top: row.getBoundingClientRect().top };
		},
		anchorTop: (key) =>
			rows()
				.find((row) => row.dataset.chatMessageIndex === key)
				?.getBoundingClientRect().top,
		resume: async () => {
			document.querySelector('button[aria-label^="Jump to"]').click();
			await frames();
			return metadata();
		},
		replaceOnResume: async () => {
			emit(2);
			native.settings.chat.message_limit = 1;
			document.querySelector('button[aria-label^="Jump to"]').click();
			await waitCount(1);
			await frames();
			return { ...metadata(), expectedLast: String(next - 1) };
		},
		compare: async (png) => {
			const bytes = Uint8Array.from(atob(png), (character) => character.charCodeAt(0));
			const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
			const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
			const ctx = surface.getContext('2d');
			ctx.drawImage(bitmap, 0, 0);
			const pixels = ctx.getImageData(0, 0, bitmap.width, bitmap.height).data;
			bitmap.close();
			if (!baseline) {
				baseline = pixels;
				return { changedPixels: 0, nonImageChangedPixels: 0 };
			}
			const images = [...feed.querySelectorAll('img')]
				.map((image) => image.getBoundingClientRect())
				.filter((rect) => rect.bottom >= clip.y && rect.top <= clip.y + clip.height);
			let changedPixels = 0,
				nonImageChangedPixels = 0;
			for (let i = 0; i < pixels.length; i += 4) {
				if (
					pixels[i] === baseline[i] &&
					pixels[i + 1] === baseline[i + 1] &&
					pixels[i + 2] === baseline[i + 2]
				)
					continue;
				changedPixels++;
				const x = clip.x + ((i / 4) % surface.width) / devicePixelRatio;
				const y = clip.y + Math.floor(i / 4 / surface.width) / devicePixelRatio;
				const edge = 1 / devicePixelRatio;
				if (
					!images.some(
						(rect) =>
							x >= rect.left - edge &&
							x <= rect.right + edge &&
							y >= rect.top - edge &&
							y <= rect.bottom + edge
					)
				)
					nonImageChangedPixels++;
			}
			return { changedPixels, nonImageChangedPixels };
		}
	};
	return { clip, dpr: devicePixelRatio };
}

async function buildFixture() {
	await mkdir(output, { recursive: true });
	await writeFile(
		path.join(output, 'native.svelte.js'),
		`
import { DEFAULT_APP_SETTINGS } from '$lib/settings';
export const settings = $state(structuredClone(DEFAULT_APP_SETTINGS));
export const page = { params: { id: 'fixture' } };
export const authState = { obj: { phase: 'authorized', token: { login: 'fixture', user_id: '1' } } };
export const getNormalizedAppSettings = () => settings;
export const native = { settings, handlers: new Map() };
export const listen = async (name, handler) => { native.handlers.set(name, handler); return () => native.handlers.delete(name); };
export const commands = { joinChat: async () => ({ status: 'ok', data: {} }), leaveChat: async () => ({ status: 'ok', data: null }), searchEmotes: async () => ({ status: 'ok', data: [] }) };
export default { debug() {}, info() {}, error() {} };
`
	);
	await writeFile(
		path.join(output, 'main.js'),
		`
import { mount, tick } from 'svelte';
import Route from '../../src/routes/app/chat/[id]/+page.svelte';
import { native } from './native.svelte.js';
import '../../src/app.css';
document.body.style.height = '100vh';
mount(Route, { target: document.body });
window.fixtureReady = (${createRouteFixture.toString()})(native, tick);
`
	);
	await writeFile(
		path.join(output, 'index.html'),
		'<!doctype html><html class="dark"><head><meta charset="UTF-8"></head><body><script type="module" src="/main.js"></script></body></html>'
	);
	const mocks = path.join(output, 'native.svelte.js');
	await build({
		configFile: false,
		root: output,
		logLevel: 'warn',
		plugins: [svelte({ configFile: false }), tailwindcss()],
		resolve: {
			alias: [
				{ find: /^\$lib\/bindings(?:\.ts)?$/, replacement: mocks },
				...[
					'$app/state',
					'$utils/log',
					'$lib/stores/auth.svelte',
					'$lib/stores/settings.svelte',
					'@tauri-apps/api/event'
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
	return {
		origin: 'http://127.0.0.1:' + server.address().port,
		close: () => new Promise((resolve) => server.close(resolve))
	};
}

const { values } = parseArgs({
	options: { browser: { type: 'string' }, scale: { type: 'string' } }
});
assert(
	values.browser,
	'Usage: node scripts/check-chat-scroll-route.mjs --browser <browser executable> [--scale <DPR>]'
);
const fixture = await buildFixture();
try {
	for (const scale of values.scale ? [Number(values.scale)] : [1, 1.1, 1.25, 1.5, 1.75, 2]) {
		await checkScale(values.browser, scale, fixture.origin, async ({ command, evaluate }) => {
			for (let i = 0; i < 100 && !(await evaluate('!!window.fixtureReady')); i++) await delay(50);
			const initial = await evaluate('window.fixtureReady');
			assert(initial, 'Route did not initialize');
			const capture = async () => {
				const shot = await command('Page.captureScreenshot', {
					format: 'png',
					fromSurface: true,
					captureBeyondViewport: false,
					clip: initial.clip
				});
				return evaluate('window.routeFixture.compare(' + JSON.stringify(shot.data) + ')');
			};
			await capture();
			let maxMovement = 0,
				maxChangedPixels = 0;
			for (let pass = 0; pass < 24; pass++) {
				const state = await evaluate('window.routeFixture.cleanup(' + [1, 3, 8, 5][pass % 4] + ')');
				maxMovement = Math.max(maxMovement, state.movement);
				assert(
					state.movement < 0.001,
					'DPR ' + scale + ': cleanup frame moved ' + JSON.stringify(state)
				);
				assert(state.spacer <= 10, 'Spacer exceeded ten CSS pixels');
				const pixels = await capture();
				maxChangedPixels = Math.max(maxChangedPixels, pixels.changedPixels);
				assert(
					pixels.changedPixels === 0 || (scale === 1.75 && pixels.nonImageChangedPixels === 0),
					'DPR ' + scale + ': retained pixels changed ' + JSON.stringify(pixels)
				);
			}
			if (scale === 1) {
				await command('Emulation.setDeviceMetricsOverride', {
					width: 800,
					height: 600,
					deviceScaleFactor: 1.25,
					mobile: false
				});
				await delay(100);
				assert(
					(await evaluate('window.routeFixture.cleanup(3)')).movement < 0.001,
					'Scale change cleanup moved'
				);
				await command('Emulation.setDeviceMetricsOverride', {
					width: 800,
					height: 600,
					deviceScaleFactor: 1.234567,
					mobile: false
				});
				await delay(100);
				assert.equal(
					(await evaluate('window.routeFixture.cleanup(3)')).spacer,
					0,
					'Unsupported scale did not use zero spacer'
				);
				await command('Emulation.clearDeviceMetricsOverride');
				await delay(100);
			}
			await evaluate(
				'window.routeFixture.setChat({ message_limit: 500, message_gc_interval_ms: 250 })'
			);
			const live = await evaluate('window.routeFixture.emit(130)');
			assert(live.count > 500, 'Live arrivals did not publish before cleanup');
			await delay(350);
			const collected = await evaluate('window.routeFixture.metadata()');
			assert.equal(collected.count, 500);
			assert(Math.abs(collected.distance) < 1, 'Live following lost bottom');
			assert((await evaluate('window.routeFixture.pause()')).paused, 'Wheel did not pause route');
			const anchor = await evaluate('window.routeFixture.anchor()');
			assert.equal(
				(await evaluate('window.routeFixture.emit(25)')).count,
				500,
				'Paused arrivals published immediately'
			);
			await delay(350);
			assert.equal(
				(await evaluate('window.routeFixture.metadata()')).count,
				525,
				'Paused history was collected'
			);
			assert(
				Math.abs(
					(await evaluate('window.routeFixture.anchorTop(' + JSON.stringify(anchor.key) + ')')) -
						anchor.top
				) < 0.001,
				'Paused anchor moved'
			);
			const resumed = await evaluate('window.routeFixture.resume()');
			assert.equal(resumed.count, 500);
			assert(Math.abs(resumed.distance) < 1, 'Resume lost bottom');
			await evaluate('window.routeFixture.pause()');
			const pausedAnchor = await evaluate('window.routeFixture.anchor()');
			await evaluate('window.routeFixture.setChat({ pause_message_gc_while_paused: false })');
			await evaluate('window.routeFixture.emit(25)');
			await delay(650);
			assert.equal(
				(await evaluate('window.routeFixture.metadata()')).count,
				500,
				'Opt-in paused cleanup did not run'
			);
			assert(
				Math.abs(
					(await evaluate(
						'window.routeFixture.anchorTop(' + JSON.stringify(pausedAnchor.key) + ')'
					)) - pausedAnchor.top
				) < 0.001,
				'Opt-in paused cleanup moved anchor'
			);
			const replaced = await evaluate('window.routeFixture.replaceOnResume()');
			assert.equal(replaced.count, 1);
			assert.equal(replaced.last, replaced.expectedLast);
			assert.equal(replaced.spacer, 0);
			console.log(
				JSON.stringify({
					route: true,
					dpr: scale,
					cleanupPasses: 24,
					maxMovement,
					maxChangedPixels,
					livePauseResume: true,
					pausedCleanup: true,
					pendingReplacement: true
				})
			);
		});
	}
} finally {
	await fixture.close();
}
