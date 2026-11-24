import { flushSync, tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings, AuthState, ChannelCache } from '$lib/bindings';
import { DEFAULT_APP_SETTINGS } from '$lib/settings';

type SyncSnapshot = { kind: 'snapshot'; value: unknown };
type MockChannelSink = { onmessage: (message: SyncSnapshot) => void };

const harness = vi.hoisted(() => ({
	nextSubscriberId: 1,
	subscribeCalls: [] as { name: string; subscriberId: string; channel: MockChannelSink }[],
	unsubscribeCalls: [] as { name: string; subscriberId: string }[],
	updateCalls: [] as { name: string; value: unknown; subscriberId?: string }[],
	channels: new Map<string, Map<string, MockChannelSink>>(),
	subscribeError: null as unknown,
	updateHandler: null as
		((state: { name: string; value: unknown; subscriberId?: string }) => Promise<boolean>) | null,
	capturedErrors: [] as { error: unknown; options?: { source?: string; title?: string } }[],
	reset() {
		this.nextSubscriberId = 1;
		this.subscribeCalls = [];
		this.unsubscribeCalls = [];
		this.updateCalls = [];
		this.channels = new Map();
		this.subscribeError = null;
		this.updateHandler = null;
		this.capturedErrors = [];
	},
	emit(name: string, value: unknown, excludeSubscriberId?: string) {
		for (const [id, channel] of this.channels.get(name) ?? []) {
			if (excludeSubscriberId && id === excludeSubscriberId) continue;
			channel.onmessage({ kind: 'snapshot', value: structuredClone(value) });
		}
	}
}));

vi.mock('@tauri-apps/api/core', () => ({
	Channel: class MockChannel<T = SyncSnapshot> {
		onmessage: (message: T) => void;
		constructor(onmessage?: (message: T) => void) {
			this.onmessage = onmessage ?? (() => {});
		}
	},
	invoke: async (
		command: string,
		args: {
			name?: string;
			subscriberId?: string;
			channel?: MockChannelSink;
			state?: { name: string; value: unknown; subscriberId?: string };
		}
	) => {
		if (command === 'subscribe_state' && args.name && args.channel) {
			if (harness.subscribeError) throw harness.subscribeError;
			const subscriberId = `sub-${harness.nextSubscriberId++}`;
			const channels = harness.channels.get(args.name) ?? new Map<string, MockChannelSink>();
			channels.set(subscriberId, args.channel);
			harness.channels.set(args.name, channels);
			harness.subscribeCalls.push({ name: args.name, subscriberId, channel: args.channel });
			return subscriberId;
		}
		if (command === 'unsubscribe_state' && args.name && args.subscriberId) {
			harness.unsubscribeCalls.push({ name: args.name, subscriberId: args.subscriberId });
			return harness.channels.get(args.name)?.delete(args.subscriberId) ?? false;
		}
		if (command === 'update_state' && args.state) {
			const payload = {
				name: args.state.name,
				value: structuredClone(args.state.value),
				subscriberId: args.state.subscriberId
			};
			harness.updateCalls.push(payload);
			if (harness.updateHandler) {
				return harness.updateHandler(payload);
			}
			harness.emit(payload.name, payload.value, payload.subscriberId);
			return true;
		}
		throw new Error(`Unexpected command ${command}`);
	}
}));

vi.mock('$lib/errors/controller', () => ({
	appErrors: {
		capture(error: unknown, options?: { source?: string; title?: string }) {
			harness.capturedErrors.push({ error, options });
		}
	}
}));

async function flushEffects() {
	flushSync();
	await tick();
	flushSync();
}

describe('shared synced stores', () => {
	beforeEach(() => {
		vi.resetModules();
		harness.reset();
	});

	it('subscribes authState and channelCache once with typed defaults and unsubscribes on close', async () => {
		const { authState, DEFAULT_AUTH_STATE } = await import('$lib/stores/auth.svelte');
		const { channelCache, DEFAULT_CHANNEL_CACHE } =
			await import('$lib/stores/channel-cache.svelte');

		expect(harness.subscribeCalls.map((call) => call.name)).toEqual([
			'auth_state',
			'channel_cache'
		]);
		expect(authState.ready).toBe(false);
		expect(authState.obj).toEqual(DEFAULT_AUTH_STATE);
		expect(channelCache.ready).toBe(false);
		expect(channelCache.obj).toEqual(DEFAULT_CHANNEL_CACHE);

		const nextAuth: AuthState = {
			phase: 'authorized',
			device_code: '',
			token: {
				access_token: 'access',
				client_id: 'client',
				login: 'viewer',
				user_id: '42',
				refresh_token: 'refresh',
				expires_in: 3600,
				profile_image_url: ''
			}
		};
		const nextCache: ChannelCache = {
			channels: {
				maya: {
					broadcaster_id: '1',
					login: 'maya',
					display_name: 'Maya',
					profile_image_url: '',
					is_live: true,
					is_partner: false,
					stream: null
				}
			}
		};

		harness.emit('auth_state', nextAuth);
		harness.emit('channel_cache', nextCache);
		await Promise.all([authState.initialized, channelCache.initialized]);

		expect(authState.ready).toBe(true);
		expect(authState.obj).toEqual(nextAuth);
		expect(channelCache.ready).toBe(true);
		expect(channelCache.obj).toEqual(nextCache);

		await Promise.all([authState.close(), channelCache.close()]);
		expect(harness.unsubscribeCalls).toEqual([
			{ name: 'auth_state', subscriberId: 'sub-1' },
			{ name: 'channel_cache', subscriberId: 'sub-2' }
		]);
		expect(harness.capturedErrors).toEqual([]);
	});

	it('captures initial subscription rejections via appErrors without unhandled rejections', async () => {
		const syncError = { code: 'unknownKey', message: 'missing store key' };
		harness.subscribeError = syncError;

		const { authState } = await import('$lib/stores/auth.svelte');
		const { channelCache } = await import('$lib/stores/channel-cache.svelte');
		const { appSettings } = await import('$lib/stores/settings.svelte');

		await expect(authState.initialized).rejects.toEqual(syncError);
		await expect(channelCache.initialized).rejects.toEqual(syncError);
		await expect(appSettings.initialized).rejects.toEqual(syncError);
		await tick();

		expect(authState.ready).toBe(false);
		expect(channelCache.ready).toBe(false);
		expect(appSettings.ready).toBe(false);
		expect(harness.capturedErrors.map((entry) => entry.options?.source)).toEqual([
			'synced-store.auth_state',
			'synced-store.channel_cache',
			'synced-store.app_settings'
		]);
	});

	it('saves settings without self-echo, rolls back appSettings.obj on failure, and preserves newer external snapshots', async () => {
		const { appSettings, settingsState, updateAppSettings } =
			await import('$lib/stores/settings.svelte');

		const initial = structuredClone(DEFAULT_APP_SETTINGS);
		harness.emit('app_settings', initial);
		await appSettings.initialized;
		await flushEffects();

		expect(settingsState.ready).toBe(true);
		expect(await updateAppSettings({ appearance: { theme: 'light' } })).toBe(true);
		await flushEffects();

		expect(harness.updateCalls).toHaveLength(1);
		expect(harness.updateCalls[0].subscriberId).toBe('sub-1');
		expect(settingsState).toMatchObject({ saving: false, result: 'saved' });
		expect(settingsState.value.appearance.theme).toBe('light');
		expect(appSettings.obj.appearance.theme).toBe('light');

		// Rejection rolls back both settingsState.value and appSettings.obj when no external snapshot intervened.
		harness.updateHandler = async () => {
			throw { code: 'persistenceFailed', message: 'disk full' };
		};
		expect(await updateAppSettings({ appearance: { theme: 'dark' } })).toBe(false);
		await flushEffects();
		expect(settingsState).toMatchObject({ saving: false, result: 'error' });
		expect(settingsState.value.appearance.theme).toBe('light');
		expect(appSettings.obj.appearance.theme).toBe('light');

		// A newer external snapshot arriving while a failing save is in flight is not overwritten on rollback.
		let finishSave!: (saved: boolean) => void;
		harness.updateHandler = () =>
			new Promise<boolean>((resolve) => {
				finishSave = resolve;
			});
		const failing = updateAppSettings({ chat: { show_badges: false } });
		await flushEffects();

		const external: AppSettings = structuredClone(initial);
		external.appearance.theme = 'light';
		external.chat.show_badges = true;
		external.chat.message_limit = 777;
		harness.emit('app_settings', external);
		await flushEffects();

		finishSave(false);
		expect(await failing).toBe(false);
		await flushEffects();

		expect(appSettings.obj.chat.message_limit).toBe(777);
		expect(settingsState.value.chat.message_limit).toBe(777);
		expect(settingsState.value.chat.show_badges).toBe(true);

		await appSettings.close();
		expect(harness.unsubscribeCalls).toEqual([{ name: 'app_settings', subscriberId: 'sub-1' }]);
	});
});
