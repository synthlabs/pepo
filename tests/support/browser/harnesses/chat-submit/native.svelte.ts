/* eslint-disable @typescript-eslint/no-explicit-any, svelte/prefer-svelte-reactivity */
import { commands as realCommands } from '../../../../../src/lib/bindings';

export const page = $state({ url: new URL(location.href), params: { id: 'maya' } });
export const native: any = (window.chatNative = {
	calls: [],
	requests: [],
	logs: [],
	debug: [],
	diagnostics: [],
	reports: [],
	pauses: [],
	joins: [],
	searches: [],
	captured: 0,
	failReport: false,
	failCapabilities: false,
	emote: {
		id: '25',
		name: 'Kappa',
		tier: '',
		emote_type: 'globals',
		emote_set_id: '0',
		format: ['static'],
		scale: ['1.0'],
		theme_mode: ['light', 'dark'],
		url: 'data:image/gif;base64,R0lGODlhAQABAAAAACw=',
		provider: 'Twitch',
		scope: 'Global'
	},
	logError(message: string) {
		return error(message);
	},
	roomError(message: string) {
		this.feed.onmessage({ cursor: 1, event: { type: 'unavailable', error: message } });
	},
	navigate(id: string) {
		page.params = { id };
	},
	resolve(index: number, result: { status: string; error?: unknown; data?: unknown }) {
		if (result.status === 'error') this.requests[index].reject(result.error);
		else this.requests[index].resolve(result.data);
	},
	reject(index: number, message: string) {
		this.requests[index].reject(new Error(message));
	}
});

export class SyncedState {
	obj: any = $state({});
	ready = true;
	initialized = Promise.resolve();
	constructor(key: string, fallback: any) {
		this.obj = structuredClone(
			key === 'auth_state'
				? { phase: 'authorized', token: { user_id: '2', login: 'jerod', profile_image_url: '' } }
				: (fallback ?? {})
		);
	}
	async sync() {
		return true;
	}
}

export const commands = {
	beginBackgroundRequest: async () => ({ status: 'ok', data: 'updates' }),
	endBackgroundRequest: async () => {},
	getChatFeedContext: async () => ({ status: 'ok', data: { epoch: 'submit-page' } }),
	subscribeChat: async (
		channel: string,
		_request: unknown,
		sink: { onmessage: (msg: any) => void }
	) => {
		native.joins.push(channel);
		native.feed = sink;
		sink.onmessage({ cursor: 0, event: { type: 'snapshot', messages: [] } });
		return {
			status: 'ok',
			data: {
				broadcaster_id: channel + '-id',
				broadcaster_login: channel,
				broadcaster_name: channel
			}
		};
	},
	unsubscribeChat: async () => {},
	acknowledgeChat: async () => {},
	searchEmotes: async (query: string) => {
		native.searches.push(query);
		if (native.fixtures) {
			return {
				status: 'ok',
				data: native.fixtures.filter((emote: { name: string }) =>
					emote.name.toLowerCase().includes(String(query).toLowerCase())
				)
			};
		}
		return {
			status: 'ok',
			data: String(query).toLowerCase().startsWith('ka') ? [native.emote] : []
		};
	},
	sendChatMessage: realCommands.sendChatMessage
};

export class Channel {
	onmessage: (message: any) => void;
	constructor(callback: (message: any) => void) {
		this.onmessage = callback;
	}
}

const listeners = new Map<string, Set<(event: any) => void>>();
export const listen = async (event: string, callback: (event: any) => void) => {
	const callbacks = listeners.get(event) ?? new Set();
	callbacks.add(callback);
	listeners.set(event, callbacks);
	return () => callbacks.delete(callback);
};

export async function error(message: string, options?: { keyValues?: Record<string, string> }) {
	native.logs.push({ message, options });
	if (options?.keyValues?.['inbound.handled'] !== 'true') {
		for (const callback of listeners.get('inbound://error-captured') ?? []) {
			callback({
				payload: {
					id: String(++native.captured),
					error: { kind: 'error', message, target: 'webview', timestamp: '0' }
				}
			});
		}
	}
}

export async function invoke(command: string, args: any) {
	switch (command) {
		case 'send_chat_message':
			native.calls.push(args);
			return new Promise((resolve, reject) => native.requests.push({ resolve, reject }));
		case 'plugin:inbound|register_submission':
		case 'plugin:inbound|cancel_submission':
			return undefined;
		case 'plugin:inbound|capabilities':
			if (native.failCapabilities) throw new Error('capabilities failure');
			return { has_config: false };
		case 'plugin:inbound|preview_system':
			return { os: 'browser', arch: 'test', locale: 'en-US' };
		case 'plugin:inbound|preview_build':
			return { app_version: 'test', app_commit: 'test', build_time: 'test' };
		case 'plugin:inbound|preview_log_tail':
			return { exists: true, byte_count: 0, preview_text: 'test logs', gzipped_size: 0 };
		case 'plugin:inbound|preview_config':
			return null;
		case 'plugin:inbound|captured_errors':
			return [];
		case 'plugin:inbound|preview_error':
			return {
				kind: 'error',
				message: 'wrong latest native error',
				target: 'other',
				timestamp: '0'
			};
		case 'plugin:inbound|submit':
			native.reports.push(args.input);
			if (native.failReport) throw new Error('report submission failure');
			return;
		default:
			throw new Error('Unexpected invoke: ' + command);
	}
}

export const PUBLIC_UPDATE_INSTALL_MODE = '';
export const checkForAppUpdates = async () => {};
export default {
	debug(...values: unknown[]) {
		native.debug.push(values);
	},
	info(event: string, record: string) {
		if (event.startsWith('chat.send.'))
			native.diagnostics.push({ level: 'info', event, record: JSON.parse(record) });
	},
	warn(event: string, record: string) {
		if (event.startsWith('chat.send.'))
			native.diagnostics.push({ level: 'warn', event, record: JSON.parse(record) });
	},
	error(...values: unknown[]) {
		void error(values.join(' '));
	}
};
