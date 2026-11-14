import { mount, unmount } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import type { ChatRoomInfo, SystemMessage, SystemMessageIcon } from '$lib/bindings';
import SystemMessageRow from '$lib/components/chat/system-message.svelte';
import { postSystemMessage, systemMessageLink } from './system-messages';

const native = vi.hoisted(() => ({ post: vi.fn(), open: vi.fn().mockResolvedValue(undefined) }));
vi.mock('$lib/bindings', () => ({ commands: { postSystemMessage: native.post } }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: native.open }));
vi.mock('$utils/log', () => ({ default: { error: vi.fn() } }));

const room: ChatRoomInfo = {
	broadcaster_id: '1',
	broadcaster_login: 'room',
	broadcaster_name: 'Room'
};
const message: SystemMessage = {
	index: 5,
	ts: '2026-11-14T19:00:00Z',
	message_id: 'notice',
	broadcaster_user_id: '1',
	broadcaster_user_login: 'room',
	broadcaster_user_name: 'Room',
	icon: null,
	segments: [
		{ type: 'user', user_id: '2', login: 'viewer', display_name: 'Viewer' },
		{ type: 'text', text: ' was ' },
		{ type: 'emphasis', text: 'timed out' },
		{ type: 'text', text: '. ' },
		{ type: 'link', text: 'Details', url: 'https://example.com/notice' }
	]
};

describe('system messages', () => {
	it('supplies creation metadata and uses the canonical command', async () => {
		native.post.mockResolvedValueOnce({ status: 'ok', data: true });
		const before = Date.now();
		expect(await postSystemMessage(room, { segments: message.segments })).toEqual({
			status: 'ok',
			data: true
		});
		const [target, input] = native.post.mock.lastCall!;
		expect(target).toBe(room);
		expect(input.message_id).toMatch(/^[\da-f-]{36}$/);
		expect(Date.parse(input.ts)).toBeGreaterThanOrEqual(before);
		expect(input.icon).toBeNull();
		expect(input.segments).toBe(message.segments);
	});

	it('preserves producer metadata and reports command failures', async () => {
		const error = { status: 'error', error: 'chat engine is not running' };
		native.post.mockResolvedValueOnce(error);
		expect(await postSystemMessage(room, { ...message, icon: 'shield' })).toBe(error);
		expect(native.post.mock.lastCall?.[1]).toEqual({
			message_id: message.message_id,
			ts: message.ts,
			segments: message.segments,
			icon: 'shield'
		});
	});

	it('renders structured text and opens links externally', async () => {
		const target = document.createElement('div');
		const component = mount(SystemMessageRow, { target, props: { message } });
		try {
			expect(target.textContent?.trim()).toBe('Viewer was timed out. Details');
			expect(target.querySelector('[data-chat-message-index="5"]')).not.toBeNull();
			expect(target.querySelector('[data-system-user-id="2"]')?.tagName).toBe('STRONG');
			expect(target.querySelector('svg')).toBeNull();
			target.querySelector('a')!.click();
			expect(native.open).toHaveBeenCalledWith('https://example.com/notice');
		} finally {
			await unmount(component);
		}
	});

	it.each(['info', 'shield', 'clock', 'success', 'warning', 'error'] as SystemMessageIcon[])(
		'renders the optional %s icon and centered alignment',
		async (icon) => {
			const target = document.createElement('div');
			const component = mount(SystemMessageRow, {
				target,
				props: { message: { ...message, icon }, centered: true }
			});
			try {
				expect(target.firstElementChild?.classList.contains('text-center')).toBe(true);
				expect(target.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
			} finally {
				await unmount(component);
			}
		}
	);

	it.each(['javascript:alert(1)', 'file:///tmp/test', 'not a URL'])(
		'renders an invalid link as text: %s',
		async (url) => {
			expect(systemMessageLink(url)).toBeUndefined();
			const target = document.createElement('div');
			const component = mount(SystemMessageRow, {
				target,
				props: { message: { ...message, segments: [{ type: 'link', text: 'Details', url }] } }
			});
			try {
				expect(target.querySelector('a')).toBeNull();
				expect(target.textContent?.trim()).toBe('Details');
			} finally {
				await unmount(component);
			}
		}
	);
});
