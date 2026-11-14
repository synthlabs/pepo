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

export class HistoryTranslationBuffer {
	private cursor: number | undefined;
	private pending: PendingTranslations = new Map();

	get size(): number {
		return this.pending.size;
	}

	isHistorical(message: Pick<ChannelMessage, 'index'>): boolean {
		return this.cursor !== undefined && message.index <= this.cursor;
	}

	has(messageId: string): boolean {
		return this.pending.has(messageId);
	}

	defer(message: ChannelMessage, value: ChannelMessageTranslation): void {
		const translation = normalizeTranslation(value);
		if (!translation) return;
		if (sameTranslation(message.translation, translation)) this.pending.delete(message.message_id);
		else this.pending.set(message.message_id, translation);
	}

	reset(current: ChannelMessage[], incoming: ChannelMessage[], cursor: number): ChannelMessage[] {
		this.cursor ??= cursor;
		const previous = new Map(current.map((message) => [message.message_id, message]));
		const messages = incoming.map((message) => {
			const displayed = previous.get(message.message_id);
			if (!displayed || !this.isHistorical(message)) return message;

			if (message.translation) this.defer(displayed, message.translation);
			return { ...message, translation: displayed.translation };
		});
		this.retain(messages);
		return messages;
	}

	flush(
		messages: ChannelMessage[],
		canPresent: (message: ChannelMessage) => boolean
	): TranslationApplyResult {
		let next = messages;
		for (const [index, message] of messages.entries()) {
			const translation = this.pending.get(message.message_id);
			if (!translation || !canPresent(message)) continue;
			this.pending.delete(message.message_id);
			if (sameTranslation(message.translation, translation)) continue;
			if (next === messages) next = messages.slice();
			next[index] = { ...message, translation };
		}
		return { messages: next, changed: next !== messages };
	}

	retain(messages: ChannelMessage[]): void {
		const retained = new Set(messages.map((message) => message.message_id));
		for (const id of this.pending.keys()) {
			if (!retained.has(id)) this.pending.delete(id);
		}
	}

	clear(): void {
		this.pending.clear();
		this.cursor = undefined;
	}
}

export function isRowOutsideViewport(
	row: Pick<DOMRect, 'top' | 'bottom'>,
	viewport: Pick<DOMRect, 'top' | 'bottom'>
): boolean {
	return (
		row.bottom > row.top &&
		viewport.bottom > viewport.top &&
		(row.bottom <= viewport.top || row.top >= viewport.bottom)
	);
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
