import { describe, expect, it } from 'vitest';
import {
	captureScrollSnapshot,
	getBatchScrollSnapshot,
	getPinnedBatchScrollSnapshot,
	isAtBottom,
	isUserScrollMovement,
	isUserScrollPauseIntent,
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

	it('keeps pinned intent when transition growth temporarily moves the DOM above bottom', () => {
		const target = createContainer({ scrollTop: 600, scrollHeight: 1100, clientHeight: 400 });

		const snapshot = getPinnedBatchScrollSnapshot(null, target.container, MESSAGE_SELECTOR, true);

		expect(snapshot.wasAtBottom).toBe(true);

		target.setScrollHeight(1300);
		const result = restoreScrollAfterRender(target.container, snapshot, MESSAGE_SELECTOR);

		expect(result.pinned).toBe(true);
		expect(target.getScrollTop()).toBe(900);
	});

	it('keeps a paused snapshot paused when transition growth changes layout', () => {
		const target = createContainer({ scrollTop: 300, scrollHeight: 1100, clientHeight: 400 });
		appendMessage(target, '4', 300);

		const snapshot = getPinnedBatchScrollSnapshot(null, target.container, MESSAGE_SELECTOR, false);

		expect(snapshot.wasAtBottom).toBe(false);

		target.setScrollHeight(1300);
		const result = restoreScrollAfterRender(target.container, snapshot, MESSAGE_SELECTOR);

		expect(result.pinned).toBe(false);
		expect(target.getScrollTop()).toBe(300);
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

	it('does not treat large message growth as user pause intent after recent wheel activity', () => {
		const target = createContainer({ scrollTop: 600, scrollHeight: 1000, clientHeight: 400 });
		const intent = { scrollTop: target.getScrollTop(), direction: 'up' as const };

		target.setScrollHeight(1800);
		const userInitiated = isUserScrollPauseIntent(target.getScrollTop(), intent);
		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			null,
			false,
			0,
			MESSAGE_SELECTOR,
			32,
			{ userInitiated, preservePinnedIntent: true }
		);

		expect(userInitiated).toBe(false);
		expect(scrollState.pinned).toBe(true);
		expect(scrollState.unreadMessageCount).toBe(0);
		expect(scrollState.deferred).toBe(true);
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

	it('keeps the original paused anchor for an interim scroll event before restore', () => {
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
		expect(scrollState.pendingSnapshot).toBe(snapshot);
		expect(scrollState.unreadMessageCount).toBe(2);
		expect(scrollState.deferred).toBe(true);

		const result = restoreScrollAfterRender(
			target.container,
			scrollState.pendingSnapshot!,
			MESSAGE_SELECTOR
		);
		expect(result.pinned).toBe(false);
		expect(target.getScrollTop()).toBe(330);
	});

	it('lets user-initiated scrolling replace a queued paused restore', () => {
		const target = createContainer({ scrollTop: 300, scrollHeight: 1000, clientHeight: 400 });
		const anchor = appendMessage(target, '4', 300);
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.layouts.set(anchor, { top: 330, height: 50 });
		target.container.scrollTop = 350;
		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			snapshot,
			true,
			2,
			MESSAGE_SELECTOR,
			32,
			{ userInitiated: true }
		);

		expect(scrollState.pinned).toBe(false);
		expect(scrollState.pendingSnapshot).not.toBe(snapshot);
		expect(scrollState.pendingSnapshot?.wasAtBottom).toBe(false);
		expect(scrollState.pendingSnapshot?.anchor).toEqual({ key: '4', topOffset: -20 });
		expect(scrollState.unreadMessageCount).toBe(2);
		expect(scrollState.deferred).toBe(false);
	});

	it('can reapply a paused anchor after a later layout reflow', () => {
		const target = createContainer({ scrollTop: 300, scrollHeight: 1000, clientHeight: 400 });
		const anchor = appendMessage(target, '4', 300);
		const snapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);

		target.layouts.set(anchor, { top: 330, height: 50 });
		restoreScrollAfterRender(target.container, snapshot, MESSAGE_SELECTOR);
		expect(target.getScrollTop()).toBe(330);

		const reflowSnapshot = captureScrollSnapshot(target.container, MESSAGE_SELECTOR);
		target.layouts.set(anchor, { top: 360, height: 50 });
		const result = restoreScrollAfterRender(target.container, reflowSnapshot, MESSAGE_SELECTOR);

		expect(result.pinned).toBe(false);
		expect(result.anchored).toBe(true);
		expect(target.getScrollTop()).toBe(360);
	});

	it('requires explicit user scroll intent to pause autoscroll during layout reflow', () => {
		const target = createContainer({ scrollTop: 600, scrollHeight: 1000, clientHeight: 400 });

		target.setScrollHeight(1100);
		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			null,
			false,
			0,
			MESSAGE_SELECTOR,
			32,
			{ preservePinnedIntent: true }
		);

		expect(scrollState.pinned).toBe(true);
		expect(scrollState.pendingSnapshot).toBeNull();
		expect(scrollState.unreadMessageCount).toBe(0);
		expect(scrollState.deferred).toBe(true);
	});

	it('lets explicit user scroll intent pause autoscroll from a previously pinned state', () => {
		const target = createContainer({ scrollTop: 599, scrollHeight: 1000, clientHeight: 400 });
		const intent = { scrollTop: 600, direction: 'up' as const };

		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			null,
			false,
			0,
			MESSAGE_SELECTOR,
			32,
			{
				userInitiated: isUserScrollPauseIntent(target.getScrollTop(), intent),
				preservePinnedIntent: true
			}
		);

		expect(scrollState.pinned).toBe(false);
		expect(scrollState.pendingSnapshot).toBeNull();
		expect(scrollState.unreadMessageCount).toBe(0);
		expect(scrollState.deferred).toBe(false);
	});

	it('ignores downward wheel intent while preserving pinned autoscroll', () => {
		const target = createContainer({ scrollTop: 600, scrollHeight: 1000, clientHeight: 400 });
		const intent = { scrollTop: target.getScrollTop(), direction: 'down' as const };

		target.setScrollHeight(1800);
		target.container.scrollTop = 650;
		const userInitiated = isUserScrollPauseIntent(target.getScrollTop(), intent);
		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			null,
			false,
			0,
			MESSAGE_SELECTOR,
			32,
			{ userInitiated, preservePinnedIntent: true }
		);

		expect(userInitiated).toBe(false);
		expect(scrollState.pinned).toBe(true);
		expect(scrollState.deferred).toBe(true);
	});

	it('keeps explicit user scroll intent pinned at exact bottom', () => {
		const target = createContainer({ scrollTop: 600, scrollHeight: 1000, clientHeight: 400 });

		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			null,
			false,
			0,
			MESSAGE_SELECTOR,
			32,
			{ userInitiated: true, preservePinnedIntent: true }
		);

		expect(scrollState.pinned).toBe(true);
		expect(scrollState.pendingSnapshot).toBeNull();
		expect(scrollState.unreadMessageCount).toBe(0);
		expect(scrollState.deferred).toBe(false);
	});

	it('detects paused user scroll movement back to bottom', () => {
		const target = createContainer({ scrollTop: 300, scrollHeight: 1000, clientHeight: 400 });
		const intent = { scrollTop: target.getScrollTop(), direction: 'down' as const };

		target.container.scrollTop = 600;
		const userInitiated = isUserScrollMovement(target.getScrollTop(), intent);
		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			null,
			false,
			2,
			MESSAGE_SELECTOR,
			32,
			{ userInitiated, preservePinnedIntent: false }
		);

		expect(userInitiated).toBe(true);
		expect(scrollState.pinned).toBe(true);
		expect(scrollState.unreadMessageCount).toBe(0);
	});

	it('does not promote an already paused viewport during layout reflow', () => {
		const target = createContainer({ scrollTop: 300, scrollHeight: 1000, clientHeight: 400 });

		target.setScrollHeight(1100);
		const scrollState = refreshScrollStateAfterScroll(
			target.container,
			null,
			false,
			2,
			MESSAGE_SELECTOR,
			32,
			{ preservePinnedIntent: false }
		);

		expect(scrollState.pinned).toBe(false);
		expect(scrollState.pendingSnapshot).toBeNull();
		expect(scrollState.unreadMessageCount).toBe(2);
		expect(scrollState.deferred).toBe(false);
	});
});
