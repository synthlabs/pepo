import { createWritableStore } from './writeable';

export function Sanitize(channel: string): string {
	let chan = channel.toLowerCase();
	if (chan.charAt(0) === '#') chan = chan.slice(1);
	return chan;
}

export type ChannelCache = Set<string>;

export const channels = createWritableStore<Set<string>>('channels', new Set<string>());
