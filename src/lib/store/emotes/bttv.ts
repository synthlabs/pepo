import { Emote } from '$lib/store/emotes/emote';

export const BTTV_FLAVOR = 'bttv';

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

export function newEmoteFromBTTV(emote: BTTVEmote): Emote {
	return {
		get id() {
			return emote.id;
		},
		get name() {
			return emote.code;
		},
		get url() {
			return `https://cdn.betterttv.net/emote/${emote.id}/3x.${emote.imageType}`;
		},
		flavor: BTTV_FLAVOR,
		ref: emote
	};
}
