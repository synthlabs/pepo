import { expect, it, vi } from 'vitest';
const commands = vi.hoisted(() => ({
	beginBackgroundRequest: vi.fn(),
	endBackgroundRequest: vi.fn()
}));
vi.mock('$lib/bindings', () => ({ commands }));
import { withBackgroundRequest } from './request';

it('releases the shared request budget when an update request fails', async () => {
	commands.beginBackgroundRequest.mockResolvedValue({ status: 'ok', data: 'lease' });
	commands.endBackgroundRequest.mockResolvedValue(undefined);
	await expect(
		withBackgroundRequest(async () => {
			throw new Error('offline');
		})
	).rejects.toThrow('offline');
	expect(commands.endBackgroundRequest).toHaveBeenCalledWith('lease');
});
