/* eslint-disable @typescript-eslint/no-explicit-any, svelte/prefer-svelte-reactivity */
export const page = $state({
	url: new URL(location.href),
	params: { id: 'rooma' } as Record<string, string>
});
export const resolve = (value: string) => value;
export const goto = async (value: string) => {
	history.pushState({}, '', value);
	page.url = new URL(value, location.origin);
	page.params = value.includes('/chat/') ? { id: value.split('/').at(-1)! } : {};
};
export const isTauriMobile = false;
export class Channel {
	onmessage: (message: any) => void;
	constructor(callback: (message: any) => void) {
		this.onmessage = callback;
	}
}
export const invoke = async (command: string) => {
	throw new Error('Unexpected native invoke in chat engine fixture: ' + command);
};
window.__TAURI_INTERNALS__ = {};

export const native: any = (window.chatNative = {
	feeds: new Map(),
	closed: [],
	acknowledgements: [],
	metadataRequests: 0,
	delays: {},
	resolutionErrors: {},
	roomFailures: {},
	users: new Map()
});

let sequence = 0;
const histories = new Map<string, any[]>();
const channelSequences = new Map<string, number>();

const info = (channel: string) => ({
	broadcaster_id: channel,
	broadcaster_login: channel,
	broadcaster_name: channel,
	game_id: '',
	game_name: '',
	broadcaster_language: 'en',
	title: channel,
	description: '',
	tags: [],
	delay: 0,
	is_branded_content: false,
	profile_image_url: ''
});

const message = (channel: string, text: string) => {
	const channel_sequence = channelSequences.get(channel) ?? 0;
	channelSequences.set(channel, channel_sequence + 1);
	return {
		ts: '2026-10-25T00:00:00Z',
		broadcaster_user_id: channel,
		broadcaster_user_login: channel,
		broadcaster_user_name: channel,
		chatter_user_id: 'user',
		chatter_user_name: 'User',
		message_id: String(++sequence),
		index: sequence,
		channel_sequence,
		text,
		fragments: [{ Text: { index: 0, text } }],
		kind: 'chat',
		message_type: 'text',
		badges: [],
		color: '#ffffff',
		translation: null
	};
};

for (const channel of ['rooma', 'roomb']) histories.set(channel, []);
for (const i of [1, 2, 3]) {
	for (const channel of ['rooma', 'roomb']) {
		histories.get(channel)!.push(message(channel, channel + '-history-' + i));
	}
}

native.push = (channel: string, text: string) => {
	const msg = message(channel, text);
	const history = histories.get(channel) ?? [];
	history.push(msg);
	histories.set(channel, history.slice(-(native.settings?.obj.chat.message_limit ?? 500)));
	for (const feed of native.feeds.values()) {
		if (feed.channel === channel) {
			feed.onmessage({ cursor: sequence, event: { type: 'message', message: msg } });
		}
	}
};

native.custom = (channel: string, overrides: Record<string, any>) => {
	const msg = Object.assign(message(channel, ''), overrides);
	const history = histories.get(channel) ?? [];
	history.push(msg);
	histories.set(channel, history);
	for (const feed of native.feeds.values()) {
		if (feed.channel === channel) {
			feed.onmessage({
				cursor: sequence,
				event: { type: 'message', message: structuredClone(msg) }
			});
		}
	}
};

native.observe = (user: any) => {
	native.users.set(user.user_id, user);
	const cursor = ++sequence;
	for (const feed of native.feeds.values()) {
		feed.onmessage({
			cursor,
			event: { type: 'users' },
			users: { type: 'delta', upserts: [user], removed: [] }
		});
	}
};

const system = (channel: string, input: any) => ({
	kind: 'system',
	index: ++sequence,
	ts: input.ts ?? '2026-11-14T19:00:00Z',
	message_id: input.message_id ?? 'system-' + sequence,
	broadcaster_user_id: channel,
	broadcaster_user_login: channel,
	broadcaster_user_name: channel,
	icon: input.icon ?? null,
	segments: input.segments
});

native.system = (channel: string, input: any) => {
	const msg = system(channel, input);
	const history = histories.get(channel) ?? [];
	history.push(msg);
	histories.set(channel, history.slice(-(native.settings?.obj.chat.message_limit ?? 500)));
	for (const feed of native.feeds.values()) {
		if (feed.channel === channel) {
			feed.onmessage({ cursor: sequence, event: { type: 'message', message: msg } });
		}
	}
};

native.seed = (channel: string, count: number, mixed = false) => {
	const rows: any[] = [];
	for (let i = 0; i < count; i++) {
		rows.push(message(channel, 'backlog-original-' + i));
		if (mixed && i % 125 === 0) {
			rows.push(
				system(channel, { segments: [{ type: 'text', text: 'Synthetic retained notice' }] })
			);
		}
	}
	histories.set(channel, rows);
};

native.translate = (channel: string, text: string, id?: string) => {
	const history = histories.get(channel) ?? [];
	const msg =
		id === undefined
			? history.findLast((item) => item.kind === 'chat')
			: history.find((item) => item.kind === 'chat' && item.message_id === id);
	const update = {
		message_id: msg.message_id,
		translation: { source_language: 'ru', target_language: 'en', translated_text: text }
	};
	msg.translation = update.translation;
	const cursor = ++sequence;
	for (const feed of native.feeds.values()) {
		if (feed.channel === channel) {
			feed.onmessage({ cursor, event: { type: 'translation', update } });
		}
	}
};

native.go = (channel: string | null) => goto(channel ? '/app/chat/' + channel : '/app/settings');
native.reset = () => {
	for (const feed of native.feeds.values()) {
		feed.onmessage({
			cursor: sequence,
			event: { type: 'reset', messages: structuredClone(histories.get(feed.channel) ?? []) },
			users: { type: 'snapshot', users: [...native.users.values()] }
		});
	}
};

export class SyncedState {
	obj: any = $state({});
	ready = $state(true);
	constructor(key: string, fallback: any) {
		this.obj = structuredClone(fallback ?? {});
		if (key === 'auth_state') {
			this.obj = {
				phase: 'authorized',
				token: { user_id: '2', login: 'jerod', profile_image_url: '' }
			};
		}
		if (key === 'channel_cache') {
			this.obj = {
				channels: {
					rooma: { display_name: 'Room A', is_live: false },
					roomb: { display_name: 'Room B', is_live: true }
				}
			};
		}
		if (key === 'app_settings') native.settings = this;
	}
	async sync() {
		this.obj = structuredClone($state.snapshot(this.obj));
		if (this === native.settings) {
			for (const [channel, history] of histories) {
				histories.set(channel, history.slice(-this.obj.chat.message_limit));
			}
		}
		return true;
	}
}

export const commands = {
	async openUserCard(target: unknown) {
		(native.cardRequests ??= []).push(target);
		return { status: 'ok', data: null };
	},
	async getChatFeedContext() {
		return { status: 'ok', data: { epoch: 'browser-page' } };
	},
	async subscribeChat(channel: string, request: unknown, sink: { onmessage: (msg: any) => void }) {
		if (native.resolutionErrors[channel]) {
			return { status: 'error', error: native.resolutionErrors[channel] };
		}
		native.feeds.set(request, { channel, onmessage: sink.onmessage });
		sink.onmessage({
			cursor: sequence,
			event: { type: 'snapshot', messages: structuredClone(histories.get(channel) ?? []) },
			users: { type: 'snapshot', users: [...native.users.values()] }
		});
		if (native.roomFailures[channel]) {
			sink.onmessage({
				cursor: sequence,
				event: { type: 'unavailable', error: native.roomFailures[channel] }
			});
		}
		await new Promise((resolveDelay) => setTimeout(resolveDelay, native.delays[channel] ?? 0));
		return {
			status: 'ok',
			data: { broadcaster_id: channel, broadcaster_login: channel, broadcaster_name: channel }
		};
	},
	async unsubscribeChat(request: unknown) {
		const feed = native.feeds.get(request);
		if (feed) native.closed.push(feed);
		native.feeds.delete(request);
	},
	async acknowledgeChat(request: unknown, cursor: unknown) {
		native.acknowledgements.push({ request, cursor });
	},
	async getChannelInfo(channel: string) {
		native.metadataRequests++;
		return { status: 'ok', data: info(channel) };
	},
	async getFollowedChannels() {
		return {
			status: 'ok',
			data: ['rooma', 'roomb'].map((login) => ({
				id: login,
				login,
				display_name: login,
				profile_image_url: ''
			}))
		};
	},
	async getChatEngineStatus() {
		return {
			connected: true,
			joined: 2,
			eligible: 2,
			lingering: 1,
			pending: 0,
			limited: 0,
			failures: []
		};
	},
	async searchEmotes() {
		return { status: 'ok', data: [] };
	},
	async logout() {
		return { status: 'ok' };
	}
};

export const listen = async () => () => {};
export default { debug() {}, info() {}, warn() {}, error() {} };
