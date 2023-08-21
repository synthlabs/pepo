import { TWITCH_EMOTE_V2 } from '$lib/util/constants';

export function getTwitchEmoteURL(id: string, scale: number, animated = false, dark = true) {
	return `${TWITCH_EMOTE_V2}/${id}/${animated ? 'default' : 'static'}/${dark ? 'dark' : 'light'}/${
		scale == 4 ? 3 : scale
	}.0`;
}
