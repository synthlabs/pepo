import { describe, expect, it } from 'vitest';
import type { Settings } from '$lib/bindings';
import { DEFAULT_SETTINGS, gridTemplateColumns, normalizeSettings, px } from './settings';

describe('settings helpers', () => {
	it('dedupes provider preferences and appends missing defaults', () => {
		const settings: Settings = {
			...DEFAULT_SETTINGS,
			emotes: {
				...DEFAULT_SETTINGS.emotes,
				providers: [
					{ id: 'bttv', enabled: false },
					{ id: 'twitch', enabled: true },
					{ id: 'bttv', enabled: true }
				]
			}
		};

		expect(normalizeSettings(settings).emotes.providers).toEqual([
			{ id: 'bttv', enabled: false },
			{ id: 'twitch', enabled: true },
			{ id: 'ffz', enabled: true },
			{ id: 'seventv', enabled: true }
		]);
	});

	it('falls back for non-positive numeric values', () => {
		expect(px(0)).toBe('1px');
		expect(gridTemplateColumns(0)).toBe('repeat(8, minmax(0, 1fr))');
	});
});
