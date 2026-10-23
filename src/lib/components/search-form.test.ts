import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import SearchFixture from './search-form.fixture.svelte';

let component: ReturnType<typeof mount> | undefined;
let animations: Animation[];
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
	animations = [];
	mobile = false;
	mediaQueries = [];
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
		value(this: Element) {
			return this.matches('[data-sidebar="panel"]') ? animations : [];
		}
	});
});

afterEach(async () => {
	if (component) await unmount(component);
	component = undefined;
	await flushUi();
	document.body.replaceChildren();
	vi.unstubAllGlobals();
	vi.useRealTimers();
	if (originalGetAnimations) {
		Object.defineProperty(Element.prototype, 'getAnimations', originalGetAnimations);
	} else {
		Reflect.deleteProperty(Element.prototype, 'getAnimations');
	}
});

async function flushUi() {
	flushSync();
	await tick();
	await vi.advanceTimersByTimeAsync(20);
	flushSync();
	await tick();
}

async function render(
	props: { open?: boolean; value?: string; onsubmit?: (value: string) => void } = {}
) {
	const target = document.createElement('div');
	document.body.append(target);
	component = mount(SearchFixture, { target, props });
	await flushUi();
}

function element<T extends HTMLElement = HTMLElement>(selector: string): T {
	const result = document.querySelector<T>(selector);
	expect(result, selector).not.toBeNull();
	return result!;
}

function input() {
	return element<HTMLInputElement>('#search');
}

function searchIcon() {
	return element<HTMLButtonElement>('[data-popover-trigger]');
}

function query() {
	return element('[data-query]').textContent;
}

function transition(property = 'width') {
	let resolve!: (value: Animation) => void;
	let reject!: (reason: Error) => void;
	const animation = {
		transitionProperty: property,
		playState: 'running',
		finished: new Promise<Animation>((res, rej) => {
			resolve = res;
			reject = rej;
		})
	} as unknown as Animation;
	return {
		animation,
		finish() {
			Object.assign(animation, { playState: 'finished' });
			resolve(animation);
		},
		cancel() {
			Object.assign(animation, { playState: 'idle' });
			reject(new DOMException('Transition cancelled', 'AbortError'));
		}
	};
}

async function toggle() {
	element('[data-sidebar="trigger"]').click();
	await flushUi();
}

async function pointerClick(target: HTMLElement) {
	target.dispatchEvent(
		new PointerEvent('pointerdown', {
			bubbles: true,
			cancelable: true,
			button: 0,
			pointerType: 'mouse',
			clientX: 100,
			clientY: 100
		})
	);
	await flushUi();
	target.dispatchEvent(
		new PointerEvent('pointerup', {
			bubbles: true,
			cancelable: true,
			button: 0,
			pointerType: 'mouse',
			clientX: 100,
			clientY: 100
		})
	);
	await flushUi();
	target.click();
	await flushUi();
}

describe('sidebar search transitions', () => {
	it('keeps the same input until the width transition finishes', async () => {
		await render({ value: 'summit1g' });
		const originalInput = input();
		const width = transition();
		const unrelated = transition('left');
		animations = [width.animation, unrelated.animation];
		await toggle();
		expect(input()).toBe(originalInput);
		expect(input().readOnly).toBe(true);
		expect(input().classList).toContain('text-transparent');
		expect(input().classList).toContain('placeholder:text-transparent');
		expect(document.querySelector('[data-popover-trigger]')).toBeNull();
		expect(document.querySelector('#search + svg + button')).toBeNull();
		expect(query()).toBe('summit1g');
		width.finish();
		await flushUi();
		expect(document.querySelector('#search')).toBeNull();
		expect(searchIcon()).toBeTruthy();
		expect(query()).toBe('summit1g');
	});

	it('opens with a narrow read-only input and restores editing on completion', async () => {
		await render({ open: false, value: 'summit1g' });
		expect(searchIcon()).toBeTruthy();
		const width = transition();
		animations = [width.animation];
		await toggle();
		expect(input().readOnly).toBe(true);
		expect(document.querySelector('[data-popover-trigger]')).toBeNull();
		width.finish();
		await flushUi();
		expect(input().readOnly).toBe(false);
		expect(input().value).toBe('summit1g');
		expect(input().classList).not.toContain('text-transparent');
		expect(element('#search').parentElement?.querySelector('button')).not.toBeNull();
	});

	it.each([true, false])('ignores stale completion after reversing from open=%s', async (open) => {
		await render({ open });
		const first = transition();
		animations = [first.animation];
		await toggle();
		const second = transition();
		animations = [second.animation];
		await toggle();
		first.finish();
		await flushUi();
		expect(input().readOnly).toBe(true);
		expect(document.querySelector('[data-popover-trigger]')).toBeNull();
		second.finish();
		await flushUi();
		if (open) expect(input().readOnly).toBe(false);
		else expect(searchIcon()).toBeTruthy();
	});

	it('rechecks an interrupted transition before settling', async () => {
		await render();
		const first = transition();
		animations = [first.animation];
		await toggle();
		const replacement = transition();
		animations = [replacement.animation];
		first.cancel();
		await flushUi();
		expect(input().readOnly).toBe(true);
		replacement.cancel();
		animations = [];
		await flushUi();
		expect(searchIcon()).toBeTruthy();
	});

	it('settles immediately without an animation', async () => {
		await render({ open: false });
		expect(searchIcon()).toBeTruthy();
		await toggle();
		expect(input().readOnly).toBe(false);
		await toggle();
		expect(searchIcon()).toBeTruthy();
	});

	it('stops observing a transition after unmount', async () => {
		await render();
		const width = transition();
		animations = [width.animation];
		await toggle();
		await unmount(component!);
		component = undefined;
		width.finish();
		await flushUi();
		expect(document.querySelector('#search')).toBeNull();
		expect(document.querySelector('[data-popover-trigger]')).toBeNull();
	});
});

describe('search query and focus handoff', () => {
	it.each(['keyboard', 'trigger', 'rail'])(
		'preserves a floating query on expansion by %s',
		async (method) => {
			await render({ open: false, value: 'summit1g' });
			searchIcon().click();
			await flushUi();
			const floating = element<HTMLInputElement>('#search-floating');
			floating.focus();
			await flushUi();
			const width = transition();
			animations = [width.animation];
			if (method === 'keyboard') {
				floating.dispatchEvent(
					new KeyboardEvent('keydown', { key: 'b', ctrlKey: true, bubbles: true })
				);
				await flushUi();
			} else {
				await pointerClick(element(`[data-sidebar="${method}"]`));
			}
			expect(document.querySelector('#search-floating')).toBeNull();
			expect(input().value).toBe('summit1g');
			expect(query()).toBe('summit1g');
			expect(document.activeElement?.id).toBe('search');
			width.finish();
			await flushUi();
			expect(document.activeElement).toBe(input());
		}
	);

	it('moves focus from the disappearing clear button to the input, then to the icon', async () => {
		await render({ value: 'summit1g' });
		element<HTMLButtonElement>('#search').parentElement!.querySelector('button')!.focus();
		const width = transition();
		animations = [width.animation];
		await toggle();
		expect(document.activeElement).toBe(input());
		width.finish();
		await flushUi();
		expect(document.activeElement).toBe(searchIcon());
		await toggle();
		expect(document.activeElement).toBe(input());
	});

	it('does not steal focus moved outside search during collapse', async () => {
		await render();
		input().focus();
		const width = transition();
		animations = [width.animation];
		await toggle();
		const outside = element('[data-outside]');
		outside.focus();
		width.finish();
		await flushUi();
		expect(document.activeElement).toBe(outside);
		await toggle();
		expect(document.activeElement).toBe(outside);
	});

	it.each(['escape', 'outside'])(
		'clears a floating query on ordinary %s dismissal',
		async (method) => {
			await render({ open: false, value: 'summit1g' });
			searchIcon().click();
			await flushUi();
			if (method === 'escape') {
				element('#search-floating').dispatchEvent(
					new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })
				);
				await flushUi();
			} else {
				await pointerClick(element('[data-outside]'));
			}
			expect(query()).toBe('');
			expect(document.querySelector('#search-floating')).toBeNull();
		}
	);

	it('preserves clear, Escape, and submit behavior', async () => {
		const submit = vi.fn();
		await render({ value: 'summit1g', onsubmit: submit });
		element('#search').parentElement!.querySelector('button')!.click();
		await flushUi();
		expect(input().value).toBe('');
		expect(document.activeElement).toBe(input());
		input().value = 'summit1g';
		input().dispatchEvent(new Event('input', { bubbles: true }));
		input().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		await flushUi();
		expect(input().value).toBe('');
		expect(document.activeElement).toBe(input());
		input().value = ' Summit1G ';
		input().dispatchEvent(new Event('input', { bubbles: true }));
		input().closest('form')!.requestSubmit();
		await flushUi();
		expect(submit).toHaveBeenCalledWith('summit1g');
		expect(query()).toBe('');
	});

	it('keeps mobile search editable and ignores an obsolete desktop transition', async () => {
		await render({ value: 'summit1g' });
		const width = transition();
		animations = [width.animation];
		await toggle();
		mobile = true;
		for (const media of mediaQueries) media.dispatchEvent(new Event('change'));
		await flushUi();
		width.finish();
		await toggle();
		expect(input().readOnly).toBe(false);
		expect(input().value).toBe('summit1g');
		input().focus();
		input().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		await flushUi();
		expect(input().value).toBe('');
		expect(document.activeElement).toBe(input());
		expect(document.querySelector('[data-popover-trigger]')).toBeNull();
	});
});
