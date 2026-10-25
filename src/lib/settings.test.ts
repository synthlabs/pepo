import { describe, expect, it } from 'vitest';
import type { AppSettings } from '$lib/bindings';
import { DEFAULT_APP_SETTINGS, gridTemplateColumns, normalizeAppSettings, px } from './settings';

describe('settings helpers', () => {
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
			chat: {
				...DEFAULT_APP_SETTINGS.chat,
				translation_layout: 'timestamp_end'
			}
		});

		expect(settings.chat.translation_layout).toBe('timestamp_end');
	});

	it('accepts the connector chat translation layout', () => {
		const settings = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			chat: {
				...DEFAULT_APP_SETTINGS.chat,
				translation_layout: 'connector'
			}
		});

		expect(settings.chat.translation_layout).toBe('connector');
	});

	it('falls back to the default chat translation layout when missing or invalid', () => {
		const missingLayout = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			chat: {
				...DEFAULT_APP_SETTINGS.chat,
				translation_layout: undefined
			}
		} as unknown as AppSettings);
		const invalidLayout = normalizeAppSettings({
			...DEFAULT_APP_SETTINGS,
			chat: {
				...DEFAULT_APP_SETTINGS.chat,
				translation_layout: 'future_layout'
			}
		} as unknown as AppSettings);

		expect(missingLayout.chat.translation_layout).toBe('message_text');
		expect(invalidLayout.chat.translation_layout).toBe('message_text');
	});

	it('formats positive layout values', () => {
		expect(px(0)).toBe('1px');
		expect(gridTemplateColumns(0)).toBe('repeat(8, minmax(0, 1fr))');
	});
});
