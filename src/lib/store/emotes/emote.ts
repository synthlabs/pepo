import { HelixEmote } from '@twurple/api';
import type { FFZEmote } from '$lib/store/emotes/ffz';
import type { BTTVEmote } from '$lib/store/emotes/bttv';
import type { SeventvEmote } from './seventv';

export interface IInvalidEmote {
	id: string;
	name: string;
	url: string;
}

export const InvalidEmote: IInvalidEmote = { id: 'invalid', name: 'invalid', url: '' };

export declare type EmoteRef = HelixEmote | BTTVEmote | FFZEmote | SeventvEmote | IInvalidEmote;

export interface Emote {
	id: string;
	name: string;
	url: string;
	flavor?: string;
	ref?: EmoteRef;
}
