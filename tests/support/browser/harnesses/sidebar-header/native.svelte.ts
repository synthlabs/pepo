/* eslint-disable @typescript-eslint/no-explicit-any, svelte/prefer-svelte-reactivity */
const channel = { id: '1', login: 'maya', display_name: 'Maya', profile_image_url: '' };
export const commands = {
	getFollowedChannels: async () => ({ status: 'ok', data: [channel] })
};
export class SyncedState {
	obj: any;
	constructor(key: string, fallback: any) {
		this.obj =
			key === 'auth_state'
				? { phase: 'authorized', token: { user_id: '2', login: 'jerod', profile_image_url: '' } }
				: fallback;
	}
	close() {
		return Promise.resolve();
	}
}
export const page = { url: new URL('http://localhost/app/chat/maya') };
export const goto = () => {};
export const resolve = (value: string) => value;
export default { debug() {}, error() {} };
