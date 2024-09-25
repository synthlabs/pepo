import { StatusCodes } from 'http-status-codes';
import { browser } from '$app/environment';

export class TwitchToken {
	token = $state('');
	client_id = $state('');
	#_key = 'twitch_token';

	useLocalStorage() {
		if (browser) {
			const item = localStorage.getItem(this.#_key);
			if (item) this.parse(item);
		}

		$effect(() => {
			localStorage.setItem(this.#_key, this.serialize());
		});
	}

	parse(item: string) {
		const obj = JSON.parse(item);
		this.token = obj.token;
		this.client_id = obj.client_id;
	}

	serialize(): string {
		return JSON.stringify({ token: this.token, client_id: this.client_id });
	}

	async validate(): Promise<boolean> {
		try {
			const response = await fetch('https://id.twitch.tv/oauth2/validate', {
				method: 'GET',
				headers: { Authorization: `OAuth ${this.token}` }
			});
			return response.status === StatusCodes.OK;
		} catch (e) {
			return false;
		}
	}
}

export const currentToken = new TwitchToken();
