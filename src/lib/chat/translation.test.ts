import { describe, expect, it } from 'vitest';
import type {
	ChannelMessage,
	ChannelMessageTranslation,
	ChannelMessageTranslationUpdate
} from '$lib/bindings';
import { applyTranslationUpdate, attachPendingTranslation } from './translation';

describe('chat translation helpers', () => {
	it('applies a translation by message id', () => {
		const pending = new Map<string, ChannelMessageTranslation>();
		const messages = [message('one'), message('two')];

		const result = applyTranslationUpdate(messages, update('two', 'привет'), pending);

		expect(result.changed).toBe(true);
		expect(result.messages[1].translation?.translated_text).toBe('привет');
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
		index: 1
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
