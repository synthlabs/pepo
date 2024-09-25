import type { ApiClient, HelixChatBadgeSet, HelixChatBadgeVersion } from '@twurple/api';
import Logger from '$lib/logger/log';

export class Badge {
	id: string;
	versions: HelixChatBadgeVersion[];
	versionMap = new Map<string, HelixChatBadgeVersion>();

	constructor(id: string, versions: HelixChatBadgeVersion[]) {
		this.id = id;
		this.versions = versions.map((v) => {
			this.versionMap.set(v.id, v);
			return v;
		});
	}

	public HasVersion = (id: string): boolean => {
		return this.versionMap.has(id);
	};

	public GetVersion = (id: string): HelixChatBadgeVersion | undefined => {
		return this.versionMap.get(id);
	};
}

declare type Store = Map<string, Badge>;

// TODO: make this cache pattern generic
export class BadgeCache {
	#globalStore: Store = new Map<string, Badge>();
	#scopedStore = new Map<string, Store>();

	Set(id: string, badge: Badge) {
		this.#globalStore.set(id, badge);
	}

	Has(id: string): boolean {
		return this.#globalStore.has(id);
	}

	Get(id: string): Badge | undefined {
		return this.#globalStore.get(id);
	}

	ScopedSet(channel: string, id: string, badge: Badge) {
		if (!this.#scopedStore.has(channel)) {
			Logger.trace('setting store');
			this.#scopedStore.set(channel, new Map<string, Badge>());
		}
		// we guarentee the scope exists above
		this.#scopedStore.get(channel)?.set(id, badge);
	}

	ScopedHas(channel: string, id: string): boolean {
		return this.#scopedStore.get(channel)?.has(id) ?? false;
	}

	ScopedGet(channel: string, id: string): Badge | undefined {
		// if we don't even have this scope, see if the global cache has it
		if (!this.#scopedStore.has(channel)) return this.Get(id);

		// we have it in our scoped store
		if (this.#scopedStore.get(channel)?.has(id)) {
			return this.#scopedStore.get(channel)?.get(id) ?? undefined;
		}

		// we didn't find it scoped so see if the global scope has it
		return this.Get(id);
	}

	HasScope(channel: string): boolean {
		return this.#scopedStore.has(channel);
	}
}

export const GlobalBadgeCache = new BadgeCache();

export function newBadgeFromHelix(badgeSet: HelixChatBadgeSet): Badge {
	let versions = badgeSet.versions.map((b: HelixChatBadgeVersion) => {
		return b.id;
	});
	Logger.trace(badgeSet.id, versions);
	return new Badge(badgeSet.id, badgeSet.versions);
}

export async function loadChannelBadges(
	id: string,
	channel: string,
	client: ApiClient,
	cache: BadgeCache
) {
	Logger.debug(`[BadgeCache] loading channel: ${id}`);

	const badges = await client.chat.getChannelBadges(id);
	badges.map((b) => cache.ScopedSet(channel, b.id, newBadgeFromHelix(b)));
}

export async function loadGlobalBadges(client: ApiClient, cache: BadgeCache) {
	Logger.debug('[BadgeCache] loading twitch global badges');
	const badges = await client.chat.getGlobalBadges();
	badges.map((b) => cache.Set(b.id, newBadgeFromHelix(b)));
}
