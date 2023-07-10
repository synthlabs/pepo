import { writable } from 'svelte/store';

function getRealObjectType(obj: unknown): string {
	// @ts-ignore: Object is possibly 'null'.
	return Object.prototype.toString
		.call(obj)
		.match(/\[\w+ (\w+)\]/)[1]
		.toLowerCase();
}

// https://stackoverflow.com/a/61300826/2933427
// TODO: clean up the set grossness
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
				if (getRealObjectType(startValue) === 'set') {
					set(new Set(obj));
				} else {
					set(obj);
				}
			}

			subscribe((c) => {
				let out = '';
				if (getRealObjectType(c) === 'set') {
					out = JSON.stringify(Array.from(c));
				} else {
					out = JSON.stringify(c);
				}
				localStorage.setItem(key, out);
			});
		}
	};
};
