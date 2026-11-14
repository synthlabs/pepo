import type { AppSettings, EmoteProviderId } from '$lib/bindings';
import { DEFAULT_APP_SETTINGS } from '$lib/settings';
import type { EditableSettingsSection, SettingsPatch, SettingsUpdate } from './autosave';

export const SETTINGS_CATEGORIES = [
	{ id: 'chat', label: 'Chat', description: 'How messages look and behave.' },
	{ id: 'emotes', label: 'Emotes', description: 'Emote providers, autocomplete, and display.' },
	{
		id: 'translation',
		label: 'Translation',
		description: 'Read chat in your language.'
	},
	{ id: 'app', label: 'App', description: 'Appearance and the channel sidebar.' }
] as const;
export type SettingsCategory = (typeof SETTINGS_CATEGORIES)[number]['id'];
type SettingPath = {
	[Section in EditableSettingsSection]: `${Section}.${keyof AppSettings[Section] & string}`;
}[EditableSettingsSection];

export interface SettingControl {
	id: string;
	path: SettingPath;
	category: SettingsCategory;
	label: string;
	description: string;
	kind: 'toggle' | 'number' | 'select' | 'theme' | 'locale' | 'provider' | 'provider-order';
	more?: boolean;
	unit?: string;
	max?: number;
	provider?: EmoteProviderId;
	options?: { value: string; label: string }[];
}

const U32_MAX = 4_294_967_295;
const chatMore = { category: 'chat', more: true } as const;
const emoteMore = { category: 'emotes', more: true } as const;
const translationMore = { category: 'translation', more: true } as const;

export const SETTINGS_CONTROLS: SettingControl[] = [
	{
		...chatMore,
		id: 'chat.center_system_messages',
		path: 'chat.center_system_messages',
		label: 'Center system messages',
		description: 'Center inline notices instead of aligning them to the left.',
		kind: 'toggle'
	},
	{
		id: 'chat.show_history_divider',
		path: 'chat.show_history_divider',
		category: 'chat',
		label: 'Show history divider',
		description: 'Mark where loaded history ends and new messages begin each time you open a room.',
		kind: 'toggle'
	},

	{
		id: 'chat.show_timestamps',
		path: 'chat.show_timestamps',
		category: 'chat',
		label: 'Show timestamps',
		description: 'Show when each message was sent.',
		kind: 'toggle'
	},
	{
		id: 'chat.show_badges',
		path: 'chat.show_badges',
		category: 'chat',
		label: 'Show badges',
		description: 'Show subscriber, moderator, and other chat badges.',
		kind: 'toggle'
	},
	{
		id: 'chat.show_emotes',
		path: 'chat.show_emotes',
		category: 'chat',
		label: 'Show emotes',
		description: 'Display emote images in chat messages.',
		kind: 'toggle'
	},
	{
		id: 'chat.alternate_backgrounds',
		path: 'chat.alternate_backgrounds',
		category: 'chat',
		label: 'Alternating backgrounds',
		description: 'Alternate message backgrounds to make chat easier to follow.',
		kind: 'toggle'
	},
	{
		id: 'chat.message_limit',
		path: 'chat.message_limit',
		category: 'chat',
		label: 'Message history limit',
		description:
			'Number of messages to retain per channel and the normal cleanup target for the current chat.',
		kind: 'number',
		unit: 'messages'
	},
	{
		...chatMore,
		id: 'chat.join_mode',
		path: 'chat.join_mode',
		label: 'Channel join mode',
		description:
			'Collect followed channels, live followed channels, or join rooms as you open them. Opened rooms stay joined until higher-priority rooms need their slots.',
		kind: 'select',
		options: [
			{ value: 'all', label: 'All followed channels' },
			{ value: 'live', label: 'Live followed channels' },
			{ value: 'single', label: 'Single (join on demand)' }
		]
	},
	{
		...chatMore,
		id: 'chat.timestamp_locale',
		path: 'chat.timestamp_locale',
		label: 'Timestamp locale',
		description: 'Language and region for timestamps, such as en or en-GB.',
		kind: 'locale'
	},
	{
		...chatMore,
		id: 'chat.timestamp_style',
		path: 'chat.timestamp_style',
		label: 'Timestamp style',
		description: 'Choose how much detail timestamps show.',
		kind: 'select',
		options: [
			{ value: 'short', label: 'Short' },
			{ value: 'medium', label: 'Medium' },
			{ value: 'long', label: 'Long' },
			{ value: 'full', label: 'Full' }
		]
	},
	{
		id: 'translation.enabled',
		path: 'translation.enabled',
		category: 'translation',
		label: 'Translate chat',
		description: 'Translate incoming messages automatically.',
		kind: 'toggle'
	},
	{
		id: 'translation.target_language',
		path: 'translation.target_language',
		category: 'translation',
		label: 'Translate into',
		description: 'Used across all channels.',
		kind: 'select',
		options: [
			{ value: 'eng', label: 'English' },
			{ value: 'rus', label: 'Russian' }
		]
	},
	{
		category: 'translation',
		id: 'translation.layout',
		path: 'translation.layout',
		label: 'Translation layout',
		description: 'Position translated text and its language label.',
		kind: 'select',
		options: [
			{ value: 'language_tag', label: 'Language tag' },
			{ value: 'message_text', label: 'Aligned with message' },
			{ value: 'timestamp_end', label: 'After timestamp' },
			{ value: 'connector', label: 'Connector' }
		]
	},
	{
		...translationMore,
		id: 'translation.translate_history',
		path: 'translation.translate_history',
		label: 'Translate retained history',
		description: 'Also translate untranslated messages already in the open chat’s history.',
		kind: 'toggle'
	},
	{
		...translationMore,
		id: 'translation.detection_policy',
		path: 'translation.detection_policy',
		label: 'Language detection',
		description: 'Skip uncertain language detection or use the detector’s best guess.',
		kind: 'select',
		options: [
			{ value: 'reliable_only', label: 'Reliable only' },
			{ value: 'best_guess', label: 'Best guess' }
		]
	},
	{
		...translationMore,
		id: 'translation.queue_policy',
		path: 'translation.queue_policy',
		label: 'Translation order',
		description:
			'Newest first favors live messages and drops the oldest waiting request when full. Oldest first keeps arrival order and skips new requests when full.',
		kind: 'select',
		options: [
			{ value: 'newest_first', label: 'Newest first' },
			{ value: 'oldest_first', label: 'Oldest first' }
		]
	},
	{
		...translationMore,
		id: 'translation.queue_capacity',
		path: 'translation.queue_capacity',
		label: 'Translation queue capacity',
		description:
			'Maximum unfinished requests across open chats, including requests being translated.',
		kind: 'number',
		unit: 'requests',
		max: 256
	},
	{
		...translationMore,
		id: 'translation.max_in_flight',
		path: 'translation.max_in_flight',
		label: 'Concurrent translations',
		description: 'Maximum requests submitted together across open chats, up to the queue capacity.',
		kind: 'number',
		unit: 'requests',
		max: 256
	},
	{
		...chatMore,
		id: 'chat.pause_message_gc_while_paused',
		path: 'chat.pause_message_gc_while_paused',
		label: 'Keep history while paused',
		description: 'Wait to remove older messages while chat scrolling is paused.',
		kind: 'toggle'
	},
	{
		...chatMore,
		id: 'chat.batch_live_messages',
		path: 'chat.batch_live_messages',
		label: 'Batch live messages',
		description: 'Display incoming live messages together at regular intervals.',
		kind: 'toggle'
	},
	{
		...chatMore,
		id: 'chat.message_batch_interval_ms',
		path: 'chat.message_batch_interval_ms',
		label: 'Message batch interval',
		description: 'Time between displaying queued messages while paused or batching.',
		kind: 'number',
		unit: 'ms',
		max: U32_MAX
	},
	{
		...chatMore,
		id: 'chat.message_gc_interval_ms',
		path: 'chat.message_gc_interval_ms',
		label: 'History cleanup interval',
		description: 'Time before removing messages beyond the history limit.',
		kind: 'number',
		unit: 'ms',
		max: U32_MAX
	},
	{
		...chatMore,
		id: 'chat.autoscroll_threshold_px',
		path: 'chat.autoscroll_threshold_px',
		label: 'Autoscroll threshold',
		description: 'Distance from the bottom that keeps chat following new messages.',
		kind: 'number',
		unit: 'px',
		max: U32_MAX
	},
	...(['twitch', 'bttv', 'ffz', 'seventv'] as const).map((provider) => ({
		id: `emotes.providers.${provider}`,
		path: 'emotes.providers' as const,
		category: 'emotes' as const,
		label: `${{ twitch: 'Twitch', bttv: 'BTTV', ffz: 'FFZ', seventv: '7TV' }[provider]} emotes`,
		description: `Include ${{ twitch: 'Twitch', bttv: 'BetterTTV', ffz: 'FrankerFaceZ', seventv: '7TV' }[provider]} in emote lookup and search.`,
		kind: 'provider' as const,
		provider
	})),
	{
		id: 'emotes.autocomplete_enabled',
		path: 'emotes.autocomplete_enabled',
		category: 'emotes',
		label: 'Emote autocomplete',
		description: 'Suggest emotes when you type a colon followed by a name.',
		kind: 'toggle'
	},
	{
		id: 'emotes.inline_emote_px',
		path: 'emotes.inline_emote_px',
		category: 'emotes',
		label: 'Emote size',
		description: 'Height of emotes in chat and the picker.',
		kind: 'number',
		unit: 'px',
		max: U32_MAX
	},
	{
		...emoteMore,
		id: 'emotes.providers.order',
		path: 'emotes.providers',
		label: 'Provider priority',
		description: 'Use the first matching provider when emote names overlap.',
		kind: 'provider-order'
	},
	{
		...emoteMore,
		id: 'emotes.autocomplete_min_chars',
		path: 'emotes.autocomplete_min_chars',
		label: 'Autocomplete minimum characters',
		description: 'Characters required after a colon before suggesting emotes.',
		kind: 'number',
		unit: 'characters'
	},
	{
		...emoteMore,
		id: 'emotes.search_debounce_ms',
		path: 'emotes.search_debounce_ms',
		label: 'Emote search delay',
		description: 'Wait briefly after typing before searching for emotes.',
		kind: 'number',
		unit: 'ms'
	},
	{
		...emoteMore,
		id: 'emotes.autocomplete_result_limit',
		path: 'emotes.autocomplete_result_limit',
		label: 'Autocomplete result limit',
		description: 'Maximum number of emote suggestions.',
		kind: 'number',
		unit: 'results'
	},
	{
		...emoteMore,
		id: 'emotes.picker_result_limit',
		path: 'emotes.picker_result_limit',
		label: 'Picker result limit',
		description: 'Maximum number of emotes in picker search results.',
		kind: 'number',
		unit: 'results'
	},
	{
		...emoteMore,
		id: 'emotes.picker_columns',
		path: 'emotes.picker_columns',
		label: 'Picker columns',
		description: 'Number of columns in the emote picker.',
		kind: 'number',
		unit: 'columns'
	},
	{
		...emoteMore,
		id: 'emotes.picker_max_height_px',
		path: 'emotes.picker_max_height_px',
		label: 'Picker maximum height',
		description: 'Maximum height of the emote picker.',
		kind: 'number',
		unit: 'px',
		max: U32_MAX
	},
	{
		...emoteMore,
		id: 'emotes.inline_badge_px',
		path: 'emotes.inline_badge_px',
		label: 'Badge size',
		description: 'Height of badges in chat.',
		kind: 'number',
		unit: 'px',
		max: U32_MAX
	},
	{
		id: 'network.connect_timeout_secs',
		path: 'network.connect_timeout_secs',
		category: 'app',
		more: true,
		kind: 'number',
		unit: 'seconds',
		max: 3600,
		label: 'API connection timeout',
		description:
			'Time allowed to connect to Twitch and metadata services. Restart Pepo after changing this.'
	},
	{
		id: 'network.request_timeout_secs',
		path: 'network.request_timeout_secs',
		category: 'app',
		more: true,
		kind: 'number',
		unit: 'seconds',
		max: 3600,
		label: 'API request timeout',
		description:
			'Time allowed for a small API request. Large downloads and report uploads use separate limits. Restart Pepo after changing this.'
	},
	{
		id: 'providers.http_connect_timeout_secs',
		path: 'providers.http_connect_timeout_secs',
		category: 'app',
		more: true,
		kind: 'number',
		unit: 'seconds',
		max: 3600,
		label: 'Emote provider connection timeout',
		description: 'Connection timeout for BTTV, FFZ, and 7TV. Applies to new requests.'
	},
	{
		id: 'providers.http_request_timeout_secs',
		path: 'providers.http_request_timeout_secs',
		category: 'app',
		more: true,
		kind: 'number',
		unit: 'seconds',
		max: 3600,
		label: 'Emote provider request timeout',
		description: 'Time allowed for an emote metadata request. Applies to new requests.'
	},
	{
		id: 'network.metadata_concurrency',
		path: 'network.metadata_concurrency',
		category: 'app',
		more: true,
		kind: 'number',
		unit: 'requests',
		max: 32,
		label: 'Background request limit',
		description:
			'Maximum optional metadata requests in progress, with one request per provider. Opened channels take priority. Applies to subsequent requests.'
	},
	{
		id: 'network.retry_base_secs',
		path: 'network.retry_base_secs',
		category: 'app',
		more: true,
		kind: 'number',
		unit: 'seconds',
		max: 86400,
		label: 'Initial retry delay',
		description: 'Delay after the first failed metadata request. Later failures double the delay.'
	},
	{
		id: 'network.retry_max_secs',
		path: 'network.retry_max_secs',
		category: 'app',
		more: true,
		kind: 'number',
		unit: 'seconds',
		max: 86400,
		label: 'Maximum retry delay',
		description: 'Longest normal metadata retry delay. Server-requested waits are respected.'
	},
	{
		id: 'providers.stale_metadata_fallback',
		path: 'providers.stale_metadata_fallback',
		category: 'app',
		more: true,
		kind: 'toggle',
		label: 'Keep cached emotes and badges until refreshed',
		description:
			'Use saved metadata even when it is older than the retention period. Successful refreshes replace it.'
	},

	{
		id: 'appearance.theme',
		path: 'appearance.theme',
		category: 'app',
		label: 'Theme',
		description: 'Colors used across Pepo.',
		kind: 'theme',
		options: [
			{ value: 'system', label: 'System' },
			{ value: 'light', label: 'Light' },
			{ value: 'dark', label: 'Dark' }
		]
	},
	{
		id: 'layout.sidebar_open',
		path: 'layout.sidebar_open',
		category: 'app',
		label: 'Sidebar expanded',
		description: 'Show channel names and stream details in the sidebar.',
		kind: 'toggle'
	},
	{
		id: 'diagnostics.capture_window_errors',
		path: 'diagnostics.capture_window_errors',
		category: 'app',
		label: 'Report uncaught errors',
		description: 'Offer a bug report when the app hits an unexpected error.',
		kind: 'toggle'
	}
];

export const PROVIDER_LABELS: Record<EmoteProviderId, string> = {
	twitch: 'Twitch',
	bttv: 'BTTV',
	ffz: 'FFZ',
	seventv: '7TV'
};

export function controlValue(
	settings: AppSettings,
	control: SettingControl
): string | number | boolean {
	if (control.kind === 'provider') {
		return (
			settings.emotes.providers.find((provider) => provider.id === control.provider)?.enabled ??
			true
		);
	}
	const [section, field] = control.path.split('.') as [EditableSettingsSection, string];
	return (settings[section] as unknown as Record<string, string | number | boolean>)[field];
}

export function controlUpdate(
	control: SettingControl,
	value: string | number | boolean
): SettingsUpdate {
	if (control.kind === 'provider') {
		const id = control.provider;
		return (current) => ({
			emotes: {
				providers: current.emotes.providers.map((provider) => ({
					...provider,
					enabled: provider.id === id ? Boolean(value) : provider.enabled
				}))
			}
		});
	}
	const [section, field] = control.path.split('.');
	return { [section]: { [field]: value } } as SettingsPatch;
}

export function matchingControls(query: string): SettingControl[] {
	const terms = query.trim().toLowerCase().split(/\s+/);
	return SETTINGS_CONTROLS.filter((control) => {
		const category = SETTINGS_CATEGORIES.find((item) => item.id === control.category)!;
		const text = [
			control.label,
			control.description,
			category.label,
			...(control.options?.map((option) => option.label) ?? [])
		]
			.join(' ')
			.toLowerCase();
		return terms.every((term) => text.includes(term));
	});
}

export function resetSettingsPatch(category: SettingsCategory | 'all'): SettingsPatch {
	const defaults = structuredClone(DEFAULT_APP_SETTINGS);
	if (category === 'chat') return { chat: defaults.chat };
	if (category === 'translation') return { translation: defaults.translation };
	if (category === 'emotes') return { emotes: defaults.emotes };
	if (category === 'app')
		return {
			appearance: defaults.appearance,
			layout: defaults.layout,
			diagnostics: defaults.diagnostics,
			network: defaults.network,
			providers: {
				http_connect_timeout_secs: defaults.providers.http_connect_timeout_secs,
				http_request_timeout_secs: defaults.providers.http_request_timeout_secs,
				stale_metadata_fallback: defaults.providers.stale_metadata_fallback
			}
		};
	return {
		appearance: defaults.appearance,
		layout: defaults.layout,
		diagnostics: defaults.diagnostics,
		chat: defaults.chat,
		translation: defaults.translation,
		emotes: defaults.emotes,
		network: defaults.network,
		providers: {
			http_connect_timeout_secs: defaults.providers.http_connect_timeout_secs,
			http_request_timeout_secs: defaults.providers.http_request_timeout_secs,
			stale_metadata_fallback: defaults.providers.stale_metadata_fallback
		}
	};
}

export function validateControlInput(
	control: SettingControl,
	input: string
): { value: string | number; error?: never } | { error: string; value?: never } {
	const text = input.trim();
	if (control.kind === 'locale') {
		try {
			if (!text) throw new RangeError();
			new Intl.DateTimeFormat(text).format(0);
			return { value: text };
		} catch {
			return { error: 'Enter a valid locale, such as en or en-GB.' };
		}
	}
	const value = Number(text);
	const max = control.max ?? Number.MAX_SAFE_INTEGER;
	if (!/^\d+$/.test(text) || !Number.isSafeInteger(value) || value < 1 || value > max) {
		return { error: `Enter a whole number from 1 to ${max.toLocaleString('en')}.` };
	}
	return { value };
}
