import type { Emote } from '$lib/store/emotes/emote';

export interface SeventvChannelResp {
	id: string;
	platform: string;
	username: string;
	display_name: string;
	emote_set: SeventvEmoteSet;
}

export interface SeventvEmoteSet {
	id: number;
	name: string;
	flags: number;
	tags: string[];
	immutable?: boolean;
	privileged?: boolean;
	emotes: SeventvEmote[];
	emote_count: number;
	capacity?: number;
	owner?: SeventvUser;
}

export interface SeventvEmote {
	id: string;
	name: string;
	flags: number;
	timestamp: number;
	actor_id?: string;
	data: SeventvEmoteDetails;
}

export interface SeventvEmoteDetails {
	id: string;
	name: string;
	flags: number;
	tags: string[];
	lifecycle?: number;
	state?: string[];
	listed: boolean;
	animated: boolean;
	owner?: SeventvUser;
	host: SeventvEmoteData;
}

export interface SeventvEmoteData {
	url: string;
	files: SeventvFiles[];
}

export interface SeventvFiles {
	name: string;
	static_name: string;
	width: number;
	height: number;
	frame_count: number;
	size: number;
	format: string;
}

export interface SeventvUser {
	id: string;
	username: string;
	display_name: string;
	avatar_url: string;
	roles?: string[];
}

export const SEVENTV_FLAVOR = 'seventv';
const WEBP_FORMAT = 'WEBP';
const SIZE_4X = '4x.webp';
const SIZE_2X = '2x.webp';
const SIZE_1X = '1x.webp';

export function newEmoteFromSeventv(emote: SeventvEmote): Emote {
	return {
		get id() {
			return emote.id;
		},
		get name() {
			return emote.name;
		},
		get url() {
			// TODO: clean this up
			const base = emote.data.host.url;

			const file = emote.data.host.files.find((f) => {
				if (f.format === WEBP_FORMAT && f.name === SIZE_4X) return f;
			});
			return `https:${base}/${file?.name}`;
		},
		flavor: SEVENTV_FLAVOR,
		ref: emote
	};
}
