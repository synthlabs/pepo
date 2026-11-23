import type {} from 'vitest/jsdom';

const browserWindow = typeof jsdom !== 'undefined' ? jsdom.window : globalThis.window;

if (browserWindow) {
	for (const storageName of ['localStorage', 'sessionStorage'] as const) {
		Object.defineProperty(globalThis, storageName, {
			configurable: true,
			enumerable: true,
			get: () => browserWindow[storageName]
		});
	}
}
