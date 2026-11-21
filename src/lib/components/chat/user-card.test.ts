import { flushSync, mount, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import type { UserCardState } from '$lib/bindings';
import UserCard from './user-card.svelte';

export function cardState(patch: Partial<UserCardState> = {}): UserCardState {
	return {
		user: { user_id: '1', login: 'maya', display_name: '마야', color: '#123456' },
		profile_image_url: null,
		channel_id: 'room',
		channel_login: 'room',
		badges: null,
		profile_status: 'loading',
		error: null,
		pinned: false,
		chrome: 'compact',
		revision: 1,
		...patch
	};
}

describe('user card', () => {
	it('keeps identity and actions available during loading and lets a pin override blur', async () => {
		const target = document.createElement('div');
		document.body.append(target);
		const onpin = vi.fn();
		const onclose = vi.fn();
		const onprofile = vi.fn();
		const component = mount(UserCard, {
			target,
			props: {
				state: cardState(),
				closeOnUnfocus: true,
				onpin,
				onclose,
				onprofile,
				onretry: vi.fn()
			}
		});
		try {
			flushSync();
			expect(target.textContent).toContain('마야');
			expect(target.textContent).toContain('@maya');
			expect(target.textContent).toContain('Loading profile');
			expect(target.querySelector('[data-user-card-badges-unknown]')).not.toBeNull();
			(target.querySelector('[aria-label="Pin user card"]') as HTMLButtonElement).click();
			expect(onpin).toHaveBeenCalledWith(true);
			(target.querySelector('[aria-label="Close user card"]') as HTMLButtonElement).click();
			expect(onclose).toHaveBeenCalledOnce();
			[...target.querySelectorAll('button')]
				.find((button) => button.textContent?.includes('View on Twitch'))!
				.click();
			expect(onprofile).toHaveBeenCalledOnce();
		} finally {
			await unmount(component);
			target.remove();
		}
	});

	it.each(['error', 'unavailable'] as const)(
		'shows a retry without discarding identity for %s',
		async (status) => {
			const target = document.createElement('div');
			const onretry = vi.fn();
			const component = mount(UserCard, {
				target,
				props: {
					state: cardState({
						profile_status: status,
						badges: [],
						chrome: 'native',
						error: status === 'error' ? 'Try again shortly.' : null
					}),
					closeOnUnfocus: false,
					onretry,
					onpin: vi.fn(),
					onclose: vi.fn(),
					onprofile: vi.fn()
				}
			});
			try {
				flushSync();
				expect(target.textContent).toContain('@maya');
				expect(target.querySelector('[aria-label="Window controls"]')).toBeNull();
				expect(target.querySelector('[data-user-card-badges-unknown]')).toBeNull();
				[...target.querySelectorAll('button')]
					.find((button) => button.textContent === 'Retry')!
					.click();
				expect(onretry).toHaveBeenCalledOnce();
			} finally {
				await unmount(component);
			}
		}
	);
});
