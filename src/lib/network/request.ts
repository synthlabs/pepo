import { commands } from '$lib/bindings';

export async function withBackgroundRequest<T>(request: () => Promise<T>): Promise<T> {
	const lease = await commands.beginBackgroundRequest('updates');
	if (lease.status === 'error') throw new Error(lease.error);
	try {
		return await request();
	} finally {
		await commands.endBackgroundRequest(lease.data);
	}
}
