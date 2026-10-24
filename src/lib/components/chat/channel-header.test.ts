import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ChannelHeader } from '$lib/chat/channel-header';
import HeaderFixture from './channel-header.fixture.svelte';

let component: ReturnType<typeof mount> | undefined;
let headerWidth: number;
let observers: { resize: () => void; disconnect: ReturnType<typeof vi.fn> }[];

const live: ChannelHeader = {
	channelName: 'Maya',
	isPartner: true,
	state: 'live',
	title: 'Original title 🦊',
	category: 'Terraria',
	language: 'English',
	tags: ['Hair', 'DropsEnabled'],
	viewerCount: 4646,
	startedAt: '2026-01-01T00:00:00Z'
};

beforeEach(() => {
	vi.useFakeTimers();
	vi.setSystemTime(new Date('2026-01-01T02:47:45Z'));
	headerWidth = 1100;
	observers = [];
	vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
		let width: number;
		if (this.hasAttribute('data-channel-header')) width = headerWidth;
		else {
			const name = this.getAttribute('data-header-measure');
			width =
				(
					{
						identity: 100,
						title: 300,
						viewers: 60,
						category: 80,
						language: 48,
						tags: 48,
						uptime: 72
					} as Record<string, number>
				)[name ?? ''] ?? 0;
		}
		return {
			width,
			height: 20,
			top: 0,
			right: width,
			bottom: 20,
			left: 0,
			x: 0,
			y: 0,
			toJSON() {}
		};
	});
	vi.stubGlobal(
		'ResizeObserver',
		class {
			disconnect = vi.fn();
			constructor(callback: ResizeObserverCallback) {
				observers.push({ resize: () => callback([], this), disconnect: this.disconnect });
			}
			observe() {}
			unobserve() {}
		}
	);
});

afterEach(async () => {
	if (component) await unmount(component);
	component = undefined;
	document.body.replaceChildren();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

async function flushUi() {
	flushSync();
	await tick();
}

async function render(
	header: ChannelHeader | null = live,
	nextHeader: ChannelHeader | null = null
) {
	const target = document.createElement('div');
	document.body.append(target);
	component = mount(HeaderFixture, { target, props: { header, nextHeader } });
	await flushUi();
}

function element(selector: string) {
	return document.querySelector<HTMLElement>(selector);
}
function text(selector: string) {
	return element(selector)?.textContent?.trim();
}
async function resize(width: number) {
	headerWidth = width;
	for (const observer of observers) observer.resize();
	await flushUi();
}

describe('channel header', () => {
	it('shows informational live metadata and a disabled video placeholder', async () => {
		await render();
		expect(text('[data-header-identity]')).toBe('Maya');
		expect(element('[data-header-identity] [aria-label="Twitch partner"]')).not.toBeNull();
		expect(text('[data-header-title]')).toBe('Original title 🦊');
		expect(element('[data-header-title]')?.title).toBe('Original title 🦊');
		expect(text('[data-header-category]')).toBe('Terraria');
		expect(element('[data-header-tags]')?.children).toHaveLength(2);
		expect(text('[data-header-language]')).toBe('English');
		expect(text('[data-header-tags-trigger]')).toBe('Tags');
		expect(element('[data-header-tags-tooltip]')).toBeNull();
		expect(text('[data-header-viewers]')).toBe('4,646');
		expect(text('[data-header-uptime]')).toBe('2:47:45');
		expect((element('[aria-label="Stream video coming soon"]') as HTMLButtonElement).disabled).toBe(
			true
		);
		expect(document.querySelector('a')).toBeNull();
	});

	it.each(['hover', 'focus'])(
		'shows every remaining tag on %s without truncating long names',
		async (method) => {
			const tags = ['Hair', 'DropsEnabled', 'AnExceptionallyLongTagThatShouldRemainFullyReadable'];
			await render({ ...live, tags });
			const trigger = element('[data-header-tags-trigger]')!;
			if (method === 'hover') {
				trigger.dispatchEvent(new PointerEvent('pointerenter', { pointerType: 'mouse' }));
			} else {
				document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
				trigger.focus();
			}
			await flushUi();
			await vi.advanceTimersByTimeAsync(20);
			await flushUi();
			const tooltip = element('[data-header-tags-tooltip]');
			expect(tooltip).not.toBeNull();
			expect(Array.from(tooltip!.children).map((tag) => tag.textContent)).toEqual(tags);
			expect(text('[data-header-language]')).toBe('English');
		}
	);

	it('keeps only the language pill when there are no remaining tags', async () => {
		await render({ ...live, tags: [] });
		expect(text('[data-header-language]')).toBe('English');
		expect(element('[data-header-tags-trigger]')).toBeNull();
	});

	it('supports tags without stream language metadata', async () => {
		await render({ ...live, language: null });
		expect(element('[data-header-language]')).toBeNull();
		expect(text('[data-header-tags-trigger]')).toBe('Tags');
	});

	it('removes Tags, language, category, and uptime in order as the container shrinks', async () => {
		await render();
		await resize(700);
		expect(element('[data-header-tags]')?.children).toHaveLength(2);
		expect(text('[data-header-tags]')).not.toContain('DropsEnabled');
		await resize(650);
		expect(element('[data-header-tags-trigger]')).toBeNull();
		expect(text('[data-header-language]')).toBe('English');
		await resize(600);
		expect(element('[data-header-tags]')).toBeNull();
		expect(element('[data-header-category]')).not.toBeNull();
		await resize(500);
		expect(element('[data-header-category]')).toBeNull();
		expect(element('[data-header-uptime]')).not.toBeNull();
		await resize(400);
		expect(element('[data-header-uptime]')).toBeNull();
		expect(text('[data-header-identity]')).toBe('Maya');
		expect(text('[data-header-viewers]')).toBe('4,646');
		await resize(1100);
		expect(element('[data-header-tags]')?.children).toHaveLength(2);
		expect(element('[data-header-category]')).not.toBeNull();
		expect(element('[data-header-uptime]')).not.toBeNull();
	});

	it('ticks uptime and replaces the timer and resize observer on channel changes', async () => {
		await render(live, { ...live, channelName: 'Luna', startedAt: '2026-01-01T02:47:40Z' });
		expect(vi.getTimerCount()).toBe(1);
		await vi.advanceTimersByTimeAsync(1000);
		await flushUi();
		expect(text('[data-header-uptime]')).toBe('2:47:46');
		element('[data-switch-channel]')!.click();
		await flushUi();
		expect(text('[data-header-identity]')).toBe('Luna');
		expect(text('[data-header-uptime]')).toBe('0:00:06');
		expect(vi.getTimerCount()).toBe(1);
		expect(observers[0].disconnect).toHaveBeenCalledOnce();
		await unmount(component!);
		component = undefined;
		expect(vi.getTimerCount()).toBe(0);
		expect(observers.at(-1)!.disconnect).toHaveBeenCalledOnce();
	});

	it('shows offline identity without stale live metadata', async () => {
		await render({
			...live,
			state: 'offline',
			title: null,
			category: null,
			language: null,
			tags: [],
			viewerCount: null,
			startedAt: null
		});
		expect(text('[data-header-title]')).toBe('Offline');
		expect(element('[data-header-identity] [aria-label="Twitch partner"]')).not.toBeNull();
		expect(element('[data-header-viewers]')).toBeNull();
		expect(element('[data-header-uptime]')).toBeNull();
		expect(vi.getTimerCount()).toBe(0);
	});

	it('shows loading identity without claiming offline status or partnership', async () => {
		await render({
			...live,
			channelName: 'maya',
			state: 'loading',
			isPartner: false,
			title: null,
			category: null,
			language: null,
			tags: [],
			viewerCount: null,
			startedAt: null
		});
		expect(text('[data-header-identity]')).toBe('maya');
		expect(text('[data-header-title]')).toBe('');
		expect(element('[data-header-identity] [aria-label="Twitch partner"]')).toBeNull();
	});

	it('keeps zero viewers visible and omits invalid uptime', async () => {
		await render({ ...live, viewerCount: 0, startedAt: 'invalid' });
		expect(text('[data-header-viewers]')).toBe('0');
		expect(element('[data-header-uptime]')).toBeNull();
		expect(vi.getTimerCount()).toBe(0);
	});

	it('removes channel content and timers when no channel is selected', async () => {
		await render();
		element('[data-switch-channel]')!.click();
		await flushUi();
		expect(element('[data-channel-header]')).toBeNull();
		expect(element('[aria-label="Stream video coming soon"]')).toBeNull();
		expect(vi.getTimerCount()).toBe(0);
	});
});
