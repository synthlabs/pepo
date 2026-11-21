import { commands } from '$lib/bindings';
import { appErrors } from '$lib/errors/controller';
import type { UsernameActivation } from './username';

export async function openUserCard(target: UsernameActivation): Promise<void> {
	const { x, y, width, height } = target.anchor.getBoundingClientRect();
	try {
		const result = await commands.openUserCard({
			user_id: target.userId,
			login: target.login,
			display_name: target.displayName,
			channel_id: target.channelId,
			channel_login: target.channelLogin,
			anchor: { x, y, width, height }
		});
		if (result.status === 'error') throw result.error;
	} catch (error) {
		appErrors.capture(error, { source: 'user-card.open', title: 'Could not open user card' });
	}
}
