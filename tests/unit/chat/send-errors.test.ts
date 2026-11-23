import { describe, expect, it, vi } from 'vitest';
const capture = vi.hoisted(() => vi.fn());
vi.mock('$lib/errors/controller', () => ({ appErrors: { capture } }));
vi.mock('$utils/inbound', () => ({ errorDetails: (error: unknown) => String(error) }));
import { captureChatSendError } from '$lib/chat/send-errors';

describe('correlated chat send feedback', () => {
	it('keeps rejection guidance and technical capture policy while attaching the attempt ID', () => {
		const rejection = {
			kind: 'rejected',
			message: 'Try later',
			details: 'MsgRejected',
			attempt_id: 'native'
		};
		expect(captureChatSendError('maya', rejection, 'frontend')).toBe('Try later');
		expect(capture).toHaveBeenLastCalledWith(
			{ ...rejection, channel: 'maya', attempt_id: 'frontend' },
			expect.objectContaining({ source: 'chat.send' }),
			false
		);
		expect(captureChatSendError('maya', new Error('invoke failed'), 'invoke-id')).toBeNull();
		expect(capture).toHaveBeenLastCalledWith(
			expect.objectContaining({ kind: 'unexpected', attempt_id: 'invoke-id' }),
			expect.anything(),
			true
		);
	});
});
