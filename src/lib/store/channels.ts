import { createWritableStore } from './writeable';

export function Sanitize(channel: string): string {
	let chan = channel.toLowerCase();
	if (chan.charAt(0) === '#') chan = chan.slice(1);
	return chan;
}

export function IsIterable(input: any): boolean {
	if (input === null || input === undefined) {
		return false;
	}

	return typeof input[Symbol.iterator] === 'function';
}

export function Decode(obj: any): Set<string> {
	console.log('channelCache Decode: ', obj);
	if (IsIterable(obj)) {
		return new Set(obj);
	}

	return new Set();
}

export function Encode(obj: Set<string>): any {
	console.log('channelCache Encode: ', obj);
	return Array.from(obj);
}

export type ChannelCache = Set<string>;

export const channels = createWritableStore<Set<string>>('channels', new Set<string>());
