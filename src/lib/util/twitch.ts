import Logger from '$lib/logger/log';
import { TwitchApiClient } from '$lib/store/runes/apiclient.svelte';
import { TWITCH_EMOTE_V2 } from '$lib/util/constants';
import { HelixStream, HelixUser } from '@twurple/api';

export function getTwitchEmoteURL(id: string, scale: number, animated = true, dark = true) {
	return `${TWITCH_EMOTE_V2}/${id}/${animated ? 'default' : 'static'}/${dark ? 'dark' : 'light'}/${
		scale == 4 ? 3 : scale
	}.0`;
}

export const getUserByName = async (
	userName: string,
	client: TwitchApiClient
): Promise<HelixUser | null> => {
	const user = await client.api.users.getUserByName(userName);
	if (!user) {
		Logger.debug('failed to get user');
		return null;
	}
	return user;
};

export const getStream = async (
	userName: string,
	client: TwitchApiClient
): Promise<HelixStream | null> => {
	const user = await client.api.users.getUserByName(userName);
	if (!user) {
		Logger.debug('failed to get user');
		return null;
	}
	return await user.getStream();
};
