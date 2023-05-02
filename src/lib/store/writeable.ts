import { writable } from 'svelte/store';

// https://stackoverflow.com/a/61300826/2933427
export const createWritableStore = <T>(key: string, startValue: T) => {
	const { subscribe, set } = writable(startValue);

	return {
		subscribe,
		set,
		useLocalStorage: () => {
			const json = localStorage.getItem(key);
			let obj: T;
			if (json && json !== 'undefined') {
				obj = JSON.parse(json);
			} else {
				obj = undefined as T;
			}
			set(obj);

			subscribe((current) => {
				localStorage.setItem(key, JSON.stringify(current));
			});
		}
	};
};
