import { describe, expect, it } from 'vitest';
import type { NetworkIssue } from '$lib/bindings';
import { summarizeNetworkIssues } from '$lib/network/summary';

function issue(feature: string, patch: Partial<NetworkIssue> = {}): NetworkIssue {
	return {
		id: feature,
		feature,
		category: 'timeout',
		message: 'Timed out',
		failures: 1,
		affected: 1,
		cached: true,
		last_failure_at: 0,
		retry_at: 15000,
		retryable: true,
		core: false,
		can_retry: true,
		...patch
	};
}

describe('network status summary', () => {
	it('lists each refreshing feature once in alphabetical order', () => {
		const summary = summarizeNetworkIssues([
			issue('BTTV emotes'),
			issue('7TV emotes'),
			issue('7TV emotes', { category: 'connection' })
		]);
		expect(summary.groups).toEqual([
			{
				heading: 'Some features are still refreshing in the background:',
				features: ['7TV emotes', 'BTTV emotes']
			}
		]);
		expect(summary.urgent).toBe(false);
	});
	it('keeps reconnecting and unavailable features out of the refreshing list', () => {
		const summary = summarizeNetworkIssues([
			issue('7TV emotes'),
			issue('Chat', { core: true }),
			issue('7TV emotes', { retryable: false }),
			issue('BTTV emotes')
		]);
		expect(summary.groups).toEqual([
			{ heading: 'Some features need attention:', features: ['7TV emotes'] },
			{ heading: 'Reconnecting:', features: ['Chat'] },
			{
				heading: 'Some features are still refreshing in the background:',
				features: ['BTTV emotes']
			}
		]);
		expect(summary.urgent).toBe(true);
		expect(summary.text).toContain('Reconnecting: Chat');
	});
	it('has no indicator content after recovery', () => {
		expect(summarizeNetworkIssues([])).toEqual({ groups: [], urgent: false, text: '' });
	});
});
