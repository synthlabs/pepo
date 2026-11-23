/* eslint-disable @typescript-eslint/no-explicit-any, svelte/prefer-svelte-reactivity */
export const params = new URLSearchParams(location.search);
export const page = $state({
	url: new URL('/app/chat/maya', location.origin),
	params: { id: 'maya' } as Record<string, string>
});
export const resolve = (value: string) => value;
export const goto = async (value: string) => {
	page.url = new URL(value, location.origin);
	page.params = value.includes('/chat/') ? { id: 'maya' } : {};
};
export const isTauriMobile = false;
export const native: any = (window.networkFixture = {
	listeners: new Map(),
	retries: [],
	logs: [],
	snapshot: { revision: 0, issues: [] },
	networkListeners: 0,
	snapshots: 0
});
window.__TAURI_INTERNALS__ = {};
export class SyncedState {
	obj: any = $state({});
	ready = $state(true);
	initialized = Promise.resolve();
	constructor(key: string, fallback: any) {
		this.obj =
			key === 'auth_state'
				? {
						phase: 'authorized',
						device_code: '',
						token: { user_id: '2', login: 'jerod', profile_image_url: '' }
					}
				: key === 'channel_cache'
					? { channels: { maya: { broadcaster_id: '1', display_name: 'Maya', is_live: false } } }
					: structuredClone(fallback ?? {});
	}
	async sync() {
		return true;
	}
}
export const commands = {
	getNetworkStatus: () => {
		native.snapshots++;
		return native.snapshots === 1
			? new Promise((resolveStatus) => {
					native.initial = resolveStatus;
				})
			: Promise.resolve(native.snapshot);
	},
	retryNetworkOperation: async (id: string) => {
		native.retries.push(id);
	},
	getChatEngineStatus: async () => ({
		connected: true,
		joined: 1,
		eligible: 1,
		lingering: 0,
		pending: 0,
		limited: 0,
		failures: []
	}),
	getFollowedChannels: async () => ({
		status: 'ok',
		data: [{ id: '1', login: 'maya', display_name: 'Maya', profile_image_url: '' }]
	}),
	getChannelInfo: async () => ({
		status: 'ok',
		data: { broadcaster_id: '1', broadcaster_login: 'maya', broadcaster_name: 'Maya' }
	}),
	logout: async () => ({ status: 'ok' })
};
export async function listen(name: string, callback: (event: any) => void) {
	native.listeners.set(name, callback);
	if (name === 'network_status') native.networkListeners++;
	return () => {
		native.listeners.delete(name);
		if (name === 'network_status') native.networkListeners--;
	};
}
export async function invoke(command: string) {
	if (command === 'plugin:inbound|captured_errors') return [];
	if (command === 'plugin:inbound|capabilities') return { has_config: false };
	return null;
}
export const error = async (message: string, options?: unknown) => {
	native.logs.push({ message, options });
};
export const warn = async () => {};
export const info = async () => {};
export const debug = async () => {};
export const trace = async () => {};
export default { info, warn, error, debug, trace };
