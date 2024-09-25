import { browser } from '$app/environment';
import type { HelixUser } from '@twurple/api';

export class User {
	id = $state('0000000');
	name = $state('anonymous');
	displayName = $state('Anonymous');
	description = $state('Not logged in');
	color = $state('');
	profilePictureUrl = $state('');
	type = $state('anon');
	broadcasterType = $state('none');
	#_key = 'user';

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
		this.id = obj.id;
		this.name = obj.name;
		this.displayName = obj.displayName;
		this.description = obj.description;
		this.color = obj.color;
		this.profilePictureUrl = obj.profilePictureUrl;
		this.type = obj.type;
		this.broadcasterType = obj.broadcasterType;
	}

	serialize(): string {
		return JSON.stringify({
			id: this.id,
			name: this.name,
			displayName: this.displayName,
			description: this.description,
			color: this.color,
			profilePictureUrl: this.profilePictureUrl,
			type: this.type,
			broadcasterType: this.broadcasterType
		});
	}

	fromHelix(u: HelixUser | null) {
		if (!u) return;

		this.id = u.id;
		this.name = u.name;
		this.displayName = u.displayName;
		this.description = u.description;
		this.profilePictureUrl = u.profilePictureUrl;
		this.type = u.type;
		this.broadcasterType = u.broadcasterType;
	}

	get isAnon() {
		return this.type === 'anon';
	}
}

export const currentUser = new User();
