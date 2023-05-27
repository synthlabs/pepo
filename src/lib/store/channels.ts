import { createWritableStore } from './writeable';

export function Sanitize(channel: string): string {
	let chan = channel.toLowerCase();
	if (chan.charAt(0) === '#') chan = chan.slice(1);
	return chan;
}

export type ChannelCache = string[];

export const channels = createWritableStore('channels', [] as string[]);
