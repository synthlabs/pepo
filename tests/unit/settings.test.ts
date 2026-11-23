import { describe, expect, it } from 'vitest';
import type { AppSettings } from '$lib/bindings';
import { DEFAULT_APP_SETTINGS, gridTemplateColumns, normalizeAppSettings, px } from '$lib/settings';

describe('settings helpers', () => {
	it('defaults system backgrounds off for legacy settings and preserves explicit choices', () => {
		expect(DEFAULT_APP_SETTINGS.chat.show_system_message_background).toBe(false);
		const legacy = JSON.parse(JSON.stringify(DEFAULT_APP_SETTINGS));
		delete legacy.chat.show_system_message_background;
		expect(normalizeAppSettings(legacy).chat.show_system_message_background).toBe(false);
		for (const enabled of [true, false]) {
			legacy.chat.show_system_message_background = enabled;
			expect(normalizeAppSettings(legacy).chat.show_system_message_background).toBe(enabled);
		}
		legacy.chat.show_system_message_background = 'yes';
		expect(normalizeAppSettings(legacy).chat.show_system_message_background).toBe(false);
	});

	it('discards legacy sidebar settings without mutating the saved object', () => {
		const legacy = {
			...structuredClone(DEFAULT_APP_SETTINGS),
			layout: { sidebar_open: false }
		};
		legacy.appearance.theme = 'light';
		legacy.chat.message_limit = 900;
		const normalized = normalizeAppSettings(legacy);
		expect(normalized).not.toHaveProperty('layout');
		expect(JSON.parse(JSON.stringify(normalized))).not.toHaveProperty('layout');
		expect(normalized.appearance.theme).toBe('light');
		expect(normalized.chat.message_limit).toBe(900);
		expect(normalized.schema_version).toBe(1);
		expect(legacy.layout.sidebar_open).toBe(false);
	});

	it('defaults retained-history translation off and preserves an explicit opt-in', () => {
		expect(DEFAULT_APP_SETTINGS.translation.translate_history).toBe(false);
		const legacy = { ...DEFAULT_APP_SETTINGS, translation: { enabled: true } } as AppSettings;
		expect(normalizeAppSettings(legacy).translation.translate_history).toBe(false);
		const enabled = structuredClone(DEFAULT_APP_SETTINGS);
		enabled.translation.translate_history = true;
		expect(normalizeAppSettings(enabled).translation.translate_history).toBe(true);
	});
	it('fills older translation preferences and normalizes processing bounds', () => {
		const older = {
			...DEFAULT_APP_SETTINGS,
			translation: { enabled: true, target_language: 'rus', layout: 'connector' }
		} as AppSettings;
		expect(normalizeAppSettings(older).translation).toEqual({
			...DEFAULT_APP_SETTINGS.translation,
			...older.translation
		});
		const configured = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			translation: {
				...DEFAULT_APP_SETTINGS.translation,
				translate_history: true,
				detection_policy: 'best_guess',
				queue_policy: 'oldest_first',
				queue_capacity: 0,
				max_in_flight: 999
			}
		});
		expect(configured.translation).toMatchObject({
			translate_history: true,
			detection_policy: 'best_guess',
			queue_policy: 'oldest_first',
			queue_capacity: 1,
			max_in_flight: 256
		});
		for (const invalid of [NaN, Infinity, 1.5, undefined]) {
			const settings = structuredClone(DEFAULT_APP_SETTINGS);
			settings.translation.queue_capacity = invalid as number;
			settings.translation.max_in_flight = invalid as number;
			expect(normalizeAppSettings(settings).translation).toMatchObject({
				queue_capacity: 256,
				max_in_flight: 16
			});
		}
	});
	it('defaults translation independently of the discarded legacy layout', () => {
		const legacy = structuredClone(DEFAULT_APP_SETTINGS) as Partial<AppSettings>;
		delete legacy.translation;
		Object.assign(legacy.chat!, { translation_layout: 'connector' });
		expect(normalizeAppSettings(legacy as AppSettings).translation).toEqual(
			DEFAULT_APP_SETTINGS.translation
		);
		const invalid = {
			...DEFAULT_APP_SETTINGS,
			translation: { enabled: 'yes', target_language: 'deu', layout: 'future' }
		} as unknown as AppSettings;
		expect(normalizeAppSettings(invalid).translation).toEqual(DEFAULT_APP_SETTINGS.translation);
		const configured = {
			...DEFAULT_APP_SETTINGS,
			translation: {
				...DEFAULT_APP_SETTINGS.translation,
				enabled: true,
				target_language: 'rus',
				layout: 'connector' as const
			}
		};
		expect(normalizeAppSettings(configured).translation).toEqual(configured.translation);
	});
	it('defaults the diagnostics section and rejects non-boolean capture flags', () => {
		const legacy = structuredClone(DEFAULT_APP_SETTINGS) as Partial<AppSettings>;
		delete legacy.diagnostics;
		expect(normalizeAppSettings(legacy as AppSettings).diagnostics).toEqual({
			capture_window_errors: true
		});
		const invalid = structuredClone(DEFAULT_APP_SETTINGS);
		invalid.diagnostics.capture_window_errors = 'no' as unknown as boolean;
		expect(normalizeAppSettings(invalid).diagnostics.capture_window_errors).toBe(true);
		const disabled = structuredClone(DEFAULT_APP_SETTINGS);
		disabled.diagnostics.capture_window_errors = false;
		expect(normalizeAppSettings(disabled).diagnostics.capture_window_errors).toBe(false);
	});

	it('defaults missing history policy fields and preserves explicit toggles', () => {
		const legacy = structuredClone(DEFAULT_APP_SETTINGS) as Partial<AppSettings>;
		legacy.chat = { message_limit: 500 } as AppSettings['chat'];
		const defaults = normalizeAppSettings(legacy as AppSettings).chat;
		expect(defaults.pause_message_gc_while_paused).toBe(true);
		expect(defaults.batch_live_messages).toBe(false);
		expect(defaults.message_batch_interval_ms).toBe(250);
		expect(defaults.message_gc_interval_ms).toBe(250);

		const configured = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			chat: {
				...DEFAULT_APP_SETTINGS.chat,
				pause_message_gc_while_paused: false,
				batch_live_messages: true,
				message_batch_interval_ms: 0,
				message_gc_interval_ms: -1
			}
		}).chat;
		expect(configured.pause_message_gc_while_paused).toBe(false);
		expect(configured.batch_live_messages).toBe(true);
		expect(configured.message_batch_interval_ms).toBe(250);
		expect(configured.message_gc_interval_ms).toBe(250);
	});
	it('dedupes provider preferences and appends missing defaults', () => {
		const settings: AppSettings = {
			...DEFAULT_APP_SETTINGS,
			emotes: {
				...DEFAULT_APP_SETTINGS.emotes,
				providers: [
					{ id: 'bttv', enabled: false },
					{ id: 'twitch', enabled: true },
					{ id: 'bttv', enabled: true }
				]
			}
		};

		expect(normalizeAppSettings(settings).emotes.providers).toEqual([
			{ id: 'bttv', enabled: false },
			{ id: 'twitch', enabled: true },
			{ id: 'ffz', enabled: true },
			{ id: 'seventv', enabled: true }
		]);
	});

	it('falls back for non-positive numeric values without changing boolean toggles', () => {
		const settings = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			channel_cache: {
				...DEFAULT_APP_SETTINGS.channel_cache,
				recurring_poll_enabled: false,
				poll_interval_secs: 0,
				error_log_throttle_enabled: false,
				error_log_throttle_secs: 0,
				user_lookup_chunk_size: 0
			},
			eventsub: {
				...DEFAULT_APP_SETTINGS.eventsub,
				debug_cost_watcher_enabled: false,
				repeated_log_throttle_enabled: false,
				socket_idle_timeout_secs: 0,
				retry_base_secs: 0,
				retry_max_secs: 0,
				debug_cost_watcher_interval_secs: 0,
				unparseable_warning_throttle_secs: 0,
				subscription_error_throttle_secs: 0
			}
		});

		expect(settings.channel_cache.recurring_poll_enabled).toBe(false);
		expect(settings.channel_cache.poll_interval_secs).toBe(60);
		expect(settings.channel_cache.error_log_throttle_enabled).toBe(false);
		expect(settings.channel_cache.error_log_throttle_secs).toBe(300);
		expect(settings.channel_cache.user_lookup_chunk_size).toBe(100);
		expect(settings.eventsub.debug_cost_watcher_enabled).toBe(false);
		expect(settings.eventsub.repeated_log_throttle_enabled).toBe(false);
		expect(settings.eventsub.socket_idle_timeout_secs).toBe(30);
		expect(settings.eventsub.retry_base_secs).toBe(5);
		expect(settings.eventsub.retry_max_secs).toBe(60);
		expect(settings.eventsub.debug_cost_watcher_interval_secs).toBe(30);
		expect(settings.eventsub.unparseable_warning_throttle_secs).toBe(60);
		expect(settings.eventsub.subscription_error_throttle_secs).toBe(300);
	});

	it('accepts the timestamp-end chat translation layout', () => {
		const settings = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			translation: {
				...DEFAULT_APP_SETTINGS.translation,
				layout: 'timestamp_end'
			}
		});

		expect(settings.translation.layout).toBe('timestamp_end');
	});

	it('accepts the connector chat translation layout', () => {
		const settings = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			translation: {
				...DEFAULT_APP_SETTINGS.translation,
				layout: 'connector'
			}
		});

		expect(settings.translation.layout).toBe('connector');
	});

	it('falls back to the default chat translation layout when missing or invalid', () => {
		const missingLayout = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			translation: {
				...DEFAULT_APP_SETTINGS.translation,
				layout: undefined
			}
		} as unknown as AppSettings);
		const invalidLayout = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			translation: {
				...DEFAULT_APP_SETTINGS.translation,
				layout: 'future_layout'
			}
		} as unknown as AppSettings);

		expect(missingLayout.translation.layout).toBe('message_text');
		expect(invalidLayout.translation.layout).toBe('message_text');
	});

	it('formats positive layout values', () => {
		expect(px(0)).toBe('1px');
		expect(gridTemplateColumns(0)).toBe('repeat(8, minmax(0, 1fr))');
	});
});

describe('chat engine settings', () => {
	it('defaults join policy and divider for existing settings while preserving explicit choices', () => {
		const old = structuredClone(DEFAULT_APP_SETTINGS);
		delete (old.chat as Partial<typeof old.chat>).join_mode;
		delete (old.chat as Partial<typeof old.chat>).show_history_divider;
		expect(normalizeAppSettings(old).chat.join_mode).toBe('all');
		expect(normalizeAppSettings(old).chat.show_history_divider).toBe(true);
		Object.assign(old.chat, { join_mode: 'single' });
		old.chat.show_history_divider = false;
		expect(normalizeAppSettings(old).chat.join_mode).toBe('open');
		expect(normalizeAppSettings(old).chat.show_history_divider).toBe(false);
	});
	it('migrates legacy policies and defaults moderation notices while preserving opt-out', () => {
		for (const [saved, expected] of [
			['live', 'recent'],
			['single', 'open'],
			['all', 'all'],
			['recent', 'recent'],
			['open', 'open']
		]) {
			const value = structuredClone(DEFAULT_APP_SETTINGS);
			Object.assign(value.chat, { join_mode: saved });
			expect(normalizeAppSettings(value).chat.join_mode).toBe(expected);
		}
		const old = structuredClone(DEFAULT_APP_SETTINGS);
		delete (old.chat as Partial<typeof old.chat>).moderation_notices_enabled;
		expect(normalizeAppSettings(old).chat.moderation_notices_enabled).toBe(true);
		old.chat.moderation_notices_enabled = false;
		expect(normalizeAppSettings(old).chat.moderation_notices_enabled).toBe(false);
	});
});
