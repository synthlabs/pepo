import { describe, expect, it } from 'vitest';
import { NetworkHealth } from '$lib/network/health';
import { DEFAULT_APP_SETTINGS, normalizeAppSettings } from '$lib/settings';
import { applySettingsPatch } from '$lib/settings/autosave';
import type { NetworkIssue } from '$lib/bindings';

function issue(): NetworkIssue {
	return {
		id: 'ffz:timeout',
		feature: 'FFZ emotes',
		category: 'timeout',
		message: 'Timed out',
		failures: 17,
		affected: 17,
		cached: true,
		last_failure_at: 1000,
		retry_at: 16000,
		retryable: true,
		core: false,
		can_retry: true
	};
}
describe('network health', () => {
	it('keeps the startup burst grouped and ignores a late initial snapshot', () => {
		const health = new NetworkHealth();
		health.receive({ revision: 17, issues: [issue()] });
		health.receive({ revision: 0, issues: [] });
		expect(health.issues()).toEqual([issue()]);
		health.receive({ revision: 18, issues: [] });
		expect(health.issues()).toEqual([]);
	});
	it('does not lose update-check failures when native metadata recovers', () => {
		const health = new NetworkHealth();
		health.setExternal('external:updates', 'Update checks', 'Unavailable');
		health.receive({ revision: 1, issues: [issue()] });
		health.receive({ revision: 2, issues: [] });
		expect(health.issues().map((row) => row.id)).toEqual(['external:updates']);
		health.setExternal('external:updates', 'Update checks', null);
		expect(health.issues()).toEqual([]);
	});
	it('preserves saved provider timeouts when changing recovery settings', () => {
		const saved = structuredClone(DEFAULT_APP_SETTINGS);
		saved.providers.http_connect_timeout_secs = 5;
		saved.providers.http_request_timeout_secs = 15;
		const updated = normalizeAppSettings(
			applySettingsPatch(saved, { network: { metadata_concurrency: 2 } })
		);
		expect(updated.providers.http_connect_timeout_secs).toBe(5);
		expect(updated.providers.http_request_timeout_secs).toBe(15);
		expect(updated.network.metadata_concurrency).toBe(2);
		expect(updated.network.request_timeout_secs).toBe(30);
	});
});
