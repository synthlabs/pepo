import type {
	ChannelMessage,
	ChannelMessageTranslation,
	ChannelMessageTranslationUpdate
} from '$lib/bindings';

export type PendingTranslations = Map<string, ChannelMessageTranslation>;

export interface TranslationApplyResult {
	messages: ChannelMessage[];
	changed: boolean;
}

export function attachPendingTranslation(
	message: ChannelMessage,
	pendingTranslations: PendingTranslations
): ChannelMessage {
	const pendingTranslation = pendingTranslations.get(message.message_id);
	if (pendingTranslation) {
		pendingTranslations.delete(message.message_id);
		return { ...message, translation: pendingTranslation };
	}

	const translation = normalizeTranslation(message.translation);
	return translation ? { ...message, translation } : { ...message, translation: null };
}

export function applyTranslationUpdate(
	messages: ChannelMessage[],
	update: ChannelMessageTranslationUpdate,
	pendingTranslations: PendingTranslations
): TranslationApplyResult {
	const translation = normalizeTranslation(update.translation);
	if (!translation) return { messages, changed: false };

	const index = messages.findIndex((message) => message.message_id === update.message_id);
	if (index === -1) {
		pendingTranslations.set(update.message_id, translation);
		return { messages, changed: false };
	}

	const current = messages[index];
	if (sameTranslation(current.translation, translation)) {
		return { messages, changed: false };
	}

	const nextMessages = messages.slice();
	nextMessages[index] = { ...current, translation };

	return { messages: nextMessages, changed: true };
}

function normalizeTranslation(
	translation: ChannelMessageTranslation | null | undefined
): ChannelMessageTranslation | null {
	const translatedText = translation?.translated_text.trim();
	if (!translation || !translatedText) return null;

	return {
		...translation,
		source_language: translation.source_language.trim(),
		target_language: translation.target_language.trim(),
		translated_text: translatedText
	};
}

function sameTranslation(
	left: ChannelMessageTranslation | null | undefined,
	right: ChannelMessageTranslation
): boolean {
	return (
		!!left &&
		left.source_language === right.source_language &&
		left.target_language === right.target_language &&
		left.translated_text === right.translated_text
	);
}
