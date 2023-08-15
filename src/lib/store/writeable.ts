import { writable } from 'svelte/store';

// https://stackoverflow.com/a/61300826/2933427
// TODO: clean up the set grossness
export const createWritableStore = <T>(key: string, startValue: T) => {
	const { subscribe, set } = writable(startValue);

	return {
		subscribe,
		set,
		useLocalStorage: (decode = (x: any): T => x, encode = (x: T): any => x) => {
			const json = localStorage.getItem(key);

			if (json && json !== 'undefined') {
				let obj = JSON.parse(json);
				set(decode(obj));
			}

			subscribe((c) => {
				let out = JSON.stringify(encode(c));
				localStorage.setItem(key, out);
			});
		}
	};
};
