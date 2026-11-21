import { flushSync, mount, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import Username from './username.svelte';
import UsernameFixture from './username.fixture.svelte';
import { UserDirectory } from '$lib/chat/users.svelte';

describe('username rendering', () => {
	it('updates an unknown mention from observed metadata without changing typed text', async () => {
		const directory = new UserDirectory();
		const onactivate = vi.fn();
		const target = document.createElement('div');
		const component = mount(UsernameFixture, {
			target,
			props: {
				kind: 'mention',
				login: 'maya',
				originalText: '@MaYa',
				channelId: 'room',
				channelLogin: 'room',
				directory,
				onactivate
			}
		});
		try {
			const button = target.querySelector('button')!;
			expect(button.textContent).toBe('@MaYa');
			expect(button.classList.contains('font-bold')).toBe(true);
			flushSync(() =>
				directory.apply({
					type: 'delta',
					removed: [],
					upserts: [{ user_id: '1', login: 'maya', display_name: '마야', color: '#123456' }]
				})
			);
			expect(button.textContent).toBe('@MaYa');
			expect(button.style.color).toBe('rgb(18, 52, 86)');
			button.click();
			expect(onactivate).toHaveBeenCalledWith(
				expect.objectContaining({
					userId: '1',
					login: 'maya',
					displayName: '마야',
					channelId: 'room',
					anchor: button
				})
			);
			flushSync(() =>
				component.updateSettings({ use_display_names_in_mentions: true, bold_mentions: false })
			);
			expect(button.textContent).toBe('@마야 (maya)');
			expect(button.classList.contains('font-bold')).toBe(false);
			flushSync(() => component.updateSettings({ show_login_for_different_display_name: false }));
			expect(button.textContent).toBe('@마야');
			flushSync(() => directory.apply({ type: 'delta', removed: ['1'], upserts: [] }));
			expect(button.textContent).toBe('@maya');
			expect(button.style.color).toBe('');
		} finally {
			await unmount(component);
		}
	});

	it('keeps author metadata as sent and makes activation a no-op by default', async () => {
		const directory = new UserDirectory();
		directory.apply({
			type: 'snapshot',
			users: [{ user_id: '1', login: 'maya', display_name: 'Changed', color: '#654321' }]
		});
		const target = document.createElement('div');
		const component = mount(Username, {
			target,
			props: {
				kind: 'author',
				userId: '1',
				login: 'maya',
				displayName: '마야',
				color: '#123456',
				channelId: 'room',
				channelLogin: 'room',
				directory
			}
		});
		try {
			const button = target.querySelector('button')!;
			expect(button.textContent).toBe('마야 (maya)');
			expect(button.style.color).toBe('rgb(18, 52, 86)');
			expect(button.type).toBe('button');
			expect(button.classList.contains('select-text')).toBe(true);
			button.click();
			expect(target.querySelectorAll('button')).toHaveLength(1);
			expect(target.querySelector('[role="dialog"]')).toBeNull();
		} finally {
			await unmount(component);
		}
	});
});
