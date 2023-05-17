import Logger from '$lib/logger/log';
import * as types from '$lib/config/constants';

import { parseChatMessage } from '@twurple/common';
import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';
import type { BasicParsedMessagePart } from '@twurple/common/lib/emotes/ParsedMessagePart';
import { diff } from 'deep-object-diff';
import { Parser as TwitchParser } from '$lib/parsers/twitch';

export interface Message {
	id: string;
	ts: string;
	username: string;
	messageParts: BasicParsedMessagePart[];
	color: string;
	raw?: TwitchPrivateMessage;
}

const isObjectEmpty = (objectName: object) => {
	return Object.keys(objectName).length === 0;
};

export function ParseTwitchMsg(msg: string, raw: TwitchPrivateMessage): Message {
	const twurpleParse = parseChatMessage(msg, raw.emoteOffsets);
	const customParse = TwitchParser.ParseMessage(msg);

	const d = diff(customParse, twurpleParse);
	if (!isObjectEmpty(d)) Logger.debug(d);

	return {
		id: raw.id,
		ts: raw.date.toLocaleTimeString('en', { timeStyle: 'short' }),
		username: raw.userInfo.displayName,
		messageParts: twurpleParse,
		color: raw.userInfo.color ?? types.GREY_NAME_COLOR,
		raw: raw
	};
}
