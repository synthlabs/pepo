import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type { ObservedUser, UserDirectoryUpdate } from '$lib/bindings';

export class UserDirectory {
	revision = $state(0);
	private users = new SvelteMap<string, ObservedUser>();
	private logins = new SvelteMap<string, string>();
	private epoch: string | undefined;

	begin(epoch: string) {
		if (this.epoch === epoch) return;
		this.clear();
		this.epoch = epoch;
	}

	clear() {
		this.revision += 1;
		this.users.clear();
		this.logins.clear();
		this.epoch = undefined;
	}

	get(userId: string | null | undefined, login: string): ObservedUser | undefined {
		const id = userId || this.logins.get(login.toLowerCase());
		return id ? this.users.get(id) : undefined;
	}

	apply(update: UserDirectoryUpdate) {
		this.revision += 1;
		if (update.type === 'snapshot') {
			const incoming = new SvelteSet(update.users.map((user) => user.user_id));
			for (const id of this.users.keys()) if (!incoming.has(id)) this.remove(id);
		} else {
			for (const id of update.removed) this.remove(id);
		}
		for (const user of update.type === 'snapshot' ? update.users : update.upserts) {
			const previous = this.users.get(user.user_id);
			const login = user.login.toLowerCase();
			if (previous && previous.login !== login && this.logins.get(previous.login) === user.user_id)
				this.logins.delete(previous.login);
			this.logins.set(login, user.user_id);
			if (
				!previous ||
				previous.login !== login ||
				previous.display_name !== user.display_name ||
				previous.color !== user.color
			)
				this.users.set(user.user_id, { ...user, login });
		}
	}

	private remove(id: string) {
		const previous = this.users.get(id);
		if (previous && this.logins.get(previous.login) === id) this.logins.delete(previous.login);
		this.users.delete(id);
	}
}

export const userDirectory = new UserDirectory();
