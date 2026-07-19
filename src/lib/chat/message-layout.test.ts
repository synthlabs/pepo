import { describe, expect, it } from 'vitest';

import {
	chatBadgePlaceholderWidth,
	translationHasBadgePlaceholder,
	translationHasTimestampPlaceholder
} from './message-layout';

describe('chat message layout helpers', () => {
	it('returns zero width when there are no badges', () => {
		expect(chatBadgePlaceholderWidth(0, 20)).toBe('0px');
		expect(chatBadgePlaceholderWidth(-1, 20)).toBe('0px');
	});

	it('sizes a single badge without a gap', () => {
		expect(chatBadgePlaceholderWidth(1, 20)).toBe('20px');
	});

	it('includes the badge gap between multiple badges', () => {
		expect(chatBadgePlaceholderWidth(3, 20)).toBe('68px');
	});

	it('uses the configured badge size', () => {
		expect(chatBadgePlaceholderWidth(2, 24)).toBe('52px');
	});

	it('maps translation layouts to their placeholder behavior', () => {
		expect(translationHasTimestampPlaceholder('language_tag')).toBe(true);
		expect(translationHasBadgePlaceholder('language_tag')).toBe(true);
		expect(translationHasTimestampPlaceholder('message_text')).toBe(true);
		expect(translationHasBadgePlaceholder('message_text')).toBe(true);
		expect(translationHasTimestampPlaceholder('timestamp_end')).toBe(true);
		expect(translationHasBadgePlaceholder('timestamp_end')).toBe(false);
		expect(translationHasTimestampPlaceholder('connector')).toBe(false);
		expect(translationHasBadgePlaceholder('connector')).toBe(false);
	});
});
