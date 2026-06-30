import type { AppearanceTheme, AppSettings, EmoteProviderId } from '$lib/bindings';

export const DEFAULT_APP_SETTINGS: AppSettings = {
	schema_version: 1,
	appearance: {
		theme: 'dark'
	},
	layout: {
		sidebar_open: true
	},
	chat: {
		message_limit: 500,
		autoscroll_threshold_px: 32,
		show_timestamps: true,
		timestamp_locale: 'en',
		timestamp_style: 'short',
		translation_layout: 'message_text',
		show_badges: true,
		show_emotes: true,
		alternate_backgrounds: true
	},
	emotes: {
		providers: [
			{ id: 'twitch', enabled: true },
			{ id: 'bttv', enabled: true },
			{ id: 'ffz', enabled: true },
			{ id: 'seventv', enabled: true }
		],
		autocomplete_enabled: true,
		autocomplete_min_chars: 2,
		search_debounce_ms: 75,
		autocomplete_result_limit: 25,
		picker_result_limit: 50,
		picker_columns: 8,
		picker_max_height_px: 192,
		inline_emote_px: 28,
		inline_badge_px: 20
	},
	channel_cache: {
		recurring_poll_enabled: true,
		poll_interval_secs: 60,
		error_log_throttle_enabled: true,
		error_log_throttle_secs: 300,
		user_lookup_chunk_size: 100
	},
	auth: {
		login_activation_delay_ms: 500,
		refresh_supervisor_tick_secs: 15,
		validation_interval_secs: 300,
		refresh_if_remaining_lt_secs: 600
	},
	eventsub: {
		socket_idle_timeout_secs: 30,
		retry_base_secs: 5,
		retry_max_secs: 60,
		debug_cost_watcher_enabled: true,
		debug_cost_watcher_interval_secs: 30,
		repeated_log_throttle_enabled: true,
		unparseable_warning_throttle_secs: 60,
		subscription_error_throttle_secs: 300
	},
	providers: {
		http_connect_timeout_secs: 5,
		http_request_timeout_secs: 15,
		metadata_retention_enabled: true,
		metadata_retention_secs: 30 * 24 * 60 * 60
	}
};

const PROVIDER_ORDER: EmoteProviderId[] = ['twitch', 'bttv', 'ffz', 'seventv'];

export function normalizeAppSettings(settings: AppSettings): AppSettings {
	const source = settings ?? DEFAULT_APP_SETTINGS;
	const appearance = source.appearance ?? DEFAULT_APP_SETTINGS.appearance;
	const layout = source.layout ?? DEFAULT_APP_SETTINGS.layout;
	const chat = source.chat ?? DEFAULT_APP_SETTINGS.chat;
	const emotes = source.emotes ?? DEFAULT_APP_SETTINGS.emotes;
	const channelCache = source.channel_cache ?? DEFAULT_APP_SETTINGS.channel_cache;
	const auth = source.auth ?? DEFAULT_APP_SETTINGS.auth;
	const eventsub = source.eventsub ?? DEFAULT_APP_SETTINGS.eventsub;
	const providersSettings = source.providers ?? DEFAULT_APP_SETTINGS.providers;
	const providers: AppSettings['emotes']['providers'] = [];
	const seen = new Set<EmoteProviderId>();

	for (const provider of emotes.providers ?? []) {
		if (!seen.has(provider.id)) {
			providers.push(provider);
			seen.add(provider.id);
		}
	}

	for (const id of PROVIDER_ORDER) {
		if (!seen.has(id)) {
			providers.push({ id, enabled: true });
		}
	}

	return {
		...DEFAULT_APP_SETTINGS,
		...source,
		appearance: {
			...DEFAULT_APP_SETTINGS.appearance,
			...appearance
		},
		layout: {
			...DEFAULT_APP_SETTINGS.layout,
			...layout
		},
		chat: {
			...DEFAULT_APP_SETTINGS.chat,
			...chat,
			message_limit: positive(chat.message_limit, DEFAULT_APP_SETTINGS.chat.message_limit),
			autoscroll_threshold_px: positive(
				chat.autoscroll_threshold_px,
				DEFAULT_APP_SETTINGS.chat.autoscroll_threshold_px
			),
			translation_layout:
				chat.translation_layout === 'language_tag' ||
				chat.translation_layout === 'message_text' ||
				chat.translation_layout === 'timestamp_end'
					? chat.translation_layout
					: DEFAULT_APP_SETTINGS.chat.translation_layout,
			timestamp_locale:
				(chat.timestamp_locale ?? '').trim() || DEFAULT_APP_SETTINGS.chat.timestamp_locale
		},
		emotes: {
			...DEFAULT_APP_SETTINGS.emotes,
			...emotes,
			providers,
			autocomplete_min_chars: positive(
				emotes.autocomplete_min_chars,
				DEFAULT_APP_SETTINGS.emotes.autocomplete_min_chars
			),
			search_debounce_ms: positive(
				emotes.search_debounce_ms,
				DEFAULT_APP_SETTINGS.emotes.search_debounce_ms
			),
			autocomplete_result_limit: positive(
				emotes.autocomplete_result_limit,
				DEFAULT_APP_SETTINGS.emotes.autocomplete_result_limit
			),
			picker_result_limit: positive(
				emotes.picker_result_limit,
				DEFAULT_APP_SETTINGS.emotes.picker_result_limit
			),
			picker_columns: positive(emotes.picker_columns, DEFAULT_APP_SETTINGS.emotes.picker_columns),
			picker_max_height_px: positive(
				emotes.picker_max_height_px,
				DEFAULT_APP_SETTINGS.emotes.picker_max_height_px
			),
			inline_emote_px: positive(
				emotes.inline_emote_px,
				DEFAULT_APP_SETTINGS.emotes.inline_emote_px
			),
			inline_badge_px: positive(
				emotes.inline_badge_px,
				DEFAULT_APP_SETTINGS.emotes.inline_badge_px
			)
		},
		channel_cache: {
			...DEFAULT_APP_SETTINGS.channel_cache,
			...channelCache,
			poll_interval_secs: positive(
				channelCache.poll_interval_secs,
				DEFAULT_APP_SETTINGS.channel_cache.poll_interval_secs
			),
			error_log_throttle_secs: positive(
				channelCache.error_log_throttle_secs,
				DEFAULT_APP_SETTINGS.channel_cache.error_log_throttle_secs
			),
			user_lookup_chunk_size: positive(
				channelCache.user_lookup_chunk_size,
				DEFAULT_APP_SETTINGS.channel_cache.user_lookup_chunk_size
			)
		},
		auth: {
			...DEFAULT_APP_SETTINGS.auth,
			...auth,
			login_activation_delay_ms: positive(
				auth.login_activation_delay_ms,
				DEFAULT_APP_SETTINGS.auth.login_activation_delay_ms
			),
			refresh_supervisor_tick_secs: positive(
				auth.refresh_supervisor_tick_secs,
				DEFAULT_APP_SETTINGS.auth.refresh_supervisor_tick_secs
			),
			validation_interval_secs: positive(
				auth.validation_interval_secs,
				DEFAULT_APP_SETTINGS.auth.validation_interval_secs
			),
			refresh_if_remaining_lt_secs: positive(
				auth.refresh_if_remaining_lt_secs,
				DEFAULT_APP_SETTINGS.auth.refresh_if_remaining_lt_secs
			)
		},
		eventsub: {
			...DEFAULT_APP_SETTINGS.eventsub,
			...eventsub,
			socket_idle_timeout_secs: positive(
				eventsub.socket_idle_timeout_secs,
				DEFAULT_APP_SETTINGS.eventsub.socket_idle_timeout_secs
			),
			retry_base_secs: positive(
				eventsub.retry_base_secs,
				DEFAULT_APP_SETTINGS.eventsub.retry_base_secs
			),
			retry_max_secs: positive(
				eventsub.retry_max_secs,
				DEFAULT_APP_SETTINGS.eventsub.retry_max_secs
			),
			debug_cost_watcher_interval_secs: positive(
				eventsub.debug_cost_watcher_interval_secs,
				DEFAULT_APP_SETTINGS.eventsub.debug_cost_watcher_interval_secs
			),
			unparseable_warning_throttle_secs: positive(
				eventsub.unparseable_warning_throttle_secs,
				DEFAULT_APP_SETTINGS.eventsub.unparseable_warning_throttle_secs
			),
			subscription_error_throttle_secs: positive(
				eventsub.subscription_error_throttle_secs,
				DEFAULT_APP_SETTINGS.eventsub.subscription_error_throttle_secs
			)
		},
		providers: {
			...DEFAULT_APP_SETTINGS.providers,
			...providersSettings,
			http_connect_timeout_secs: positive(
				providersSettings.http_connect_timeout_secs,
				DEFAULT_APP_SETTINGS.providers.http_connect_timeout_secs
			),
			http_request_timeout_secs: positive(
				providersSettings.http_request_timeout_secs,
				DEFAULT_APP_SETTINGS.providers.http_request_timeout_secs
			),
			metadata_retention_secs: positive(
				providersSettings.metadata_retention_secs,
				DEFAULT_APP_SETTINGS.providers.metadata_retention_secs
			)
		}
	};
}

export function formatTimestamp(ts: string, settings: AppSettings): string {
	return new Date(ts).toLocaleTimeString(settings.chat.timestamp_locale, {
		timeStyle: settings.chat.timestamp_style as Intl.DateTimeFormatOptions['timeStyle']
	});
}

export function applyThemePreference(theme: AppearanceTheme): () => void {
	const root = document.documentElement;
	const media = window.matchMedia('(prefers-color-scheme: dark)');
	const update = () => {
		const dark = theme === 'dark' || (theme === 'system' && media.matches);
		root.classList.toggle('dark', dark);
		root.classList.toggle('light', !dark);
	};

	update();

	if (theme !== 'system') return () => {};

	media.addEventListener('change', update);
	return () => media.removeEventListener('change', update);
}

export function gridTemplateColumns(columns: number): string {
	return `repeat(${positive(columns, DEFAULT_APP_SETTINGS.emotes.picker_columns)}, minmax(0, 1fr))`;
}

export function px(value: number): string {
	return `${Math.max(1, Math.floor(value))}px`;
}

function positive(value: number, fallback: number): number {
	return Number.isFinite(value) && value > 0 ? Math.floor(value) : fallback;
}
