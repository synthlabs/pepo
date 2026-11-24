import { SyncedState } from 'tauri-svelte-synced-store';
import type { ChannelCache } from '$lib/bindings';
import { appErrors } from '$lib/errors/controller';

export const DEFAULT_CHANNEL_CACHE: ChannelCache = {
	channels: {}
};

export const channelCache = new SyncedState<ChannelCache>(
	'channel_cache',
	structuredClone(DEFAULT_CHANNEL_CACHE)
);

void channelCache.initialized.catch((error) => {
	appErrors.capture(error, {
		source: 'synced-store.channel_cache',
		title: 'Could not sync channel cache'
	});
});

import.meta.hot?.dispose(() => {
	void channelCache.close();
});
