import type { AppearanceTheme, EmoteProviderId, Settings } from '$lib/bindings';

export const DEFAULT_SETTINGS: Settings = {
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
	}
};

const PROVIDER_ORDER: EmoteProviderId[] = ['twitch', 'bttv', 'ffz', 'seventv'];

export function normalizeSettings(settings: Settings): Settings {
	const providers = [];
	const seen = new Set<EmoteProviderId>();

	for (const provider of settings.emotes.providers ?? []) {
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
		...DEFAULT_SETTINGS,
		...settings,
		appearance: {
			...DEFAULT_SETTINGS.appearance,
			...settings.appearance
		},
		layout: {
			...DEFAULT_SETTINGS.layout,
			...settings.layout
		},
		chat: {
			...DEFAULT_SETTINGS.chat,
			...settings.chat,
			message_limit: positive(settings.chat.message_limit, DEFAULT_SETTINGS.chat.message_limit),
			autoscroll_threshold_px: positive(
				settings.chat.autoscroll_threshold_px,
				DEFAULT_SETTINGS.chat.autoscroll_threshold_px
			),
			timestamp_locale:
				settings.chat.timestamp_locale.trim() || DEFAULT_SETTINGS.chat.timestamp_locale
		},
		emotes: {
			...DEFAULT_SETTINGS.emotes,
			...settings.emotes,
			providers,
			autocomplete_min_chars: positive(
				settings.emotes.autocomplete_min_chars,
				DEFAULT_SETTINGS.emotes.autocomplete_min_chars
			),
			search_debounce_ms: positive(
				settings.emotes.search_debounce_ms,
				DEFAULT_SETTINGS.emotes.search_debounce_ms
			),
			autocomplete_result_limit: positive(
				settings.emotes.autocomplete_result_limit,
				DEFAULT_SETTINGS.emotes.autocomplete_result_limit
			),
			picker_result_limit: positive(
				settings.emotes.picker_result_limit,
				DEFAULT_SETTINGS.emotes.picker_result_limit
			),
			picker_columns: positive(
				settings.emotes.picker_columns,
				DEFAULT_SETTINGS.emotes.picker_columns
			),
			picker_max_height_px: positive(
				settings.emotes.picker_max_height_px,
				DEFAULT_SETTINGS.emotes.picker_max_height_px
			),
			inline_emote_px: positive(
				settings.emotes.inline_emote_px,
				DEFAULT_SETTINGS.emotes.inline_emote_px
			),
			inline_badge_px: positive(
				settings.emotes.inline_badge_px,
				DEFAULT_SETTINGS.emotes.inline_badge_px
			)
		}
	};
}

export function formatTimestamp(ts: string, settings: Settings): string {
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
	return `repeat(${positive(columns, DEFAULT_SETTINGS.emotes.picker_columns)}, minmax(0, 1fr))`;
}

export function px(value: number): string {
	return `${Math.max(1, Math.floor(value))}px`;
}

function positive(value: number, fallback: number): number {
	return Number.isFinite(value) && value > 0 ? Math.floor(value) : fallback;
}
