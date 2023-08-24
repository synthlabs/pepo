import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';

export type BrowserCacheOptions = {
	/**
	 * Cache limit
	 */
	limit?: number;
};

export class BrowserCache<T> {
	limit: number;
	store: Map<string, T[]>;

	constructor(options?: BrowserCacheOptions) {
		this.limit = options?.limit ?? -1;
		this.store = new Map<string, T[]>();
	}

	public add = (channel: string, msg: T) => {
		if (!this.store.has(channel)) {
			this.store.set(channel, [msg]);
		}
		this.store.get(channel)?.push(msg);
		const len = this.store.get(channel)?.length ?? 0;
		if (this.limit > 0 && len > this.limit) {
			this.store.get(channel)?.shift();
		}
	};

	public set = (channel: string, msgs: T[]) => {
		this.store.set(channel, msgs);
	};

	public get = (channel: string): T[] => {
		return this.store.get(channel) ?? [];
	};
}
