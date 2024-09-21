import { ApiClient, HelixEmote } from '@twurple/api';
import Logger from '$lib/logger/log';

export interface IInvalidEmote {
	id: string;
	name: string;
	url: string;
}

const InvalidEmote: IInvalidEmote = { id: 'invalid', name: 'invalid', url: '' };

export class BTTVEmote {
	id!: string;
	code!: string;
	imageType!: string;
	animated?: boolean;
	modifier?: boolean;
	height?: number;
	width?: number;
	userId?: string;
}

export declare type EmoteRef = HelixEmote | BTTVEmote | IInvalidEmote;

export class Emote {
	#emoteRef: EmoteRef;
	constructor(emote: EmoteRef) {
		this.#emoteRef = emote;
	}

	get ref() {
		return this.#emoteRef;
	}

	get id() {
		return this.#emoteRef.id;
	}

	get name() {
		if ('code' in this.#emoteRef && 'imageType' in this.#emoteRef) {
			return this.#emoteRef.code;
		}
		return this.#emoteRef.name;
	}

	get url() {
		if (this.#emoteRef instanceof HelixEmote) {
			return this.#emoteRef.getStaticImageUrl('3.0', 'dark');
		}
		if ('code' in this.#emoteRef && 'imageType' in this.#emoteRef) {
			return `https://cdn.betterttv.net/emote/${this.#emoteRef.id}/3x.${this.#emoteRef.imageType}`;
		}
		return this.#emoteRef.url;
	}
}

export class EmoteCache {
	#store = new Map<string, Emote>();
	#reverseIndex = new Map<string, string>();

	Set(id: string, emote: EmoteRef) {
		Logger.trace(`[EmoteCache] set ${id}`);
		if ('code' in emote) {
			Logger.trace(`[EmoteCache] bttv emote, setting index ${emote.code}=${id}`);
			this.#reverseIndex.set(emote.code, id);
		}
		this.#store.set(id, new Emote(emote));
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
		if (!emote) return new Emote(InvalidEmote);
		return emote;
	}

	GetByName(name: string): Emote {
		const id = this.#reverseIndex.get(name);
		if (!id) {
			return new Emote(InvalidEmote);
		}
		return this.Get(id);
	}

	// gross but for now it works
	get BTTVGlobalEmotes() {
		let res: BTTVEmote[] = [];
		this.#store.forEach((v, k) => {
			if (v instanceof BTTVEmote) {
				res.push(v);
			}
		});
		return res;
	}

	// passthroughHelixGet(id: string): HelixEmote | undefined {
	// 	const mote = this.store.get(id);
	// 	if (!mote) return undefined;

	// 	if (mote.id === 'invalid') return undefined;

	// 	return mote as HelixEmote;
	// }
}

export const GlobalEmoteCache = new EmoteCache();

export async function loadChannelEmotes(id: string, client: ApiClient, cache: EmoteCache) {
	Logger.debug(`[EmoteCache] loading channel: ${id}`);
	const channelEmotes = await client.chat.getChannelEmotes(id);
	channelEmotes.map((e) => cache.Set(e.id, e));
}

export async function loadGlobalEmotes(client: ApiClient, cache: EmoteCache) {
	Logger.debug('[EmoteCache] loading twitch global emotes');

	const globalEmotes = await client.chat.getGlobalEmotes();
	globalEmotes.map((e) => cache.Set(e.id, e));

	Logger.debug('[EmoteCache] loading bttv global emotes');
	const resp = await fetch('https://api.betterttv.net/3/cached/emotes/global', {
		method: 'GET'
	});
	const bttvEmotes: BTTVEmote[] = await resp.json();
	bttvEmotes.map((e: BTTVEmote) => cache.Set(e.id, e));
}
