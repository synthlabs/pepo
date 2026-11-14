import type {
	ChannelMessage,
	ChatTimelineEntry,
	ChannelMessageTranslation,
	ChannelMessageTranslationUpdate
} from '$lib/bindings';

export type PendingTranslations = Map<string, ChannelMessageTranslation>;

type TranslationEntry = ChannelMessage | ChatTimelineEntry;

export function isChatMessage(message: TranslationEntry): message is ChannelMessage {
	return !('kind' in message) || message.kind === 'chat';
}

export interface TranslationApplyResult<T = ChannelMessage> {
	messages: T[];
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

	reset<T extends TranslationEntry>(current: T[], incoming: T[], cursor: number): T[] {
		this.cursor ??= cursor;
		const previous = new Map(
			current.filter(isChatMessage).map((message) => [message.message_id, message])
		);
		const messages = incoming.map((message) => {
			if (!isChatMessage(message)) return message;
			const displayed = previous.get(message.message_id);
			if (!displayed || !isChatMessage(displayed) || !this.isHistorical(message)) return message;

			if (message.translation) this.defer(displayed, message.translation);
			return { ...message, translation: displayed.translation };
		});
		this.retain(messages);
		return messages;
	}

	flush<T extends TranslationEntry>(
		messages: T[],
		canPresent: (message: ChannelMessage) => boolean
	): TranslationApplyResult<T> {
		let next = messages;
		for (const [index, message] of messages.entries()) {
			if (!isChatMessage(message)) continue;
			const translation = this.pending.get(message.message_id);
			if (!translation || !canPresent(message)) continue;
			this.pending.delete(message.message_id);
			if (sameTranslation(message.translation, translation)) continue;
			if (next === messages) next = messages.slice();
			next[index] = { ...message, translation };
		}
		return { messages: next, changed: next !== messages };
	}

	retain(messages: TranslationEntry[]): void {
		const retained = new Set(messages.filter(isChatMessage).map((message) => message.message_id));
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

export function attachPendingTranslation<T extends TranslationEntry>(
	message: T,
	pendingTranslations: PendingTranslations
): T {
	if (!isChatMessage(message)) return message;
	const pendingTranslation = pendingTranslations.get(message.message_id);
	if (pendingTranslation) {
		pendingTranslations.delete(message.message_id);
		return { ...message, translation: pendingTranslation };
	}

	const translation = normalizeTranslation(message.translation);
	return translation ? { ...message, translation } : { ...message, translation: null };
}

export function applyTranslationUpdate<T extends TranslationEntry>(
	messages: T[],
	update: ChannelMessageTranslationUpdate,
	pendingTranslations: PendingTranslations
): TranslationApplyResult<T> {
	const translation = normalizeTranslation(update.translation);
	if (!translation) return { messages, changed: false };

	const index = messages.findIndex(
		(message) => isChatMessage(message) && message.message_id === update.message_id
	);
	if (index === -1) {
		pendingTranslations.set(update.message_id, translation);
		return { messages, changed: false };
	}

	const current = messages[index];
	if (!isChatMessage(current)) return { messages, changed: false };
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
