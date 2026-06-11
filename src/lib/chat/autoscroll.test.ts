import { describe, expect, it } from 'vitest';
import {
	capturePinnedIntent,
	captureScrollSnapshot,
	getBatchScrollSnapshot,
	isAtBottom,
	refreshScrollStateAfterScroll,
	restoreScrollAfterRender
} from './autoscroll';

const MESSAGE_SELECTOR = '[data-chat-message-index]';

interface LayoutBox {
	top: number;
	height: number;
}

interface TestContainer {
	container: HTMLElement;
	layouts: Map<HTMLElement, LayoutBox>;
	getScrollTop: () => number;
	setScrollHeight: (height: number) => void;
}

function createContainer({
	scrollTop,
	scrollHeight,
	clientHeight
}: {
	scrollTop: number;
	scrollHeight: number;
	clientHeight: number;
}): TestContainer {
	const container = document.createElement('div');
	const layouts = new Map<HTMLElement, LayoutBox>();
	let currentScrollTop = scrollTop;
	let currentScrollHeight = scrollHeight;

	Object.defineProperty(container, 'scrollTop', {
		configurable: true,
		get: () => currentScrollTop,
		set: (value: number) => {
			currentScrollTop = value;
		}
	});
	Object.defineProperty(container, 'scrollHeight', {
		configurable: true,
		get: () => currentScrollHeight
	});
	Object.defineProperty(container, 'clientHeight', {
		configurable: true,
		get: () => clientHeight
	});

	container.getBoundingClientRect = () => rect(0, clientHeight);

	return {
		container,
		layouts,
		getScrollTop: () => currentScrollTop,
		setScrollHeight: (height: number) => {
			currentScrollHeight = height;
		}
	};
}

function appendMessage(
	target: TestContainer,
	key: string,
	top: number,
	height = 50
): HTMLElement {
	const message = document.createElement('div');
	message.dataset.chatMessageIndex = key;
	target.layouts.set(message, { top, height });
	message.getBoundingClientRect = () => {
		const layout = target.layouts.get(message);
		if (!layout) return rect(0, 0);

		return rect(layout.top - target.getScrollTop(), layout.height);
	};
	target.container.appendChild(message);
	return message;
}

function rect(top: number, height: number): DOMRect {
	return {
		x: 0,
		y: top,
		top,
		bottom: top + height,
		left: 0,
		right: 100,
		width: 100,
		height,
		toJSON: () => ({})
	} as DOMRect;
}

describe('autoscroll helpers', () => {
	it('keeps a bottom-pinned viewport pinned after content grows', () => {
		const target = createContainer({ scrollTop: 600, scrollHeight: 1000, clientHeight: 400 });
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.setScrollHeight(1200);
		const result = restoreScrollAfterRender(target.container, snapshot, MESSAGE_SELECTOR);

		expect(result.pinned).toBe(true);
		expect(target.getScrollTop()).toBe(800);
	});

	it('does not scroll to bottom when the user is paused above the tail', () => {
		const target = createContainer({ scrollTop: 300, scrollHeight: 1000, clientHeight: 400 });
		appendMessage(target, '4', 300);
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.setScrollHeight(1200);
		const result = restoreScrollAfterRender(target.container, snapshot, MESSAGE_SELECTOR);

		expect(result.pinned).toBe(false);
		expect(result.anchored).toBe(true);
		expect(target.getScrollTop()).toBe(300);
	});

	it('uses a small threshold when deciding whether the viewport is at bottom', () => {
		const target = createContainer({ scrollTop: 570, scrollHeight: 1000, clientHeight: 400 });

		expect(isAtBottom(target.container, 32)).toBe(true);

		target.container.scrollTop = 567;
		expect(isAtBottom(target.container, 32)).toBe(false);
	});

	it('keeps the same visible row stable when older messages are trimmed', () => {
		const target = createContainer({ scrollTop: 500, scrollHeight: 1000, clientHeight: 300 });
		const anchor = appendMessage(target, '5', 500);
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.layouts.set(anchor, { top: 400, height: 50 });
		const result = restoreScrollAfterRender(target.container, snapshot, MESSAGE_SELECTOR);

		expect(result.pinned).toBe(false);
		expect(result.anchored).toBe(true);
		expect(target.getScrollTop()).toBe(400);
	});

	it('keeps the first snapshot for a burst of messages before the queued restore runs', () => {
		const target = createContainer({ scrollTop: 600, scrollHeight: 1000, clientHeight: 400 });
		const firstSnapshot = getBatchScrollSnapshot(null, target.container, MESSAGE_SELECTOR);

		target.setScrollHeight(1050);
		const burstSnapshot = getBatchScrollSnapshot(firstSnapshot, target.container, MESSAGE_SELECTOR);

		expect(burstSnapshot).toBe(firstSnapshot);

		target.setScrollHeight(1200);
		const result = restoreScrollAfterRender(target.container, burstSnapshot, MESSAGE_SELECTOR);

		expect(result.pinned).toBe(true);
		expect(target.getScrollTop()).toBe(800);
	});

	it('keeps bottom-pinned intent from a pending snapshot even if the DOM has grown', () => {
		const target = createContainer({ scrollTop: 600, scrollHeight: 1000, clientHeight: 400 });
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.setScrollHeight(1400);

		expect(capturePinnedIntent(target.container, snapshot, false, 32)).toBe(true);
	});

	it('keeps paused intent from a pending snapshot', () => {
		const target = createContainer({ scrollTop: 300, scrollHeight: 1000, clientHeight: 400 });
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.container.scrollTop = 600;

		expect(capturePinnedIntent(target.container, snapshot, true, 32)).toBe(false);
	});

	it('uses the current viewport state when no pending snapshot exists', () => {
		const target = createContainer({ scrollTop: 570, scrollHeight: 1000, clientHeight: 400 });

		expect(capturePinnedIntent(target.container, null, false, 32)).toBe(true);

		target.container.scrollTop = 300;
		expect(capturePinnedIntent(target.container, null, true, 32)).toBe(false);
		expect(capturePinnedIntent(null, null, true, 32)).toBe(true);
	});

	it('does not let an interim scroll event pause a bottom-pinned burst before restore', () => {
		const target = createContainer({ scrollTop: 600, scrollHeight: 1000, clientHeight: 400 });
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.setScrollHeight(1200);
		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			snapshot,
			true,
			0,
			MESSAGE_SELECTOR
		);

		expect(scrollState.pinned).toBe(true);
		expect(scrollState.pendingSnapshot).toBe(snapshot);
		expect(scrollState.unreadMessageCount).toBe(0);
		expect(scrollState.deferred).toBe(true);

		const result = restoreScrollAfterRender(
			target.container,
			scrollState.pendingSnapshot!,
			MESSAGE_SELECTOR
		);
		expect(result.pinned).toBe(true);
		expect(target.getScrollTop()).toBe(800);
	});

	it('keeps bottom pinned when buffer trim causes an interim scroll position before restore', () => {
		const target = createContainer({ scrollTop: 700, scrollHeight: 1000, clientHeight: 300 });
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.setScrollHeight(1100);
		target.container.scrollTop = 650;
		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			snapshot,
			true,
			0,
			MESSAGE_SELECTOR
		);

		expect(scrollState.pinned).toBe(true);
		expect(scrollState.pendingSnapshot).toBe(snapshot);
		expect(scrollState.deferred).toBe(true);

		const result = restoreScrollAfterRender(
			target.container,
			scrollState.pendingSnapshot!,
			MESSAGE_SELECTOR
		);
		expect(result.pinned).toBe(true);
		expect(target.getScrollTop()).toBe(800);
	});

	it('still refreshes the anchor for a queued restore when the user was already paused', () => {
		const target = createContainer({ scrollTop: 300, scrollHeight: 1000, clientHeight: 400 });
		const anchor = appendMessage(target, '4', 300);
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.layouts.set(anchor, { top: 330, height: 50 });
		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			snapshot,
			true,
			2,
			MESSAGE_SELECTOR
		);

		expect(scrollState.pinned).toBe(false);
		expect(scrollState.pendingSnapshot).not.toBe(snapshot);
		expect(scrollState.pendingSnapshot?.wasAtBottom).toBe(false);
		expect(scrollState.pendingSnapshot?.anchor).toEqual({ key: '4', topOffset: 30 });
		expect(scrollState.unreadMessageCount).toBe(2);
		expect(scrollState.deferred).toBe(false);
	});
});
