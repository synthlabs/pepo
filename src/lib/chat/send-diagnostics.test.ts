import { describe, expect, it, vi } from 'vitest';
import type { Emote } from '$lib/bindings';
import { ComposerDiagnostics, messageMetrics, unusualCodepoints } from './send-diagnostics';

const emote = {
	id: '25',
	name: 'Kappa',
	provider: 'Twitch',
	scope: 'Global'
} as Emote;
const picker = { picker_visible: true, result_count: 4, selected_index: 2 };
const logger = () => ({ info: vi.fn(), warn: vi.fn() });

describe('chat send diagnostics', () => {
	it('snapshots completion and the last 16 relevant keys without retaining a submitted draft', () => {
		let time = 0;
		const composer = new ComposerDiagnostics(() => time);
		const log = logger();
		composer.key(new KeyboardEvent('keydown', { key: 'a' }), picker);
		for (let i = 0; i < 20; i++) {
			time = i;
			composer.key(
				new KeyboardEvent('keydown', {
					key: 'Tab',
					shiftKey: true,
					repeat: true,
					isComposing: true
				}),
				picker
			);
		}
		composer.inserted(emote, 'colon', 'keyboard', 'Kappa ');
		time = 30;
		const first = composer.begin('Kappa ', 'maya', '1', log);
		expect(first.diagnostics.key_events).toHaveLength(16);
		expect(first.diagnostics.key_events[0]).toEqual({
			...picker,
			key: 'Tab',
			shift: true,
			repeat: true,
			is_composing: true,
			age_ms: 26
		});
		expect(first.diagnostics.last_completion).toMatchObject({
			mode: 'colon',
			method: 'keyboard',
			emote_name: 'Kappa',
			edited_since_selection: false
		});
		composer.inserted({ ...emote, name: 'Next' }, 'picker', 'mouse', 'Next ');
		composer.inputChanged('Next edited');
		const second = composer.begin('Next edited', 'other', '2', log);
		expect(second.diagnostics.attempt_id).not.toBe(first.diagnostics.attempt_id);
		expect(second.diagnostics.last_completion).toMatchObject({
			method: 'mouse',
			edited_since_selection: true
		});
		expect(second.diagnostics.key_events).toEqual([]);
		expect(first.diagnostics.last_completion?.emote_name).toBe('Kappa');
		const clean = composer.begin('plain', 'maya', '1', log);
		expect(clean.diagnostics.last_completion).toBeNull();
	});

	it('clears abandoned context and remembers edits even when the completed text is restored', () => {
		const composer = new ComposerDiagnostics();
		composer.inserted(emote, 'colon', 'mouse', 'Kappa ');
		composer.inputChanged('Kappa changed');
		composer.inputChanged('Kappa ');
		expect(
			composer.begin('Kappa ', 'maya', '1', logger()).diagnostics.last_completion
				?.edited_since_selection
		).toBe(true);
		composer.inserted(emote, 'colon', 'keyboard', 'Kappa ');
		composer.key(new KeyboardEvent('keydown', { key: 'Enter' }), picker);
		composer.inputChanged('');
		const next = composer.begin('manual', 'maya', '1', logger()).diagnostics;
		expect(next.last_completion).toBeNull();
		expect(next.key_events).toEqual([]);
	});

	it('uses lossless JSON and scalar offsets for whitespace, hidden characters and supplementary Unicode', () => {
		const message = 'Kappa \u200b\n🥳';
		expect(messageMetrics(message)).toEqual({ utf8_bytes: 14, utf16_length: 10, scalar_length: 9 });
		expect(unusualCodepoints(message)).toEqual([
			{ index: 5, codepoint: 'U+0020' },
			{ index: 6, codepoint: 'U+200B' },
			{ index: 7, codepoint: 'U+000A' },
			{ index: 8, codepoint: 'U+1F973' }
		]);
		const log = logger();
		const attempt = new ComposerDiagnostics().begin(message, 'maya', '1', log);
		attempt.failed(
			{ kind: 'rejected', access_token: 'secret', details: 'Authorization: secret' },
			'returned'
		);
		const record = JSON.parse(log.warn.mock.calls[0][1]);
		expect(record.message).toBe(message);
		expect(record.unusual_codepoints).toEqual(unusualCodepoints(message));
		expect(record.attempt_id).toBe(attempt.diagnostics.attempt_id);
		expect(log.warn.mock.calls[0][1]).not.toContain('secret');
	});

	it('logs compact INFO success records and correlated invoke failures without successful text', () => {
		const log = logger();
		const composer = new ComposerDiagnostics();
		composer.inserted(emote, 'colon', 'keyboard', 'private-success');
		const success = composer.begin('private-success', 'maya', '1', log);
		success.sent();
		expect(log.info).toHaveBeenCalledTimes(2);
		expect(JSON.stringify(log.info.mock.calls)).not.toMatch(/private-success|Kappa/);
		const failure = composer.begin('failed text', 'maya', '1', log);
		failure.failed(new Error('Authorization: secret'), 'invoke');
		const record = JSON.parse(log.warn.mock.calls[0][1]);
		expect(record).toMatchObject({
			stage: 'invoke',
			kind: 'unexpected',
			message: 'failed text',
			attempt_id: failure.diagnostics.attempt_id
		});
		expect(log.warn.mock.calls[0][1]).not.toContain('secret');
	});

	it('does not turn a logging failure into a failed send', () => {
		const broken = () => {
			throw new Error('logger unavailable');
		};
		const attempt = new ComposerDiagnostics().begin('draft', 'maya', '1', {
			info: broken,
			warn: broken
		});
		expect(() => attempt.sent()).not.toThrow();
		expect(() => attempt.failed('failure', 'invoke')).not.toThrow();
	});
});
