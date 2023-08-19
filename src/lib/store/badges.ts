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

// TODO: make this cache pattern generic
export class BadgeCache {
	client!: ApiClient;
	store = new Map<string, Badge>();

	async UseClient(client: ApiClient) {
		Logger.debug('[BadgeCache] setting api client');
		this.client = client;

		const badges = await this.client.chat.getGlobalBadges();
		badges.map(this.parseBadgeSet).map((b) => {
			this.Set(b.id, b);
		});
	}

	async LoadChannel(id: string) {
		Logger.debug(`[BadgeCache] loading channel: ${id}`);

		const badges = await this.client.chat.getChannelBadges(id);

		badges.map(this.parseBadgeSet).map((b) => {
			this.Set(b.id, b);
		});
	}

	Set(id: string, badge: Badge) {
		this.store.set(id, badge);
	}

	Has(id: string): boolean {
		return this.store.has(id);
	}

	// TODO: HasChannel - to guard excess LoadChannel's

	Get(id: string): Badge | undefined {
		return this.store.get(id);
	}

	private parseBadgeSet(badgeSet: HelixChatBadgeSet): Badge {
		let versions = badgeSet.versions.map((b: HelixChatBadgeVersion) => {
			return b.id;
		});
		Logger.trace(badgeSet.id, versions);
		return new Badge(badgeSet.id, badgeSet.versions);
	}
}

export const GlobalBadgeCache = new BadgeCache();
