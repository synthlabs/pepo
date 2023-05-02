import { createWritableStore } from './writeable';

export interface User {
	id: string;
	name: string;
	color: string;
}

export const user = createWritableStore('user', {} as User);
