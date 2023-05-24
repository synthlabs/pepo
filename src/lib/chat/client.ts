import Logger from '$lib/logger/log';
import { ChatClient as TwurpleChatClient, type ChatClientOptions } from '@twurple/chat';

export class ChatClient extends TwurpleChatClient {
	constructor(config?: ChatClientOptions) {
		Logger.debug('custom chat client');
		super(config);
	}
}
