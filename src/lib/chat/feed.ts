import { Channel } from '@tauri-apps/api/core';
import {
	commands,
	type ChatFeedContext,
	type ChatFeedRequest,
	type ChatRoomInfo,
	type FeedFrame
} from '$lib/bindings';

export interface RoomFeedTransport {
	context(): Promise<ChatFeedContext>;
	subscribe(
		channel: string,
		request: ChatFeedRequest,
		receive: (frame: FeedFrame) => void
	): Promise<ChatRoomInfo>;
	acknowledge(request: ChatFeedRequest, cursor: number): Promise<unknown>;
	unsubscribe(request: ChatFeedRequest): Promise<unknown>;
}

const nativeTransport: RoomFeedTransport = {
	async context() {
		const result = await commands.getChatFeedContext();
		if (result.status === 'error') throw new Error(result.error);
		return result.data;
	},
	async subscribe(channel, request, receive) {
		const result = await commands.subscribeChat(channel, request, new Channel<FeedFrame>(receive));
		if (result.status === 'error') throw new Error(result.error);
		return result.data;
	},
	acknowledge: (request, cursor) => commands.acknowledgeChat(request, cursor),
	unsubscribe: (request) => commands.unsubscribeChat(request)
};

let nextRevision = 0;

export function openRoomFeed(
	channel: string,
	receive: (frame: FeedFrame) => void,
	transport: RoomFeedTransport = nativeTransport
) {
	const revision = ++nextRevision;
	let request: ChatFeedRequest | undefined;
	let closed = false;
	let cursor = -1;
	let pendingCount = 0;
	let timer: ReturnType<typeof setTimeout> | undefined;
	const unsubscribe = () => {
		if (request) void transport.unsubscribe(request).catch(() => {});
	};
	const acknowledge = () => {
		if (timer !== undefined) clearTimeout(timer);
		timer = undefined;
		if (closed || !request || cursor < 0 || pendingCount === 0) return;
		pendingCount = 0;
		void transport.acknowledge(request, cursor).catch(() => {});
	};
	const ready = transport.context().then((context) => {
		request = { epoch: context.epoch, revision };
		if (closed) {
			unsubscribe();
			throw new Error('subscription canceled');
		}
		return transport.subscribe(channel, request, (frame) => {
			if (closed || frame.cursor < cursor) return;
			receive(frame);
			cursor = frame.cursor;
			pendingCount += 1;
			if (pendingCount >= 32) acknowledge();
			else if (timer === undefined) timer = setTimeout(acknowledge, 16);
		});
	});
	void ready.catch(() => {});
	function dispose() {
		if (closed) return;
		closed = true;
		if (timer !== undefined) clearTimeout(timer);
		window.removeEventListener('beforeunload', dispose);
		unsubscribe();
	}
	window.addEventListener('beforeunload', dispose);
	return { ready, dispose };
}
