import type {} from 'vitest/jsdom';

const browserWindow = jsdom.window;

for (const storageName of ['localStorage', 'sessionStorage'] as const) {
	Object.defineProperty(globalThis, storageName, {
		configurable: true,
		enumerable: true,
		get: () => browserWindow[storageName]
	});
}
