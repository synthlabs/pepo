import type { Emote } from '$lib/store/emotes/emote';

export interface FFZEmoteSetResp {
	default_sets: number[];
	sets: {
		[id: string]: FFZEmoteSet;
	};
}

export interface FFZRoomResp {
	room: FFZRoom;
	sets: {
		[id: string]: FFZEmoteSet;
	};
}

export interface FFZRoom {
	_id: number;
	twitch_id: number;
	youtube_id: string;
	id: string;
	is_group: boolean;
	display_name: string;
	set: string;
	moderator_badge: string;
	vip_badge: FFZEmoteUrlSizes;
	mod_urls: FFZEmoteUrlSizes;
}

export interface FFZEmoteSet {
	id: number;
	_type: number;
	icon?: string;
	title?: string;
	description?: string;
	css?: string;
	emoticons: FFZEmote[];
}

export interface FFZEmote {
	id: string;
	name: string;
	height: number;
	width: number;
	public: boolean;
	hidden: boolean;
	modifier: boolean;
	modifier_flags: number;
	created_at: string;
	urls: FFZEmoteUrlSizes;
}

export interface FFZEmoteUrlSizes {
	'1': string;
	'2': string;
	'4': string;
}

export const FFZ_FLAVOR = 'ffz';

export function newEmoteFromFFZ(emote: FFZEmote): Emote {
	return {
		get id() {
			return emote.id;
		},
		get name() {
			return emote.name;
		},
		get url() {
			return emote.urls['4'];
		},
		flavor: FFZ_FLAVOR,
		ref: emote
	};
}
