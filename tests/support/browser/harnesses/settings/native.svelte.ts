/* eslint-disable @typescript-eslint/no-explicit-any, svelte/prefer-svelte-reactivity */
const params = new URLSearchParams(location.search);
if (!sessionStorage.getItem('sidebar-check-started')) {
	if (params.has('open')) {
		localStorage.setItem('sidebar:state', String(params.get('open') !== 'false'));
	} else {
		localStorage.removeItem('sidebar:state');
	}
	sessionStorage.setItem('sidebar-check-started', 'true');
}

export const page = $state({
	url: new URL(location.href),
	params: (location.pathname.includes('/chat/') ? { id: 'maya' } : {}) as Record<string, string>
});
export const resolve = (value: string) => value;
export const goto = async (value: string, options: { replaceState?: boolean } = {}) => {
	history[options.replaceState ? 'replaceState' : 'pushState']({}, '', value);
	page.url = new URL(value, location.origin);
	page.params = value.includes('/chat/') ? { id: 'maya' } : {};
};

document.addEventListener('click', (event) => {
	const anchor = event.target instanceof Element ? event.target.closest('a[href]') : null;
	if (
		!anchor ||
		event.defaultPrevented ||
		event.button !== 0 ||
		event.ctrlKey ||
		event.metaKey ||
		event.shiftKey ||
		event.altKey
	) {
		return;
	}
	const href = new URL((anchor as HTMLAnchorElement).href, location.origin);
	if (
		href.origin !== location.origin ||
		(href.pathname !== '/app' && !href.pathname.startsWith('/app/'))
	) {
		return;
	}
	event.preventDefault();
	void goto(href.pathname + href.search);
});

window.addEventListener('popstate', () => {
	page.url = new URL(location.href);
	page.params = location.pathname.includes('/chat/') ? { id: 'maya' } : {};
});

export const isTauriMobile = false;
const pendingReleases: (() => void)[] = [];
export const native: any = (window.settingsNative = {
	delay: 0,
	hold: false,
	release() {
		this.hold = false;
		while (pendingReleases.length > 0) pendingReleases.shift()?.();
	},
	fail: false,
	reject: false,
	writes: [],
	states: {},
	errors: []
});
export const appErrors = {
	capture: (...args: unknown[]) => native.errors.push(args)
};

export class SyncedState {
	obj: any = $state({});
	ready = $state(false);
	initialized: Promise<void>;
	constructor(key: string, fallback: any) {
		let value = structuredClone(fallback ?? {});
		if (key === 'auth_state') {
			value = {
				phase: 'authorized',
				token: { user_id: '2', login: 'jerod', profile_image_url: '' }
			};
		}
		if (key === 'channel_cache') {
			value = { channels: { maya: { display_name: 'Maya', is_live: false } } };
		}
		if (key === 'app_settings') {
			if (!sessionStorage.getItem('settings-check-started')) {
				localStorage.removeItem('settings-check');
				sessionStorage.setItem('settings-check-started', 'true');
			}
			value = JSON.parse(localStorage.getItem('settings-check') ?? 'null') ?? value;
			if (params.has('theme')) value.appearance.theme = params.get('theme');
			native.states[key] = this;
		}
		this.obj = value;
		this.initialized = new Promise((resolveInit) =>
			setTimeout(
				() => {
					this.obj = structuredClone(value);
					this.ready = true;
					resolveInit();
				},
				params.has('delayed') ? 500 : 0
			)
		);
	}
	async sync() {
		const value = $state.snapshot(this.obj);
		native.writes.push(value);
		const fail = native.fail;
		const reject = native.reject;
		if (native.hold) {
			await new Promise<void>((resolveHold) => {
				pendingReleases.push(resolveHold);
			});
		}
		if (native.delay > 0) {
			await new Promise((resolveDelay) => setTimeout(resolveDelay, native.delay));
		}
		if (reject) throw new Error('Simulated native failure');
		if (fail) return false;
		localStorage.setItem('settings-check', JSON.stringify(value));
		this.obj = structuredClone(value);
		return true;
	}
	close() {
		return Promise.resolve();
	}
}

export const commands = {
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
export default { debug() {}, info() {}, error() {} };
