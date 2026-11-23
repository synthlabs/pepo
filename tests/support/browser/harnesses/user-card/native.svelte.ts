/* eslint-disable @typescript-eslint/no-explicit-any, svelte/prefer-svelte-reactivity */
const callbacks = new Set<(event: { payload: any }) => void>();
const preferences = $state({ chat: { user_card_close_on_unfocus: true } });
let current: any = {
	user: { user_id: '1', login: 'maya', display_name: '마야', color: null },
	profile_image_url: null,
	channel_id: 'room',
	channel_login: 'room',
	badges: null,
	profile_status: 'ready',
	error: null,
	pinned: false,
	chrome: new URL(location.href).searchParams.get('chrome') ?? 'compact',
	revision: 1
};

const native: any = (window.cardNative = {
	ready: false,
	heights: [] as number[],
	closes: 0,
	retries: 0,
	urls: [] as string[],
	errors: [] as string[],
	update(patch: Record<string, any>) {
		current = { ...current, ...patch, revision: current.revision + 1 };
		for (const callback of callbacks) callback({ payload: current });
	},
	policy(value: boolean) {
		preferences.chat.user_card_close_on_unfocus = value;
	}
});

const ok = <T>(data: T) => ({ status: 'ok' as const, data });
export const commands = {
	async getUserCardState() {
		return ok(current);
	},
	async readyUserCard(height: number) {
		native.heights.push(height);
		native.ready = true;
		return ok(null);
	},
	async setUserCardPinned(pinned: boolean) {
		native.update({ pinned });
		return ok(null);
	},
	async refreshUserCard() {
		native.retries++;
		native.update({ profile_status: 'loading', error: null });
		return ok(null);
	}
};
export const getCurrentWindow = () => ({
	label: 'user-card-1',
	async listen(_event: string, callback: (event: { payload: any }) => void) {
		callbacks.add(callback);
		return () => callbacks.delete(callback);
	},
	async close() {
		native.closes++;
	}
});
export const appSettings = { initialized: Promise.resolve() };
export const getNormalizedAppSettings = () => preferences;
export const openUrl = async (url: string) => {
	native.urls.push(url);
};
export const appErrors = {
	capture(error: unknown) {
		native.errors.push(String(error));
	}
};
