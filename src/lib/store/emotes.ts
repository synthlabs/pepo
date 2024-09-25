import { ApiClient } from '@twurple/api';
import Logger from '$lib/logger/log';
import { type Emote, InvalidEmote } from '$lib/store/emotes/emote';
import { BTTV_FLAVOR, type BTTVEmote, newEmoteFromBTTV } from '$lib/store/emotes/bttv';
import { FFZ_FLAVOR } from '$lib/store/emotes/ffz';
import { newEmoteFromHelix } from '$lib/store/emotes/helix';

export class EmoteCache {
	#store = new Map<string, Emote>();
	#reverseIndex = new Map<string, string>();

	Set(id: string, emote: Emote) {
		Logger.trace(`[EmoteCache] set ${id}`);
		Logger.trace(`[EmoteCache] setting reverse index ${emote.name}=${id}`);

		this.#store.set(id, emote);
		this.#reverseIndex.set(emote.name, id);
	}

	Has(id: string): boolean {
		return this.#store.has(id);
	}

	HasName(name: string): boolean {
		return this.#reverseIndex.has(name);
	}

	// TODO: HasChannel - to guard excess LoadChannel's

	Get(id: string): Emote {
		const emote = this.#store.get(id);
		if (!emote) {
			return InvalidEmote;
		}
		return emote;
	}

	GetByName(name: string): Emote {
		const id = this.#reverseIndex.get(name);
		if (!id) {
			return InvalidEmote;
		}
		return this.Get(id);
	}
}

export const GlobalEmoteCache = new EmoteCache();

export async function loadChannelEmotes(id: string, client: ApiClient, cache: EmoteCache) {
	Logger.debug(`[EmoteCache] loading channel: ${id}`);
	// const channelEmotes = await client.chat.getChannelEmotes(id);
	// channelEmotes.map((e) => cache.Set(e.id, newEmoteFromHelix(e)));
}

export async function loadGlobalEmotes(client: ApiClient, cache: EmoteCache) {
	Logger.debug('[EmoteCache] loading twitch global emotes');

	const globalEmotes = await client.chat.getGlobalEmotes();
	globalEmotes.map((e) => cache.Set(e.id, newEmoteFromHelix(e)));

	Logger.debug('[EmoteCache] loading bttv global emotes');
	const resp = await fetch('https://api.betterttv.net/3/cached/emotes/global', {
		method: 'GET'
	});
	const bttvEmotes: BTTVEmote[] = await resp.json();
	bttvEmotes.map((e: BTTVEmote) => cache.Set(e.id, newEmoteFromBTTV(e)));
}
