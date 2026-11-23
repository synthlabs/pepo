import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import SidebarFixture from '../../../../support/unit/fixtures/sidebar-provider.svelte';
import { SIDEBAR_STORAGE_KEY } from '$lib/components/ui/sidebar/constants';

let component: ReturnType<typeof mount> | undefined;
let mobile: boolean;
let mediaQueries: MockMediaQuery[];
const originalGetAnimations = Object.getOwnPropertyDescriptor(Element.prototype, 'getAnimations');

class MockMediaQuery extends EventTarget {
	constructor(readonly media: string) {
		super();
	}
	get matches() {
		return this.media.includes('767') && mobile;
	}
}

beforeEach(() => {
	vi.useFakeTimers();
	mobile = false;
	mediaQueries = [];
	window.localStorage.clear();
	document.cookie = `${SIDEBAR_STORAGE_KEY}=; path=/; max-age=0`;
	vi.stubGlobal('matchMedia', (query: string) => {
		const media = new MockMediaQuery(query);
		mediaQueries.push(media);
		return media;
	});
	vi.stubGlobal(
		'ResizeObserver',
		class {
			observe() {}
			unobserve() {}
			disconnect() {}
		}
	);
	Object.defineProperty(Element.prototype, 'getAnimations', {
		configurable: true,
		value: () => []
	});
});

afterEach(async () => {
	if (component) await unmount(component);
	component = undefined;
	await flushUi();
	document.body.replaceChildren();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
	if (originalGetAnimations) {
		Object.defineProperty(Element.prototype, 'getAnimations', originalGetAnimations);
	} else {
		Reflect.deleteProperty(Element.prototype, 'getAnimations');
	}
	window.localStorage.clear();
	document.cookie = `${SIDEBAR_STORAGE_KEY}=; path=/; max-age=0`;
	vi.useRealTimers();
});

async function flushUi() {
	flushSync();
	await tick();
	await vi.advanceTimersByTimeAsync(1);
	flushSync();
	await tick();
}

async function render(props: { open?: boolean; onOpenChange?: (open: boolean) => void } = {}) {
	if (component) await unmount(component);
	const target = document.createElement('div');
	document.body.append(target);
	component = mount(SidebarFixture, { target, props });
	await flushUi();
}

function state() {
	return document.querySelector('[data-collapsible]')?.getAttribute('data-state');
}

async function toggle() {
	document.querySelector<HTMLButtonElement>('[data-sidebar="trigger"]')!.click();
	await flushUi();
}

describe('local sidebar preference', () => {
	it.each([null, 'true', 'false', 'invalid', 'null', ''])('initializes from %s', async (saved) => {
		if (saved !== null) window.localStorage.setItem(SIDEBAR_STORAGE_KEY, saved);
		const write = vi.spyOn(Storage.prototype, 'setItem');
		await render();
		expect(state()).toBe(saved === 'false' ? 'collapsed' : 'expanded');
		expect(write).not.toHaveBeenCalled();
	});

	it('ignores the old sidebar cookie', async () => {
		document.cookie = `${SIDEBAR_STORAGE_KEY}=false; path=/`;
		await render();
		expect(state()).toBe('expanded');
		expect(window.localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBeNull();
	});

	it.each(['trigger', 'rail', 'shortcut'])('persists changes through the %s', async (method) => {
		await render();
		if (method === 'shortcut') {
			window.dispatchEvent(new KeyboardEvent('keydown', { key: 'b', ctrlKey: true }));
		} else {
			document.querySelector<HTMLButtonElement>(`[data-sidebar="${method}"]`)!.click();
		}
		await flushUi();
		expect(state()).toBe('collapsed');
		expect(window.localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe('false');
		expect(document.cookie).not.toContain(SIDEBAR_STORAGE_KEY);
		await render();
		expect(state()).toBe('collapsed');
		await toggle();
		expect(state()).toBe('expanded');
		expect(window.localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe('true');
		await render();
		expect(state()).toBe('expanded');
	});

	it('preserves the explicit open prop and change callback', async () => {
		window.localStorage.setItem(SIDEBAR_STORAGE_KEY, 'true');
		const onOpenChange = vi.fn();
		await render({ open: false, onOpenChange });
		expect(state()).toBe('collapsed');
		await toggle();
		expect(onOpenChange).toHaveBeenCalledExactlyOnceWith(true);
		expect(state()).toBe('expanded');
		expect(window.localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe('true');
	});

	it('starts expanded and keeps toggling when storage is inaccessible', async () => {
		vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
			throw new DOMException('Blocked', 'SecurityError');
		});
		await render();
		expect(state()).toBe('expanded');
		await toggle();
		expect(state()).toBe('collapsed');
		await toggle();
		expect(state()).toBe('expanded');
	});

	it('keeps the in-memory choice when storage writes fail', async () => {
		vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
			throw new DOMException('Full', 'QuotaExceededError');
		});
		await render();
		await toggle();
		expect(state()).toBe('collapsed');
		expect(window.localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBeNull();
		await toggle();
		expect(state()).toBe('expanded');
	});

	it('keeps the narrow-window drawer temporary and preserves desktop collapse', async () => {
		mobile = true;
		window.localStorage.setItem(SIDEBAR_STORAGE_KEY, 'false');
		const write = vi.spyOn(Storage.prototype, 'setItem');
		await render();
		expect(document.querySelector('[data-mobile="true"]')).toBeNull();
		await toggle();
		expect(document.querySelector('[data-mobile="true"]')).not.toBeNull();
		expect(write).not.toHaveBeenCalled();
		await render();
		expect(document.querySelector('[data-mobile="true"]')).toBeNull();
		mobile = false;
		for (const media of mediaQueries) media.dispatchEvent(new Event('change'));
		await flushUi();
		expect(state()).toBe('collapsed');
		expect(window.localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe('false');
		expect(write).not.toHaveBeenCalled();
	});
});
