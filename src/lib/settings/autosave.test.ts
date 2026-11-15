import { describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '$lib/bindings';
import { DEFAULT_APP_SETTINGS } from '$lib/settings';
import { SettingsAutosave, type SettingsSnapshot } from './autosave';

function fixture() {
	const requests: {
		value: AppSettings;
		resolve: (saved: boolean) => void;
		reject: (error: Error) => void;
	}[] = [];
	let state: SettingsSnapshot | undefined;
	const sync = vi.fn(
		(value: AppSettings) =>
			new Promise<boolean>((resolve, reject) => requests.push({ value, resolve, reject }))
	);
	const writer = new SettingsAutosave(sync, (snapshot) => {
		state = snapshot;
	});
	return {
		writer,
		requests,
		sync,
		get state() {
			return state!;
		},
		initialize() {
			writer.receive(structuredClone(DEFAULT_APP_SETTINGS));
		}
	};
}

describe('settings autosave', () => {
	it('persists translation processing preferences without replacing neighboring preferences', async () => {
		const test = fixture();
		test.initialize();
		const change = test.writer.update({
			translation: {
				translate_history: true,
				detection_policy: 'best_guess',
				queue_policy: 'oldest_first',
				queue_capacity: 32,
				max_in_flight: 8
			}
		});
		test.requests[0].resolve(true);
		expect(await change).toBe(true);
		expect(test.requests[0].value.translation).toEqual({
			...DEFAULT_APP_SETTINGS.translation,
			translate_history: true,
			detection_policy: 'best_guess',
			queue_policy: 'oldest_first',
			queue_capacity: 32,
			max_in_flight: 8
		});
		expect(test.requests[0].value.network).toEqual(DEFAULT_APP_SETTINGS.network);
		expect(test.state.value.translation).toEqual(test.requests[0].value.translation);
	});
	it('does not write before initial state is available', async () => {
		const test = fixture();
		expect(await test.writer.update({ appearance: { theme: 'light' } })).toBe(false);
		expect(test.sync).not.toHaveBeenCalled();
	});

	it('serializes edits, overlays pending values, and ignores stale echoes before and after acknowledgement', async () => {
		const test = fixture();
		test.initialize();
		const theme = test.writer.update({ appearance: { theme: 'light' } });
		const history = test.writer.update({ chat: { message_limit: 900 } });
		expect(test.requests).toHaveLength(1);
		expect(test.state.value.appearance.theme).toBe('light');
		expect(test.state.value.chat.message_limit).toBe(900);
		test.writer.receive(test.requests[0].value);
		expect(test.state.value.chat.message_limit).toBe(900);
		test.requests[0].resolve(true);
		await theme;
		expect(test.requests).toHaveLength(2);
		expect(test.requests[1].value.appearance.theme).toBe('light');
		expect(test.requests[1].value.chat.message_limit).toBe(900);
		test.requests[1].resolve(true);
		await history;
		test.writer.receive(test.requests[0].value);
		expect(test.state.value.chat.message_limit).toBe(900);
		expect(test.state.saving).toBe(false);
		expect(test.state.result).toBe('saved');
	});

	it.each(['false', 'rejection'])(
		'restores an unsuccessful edit on %s and ignores its late echo',
		async (failure) => {
			const test = fixture();
			test.initialize();
			const saved = test.writer.update({ appearance: { theme: 'light' } });
			if (failure === 'false') test.requests[0].resolve(false);
			else test.requests[0].reject(new Error('Native save failed'));
			expect(await saved).toBe(false);
			test.writer.receive(test.requests[0].value);
			expect(test.state.value.appearance.theme).toBe('dark');
			expect(test.state.result).toBe('error');
			expect(test.state.saving).toBe(false);
		}
	);

	it('keeps newer edits to the same field when an earlier save fails', async () => {
		const test = fixture();
		test.initialize();
		const earlier = test.writer.update({ chat: { message_limit: 700 } });
		const later = test.writer.update({ chat: { message_limit: 900 } });
		test.requests[0].resolve(false);
		await earlier;
		expect(test.state.value.chat.message_limit).toBe(900);
		expect(test.state.result).toBe('error');
		expect(test.requests[1].value.chat.message_limit).toBe(900);
		test.requests[1].resolve(true);
		await later;
		expect(test.state.result).toBe('saved');
	});

	it('rebases a queued emote change after a failed theme update', async () => {
		const test = fixture();
		test.initialize();
		const theme = test.writer.update({ appearance: { theme: 'light' } });
		const emotes = test.writer.update({ emotes: { autocomplete_enabled: false } });
		test.requests[0].reject(new Error('Disconnected'));
		await theme;
		expect(test.requests[1].value.appearance.theme).toBe('dark');
		expect(test.requests[1].value.emotes.autocomplete_enabled).toBe(false);
		test.requests[1].resolve(true);
		await emotes;
		expect(test.state.value.emotes.autocomplete_enabled).toBe(false);
	});

	it('rolls back to acknowledged values rather than initial defaults', async () => {
		const test = fixture();
		test.initialize();
		const first = test.writer.update({ chat: { message_limit: 800 } });
		test.requests[0].resolve(true);
		await first;
		const second = test.writer.update({ chat: { message_limit: 900 } });
		test.requests[1].resolve(false);
		await second;
		expect(test.state.value.chat.message_limit).toBe(800);
	});

	it('preserves unrelated incoming fields while applying pending patches', async () => {
		const test = fixture();
		test.initialize();
		const save = test.writer.update({ chat: { show_badges: false } });
		const external = structuredClone(DEFAULT_APP_SETTINGS);
		external.channel_cache.poll_interval_secs = 120;
		external.emotes.autocomplete_enabled = false;
		test.writer.receive(external);
		expect(test.state.value.chat.show_badges).toBe(false);
		expect(test.state.value.emotes.autocomplete_enabled).toBe(false);
		test.requests[0].resolve(true);
		await save;
		expect(test.state.value.channel_cache.poll_interval_secs).toBe(120);
		expect(DEFAULT_APP_SETTINGS.chat.show_badges).toBe(true);
	});
});
