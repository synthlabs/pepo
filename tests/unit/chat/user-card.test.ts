import { describe, expect, it, vi } from 'vitest';
const native = vi.hoisted(() => ({ open: vi.fn(), capture: vi.fn() }));
vi.mock('$lib/bindings', () => ({ commands: { openUserCard: native.open } }));
vi.mock('$lib/errors/controller', () => ({ appErrors: { capture: native.capture } }));
import { openUserCard } from '$lib/chat/user-card';

describe('user card activation', () => {
	it('serializes identity, channel, and logical anchor bounds for known and unseen users', async () => {
		const anchor = document.createElement('button');
		vi.spyOn(anchor, 'getBoundingClientRect').mockReturnValue({
			x: 100,
			y: 50,
			width: 60,
			height: 20
		} as DOMRect);
		native.open.mockResolvedValue({ status: 'ok', data: null });
		for (const userId of [null, '1']) {
			await openUserCard({
				userId,
				login: 'maya',
				displayName: '마야',
				channelId: 'room',
				channelLogin: 'room',
				anchor
			});
			expect(native.open).toHaveBeenLastCalledWith({
				user_id: userId,
				login: 'maya',
				display_name: '마야',
				channel_id: 'room',
				channel_login: 'room',
				anchor: { x: 100, y: 50, width: 60, height: 20 }
			});
		}
	});

	it('routes both native errors and invoke rejections into the existing error host', async () => {
		const target = {
			userId: null,
			login: 'maya',
			displayName: 'Maya',
			channelId: 'room',
			channelLogin: 'room',
			anchor: document.createElement('button')
		};
		for (const result of [
			() => Promise.resolve({ status: 'error', error: 'Window unavailable' }),
			() => Promise.reject(new Error('Invoke failed'))
		]) {
			native.open.mockImplementationOnce(result);
			await expect(openUserCard(target)).resolves.toBeUndefined();
			expect(native.capture).toHaveBeenLastCalledWith(expect.anything(), {
				source: 'user-card.open',
				title: 'Could not open user card'
			});
		}
	});
});
