import { describe, expect, it, vi } from 'vitest';
import type { UserCardState } from '$lib/bindings';
import { UserCardSession } from '$lib/chat/user-card-session';

function state(revision: number): UserCardState {
	return {
		user: { user_id: '1', login: 'maya', display_name: 'Maya', color: null },
		profile_image_url: null,
		channel_id: 'room',
		channel_login: 'room',
		badges: null,
		profile_status: 'loading',
		error: null,
		pinned: false,
		chrome: 'compact',
		revision
	};
}

describe('user card session', () => {
	it('subscribes before reading and rejects an older snapshot after a newer event', async () => {
		let receive!: (state: UserCardState) => void;
		const publish = vi.fn();
		const ready = vi.fn(async () => {});
		const unlisten = vi.fn();
		const session = new UserCardSession(
			{
				listen: async (listener) => {
					receive = listener;
					return unlisten;
				},
				snapshot: async () => {
					receive(state(2));
					return state(1);
				},
				ready
			},
			publish
		);
		await session.start();
		expect(publish).toHaveBeenCalledExactlyOnceWith(state(2));
		expect(ready).toHaveBeenCalledOnce();
		session.stop();
		receive(state(3));
		expect(publish).toHaveBeenCalledTimes(1);
		expect(unlisten).toHaveBeenCalledOnce();
	});

	it('cleans up a listener that resolves after the window unmounts', async () => {
		let finish!: (unlisten: () => void) => void;
		const unlisten = vi.fn();
		const snapshot = vi.fn(async () => state(1));
		const ready = vi.fn(async () => {});
		const session = new UserCardSession(
			{
				listen: () =>
					new Promise((resolve) => {
						finish = resolve;
					}),
				snapshot,
				ready
			},
			vi.fn()
		);
		const pending = session.start();
		session.stop();
		finish(unlisten);
		await pending;
		expect(unlisten).toHaveBeenCalledOnce();
		expect(snapshot).not.toHaveBeenCalled();
		expect(ready).not.toHaveBeenCalled();
	});
});
