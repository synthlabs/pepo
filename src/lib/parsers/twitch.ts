import type { BasicParsedMessagePart } from '@twurple/common/lib/emotes/ParsedMessagePart';

export class Parser {
	static ParseMessage(text: string): BasicParsedMessagePart[] {
		return [
			{
				type: 'text',
				position: 0,
				length: text.length,
				text: text
			}
		];
	}
}
