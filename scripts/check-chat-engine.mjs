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

async function checkHistoricalTranslations() {
	const native = window.chatNative;
	const viewport = document.querySelector('[aria-label="Chat messages"]');
	const rows = () => [
		...viewport.querySelectorAll('[data-chat-message-index]:not([data-chat-system-message])')
	];
	const visibleRows = () => {
		const box = viewport.getBoundingClientRect();
		return rows().filter((row) => {
			const rect = row.getBoundingClientRect();
			return rect.bottom > box.top && rect.top < box.bottom;
		});
	};
	const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
	const settle = async () => {
		await wait(100);
		for (let i = 0; i < 3; i++) await new Promise(requestAnimationFrame);
	};
	const require = (condition, message) => {
		if (!condition) throw new Error(message);
	};
	const gap = () => Math.abs(viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop);
	const translated = (row) => !!row.querySelector('[data-chat-translation]');
	await document.fonts.ready;
	await settle();
	require(rows().length === 500 && gap() < 1, 'Restored history did not open at the bottom');
	const protectedRows = visibleRows();
	const baseline = protectedRows.map((row) => row.firstElementChild.getBoundingClientRect().top);
	const above = rows()
		.filter((row) => row.getBoundingClientRect().bottom < viewport.getBoundingClientRect().top)
		.slice(-40);
	let maxMovement = 0;
	let maxGap = 0;
	let samples = 0;
	let sampling = true;
	const sample = () => {
		maxMovement = Math.max(
			maxMovement,
			...protectedRows.map((row, i) =>
				Math.abs(row.firstElementChild.getBoundingClientRect().top - baseline[i])
			)
		);
		maxGap = Math.max(maxGap, gap());
		samples++;
	};
	const sampleFrame = () =>
		requestAnimationFrame(() => {
			setTimeout(() => {
				if (sampling) sample();
			}, 0);
			if (sampling) sampleFrame();
		});
	sampleFrame();
	try {
		for (const row of [...protectedRows, ...above]) {
			const id = row.dataset.chatMessageIndex;
			native.translate(
				'backlog',
				('Translated history ' + id + ' with text that wraps at narrow widths. ').repeat(3),
				id
			);
			await wait(12);
		}
		await settle();
		require(protectedRows.every(
			(row) => !translated(row)
		), 'A visible historical translation was revealed');
		require(above.every(translated), 'Offscreen historical translations were not revealed');
		native.reset();
		await settle();
		require(protectedRows.every(
			(row) => !translated(row)
		), 'A feed reset revealed protected translations');
		sample();
	} finally {
		sampling = false;
	}
	require(maxMovement < 1 && maxGap < 1, 'Historical catch-up moved visible originals: ' +
		JSON.stringify({ maxMovement, maxGap }));
	const protectedRow = protectedRows.at(-1);
	const protectedId = protectedRow.dataset.chatMessageIndex;
	viewport.dispatchEvent(new WheelEvent('wheel', { deltaY: -500, bubbles: true }));
	viewport.scrollTop -= viewport.clientHeight * 2;
	native.translate('backlog', 'translated-during-manual-scroll', protectedId);
	await wait(80);
	require(!translated(protectedRow), 'Historical translations changed during manual scrolling');
	const anchor = visibleRows()[0];
	const anchorTop = anchor.getBoundingClientRect().top;
	await wait(300);
	await settle();
	require(translated(protectedRow), 'A deferred translation was not revealed after leaving view');
	require(Math.abs(anchor.getBoundingClientRect().top - anchorTop) <
		1, 'Offscreen reveal moved the paused anchor');
	const scrollingPast = visibleRows()[0];
	const replacement = 'paused-above-translation '.repeat(60).trim();
	native.translate('backlog', replacement, scrollingPast.dataset.chatMessageIndex);
	await settle();
	require(!scrollingPast.textContent.includes(
		replacement
	), 'A visible historical replacement was revealed');
	viewport.dispatchEvent(new WheelEvent('wheel', { deltaY: 500, bubbles: true }));
	viewport.scrollTop += viewport.clientHeight * 0.8;
	await wait(80);
	const pausedAnchor = visibleRows()[0];
	const pausedTop = pausedAnchor.getBoundingClientRect().top;
	let pausedMovement = 0;
	let pausedSampling = true;
	const samplePausedFrame = () =>
		requestAnimationFrame(() => {
			setTimeout(() => {
				if (pausedSampling)
					pausedMovement = Math.max(
						pausedMovement,
						Math.abs(pausedAnchor.getBoundingClientRect().top - pausedTop)
					);
			}, 0);
			if (pausedSampling) samplePausedFrame();
		});
	samplePausedFrame();
	await wait(350);
	await settle();
	pausedSampling = false;
	require(scrollingPast.textContent.includes(
		replacement
	), 'A translation above the paused viewport was not revealed');
	require(pausedMovement < 1, 'A translation above the paused viewport moved its anchor: ' +
		pausedMovement);
	const jump = document.querySelector('button[aria-label^="Jump to"]');
	require(!!jump, 'Offscreen reveal unexpectedly resumed chat');
	jump.click();
	await settle();
	require(translated(protectedRow) &&
		gap() < 1, 'Scrolling back did not show the completed translation');
	native.push('backlog', 'live-after-backlog');
	await settle();
	native.translate('backlog', 'live-translation-after-backlog');
	await settle();
	require(rows()
		.at(-1)
		.textContent.includes('live-translation-after-backlog'), 'Live translation was deferred');
	require(gap() < 1, 'Live translation lost the bottom');
	const lastHistorical = rows().at(-2);
	const revisitId = lastHistorical.dataset.chatMessageIndex;
	native.translate('backlog', 'completed-before-revisit', revisitId);
	await settle();
	require(!lastHistorical.textContent.includes(
		'completed-before-revisit'
	), 'A visible replacement was revealed');
	native.translate('backlog', 'completed-before-revisit', revisitId);
	native.go(null);
	await settle();
	native.go('backlog');
	await settle();
	const revisited = document.querySelector('[data-chat-message-index="' + revisitId + '"]');
	require(revisited?.textContent.includes(
		'completed-before-revisit'
	), 'Completed translations were lost on navigation');
	return {
		maxMovement,
		maxGap,
		pausedMovement,
		samples,
		visibleProtected: protectedRows.length,
		offscreenTranslated: above.length
	};
}

async function checkSystemMessages() {
	const native = window.chatNative;
	const wait = (ms = 180) => new Promise((resolve) => setTimeout(resolve, ms));
	const require = (condition, message) => {
		if (!condition) throw new Error(message);
	};
	await native.go(null);
	await wait();
	Object.assign(native.settings.obj.chat, {
		message_limit: 1000,
		alternate_backgrounds: true,
		show_timestamps: true,
		center_system_messages: false
	});
	await native.settings.sync();
	native.seed('notices', 80);
	await native.go('notices');
	await wait();
	const viewport = document.querySelector('[aria-label="Chat messages"]');
	const rows = () => [...viewport.querySelectorAll('[data-chat-message-index]')];
	const notices = () => [...viewport.querySelectorAll('[data-chat-system-message]')];
	const gap = () => Math.abs(viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop);
	const previous = rows().at(-1);
	const previousBackground = getComputedStyle(previous).backgroundColor;
	for (const icon of ['info', 'shield', 'clock', 'success', 'warning', 'error']) {
		native.system('notices', {
			icon,
			segments: [{ type: 'text', text: 'Synthetic ' + icon + ' notice' }]
		});
	}
	native.push('notices', 'chat-after-notices');
	await wait();
	require(notices().length === 6, 'Missing system icons');
	require(notices().every((row) =>
		row.querySelector('svg[aria-hidden="true"]')
	), 'System icon is not decorative');
	require(!native.settings.obj.chat.show_system_message_background && notices().every((row) =>
		getComputedStyle(row).backgroundColor === 'rgba(0, 0, 0, 0)'
	), 'System backgrounds are not off by default');
	native.settings.obj.chat.show_system_message_background = true;
	await native.settings.sync();
	await wait();
	require(getComputedStyle(rows().at(-1)).backgroundColor !==
		previousBackground, 'Notices consumed ordinary row alternation');
	const backgrounds = new Set([
		previousBackground,
		getComputedStyle(rows().at(-1)).backgroundColor
	]);
	require(!backgrounds.has(
		getComputedStyle(notices()[0]).backgroundColor
	), 'System background matches a chat shade');
	const enabledBackground = getComputedStyle(notices()[0]).backgroundColor;
	native.system('notices', {
		icon: 'shield',
		segments: [
			{ type: 'user', user_id: 'viewer', login: 'viewer', display_name: 'Viewer' },
			{ type: 'text', text: ' was timed out. ' },
			{ type: 'emphasis', text: 'Synthetic wrapping notice. ' },
			{ type: 'text', text: 'LongNotice'.repeat(45) },
			{ type: 'link', text: ' Details', url: 'https://example.com/notice' }
		]
	});
	await wait();
	const notice = notices().at(-1);
	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = 1;
	const context = canvas.getContext('2d', { willReadFrequently: true });
	const luminance = (row, value) => {
		context.clearRect(0, 0, 1, 1);
		context.fillStyle = '#fff';
		context.fillRect(0, 0, 1, 1);
		const ancestors = [];
		for (let element = row; element; element = element.parentElement) ancestors.unshift(element);
		for (const element of ancestors) {
			context.fillStyle = getComputedStyle(element).backgroundColor;
			context.fillRect(0, 0, 1, 1);
		}
		context.fillStyle = value;
		context.fillRect(0, 0, 1, 1);
		return [...context.getImageData(0, 0, 1, 1).data]
			.slice(0, 3)
			.map((v) => v / 255)
			.map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4))
			.reduce((sum, v, i) => sum + v * [0.2126, 0.7152, 0.0722][i], 0);
	};
	const link = notice.querySelector('a');
	link.focus();
	let contrast = Infinity;
	let focusContrast = Infinity;
	const checkNoticeContrast = () => {
		const style = getComputedStyle(notice);
		const background = luminance(notice, 'transparent');
		const foreground = luminance(notice, style.color);
		const ratio = (value) => (Math.max(value, background) + 0.05) /
			(Math.min(value, background) + 0.05);
		const textContrast = ratio(foreground);
		contrast = Math.min(contrast, textContrast);
		require(textContrast >= 4.5, 'System text contrast below 4.5');
		require(Math.abs(foreground - background) <
			Math.abs(luminance(notice, getComputedStyle(document.body).color) - background),
			'System text is not muted');
		const focusStyle = getComputedStyle(link);
		const outlineContrast = ratio(luminance(notice, focusStyle.outlineColor));
		focusContrast = Math.min(focusContrast, outlineContrast);
		require(link.matches(':focus-visible') &&
			parseFloat(focusStyle.outlineWidth) >= 2 && outlineContrast >= 3,
			'System link keyboard focus is not sufficiently visible');
	};
	let maxIconCenterOffset = 0;
	const checkNoticeLayout = () => {
		const settings = native.settings.obj.chat;
		const divider = document.querySelector('[data-chat-history-divider]');
		const dividerLine = divider.querySelector('span');
		for (const row of notices()) {
			const rowStyle = getComputedStyle(row);
			const timestamp = row.querySelector('span.text-xs');
			require(!!timestamp === (settings.show_timestamps && !settings.center_system_messages),
				'System timestamp ignored visibility setting');
			require(rowStyle.backgroundColor === (settings.show_system_message_background
				? enabledBackground : 'rgba(0, 0, 0, 0)'), 'System background ignored its setting');
			const content = row.querySelector('[data-chat-system-message-content]').getBoundingClientRect();
			const lines = [...row.querySelectorAll('[data-chat-system-message-line]')];
			require(lines.length === (settings.center_system_messages ? 2 : 0),
				'System divider lines ignored alignment');
			if (settings.center_system_messages) {
				const rects = lines.map((line) => line.getBoundingClientRect());
				require(Math.abs(rects[0].width - rects[1].width) <= 0.5,
					'System divider lines are not equal widths');
				require(rowStyle.columnGap === getComputedStyle(divider).columnGap,
					'System divider spacing differs from the history divider');
				for (const [i, line] of lines.entries()) {
					const rect = rects[i];
					require(line.getAttribute('aria-hidden') === 'true', 'System divider is not decorative');
					require(rect.width >= 1.5 * parseFloat(getComputedStyle(document.documentElement).fontSize) &&
						rect.height === 1, 'System divider line dimensions are incorrect');
					require(getComputedStyle(line).backgroundColor === getComputedStyle(dividerLine).backgroundColor,
						'System divider line color differs from the history divider');
					require(Math.abs(rect.top + rect.height / 2 - (content.top + content.height / 2)) <= 0.5,
						'System divider lines are not vertically centered beside the content');
				}
			}
			for (const element of row.querySelectorAll('svg, span.text-xs, strong, a')) {
				require(getComputedStyle(element).color === rowStyle.color,
					'System icon, timestamp, or text did not inherit the muted color');
			}
			const icon = row.querySelector('svg');
			if (!icon) continue;
			const slot = icon.parentElement.getBoundingClientRect();
			const rect = icon.getBoundingClientRect();
			const offset = Math.max(
				Math.abs(rect.left + rect.width / 2 - (slot.left + slot.width / 2)),
				Math.abs(rect.top + rect.height / 2 - (slot.top + slot.height / 2))
			);
			maxIconCenterOffset = Math.max(maxIconCenterOffset, offset);
			require(offset <= 0.5, 'System icon is not centered within its slot');
			require(Math.abs(slot.height - parseFloat(rowStyle.lineHeight)) <= 0.5 &&
				Math.abs(slot.top - row.getBoundingClientRect().top -
					parseFloat(rowStyle.paddingTop)) <= 0.5,
				'System icon slot is not aligned with the first text line');
		}
	};
	for (const fontSize of [16, 32]) {
		document.documentElement.style.fontSize = fontSize + 'px';
		for (const centered of [false, true]) {
			for (const timestamps of [true, false]) {
				for (const background of [false, true]) {
					for (const striped of [true, false]) {
						Object.assign(native.settings.obj.chat, {
							center_system_messages: centered,
							show_timestamps: timestamps,
							show_system_message_background: background,
							alternate_backgrounds: striped
						});
						await native.settings.sync();
						await wait();
						checkNoticeLayout();
						checkNoticeContrast();
						require(getComputedStyle(notice).textAlign ===
							(centered ? 'center' : 'start'), 'System alignment did not update');
						require(notices().every((row) => row.scrollWidth <= row.clientWidth + 1),
							'System content overflows');
						require(getComputedStyle(previous).backgroundColor ===
							(striped ? previousBackground : 'rgba(0, 0, 0, 0)'),
							'System settings changed ordinary message backgrounds');
						require(gap() < 1, 'Notice reflow lost pinned bottom');
					}
				}
			}
		}
	}
	document.documentElement.style.fontSize = '16px';
	native.settings.obj.chat.alternate_backgrounds = false;
	native.settings.obj.chat.show_timestamps = false;
	await native.settings.sync();
	await wait();
	require(getComputedStyle(notice).backgroundColor ===
		enabledBackground, 'Disabling striping removed the system background');
	require(!notice.querySelector('span.text-xs'), 'System timestamp ignored visibility setting');
	native.settings.obj.chat.alternate_backgrounds = true;
	native.settings.obj.chat.show_timestamps = true;
	await native.settings.sync();
	await wait();
	viewport.dispatchEvent(new WheelEvent('wheel', { deltaY: -100, bubbles: true }));
	viewport.scrollTop = viewport.scrollHeight / 2;
	viewport.dispatchEvent(new Event('scroll'));
	await wait();
	const box = viewport.getBoundingClientRect();
	const anchor = rows().find((row) => row.getBoundingClientRect().top >= box.top);
	const top = anchor.getBoundingClientRect().top;
	native.system('notices', { segments: [{ type: 'text', text: 'Synthetic paused notice' }] });
	native.push('notices', 'paused-chat');
	await wait();
	require(Math.abs(anchor.getBoundingClientRect().top - top) <
		1, 'Mixed arrivals moved paused anchor');
	const paused = [...document.querySelectorAll('button')].find((button) =>
		button.textContent.includes('chat paused')
	);
	require(paused?.textContent.includes('2 new messages'), 'System notice not counted as unread');
	paused.click();
	await wait();
	require(gap() < 1, 'Jump did not reach mixed timeline bottom');
	native.reset();
	await wait();
	require(notices().length === 8, 'Reset lost notices');
	await native.go(null);
	await wait();
	await native.go('notices');
	await wait();
	require(document.querySelectorAll('[data-chat-system-message]').length ===
		8, 'Channel return lost notices');
	const originalFrame = window.requestAnimationFrame;
	let callbacks = 0;
	window.requestAnimationFrame = (callback) =>
		originalFrame((time) => {
			callbacks++;
			callback(time);
		});
	try {
		native.system('notices', { segments: [{ type: 'text', text: 'Synthetic final notice' }] });
		native.push('notices', 'final-chat');
		await wait(300);
		const settled = callbacks;
		await wait(180);
		require(callbacks === settled, 'Mixed arrivals left an animation frame loop running');
	} finally {
		window.requestAnimationFrame = originalFrame;
	}
	return { contrast, focusContrast, maxIconCenterOffset, count: 9, idleFrames: callbacks };
}

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
    kind: 'chat', message_type: 'text', badges: [], color: '#ffffff', translation: null };
};
for (const channel of ['rooma', 'roomb']) histories.set(channel, []);
for (const i of [1,2,3]) for (const channel of ['rooma', 'roomb']) histories.get(channel).push(message(channel, channel + '-history-' + i));
native.push = (channel, text) => {
  const msg = message(channel, text); const history = histories.get(channel) ?? [];
  history.push(msg); histories.set(channel, history.slice(-(native.settings?.obj.chat.message_limit ?? 500)));
  for (const feed of native.feeds.values()) if (feed.channel === channel) feed.onmessage({ cursor: sequence, event: { type: 'message', message: msg } });
};
const system = (channel, input) => ({ kind: 'system', index: ++sequence, ts: input.ts ?? '2026-11-14T19:00:00Z', message_id: input.message_id ?? 'system-' + sequence, broadcaster_user_id: channel, broadcaster_user_login: channel, broadcaster_user_name: channel, icon: input.icon ?? null, segments: input.segments });
native.system = (channel, input) => {
  const msg = system(channel, input); const history = histories.get(channel) ?? [];
  history.push(msg); histories.set(channel, history.slice(-(native.settings?.obj.chat.message_limit ?? 500)));
  for (const feed of native.feeds.values()) if (feed.channel === channel) feed.onmessage({ cursor: sequence, event: { type: 'message', message: msg } });
};
native.seed = (channel, count, mixed = false) => {
  const rows = [];
  for (let i = 0; i < count; i++) {
    rows.push(message(channel, 'backlog-original-' + i));
    if (mixed && i % 125 === 0) rows.push(system(channel, { segments: [{ type: 'text', text: 'Synthetic retained notice' }] }));
  }
  histories.set(channel, rows);
};
native.translate = (channel, text, id) => {
  const history = histories.get(channel);
  const msg = id === undefined ? history.findLast(message => message.kind === 'chat') : history.find(message => message.kind === 'chat' && message.message_id === id);
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
		for (const layout of ['language_tag', 'message_text', 'timestamp_end', 'connector']) {
			await evaluate(
				sessionId,
				`chatNative.settings.obj.translation.layout = '${layout}'; chatNative.settings.sync()`
			);
			await waitFor(sessionId, "document.body.textContent.includes('translated-live')");
			assert.equal(
				await evaluate(sessionId, "document.body.textContent.includes('visible-new')"),
				true
			);
			assert.equal(
				await evaluate(sessionId, "!!document.querySelector('[data-translation-connector]')"),
				layout === 'connector'
			);
			assertStable(beforeTranslation, await readRows(sessionId));
		}
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
		await evaluate(
			sessionId,
			`document.querySelector('[aria-controls="settings-more-chat"]').click()`
		);
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
		assert.equal(
			await evaluate(sessionId, "document.querySelectorAll('[data-chat-input-error]').length"),
			1
		);
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
		assert.equal(
			await evaluate(sessionId, "document.querySelectorAll('[data-chat-input-error]').length"),
			1
		);
		assert.deepEqual(exceptions.get(sessionId) ?? [], []);
		console.log(
			JSON.stringify({
				width,
				theme,
				systems: await evaluate(sessionId, '(' + checkSystemMessages.toString() + ')()')
			})
		);
		for (const centered of [false, true]) {
			for (const background of [false, true]) {
				await evaluate(
					sessionId,
					`Object.assign(chatNative.settings.obj.chat, { center_system_messages: ${centered}, show_system_message_background: ${background} }); chatNative.settings.sync()`
				);
				await waitFor(
					sessionId,
					`getComputedStyle(document.querySelector('[data-chat-system-message]')).textAlign === '${centered ? 'center' : 'start'}' && (getComputedStyle(document.querySelector('[data-chat-system-message]')).backgroundColor !== 'rgba(0, 0, 0, 0)') === ${background}`
				);
				await evaluate(sessionId, "document.querySelector('[data-chat-system-message] a').focus()");
				const systemShot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
				await writeFile(
					path.join(
						output,
						'systems-' + width + '-' + theme + '-' + (centered ? 'center' : 'left') +
						(background ? '-background' : '') + '.png'
					),
					Buffer.from(systemShot.data, 'base64')
				);
			}
		}
		assert.deepEqual(exceptions.get(sessionId) ?? [], []);
		await send('Target.closeTarget', { targetId });
	}
	const backlogResults = [];
	for (const width of [1200, 375]) {
		for (const scale of [1, 1.25]) {
			for (const layout of ['language_tag', 'message_text', 'timestamp_end', 'connector']) {
				const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
				const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
				await send('Runtime.enable', {}, sessionId);
				await send(
					'Emulation.setDeviceMetricsOverride',
					{ width, height: 800, deviceScaleFactor: scale, mobile: false },
					sessionId
				);
				await send('Page.navigate', { url: origin + '/app/chat/rooma' }, sessionId);
				await waitFor(
					sessionId,
					"document.querySelectorAll('[data-chat-message-index]').length === 3"
				);
				await evaluate(sessionId, 'chatNative.go(null)');
				await waitFor(sessionId, 'chatNative.feeds.size === 0');
				await evaluate(
					sessionId,
					`chatNative.settings.obj.chat.message_limit = 1000; chatNative.settings.obj.translation.layout = '${layout}'; chatNative.settings.sync(); chatNative.seed('backlog', 500, true); chatNative.go('backlog')`
				);
				await waitFor(
					sessionId,
					"document.querySelectorAll('[data-chat-message-index]').length === 504"
				);
				const result = await evaluate(
					sessionId,
					'(' + checkHistoricalTranslations.toString() + ')()'
				);
				backlogResults.push({ width, scale, layout, ...result });
				console.log(JSON.stringify(backlogResults.at(-1)));
				if (scale === 1) {
					const shot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
					await writeFile(
						path.join(output, 'backlog-' + width + '-' + layout + '.png'),
						Buffer.from(shot.data, 'base64')
					);
				}
				await evaluate(
					sessionId,
					'chatNative.settings.obj.chat.message_limit = 1; chatNative.settings.sync(); chatNative.reset()'
				);
				await waitFor(
					sessionId,
					"document.querySelectorAll('[data-chat-message-index]').length === 1"
				);
				assert.deepEqual(exceptions.get(sessionId) ?? [], []);
				await send('Target.closeTarget', { targetId });
			}
		}
	}
	await writeFile(
		path.join(output, 'backlog-results.json'),
		JSON.stringify(backlogResults, null, 2)
	);
	console.log(
		'Passed chat feeds, stable alternating rows, translations, history catch-up, divider/settings toggles, trimming, recovery, and room navigation in dark/light themes at desktop/mobile widths.'
	);
} finally {
	if (send) await send('Browser.close').catch(() => browser.kill());
	else browser.kill();
	socket?.close();
	await new Promise((resolve) => server.close(resolve));
}
