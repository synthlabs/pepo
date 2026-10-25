import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ChatFeedContext, ChatRoomInfo, FeedFrame } from '$lib/bindings';
import { openRoomFeed, type RoomFeedTransport } from './feed';

function fixture() {
	const receivers: ((frame: FeedFrame) => void)[] = [];
	const complete: ((info: ChatRoomInfo) => void)[] = [];
	const transport: RoomFeedTransport = {
		context: vi.fn(async () => ({ epoch: 'page' })),
		subscribe: vi.fn((_channel, _request, receive) => {
			receivers.push(receive);
			return new Promise<ChatRoomInfo>((resolve) => complete.push(resolve));
		}),
		acknowledge: vi.fn(async () => {}),
		unsubscribe: vi.fn(async () => {})
	};
	return { transport, receivers, complete };
}
function frame(cursor: number): FeedFrame {
	return { cursor, event: { type: 'snapshot', messages: [] } };
}
async function open(f: ReturnType<typeof fixture>, channel = 'a', receive = vi.fn()) {
	const room = openRoomFeed(channel, receive, f.transport);
	await Promise.resolve();
	return room;
}
const requestMatcher = { epoch: 'page', revision: expect.any(Number) };

afterEach(() => {
	vi.useRealTimers();
});
describe('room feed lifetime and backpressure', () => {
	it('installs its receiver before readiness and acknowledges cumulatively', async () => {
		vi.useFakeTimers();
		const f = fixture();
		const receive = vi.fn();
		const room = await open(f, 'a', receive);
		f.receivers[0](frame(1));
		f.receivers[0](frame(2));
		expect(receive).toHaveBeenCalledTimes(2);
		expect(f.transport.acknowledge).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(16);
		expect(f.transport.acknowledge).toHaveBeenCalledExactlyOnceWith(requestMatcher, 2);
		room.dispose();
	});
	it('acknowledges busy streams before their window fills', async () => {
		const f = fixture();
		const room = await open(f);
		for (let cursor = 1; cursor <= 32; cursor++) f.receivers[0](frame(cursor));
		expect(f.transport.acknowledge).toHaveBeenCalledWith(requestMatcher, 32);
		room.dispose();
	});
	it('uses increasing revisions and ignores old delivery during A to B to A navigation', async () => {
		const f = fixture();
		const receives = [vi.fn(), vi.fn(), vi.fn()];
		const a = await open(f, 'a', receives[0]);
		a.dispose();
		const b = await open(f, 'b', receives[1]);
		b.dispose();
		const nextA = await open(f, 'a', receives[2]);
		f.receivers.forEach((receive, index) => receive(frame(index + 1)));
		expect(receives[0]).not.toHaveBeenCalled();
		expect(receives[1]).not.toHaveBeenCalled();
		expect(receives[2]).toHaveBeenCalledOnce();
		const requests = vi.mocked(f.transport.subscribe).mock.calls.map((call) => call[1]);
		expect(requests[0].revision).toBeLessThan(requests[1].revision);
		expect(requests[1].revision).toBeLessThan(requests[2].revision);
		expect(f.transport.unsubscribe).toHaveBeenCalledTimes(2);
		nextA.dispose();
	});
	it('cancels acknowledgement timers and disposes only once', async () => {
		vi.useFakeTimers();
		const f = fixture();
		const room = await open(f);
		f.receivers[0](frame(1));
		room.dispose();
		room.dispose();
		await vi.advanceTimersByTimeAsync(100);
		expect(f.transport.acknowledge).not.toHaveBeenCalled();
		expect(f.transport.unsubscribe).toHaveBeenCalledOnce();
	});
	it('cleans up its subscription on renderer reload', async () => {
		const f = fixture();
		const room = await open(f);
		window.dispatchEvent(new Event('beforeunload'));
		expect(f.transport.unsubscribe).toHaveBeenCalledOnce();
		room.dispose();
	});
	it('ignores stale frames without acknowledging them', async () => {
		vi.useFakeTimers();
		const f = fixture();
		const receive = vi.fn();
		const room = await open(f, 'a', receive);
		f.receivers[0](frame(2));
		f.receivers[0](frame(1));
		await vi.advanceTimersByTimeAsync(16);
		expect(receive).toHaveBeenCalledOnce();
		expect(f.transport.acknowledge).toHaveBeenCalledWith(requestMatcher, 2);
		room.dispose();
	});
	it('cancels before context arrives without starting a subscription', async () => {
		const f = fixture();
		let resolve!: (context: ChatFeedContext) => void;
		vi.mocked(f.transport.context).mockImplementation(
			() =>
				new Promise((done) => {
					resolve = done;
				})
		);
		const room = openRoomFeed('a', vi.fn(), f.transport);
		room.dispose();
		resolve({ epoch: 'page' });
		await expect(room.ready).rejects.toThrow('subscription canceled');
		expect(f.transport.subscribe).not.toHaveBeenCalled();
		expect(f.transport.unsubscribe).toHaveBeenCalledExactlyOnceWith(requestMatcher);
	});
	it('keeps the original revision when context responses arrive in reverse order', async () => {
		const f = fixture();
		const contexts: ((context: ChatFeedContext) => void)[] = [];
		vi.mocked(f.transport.context).mockImplementation(
			() => new Promise((resolve) => contexts.push(resolve))
		);
		const first = openRoomFeed('a', vi.fn(), f.transport);
		const second = openRoomFeed('b', vi.fn(), f.transport);
		contexts[1]({ epoch: 'page' });
		await Promise.resolve();
		contexts[0]({ epoch: 'page' });
		await Promise.resolve();
		const calls = vi.mocked(f.transport.subscribe).mock.calls;
		expect(calls[0][0]).toBe('b');
		expect(calls[1][0]).toBe('a');
		expect(calls[0][1].revision).toBeGreaterThan(calls[1][1].revision);
		first.dispose();
		second.dispose();
	});
	it('returns subscription failures through readiness without delivering an error frame', async () => {
		const f = fixture();
		const receive = vi.fn();
		vi.mocked(f.transport.subscribe).mockRejectedValue(new Error('channel not found'));
		const room = openRoomFeed('unknown', receive, f.transport);
		await expect(room.ready).rejects.toThrow('channel not found');
		expect(receive).not.toHaveBeenCalled();
		room.dispose();
	});
});
