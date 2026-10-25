import { describe, expect, it } from 'vitest';
import { showDividerAfter } from './history-boundary';
const rows = [1, 2, 3, 4].map((index) => ({ index }));
describe('opening history boundary', () => {
	it('appears immediately after an opening snapshot', () => {
		expect(showDividerAfter(rows.slice(0, 2), 1, { cursor: 2, hasHistory: true }, true)).toBe(true);
	});
	it('remains between snapshot messages and live arrivals', () => {
		expect(
			rows.map((_, i) => showDividerAfter(rows, i, { cursor: 2, hasHistory: true }, true))
		).toEqual([false, true, false, false]);
	});
	it('omits the divider for empty initial history', () => {
		expect(showDividerAfter(rows, 0, { cursor: 0, hasHistory: false }, true)).toBe(false);
	});
	it('preserves the boundary through toggles and removes it when history is trimmed', () => {
		const boundary = { cursor: 2, hasHistory: true };
		expect(showDividerAfter(rows, 1, boundary, false)).toBe(false);
		expect(showDividerAfter(rows, 1, boundary, true)).toBe(true);
		expect(showDividerAfter(rows.slice(2), 0, boundary, true)).toBe(false);
	});
	it('preserves the original boundary when recovering through a replacement snapshot', () => {
		const replacement = [2, 3, 4, 5].map((index) => ({ index }));
		expect(showDividerAfter(replacement, 0, { cursor: 2, hasHistory: true }, true)).toBe(true);
	});
	it('establishes a fresh boundary on reopening without changing message order', () => {
		expect(showDividerAfter(rows, 3, { cursor: 4, hasHistory: true }, true)).toBe(true);
	});
});
