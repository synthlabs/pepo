import { describe, expect, it } from 'vitest';
import { getHistoryOriginSpacer, measureHistoryOriginSpacer } from './history-origin';

describe('history origin spacer', () => {
	it('preserves the fractional origin after removing a natural inline row', () => {
		expect(getHistoryOriginSpacer(0, [28, 30.25, 36], 1)).toBeCloseTo(0.2501, 6);
	});

	it.each([
		[1, 1],
		[1.1, 10],
		[1.25, 4],
		[1.5, 2],
		[1.75, 4],
		[2, 1]
	])('preserves whole CSS and device pixels at DPR %s', (ratio, period) => {
		const currentHeight = 0.25;
		const removed = 31.75;
		const next = getHistoryOriginSpacer(currentHeight, [removed], ratio);
		const removedWithSpacer = currentHeight + removed - next;
		expect(next).toBeGreaterThanOrEqual(0);
		expect(next).toBeLessThanOrEqual(period);
		expect(removedWithSpacer / period).toBeCloseTo(Math.round(removedWithSpacer / period), 3);
		expect(removedWithSpacer * ratio).toBeCloseTo(Math.round(removedWithSpacer * ratio), 3);
	});

	it('normalizes floating point error at an alignment boundary', () => {
		expect(getHistoryOriginSpacer(0, [28.000001], 1.25)).toBe(0);
	});

	it('uses zero spacer when no period fits within ten CSS pixels', () => {
		expect(getHistoryOriginSpacer(0.25, [30.25], 1.234567)).toBe(0);
	});

	it('measures only rendered rows being removed, including the actual spacer', () => {
		const spacer = document.createElement('div');
		spacer.getBoundingClientRect = () => ({ height: 0.5 }) as DOMRect;
		const rows = ['older', 'removed', 'retained'].map((key) => {
			const row = document.createElement('div');
			row.dataset.chatMessageIndex = key;
			row.getBoundingClientRect = () => ({ height: 30.25 }) as DOMRect;
			return row;
		});
		expect(
			measureHistoryOriginSpacer(spacer, rows, new Set(['removed', 'not-rendered']), 1)
		).toBeCloseTo(0.7501, 6);
	});

	it('resets the origin when no rendered rows survive cleanup', () => {
		const row = document.createElement('div');
		row.dataset.chatMessageIndex = 'removed';
		expect(measureHistoryOriginSpacer(undefined, [row], new Set(['removed']), 1)).toBe(0);
		expect(measureHistoryOriginSpacer(undefined, [], new Set(['not-rendered']), 1)).toBe(0);
	});
});
