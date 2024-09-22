import { HelixChannelEmote, HelixEmote } from '@twurple/api';
import type { Emote } from '$lib/store/emotes/emote';

export const HELIX_FLAVOR = 'helix';

export function newEmoteFromHelix(emote: HelixEmote | HelixChannelEmote): Emote {
	return {
		get id() {
			return emote.id;
		},
		get name() {
			return emote.name;
		},
		get url() {
			return emote.getStaticImageUrl('3.0', 'dark') || '';
		},
		flavor: HELIX_FLAVOR,
		ref: emote
	};
}
