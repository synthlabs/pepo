import type { BasicParsedMessagePart } from '@twurple/common/lib/emotes/ParsedMessagePart';

export interface Parser {
	ParseMessage(text: string): BasicParsedMessagePart[];
}
