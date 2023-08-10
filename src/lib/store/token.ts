/* eslint-disable @typescript-eslint/no-explicit-any */
import { StatusCodes } from 'http-status-codes';

import { createWritableStore } from './writeable';

export class TwitchToken {
	username = '';
	user_id = '';
	client_id = '';
	oauth_token = '';
	isValid = false;
	raw = '';

	constructor(...params: any[]) {
		this.reset();

		if (params.length === 1) {
			let obj: any;
			if (typeof params[0] === 'string') {
				obj = Object.fromEntries(
					params[0]
						.replace(/;\s*$/, '')
						.split(';')
						.map((pair) => {
							const [k, v] = pair.split('=');
							if (!v) {
								return ['_unknown', pair];
							}
							return [k, v];
						})
				);
			} else if (typeof params[0] === 'object') {
				obj = params[0];
				params[0] = JSON.stringify(params[0]);
			}

			const { username, user_id, client_id, oauth_token } = obj;
			this.username = username ?? '';
			this.user_id = user_id ?? '';
			this.client_id = client_id ?? '';
			this.oauth_token = oauth_token ?? '';
			this.raw = params[0];
		}
	}

	reset() {
		this.username = '';
		this.user_id = '';
		this.client_id = '';
		this.oauth_token = '';
		this.raw = '';
	}
}

export const token = createWritableStore('token', new TwitchToken());

export async function validate(token: TwitchToken) {
	try {
		const response = await fetch('https://id.twitch.tv/oauth2/validate', {
			method: 'GET',
			headers: { Authorization: `OAuth ${token.oauth_token}` }
		});
		token.isValid = response.status === StatusCodes.OK;
	} catch (e) {
		token.isValid = false;
	}
	return token;
}

export function isValid(token: TwitchToken): boolean {
	if (!token) return false;
	const hasAllParts =
		!!token.username && !!token.user_id && !!token.client_id && !!token.oauth_token;
	return hasAllParts && token.isValid;
}
