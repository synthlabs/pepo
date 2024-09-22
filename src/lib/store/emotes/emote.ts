import Logger from '$lib/logger/log';
import { HelixEmote } from '@twurple/api';
import { FFZEmote } from '$lib/store/emotes/ffz';
import { BTTVEmote } from '$lib/store/emotes/bttv';

export interface IInvalidEmote {
	id: string;
	name: string;
	url: string;
}

export const InvalidEmote: IInvalidEmote = { id: 'invalid', name: 'invalid', url: '' };

export declare type EmoteRef = HelixEmote | BTTVEmote | FFZEmote | IInvalidEmote;

export interface Emote {
	id: string;
	name: string;
	url: string;
	flavor?: string;
	ref?: EmoteRef;
}
