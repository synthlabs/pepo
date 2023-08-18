import Logger from '$lib/logger/log';
import { Sanitize } from '$lib/store/channels';
import type { TwitchToken } from '$lib/store/token';
import { StaticAuthProvider } from '@twurple/auth';
import { ChatClient, type ChatSayMessageAttributes } from '@twurple/chat';
import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';

export type MessageHandlerFn = (text: string, msg: TwitchPrivateMessage) => void;

export class Client {
	private _token?: TwitchToken;
	private _twurpleClient: ChatClient;
	private _handlerFns: Map<string, MessageHandlerFn>;
	private _joinedChans: Set<string>;
	private _subbedChans: Set<string>;

	constructor() {
		this._twurpleClient = new ChatClient();
		this._handlerFns = new Map<string, MessageHandlerFn>();
		this._joinedChans = new Set<string>();
		this._subbedChans = new Set<string>();

		this.listeners();
	}

	private listeners = () => {
		this._twurpleClient.connect().then(() => {
			Logger.info('connected to chat');
		});

		this._twurpleClient.onMessage(this.handler);

		this._twurpleClient.onJoin((channel, user) => {
			Logger.debug(`joined ${channel} as ${user}`);
		});
		this._twurpleClient.onPart((channel, user) => {
			Logger.debug(`parted ${channel} as ${user}`);
		});
	};

	set token(token: TwitchToken) {
		Logger.debug('token updated');

		this._token = token;
		const authProvider = new StaticAuthProvider(this._token.client_id, this._token.oauth_token);
		this._twurpleClient = new ChatClient({ authProvider });

		this.listeners();
	}

	private handler = (channel: string, user: string, text: string, msg: TwitchPrivateMessage) => {
		const chan = Sanitize(channel);

		if (!this._subbedChans.has(chan)) {
			Logger.debug(`[UNSUBBED] skipping msg for ${chan}`);
			return;
		}

		if (this._handlerFns.has(chan)) {
			let fn = this._handlerFns.get(chan);
			if (fn) {
				fn(text, msg);
			}
		} else {
			Logger.warn(`got a msg for ${chan} but we have no handler for it`);
		}
	};

	private join = (chan: string) => {
		if (this._joinedChans.has(chan)) return;

		this._twurpleClient.join(chan);
		this._joinedChans.add(chan);
	};

	public sub = (channel: string, fn: MessageHandlerFn) => {
		const chan = Sanitize(channel);

		this.join(chan);
		this._subbedChans.add(chan);
		this._handlerFns.set(chan, fn);
	};

	public unsub = (channel: string) => {
		const chan = Sanitize(channel);
		Logger.debug(`unsub ${chan}`);
		this._subbedChans.delete(chan);
	};

	public say = (
		channel: string,
		text: string,
		attributes?: ChatSayMessageAttributes
	): Promise<void> => {
		return this._twurpleClient.say(channel, text, attributes);
	};
}
