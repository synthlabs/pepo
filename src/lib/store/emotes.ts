import type { ApiClient, HelixEmote } from '@twurple/api';
import Logger from '$lib/logger/log';

export interface InvalidEmote {
	id: string;
	name: string;
}

export declare type Emote = HelixEmote | InvalidEmote;

export class EmoteCache {
	client!: ApiClient;
	store = new Map<string, Emote>();

	async UseClient(client: ApiClient) {
		Logger.debug('[EmoteCache] setting api client');
		this.client = client;

		const globalEmotes = await this.client.chat.getGlobalEmotes();
		globalEmotes.map((e) => this.store.set(e.id, e));
	}

	async LoadChannel(id: string) {
		Logger.debug(`[EmoteCache] loading channel: ${id}`);
		const channelEmotes = await this.client.chat.getChannelEmotes(id);
		channelEmotes.map((e) => this.store.set(e.id, e));
	}

	Set(id: string, emote: Emote) {
		this.store.set(id, emote);
	}

	Has(id: string): boolean {
		return this.store.has(id);
	}

	Get(id: string): Emote {
		let emote = this.store.get(id);
		if (!emote) emote = { id: 'invalid', name: id };

		return emote;
	}

	passthroughGet(id: string): HelixEmote | undefined {
		const mote = this.store.get(id);
		if (!mote) return undefined;

		if (mote.id === 'invalid') return undefined;

		return mote as HelixEmote;
	}
}

export const GlobalEmoteCache = new EmoteCache();
