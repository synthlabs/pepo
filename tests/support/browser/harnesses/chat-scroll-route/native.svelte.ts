/* eslint-disable @typescript-eslint/no-explicit-any */
import { DEFAULT_APP_SETTINGS } from '$lib/settings';

export const settings = $state(structuredClone(DEFAULT_APP_SETTINGS));
export const page = { params: { id: 'fixture' } };
export const authState = {
	obj: { phase: 'authorized', token: { login: 'fixture', user_id: '1' } }
};
export const getNormalizedAppSettings = () => settings;
export const native: { settings: typeof settings; feed: any } = { settings, feed: null };
export const listen = async () => () => {};
export class Channel {
	onmessage: (message: any) => void;
	constructor(callback: (message: any) => void) {
		this.onmessage = callback;
	}
}
export const invoke = async (command: string) => {
	throw new Error('Unexpected native invoke in scroll fixture: ' + command);
};
export const commands = {
	getChatFeedContext: async () => ({ status: 'ok', data: { epoch: 'scroll-page' } }),
	subscribeChat: async (
		channel: string,
		request: unknown,
		sink: { onmessage: (msg: any) => void }
	) => {
		native.feed = { request, onmessage: sink.onmessage };
		sink.onmessage({ cursor: 0, event: { type: 'snapshot', messages: [] } });
		return {
			status: 'ok',
			data: { broadcaster_id: '1', broadcaster_login: channel, broadcaster_name: 'Fixture' }
		};
	},
	unsubscribeChat: async (request: unknown) => {
		if (native.feed?.request === request) native.feed = null;
	},
	acknowledgeChat: async () => {},
	searchEmotes: async () => ({ status: 'ok', data: [] })
};
export default { debug() {}, info() {}, error() {} };
