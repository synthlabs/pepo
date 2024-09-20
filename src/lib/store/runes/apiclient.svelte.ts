import Logger from '$lib/logger/log';
import { ApiClient } from '@twurple/api';
import { TwitchToken } from './token.svelte';
import { StaticAuthProvider } from '@twurple/auth';

export class TwitchApiClient {
	#token: TwitchToken = $state() as TwitchToken;
	#client!: ApiClient;

	constructor() {}

	set token(t: TwitchToken) {
		Logger.debug('token updated', t);
		this.#token = t;

		Logger.debug('updating api client with new auth');
		const authProvider = new StaticAuthProvider(t.client_id, t.token);
		this.#client = new ApiClient({ authProvider });
	}

	get api() {
		return this.#client;
	}

	get token() {
		return this.#token;
	}
}

export const client = new TwitchApiClient();
