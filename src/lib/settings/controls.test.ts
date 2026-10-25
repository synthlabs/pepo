import { describe, expect, it } from 'vitest';
import { DEFAULT_APP_SETTINGS } from '$lib/settings';
import {
	applySettingsPatch,
	applySettingsUpdate,
	SettingsAutosave,
	type SettingsSnapshot
} from './autosave';
import {
	SETTINGS_CONTROLS,
	controlUpdate,
	matchingControls,
	resetSettingsPatch,
	validateControlInput
} from './controls';

describe('settings controls', () => {
	it('does not reapply a failed provider toggle when another provider toggle is queued', async () => {
		let snapshot!: SettingsSnapshot;
		const requests: ((saved: boolean) => void)[] = [];
		const writer = new SettingsAutosave(
			() => new Promise<boolean>((resolve) => requests.push(resolve)),
			(value) => {
				snapshot = value;
			}
		);
		writer.receive(structuredClone(DEFAULT_APP_SETTINGS));
		const twitch = SETTINGS_CONTROLS.find((control) => control.provider === 'twitch')!;
		const bttv = SETTINGS_CONTROLS.find((control) => control.provider === 'bttv')!;
		const first = writer.update(controlUpdate(twitch, false));
		const second = writer.update(controlUpdate(bttv, false));
		requests[0](false);
		await first;
		requests[1](true);
		await second;
		expect(snapshot.value.emotes.providers.slice(0, 2)).toEqual([
			{ id: 'twitch', enabled: true },
			{ id: 'bttv', enabled: false }
		]);
	});
	it('covers every exposed field without operational fields', () => {
		const paths = [...new Set(SETTINGS_CONTROLS.map((control) => control.path))].sort();
		const expected = (['appearance', 'layout', 'diagnostics', 'chat', 'emotes'] as const)
			.flatMap((section) =>
				Object.keys(DEFAULT_APP_SETTINGS[section]).map((key) => `${section}.${key}`)
			)
			.sort();
		expect(paths).toEqual(expected);
		expect(new Set(SETTINGS_CONTROLS.map((control) => control.id)).size).toBe(
			SETTINGS_CONTROLS.length
		);
	});

	it('searches collapsed controls, category names, and option labels', () => {
		expect(matchingControls('cleanup').map((control) => control.id)).toContain(
			'chat.message_gc_interval_ms'
		);
		expect(matchingControls('connector').map((control) => control.id)).toEqual([
			'chat.translation_layout'
		]);
		expect(matchingControls('app theme').map((control) => control.id)).toEqual([
			'appearance.theme'
		]);
		expect(matchingControls('not-a-setting')).toEqual([]);
	});

	it.each(['', '0', '-1', '1.5', 'Infinity', '9007199254740992'])(
		'rejects invalid numeric input %j',
		(input) => {
			const control = SETTINGS_CONTROLS.find((item) => item.id === 'chat.message_limit')!;
			expect(validateControlInput(control, input).error).toBeDefined();
		}
	);

	it('enforces u32 bounds and accepts positive integer input', () => {
		const control = SETTINGS_CONTROLS.find((item) => item.id === 'emotes.inline_emote_px')!;
		expect(validateControlInput(control, '4294967296').error).toBeDefined();
		expect(validateControlInput(control, ' 32 ')).toEqual({ value: 32 });
	});

	it('validates locale syntax before formatting can throw', () => {
		const control = SETTINGS_CONTROLS.find((item) => item.kind === 'locale')!;
		expect(validateControlInput(control, ' en-GB ')).toEqual({ value: 'en-GB' });
		expect(validateControlInput(control, 'en_XX').error).toBeDefined();
		expect(validateControlInput(control, '').error).toBeDefined();
	});

	it('preserves provider ordering and other toggles when disabling one provider', () => {
		const settings = structuredClone(DEFAULT_APP_SETTINGS);
		settings.emotes.providers.reverse();
		const control = SETTINGS_CONTROLS.find((item) => item.provider === 'bttv')!;
		const next = applySettingsUpdate(settings, controlUpdate(control, false));
		expect(next.emotes.providers.map((provider) => provider.id)).toEqual([
			'seventv',
			'ffz',
			'bttv',
			'twitch'
		]);
		expect(next.emotes.providers.map((provider) => provider.enabled)).toEqual([
			true,
			true,
			false,
			true
		]);
	});

	it('resets only the selected category and preserves operational values on reset all', () => {
		const settings = structuredClone(DEFAULT_APP_SETTINGS);
		settings.appearance.theme = 'light';
		settings.diagnostics.capture_window_errors = false;
		settings.chat.message_limit = 900;
		settings.channel_cache.poll_interval_secs = 99;
		const chat = applySettingsPatch(settings, resetSettingsPatch('chat'));
		expect(chat.chat.message_limit).toBe(500);
		expect(chat.appearance.theme).toBe('light');
		expect(chat.diagnostics.capture_window_errors).toBe(false);
		const app = applySettingsPatch(settings, resetSettingsPatch('app'));
		expect(app.diagnostics.capture_window_errors).toBe(true);
		expect(app.chat.message_limit).toBe(900);
		const all = applySettingsPatch(settings, resetSettingsPatch('all'));
		expect(all.appearance.theme).toBe('dark');
		expect(all.channel_cache.poll_interval_secs).toBe(99);
	});
});
