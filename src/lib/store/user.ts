import { createWritableStore } from './writeable';
import type { HelixUser } from '@twurple/api';

export interface User {
	id: string;
	name: string;
	displayName: string;
	description: string;
	color: string;
	profilePictureUrl: string;
	type: string;
	broadcasterType: string;
	creationDate: Date;
}

export function NewUserFromHelix(u: HelixUser | null): User {
	if (!u) return {} as User;
	return {
		id: u.id,
		name: u.name,
		displayName: u.displayName,
		description: u.description,
		color: '',
		profilePictureUrl: u.profilePictureUrl,
		type: u.type,
		broadcasterType: u.broadcasterType,
		creationDate: u.creationDate
	};
}

export function IsAnonUser(u: User): boolean {
	return u.type === "anon"
}

const anonUser: User = {
	id: "0000000",
	name: "anonymous",
	displayName: "Anonymous",
	description: "Not logged in",
	color: "",
	profilePictureUrl: "",
	type: "anon",
	broadcasterType: "none",
	creationDate: new Date()
}

export const user = createWritableStore('user', anonUser);
