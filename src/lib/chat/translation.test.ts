import { describe, expect, it } from 'vitest';
import type {
	ChannelMessage,
	ChannelMessageTranslation,
	ChannelMessageTranslationUpdate
} from '$lib/bindings';
import {
	applyTranslationUpdate,
	attachPendingTranslation,
	HistoryTranslationBuffer,
	isRowOutsideViewport
} from './translation';

describe('chat translation helpers', () => {
	it('applies a translation by message id', () => {
		const pending = new Map<string, ChannelMessageTranslation>();
		const messages = [message('one'), message('two')];

		const result = applyTranslationUpdate(messages, update('two', 'привет'), pending);

		expect(result.changed).toBe(true);
		expect(result.messages[1].translation?.translated_text).toBe('привет');
		expect(result.messages[1].channel_sequence).toBe(messages[1].channel_sequence);
		expect(result.messages[0]).toBe(messages[0]);
		expect(pending.size).toBe(0);
	});

	it('stores an update until its message arrives', () => {
		const pending = new Map<string, ChannelMessageTranslation>();
		const result = applyTranslationUpdate([], update('late', 'hello'), pending);

		expect(result.changed).toBe(false);
		expect(pending.get('late')?.translated_text).toBe('hello');

		const translatedMessage = attachPendingTranslation(message('late'), pending);

		expect(translatedMessage.translation?.translated_text).toBe('hello');
		expect(pending.size).toBe(0);
	});

	it('ignores blank translated text', () => {
		const pending = new Map<string, ChannelMessageTranslation>();
		const messages = [message('one')];

		const result = applyTranslationUpdate(messages, update('one', '   '), pending);

		expect(result.changed).toBe(false);
		expect(result.messages).toBe(messages);
		expect(pending.size).toBe(0);
	});

	it('replaces an older translation for the same message', () => {
		const pending = new Map<string, ChannelMessageTranslation>();
		const messages = [
			message('one', {
				source_language: 'ru',
				target_language: 'en',
				translated_text: 'old'
			})
		];

		const result = applyTranslationUpdate(messages, update('one', 'new'), pending);

		expect(result.changed).toBe(true);
		expect(result.messages[0].translation?.translated_text).toBe('new');
	});
});

describe('historical translation presentation', () => {
	function fixture() {
		const buffer = new HistoryTranslationBuffer();
		const messages = [message('one'), { ...message('two'), index: 2 }];
		buffer.reset([], messages, 2);
		return { buffer, messages };
	}

	it('keeps the initial history boundary across resets and displays cached translations on entry', () => {
		const buffer = new HistoryTranslationBuffer();
		const cached = message('cached', update('cached', 'already translated').translation);
		expect(buffer.reset([], [cached], 2)).toEqual([cached]);
		buffer.reset([cached], [cached, { ...message('live'), index: 3 }], 10);
		expect(buffer.isHistorical(cached)).toBe(true);
		expect(buffer.isHistorical({ index: 3 })).toBe(false);
		expect(buffer.size).toBe(0);
	});

	it('batches eligible rows while retaining visible translations separately from missing messages', () => {
		const { buffer, messages } = fixture();
		buffer.defer(messages[0], update('one', 'first').translation);
		buffer.defer(messages[1], update('two', 'second').translation);
		const result = buffer.flush(messages, (message) => message.message_id === 'one');
		expect(result.changed).toBe(true);
		expect(result.messages[0].translation?.translated_text).toBe('first');
		expect(result.messages[1]).toBe(messages[1]);
		expect(buffer.has('two')).toBe(true);
		expect(buffer.has('one')).toBe(false);
		expect(buffer.flush(result.messages, () => false).changed).toBe(false);
		expect(buffer.flush(result.messages, () => true).messages[1].translation?.translated_text).toBe(
			'second'
		);
		expect(buffer.size).toBe(0);
	});

	it('normalizes results, ignores blanks, and reveals only the latest deferred translation', () => {
		const { buffer, messages } = fixture();
		buffer.defer(messages[0], update('one', 'old').translation);
		buffer.defer(messages[0], update('one', ' latest ').translation);
		buffer.defer(messages[0], update('one', ' ').translation);
		expect(buffer.size).toBe(1);
		expect(buffer.flush(messages, () => true).messages[0].translation?.translated_text).toBe(
			'latest'
		);
	});

	it('drops a deferred replacement when a later update matches the displayed translation', () => {
		const { buffer } = fixture();
		const displayed = message('one', update('one', 'displayed').translation);
		buffer.defer(displayed, update('one', 'replacement').translation);
		buffer.defer(displayed, update('one', 'displayed').translation);
		expect(buffer.size).toBe(0);
	});

	it('preserves displayed originals through resets containing completed historical translations', () => {
		const { buffer, messages } = fixture();
		buffer.defer(messages[0], update('one', 'first result').translation);
		const live = { ...message('live'), index: 3 };
		const incoming = [
			message('one', update('one', 'snapshot result').translation),
			{ ...live, translation: update('live', 'live result').translation }
		];
		const reset = buffer.reset([...messages, live], incoming, 20);
		expect(reset[0].translation).toBeNull();
		expect(reset[1].translation?.translated_text).toBe('live result');
		expect(buffer.flush(reset, () => true).messages[0].translation?.translated_text).toBe(
			'snapshot result'
		);
	});

	it('retains deferred results through untranslated resets and removes evicted entries', () => {
		const { buffer, messages } = fixture();
		buffer.defer(messages[0], update('one', 'first').translation);
		buffer.defer(messages[1], update('two', 'second').translation);
		buffer.reset(messages, [messages[1]], 20);
		expect(buffer.has('one')).toBe(false);
		expect(buffer.has('two')).toBe(true);
		buffer.retain([]);
		expect(buffer.size).toBe(0);
	});

	it('clears all retained work and the history boundary on disposal', () => {
		const { buffer, messages } = fixture();
		buffer.defer(messages[0], update('one', 'deferred').translation);
		buffer.clear();
		expect(buffer.size).toBe(0);
		expect(buffer.isHistorical(messages[0])).toBe(false);
		expect(buffer.flush(messages, () => true).changed).toBe(false);
	});

	it.each([
		[-20, 0, true],
		[100, 120, true],
		[-20, 0.25, false],
		[99.75, 120, false],
		[-20, 120, false],
		[20, 40, false],
		[0, 0, false]
	])('treats a row spanning %s–%s as offscreen=%s', (top, bottom, expected) => {
		expect(isRowOutsideViewport({ top, bottom }, { top: 0, bottom: 100 })).toBe(expected);
	});

	it('waits for measurable geometry before revealing translations', () => {
		expect(isRowOutsideViewport({ top: 100, bottom: 120 }, { top: 0, bottom: 0 })).toBe(false);
	});
});

function message(
	messageId: string,
	translation: ChannelMessageTranslation | null = null
): ChannelMessage {
	return {
		ts: '2026-06-07T00:00:00Z',
		broadcaster_user_id: 'broadcaster-id',
		broadcaster_user_name: 'Broadcaster',
		broadcaster_user_login: 'broadcaster',
		chatter_user_id: 'chatter-id',
		chatter_user_name: 'chatter',
		message_id: messageId,
		text: 'original',
		fragments: [{ Text: { index: 0, text: 'original' } }],
		message_type: 'text',
		badges: [],
		color: '#ffffff',
		translation,
		index: 1,
		channel_sequence: 7
	};
}

function update(messageId: string, translatedText: string): ChannelMessageTranslationUpdate {
	return {
		message_id: messageId,
		translation: {
			source_language: 'ru',
			target_language: 'en',
			translated_text: translatedText
		}
	};
}
