import { ApiClient } from '@twurple/api';
import Logger from '$lib/logger/log';
import { type Emote, InvalidEmote } from '$lib/store/emotes/emote';
import {
	BTTV_FLAVOR,
	type BTTVEmote,
	type BTTVEmoteAPIResp,
	newEmoteFromBTTV
} from '$lib/store/emotes/bttv';
import { FFZ_FLAVOR, newEmoteFromFFZ, type FFZRoomResp } from '$lib/store/emotes/ffz';
import { newEmoteFromHelix } from '$lib/store/emotes/helix';
import log from '$lib/logger/log';

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

export async function loadChannelEmotes(
	id: string,
	name: string,
	client: ApiClient,
	cache: EmoteCache
) {
	Logger.debug(`[EmoteCache] loading channel: ${name}`);
	// const channelEmotes = await client.chat.getChannelEmotes(id);
	// channelEmotes.map((e) => cache.Set(e.id, newEmoteFromHelix(e)));

	Logger.debug(`[EmoteCache] loading bttv channel emotes: ${id}`);
	const bttvResp = await fetch(`https://api.betterttv.net/3/cached/users/twitch/${id}`, {
		method: 'GET'
	});
	const parsedBttvResp: BTTVEmoteAPIResp = await bttvResp.json();
	parsedBttvResp.channelEmotes.map((e) => cache.Set(e.id, newEmoteFromBTTV(e)));
	parsedBttvResp.sharedEmotes.map((e) => cache.Set(e.id, newEmoteFromBTTV(e)));

	Logger.debug(`[EmoteCache] loading ffz channel emotes: ${id}`);
	//https://api.frankerfacez.com/v1/room/sirstendec
	const ffzResp = await fetch(`https://api.frankerfacez.com/v1/room/${name}`, {
		method: 'GET'
	});
	const parsedFfzResp: FFZRoomResp = await ffzResp.json();
	parsedFfzResp.sets[parsedFfzResp.room.set].emoticons.map((e) =>
		cache.Set(e.id, newEmoteFromFFZ(e))
	);
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
