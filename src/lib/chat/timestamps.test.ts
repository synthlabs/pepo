import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings, ChannelMessage, FeedFrame } from '$lib/bindings';

const native = vi.hoisted(() => ({
	listeners: new Map<string, Set<(event: { payload: AppSettings }) => void>>(),
	requests: [] as {
		value: AppSettings;
		resolve: (saved: boolean) => void;
		reject: (error: Error) => void;
	}[],
	receive: undefined as ((frame: FeedFrame) => void) | undefined,
	emit(value: AppSettings) {
		for (const listener of this.listeners.get('app_settings_update') ?? []) {
			listener({ payload: structuredClone(value) });
		}
	}
}));

vi.mock('@tauri-apps/api/event', () => ({
	listen: async (name: string, listener: (event: { payload: AppSettings }) => void) => {
		const listeners = native.listeners.get(name) ?? new Set();
		listeners.add(listener);
		native.listeners.set(name, listeners);
		return () => listeners.delete(listener);
	}
}));
vi.mock('@tauri-apps/api/core', () => ({
	invoke: async (command: string, args: { state?: { value: string } }) => {
		if (command === 'emit_state') return true;
		if (command !== 'update_state' || !args.state) throw new Error(`Unexpected command ${command}`);
		const value = JSON.parse(args.state.value) as AppSettings;
		return new Promise<boolean>((resolve, reject) =>
			native.requests.push({ value, resolve, reject })
		);
	}
}));
vi.mock('$app/state', () => ({ page: { params: { id: 'timestamps' } } }));
vi.mock('$lib/stores/auth.svelte', () => ({
	authState: { obj: { phase: 'unauthorized', token: null } }
}));
vi.mock('$utils/log', () => ({ default: { debug: vi.fn(), info: vi.fn(), error: vi.fn() } }));
vi.mock('svelte/transition', () => ({
	fade: () => ({ duration: 0 }),
	slide: () => ({ duration: 0 })
}));
vi.mock('$lib/chat/feed', () => ({
	openRoomFeed: (_channel: string, receive: (frame: FeedFrame) => void) => {
		native.receive = receive;
		return {
			ready: Promise.resolve({
				broadcaster_id: '1',
				broadcaster_login: 'timestamps',
				broadcaster_name: 'Timestamps'
			}),
			dispose: vi.fn()
		};
	}
}));
vi.mock('$lib/settings', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/settings')>();
	return { ...actual, formatTimestamp: vi.fn(actual.formatTimestamp) };
});

import ChatRoute from '../../routes/app/chat/[id]/+page.svelte';
import { DEFAULT_APP_SETTINGS, formatTimestamp } from '$lib/settings';
import { settingsState, updateAppSettings } from '$lib/stores/settings.svelte';

let component: ReturnType<typeof mount> | undefined;
let messages: ChannelMessage[];
let sequence = 0;

function message(index: number): ChannelMessage {
	return {
		index,
		channel_sequence: index,
		ts: '2026-10-25T16:15:00Z',
		broadcaster_user_id: '1',
		broadcaster_user_name: 'Timestamps',
		broadcaster_user_login: 'timestamps',
		chatter_user_id: String(index),
		chatter_user_name: `User${index}`,
		message_id: String(index),
		text: 'Hello',
		fragments: [{ Text: { index: 0, text: 'Hello' } }],
		message_type: 'text',
		badges: [],
		color: '#70c7ff',
		translation: null
	};
}

async function flushUi() {
	for (let pass = 0; pass < 3; pass++) {
		flushSync();
		await tick();
		await vi.advanceTimersByTimeAsync(1);
	}
}

function expectCalls(count: number) {
	expect(formatTimestamp).toHaveBeenCalledTimes(count);
	vi.mocked(formatTimestamp).mockClear();
}

function cells() {
	return [
		...document.querySelectorAll<HTMLSpanElement>('[data-chat-message-index] span.text-gray-500')
	];
}

function expectedText(ts = messages[0].ts) {
	const { timestamp_locale: locale, timestamp_style: timeStyle } = settingsState.value.chat;
	return new Intl.DateTimeFormat(locale, { timeStyle }).format(new Date(ts));
}

async function resetMessages() {
	native.receive!({ cursor: 500, event: { type: 'reset', messages: structuredClone(messages) } });
	await flushUi();
}

async function acknowledge(index: number, saved = true) {
	const request = native.requests[index];
	if (saved) native.emit(request.value);
	request.resolve(saved);
	await flushUi();
}

beforeEach(async () => {
	vi.useFakeTimers();
	vi.spyOn(console, 'log').mockImplementation(() => {});
	vi.stubGlobal('matchMedia', (media: string) => ({
		media,
		matches: false,
		addEventListener: vi.fn(),
		removeEventListener: vi.fn()
	}));
	await flushUi();
	native.requests = [];
	const baseline = structuredClone(DEFAULT_APP_SETTINGS);
	baseline.channel_cache.poll_interval_secs = 1000 + sequence++;
	baseline.chat.timestamp_locale = 'en-US';
	native.emit(baseline);
	await flushUi();
	const target = document.createElement('div');
	document.body.append(target);
	component = mount(ChatRoute, { target });
	await flushUi();
	messages = Array.from({ length: 500 }, (_, index) => message(index + 1));
	native.receive!({
		cursor: 500,
		event: { type: 'snapshot', messages: structuredClone(messages) }
	});
	await flushUi();
	expect(cells()).toHaveLength(500);
	expectCalls(500);
});

afterEach(async () => {
	for (let pass = 0; settingsState.saving && pass < 10; pass++) {
		for (const request of native.requests) request.resolve(false);
		await flushUi();
	}
	if (component) await unmount(component);
	component = undefined;
	document.body.replaceChildren();
	vi.mocked(formatTimestamp).mockClear();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('chat timestamp formatting volume', () => {
	it('does not reformat for sidebar edits, acknowledgements, or layout rollback', async () => {
		const originalCells = cells();
		const collapse = updateAppSettings({ layout: { sidebar_open: false } });
		await flushUi();
		expectCalls(0);
		await acknowledge(0);
		expect(await collapse).toBe(true);
		expectCalls(0);
		expect(settingsState).toMatchObject({ saving: false, result: 'saved' });

		const expand = updateAppSettings({ layout: { sidebar_open: true } });
		await flushUi();
		expectCalls(0);
		await acknowledge(1, false);
		expect(await expand).toBe(false);
		expectCalls(0);
		expect(settingsState.value.layout.sidebar_open).toBe(false);
		expect(settingsState).toMatchObject({ saving: false, result: 'error' });
		expect(cells()).toEqual(originalCells);
	});

	it('does not reformat for unrelated chat edits or fresh identical settings snapshots', async () => {
		const edit = updateAppSettings({ chat: { alternate_backgrounds: false } });
		await flushUi();
		expectCalls(0);
		await acknowledge(0);
		expect(await edit).toBe(true);
		expectCalls(0);
		const incoming = structuredClone(native.requests[0].value);
		incoming.network.metadata_concurrency += 1;
		native.emit(incoming);
		await flushUi();
		expectCalls(0);
		native.emit(structuredClone(incoming));
		await flushUi();
		expectCalls(0);
	});

	it.each([{ timestamp_locale: 'en-GB' }, { timestamp_style: 'long' as const }])(
		'reformats once per cell for %j and never again on acknowledgement',
		async (chat) => {
			const edit = updateAppSettings({ chat });
			await flushUi();
			expectCalls(500);
			expect(cells().every((cell) => cell.textContent === expectedText())).toBe(true);
			await acknowledge(0);
			expect(await edit).toBe(true);
			expectCalls(0);
		}
	);

	it.each(['false', 'rejection'])(
		'restores confirmed timestamp text on save %s',
		async (failure) => {
			const original = cells()[0].textContent;
			const edit = updateAppSettings({ chat: { timestamp_style: 'long' } });
			await flushUi();
			expectCalls(500);
			expect(cells()[0].textContent).toBe(expectedText());
			if (failure === 'false') native.requests[0].resolve(false);
			else native.requests[0].reject(new Error('Native save failed'));
			await flushUi();
			expect(await edit).toBe(false);
			expectCalls(500);
			expect(cells().every((cell) => cell.textContent === original)).toBe(true);
			native.emit(native.requests[0].value);
			await flushUi();
			expectCalls(0);
			expect(settingsState).toMatchObject({ saving: false, result: 'error' });
		}
	);

	it('retains a newer timestamp edit when an earlier save fails and ignores stale echoes', async () => {
		const earlier = updateAppSettings({ chat: { timestamp_locale: 'en-GB' } });
		await flushUi();
		expectCalls(500);
		const previousText = cells()[0].textContent;
		const later = updateAppSettings({ chat: { timestamp_locale: 'de-DE' } });
		await flushUi();
		expectCalls(500);
		expect(cells()[0].textContent).toBe(previousText);
		expect(native.requests).toHaveLength(1);
		const previousCells = cells();
		await acknowledge(0, false);
		expect(await earlier).toBe(false);
		expectCalls(0);
		expect(cells()).toEqual(previousCells);
		expect(native.requests[1].value.chat.timestamp_locale).toBe('de-DE');
		expect(cells()[0].textContent).toBe(expectedText());
		await acknowledge(1);
		expect(await later).toBe(true);
		expectCalls(0);
		native.emit(native.requests[0].value);
		await flushUi();
		expectCalls(0);
		expect(settingsState.value.chat.timestamp_locale).toBe('de-DE');
	});

	it('does not reformat a queued timestamp edit when the preceding layout save settles', async () => {
		const layout = updateAppSettings({ layout: { sidebar_open: false } });
		await flushUi();
		expectCalls(0);
		const locale = updateAppSettings({ chat: { timestamp_locale: 'en-GB' } });
		await flushUi();
		expectCalls(500);
		await acknowledge(0);
		expect(await layout).toBe(true);
		expectCalls(0);
		expect(native.requests[1].value.layout.sidebar_open).toBe(false);
		await acknowledge(1);
		expect(await locale).toBe(true);
		expectCalls(0);
	});

	it('only reformats changed timestamps when retained messages are replaced', async () => {
		await resetMessages();
		expectCalls(0);
		messages[0].ts = '2026-10-25T16:16:00Z';
		await resetMessages();
		expectCalls(1);
		expect(cells()[0].textContent).toBe(expectedText());
		const previousText = cells()[0].textContent;
		messages[0].ts = '2026-10-25T16:16:01Z';
		await resetMessages();
		expectCalls(1);
		expect(cells()[0].textContent).toBe(previousText);
		const layout = updateAppSettings({ layout: { sidebar_open: false } });
		await flushUi();
		expectCalls(0);
		await acknowledge(0);
		expect(await layout).toBe(true);
		expectCalls(0);
	});

	it('formats new messages once and does not reformat retained messages on removal', async () => {
		const next = message(501);
		native.receive!({ cursor: 501, event: { type: 'message', message: next } });
		await flushUi();
		expectCalls(1);
		expect(cells()).toHaveLength(501);
		messages = [...messages.slice(1), next];
		await resetMessages();
		expectCalls(0);
		expect(cells()).toHaveLength(500);
	});

	it('formats a new translation placeholder once and keeps existing cells stable', async () => {
		const visible = cells()[0];
		const translation = { source_language: 'rus', target_language: 'eng', translated_text: 'Hi' };
		const translate = async (translated_text: string) => {
			native.receive!({
				cursor: 501,
				event: {
					type: 'translation',
					update: { message_id: '1', translation: { ...translation, translated_text } }
				}
			});
			await flushUi();
		};
		await translate('Hi');
		expectCalls(1);
		const placeholder = cells()[1];
		expect(cells()[0]).toBe(visible);
		expect(visible.hasAttribute('aria-hidden')).toBe(false);
		expect(placeholder.getAttribute('aria-hidden')).toBe('true');
		expect(placeholder.classList.contains('invisible')).toBe(true);
		expect(placeholder.textContent).toBe(visible.textContent);
		await translate('Updated translation');
		expectCalls(0);
		const style = updateAppSettings({ chat: { timestamp_style: 'long' } });
		await flushUi();
		expectCalls(501);
		expect(visible.textContent).toBe(expectedText());
		expect(placeholder.textContent).toBe(visible.textContent);
		await acknowledge(0);
		expect(await style).toBe(true);
		expectCalls(0);
	});

	it('defers formatting while hidden and uses current preferences when shown again', async () => {
		const hide = updateAppSettings({ chat: { show_timestamps: false } });
		await flushUi();
		expectCalls(0);
		expect(cells()).toHaveLength(0);
		await acknowledge(0);
		expect(await hide).toBe(true);
		const edit = updateAppSettings({
			chat: { timestamp_locale: 'de-DE', timestamp_style: 'long' }
		});
		await flushUi();
		await acknowledge(1);
		expect(await edit).toBe(true);
		expectCalls(0);
		const show = updateAppSettings({ chat: { show_timestamps: true } });
		await flushUi();
		expectCalls(500);
		expect(cells().every((cell) => cell.textContent === expectedText())).toBe(true);
		await acknowledge(2);
		expect(await show).toBe(true);
		expectCalls(0);
	});
});
