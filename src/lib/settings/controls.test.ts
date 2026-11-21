import { describe, expect, it } from 'vitest';
import { DEFAULT_APP_SETTINGS, normalizeAppSettings } from '$lib/settings';
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
	it('defaults, searches, resets, and persists user-card behavior settings', async () => {
		const defaults = {
			user_card_close_on_unfocus: true,
			user_card_chrome: 'compact',
			user_card_placement: 'near_username'
		};
		const legacy = JSON.parse(JSON.stringify(DEFAULT_APP_SETTINGS));
		for (const key of Object.keys(defaults)) delete legacy.chat[key];
		expect(normalizeAppSettings(legacy).chat).toMatchObject(defaults);
		const patch = {
			user_card_close_on_unfocus: false,
			user_card_chrome: 'native',
			user_card_placement: 'os'
		} as const;
		const saved = normalizeAppSettings(applySettingsPatch(DEFAULT_APP_SETTINGS, { chat: patch }));
		expect(saved.chat).toMatchObject(patch);
		for (const key of Object.keys(defaults)) {
			const control = SETTINGS_CONTROLS.find((control) => control.id === `chat.${key}`)!;
			expect(matchingControls(control.label)).toContain(control);
			expect(control.more ?? false).toBe(key !== 'user_card_close_on_unfocus');
		}
		for (const category of ['chat', 'all'] as const)
			expect(resetSettingsPatch(category).chat).toMatchObject(defaults);
	});
	it('defaults, searches, resets, and persists the three extra username controls', async () => {
		const defaults = {
			show_login_for_different_display_name: true,
			use_display_names_in_mentions: false,
			bold_mentions: true
		};
		const legacy = structuredClone(DEFAULT_APP_SETTINGS);
		for (const field of Object.keys(defaults))
			delete (legacy.chat as unknown as Record<string, unknown>)[field];
		expect(normalizeAppSettings(legacy).chat).toMatchObject(defaults);
		let saved = structuredClone(DEFAULT_APP_SETTINGS);
		let snapshot!: SettingsSnapshot;
		const writer = new SettingsAutosave(
			async (settings) => {
				saved = structuredClone(settings);
				return true;
			},
			(value) => {
				snapshot = value;
			}
		);
		writer.receive(saved);
		for (const [field, value] of Object.entries(defaults)) {
			const control = SETTINGS_CONTROLS.find((item) => item.id === `chat.${field}`)!;
			expect(control).toMatchObject({ category: 'chat', more: true, kind: 'toggle' });
			expect(matchingControls(control.label)).toContain(control);
			expect(await writer.update(controlUpdate(control, !value))).toBe(true);
			writer.receive(JSON.parse(JSON.stringify(saved)));
			expect(snapshot.value.chat[field as keyof typeof defaults]).toBe(!value);
			for (const category of ['chat', 'all'] as const)
				expect(resetSettingsPatch(category).chat?.[field as keyof typeof defaults]).toBe(value);
		}
	});
	it('keeps moderation notices in advanced Chat settings and resets to enabled', () => {
		const control = SETTINGS_CONTROLS.find(
			(item) => item.id === 'chat.moderation_notices_enabled'
		)!;
		expect(control).toMatchObject({
			more: true,
			category: 'chat',
			kind: 'toggle',
			label: 'Receive moderation notices'
		});
		expect(matchingControls('ban timeout').map((item) => item.id)).toContain(control.id);
		expect(resetSettingsPatch('chat').chat?.moderation_notices_enabled).toBe(true);
	});
	it('keeps notice alignment in Advanced, searchable, and resettable with a legacy default', () => {
		const control = SETTINGS_CONTROLS.find((item) => item.id === 'chat.center_system_messages')!;
		expect(control.more).toBe(true);
		expect(matchingControls('system messages')).toContain(control);
		expect(controlUpdate(control, true)).toEqual({ chat: { center_system_messages: true } });
		expect(resetSettingsPatch('chat').chat?.center_system_messages).toBe(false);
		const legacy = JSON.parse(JSON.stringify(DEFAULT_APP_SETTINGS));
		delete legacy.chat.center_system_messages;
		expect(normalizeAppSettings(legacy).chat.center_system_messages).toBe(false);
	});
	it('keeps the system background beside centering, searchable, and resettable to off', () => {
		const control = SETTINGS_CONTROLS.find(
			(item) => item.id === 'chat.show_system_message_background'
		)!;
		expect(control).toMatchObject({ more: true, category: 'chat', kind: 'toggle' });
		expect(SETTINGS_CONTROLS[SETTINGS_CONTROLS.indexOf(control) - 1].id).toBe(
			'chat.center_system_messages'
		);
		expect(matchingControls('system background')).toContain(control);
		expect(controlUpdate(control, true)).toEqual({
			chat: { show_system_message_background: true }
		});
		for (const category of ['chat', 'all'] as const) {
			expect(resetSettingsPatch(category).chat?.show_system_message_background).toBe(false);
		}
	});
	it('autosaves and restores the system background independently of centering and striping', async () => {
		let snapshot!: SettingsSnapshot;
		let saved = structuredClone(DEFAULT_APP_SETTINGS);
		const writer = new SettingsAutosave(
			async (settings) => {
				saved = structuredClone(settings);
				return true;
			},
			(value) => {
				snapshot = value;
			}
		);
		writer.receive(saved);
		const control = SETTINGS_CONTROLS.find(
			(item) => item.id === 'chat.show_system_message_background'
		)!;
		for (const enabled of [true, false]) {
			expect(await writer.update(controlUpdate(control, enabled))).toBe(true);
			writer.receive(JSON.parse(JSON.stringify(saved)));
			expect(snapshot.value.chat).toMatchObject({
				show_system_message_background: enabled,
				center_system_messages: false,
				alternate_backgrounds: true
			});
			expect(saved.chat.show_system_message_background).toBe(enabled);
		}
	});
	it('keeps processing controls in Advanced and makes them searchable', () => {
		for (const field of [
			'translate_history',
			'detection_policy',
			'queue_policy',
			'queue_capacity',
			'max_in_flight'
		]) {
			const control = SETTINGS_CONTROLS.find((item) => item.id === `translation.${field}`)!;
			expect(control.more).toBe(true);
			expect(matchingControls(control.label).map((item) => item.id)).toContain(control.id);
			if (control.kind === 'number') {
				for (const value of ['0', '257', '1.5'])
					expect(validateControlInput(control, value).error).toBeDefined();
				expect(validateControlInput(control, '256')).toEqual({ value: 256 });
			}
		}
		expect(
			SETTINGS_CONTROLS.find((item) => item.id === 'translation.layout')?.more
		).toBeUndefined();
	});
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
	it('covers user preferences and the approved advanced network controls', () => {
		expect(matchingControls('sidebar')).toEqual([]);
		expect(resetSettingsPatch('app')).not.toHaveProperty('layout');
		expect(resetSettingsPatch('all')).not.toHaveProperty('layout');
		const paths = [...new Set(SETTINGS_CONTROLS.map((control) => control.path))].sort();
		const expected = (
			['appearance', 'diagnostics', 'chat', 'emotes', 'translation', 'network'] as const
		)
			.flatMap((section) =>
				Object.keys(DEFAULT_APP_SETTINGS[section]).map((key) => `${section}.${key}`)
			)
			.concat([
				'providers.http_connect_timeout_secs',
				'providers.http_request_timeout_secs',
				'providers.stale_metadata_fallback'
			])
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
			'translation.layout'
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
		settings.translation = {
			...settings.translation,
			enabled: true,
			target_language: 'rus',
			layout: 'connector'
		};
		settings.channel_cache.poll_interval_secs = 99;
		const chat = applySettingsPatch(settings, resetSettingsPatch('chat'));
		expect(chat.chat.message_limit).toBe(500);
		expect(chat.translation).toEqual(settings.translation);
		const translation = applySettingsPatch(settings, resetSettingsPatch('translation'));
		expect(translation.translation).toEqual(DEFAULT_APP_SETTINGS.translation);
		expect(translation.chat.message_limit).toBe(900);
		expect(chat.appearance.theme).toBe('light');
		expect(chat.diagnostics.capture_window_errors).toBe(false);
		const app = applySettingsPatch(settings, resetSettingsPatch('app'));
		expect(app.diagnostics.capture_window_errors).toBe(true);
		expect(app.chat.message_limit).toBe(900);
		const all = applySettingsPatch(settings, resetSettingsPatch('all'));
		expect(all.appearance.theme).toBe('dark');
		expect(all.translation).toEqual(DEFAULT_APP_SETTINGS.translation);
		expect(all.channel_cache.poll_interval_secs).toBe(99);
	});
});
