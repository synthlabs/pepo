export const DEFAULT_BOTTOM_THRESHOLD = 32;

export interface ScrollAnchor {
	key: string;
	topOffset: number;
}

export interface ScrollSnapshot {
	wasAtBottom: boolean;
	scrollTop: number;
	scrollHeight: number;
	clientHeight: number;
	distanceFromBottom: number;
	anchor: ScrollAnchor | null;
}

export interface ScrollRestoreResult {
	pinned: boolean;
	anchored: boolean;
}

export interface ScrollStateRefreshResult {
	pinned: boolean;
	pendingSnapshot: ScrollSnapshot | null;
	unreadMessageCount: number;
	deferred: boolean;
}

export interface ScrollStateRefreshOptions {
	userInitiated?: boolean;
	preservePinnedIntent?: boolean;
}

export type ScrollIntentDirection = 'up' | 'down' | 'unknown';

export interface ScrollIntentSnapshot {
	scrollTop: number;
	direction: ScrollIntentDirection;
}

export function maxScrollTop(container: HTMLElement): number {
	return Math.max(0, container.scrollHeight - container.clientHeight);
}

export function distanceFromBottom(container: HTMLElement): number {
	return Math.max(0, maxScrollTop(container) - container.scrollTop);
}

export function isAtBottom(
	container: HTMLElement,
	thresholdPx = DEFAULT_BOTTOM_THRESHOLD
): boolean {
	return distanceFromBottom(container) <= thresholdPx;
}

export function scrollToBottom(container: HTMLElement): void {
	container.scrollTop = maxScrollTop(container);
}

export function captureScrollSnapshot(
	container: HTMLElement,
	messageSelector: string,
	thresholdPx = DEFAULT_BOTTOM_THRESHOLD
): ScrollSnapshot {
	return {
		wasAtBottom: isAtBottom(container, thresholdPx),
		scrollTop: container.scrollTop,
		scrollHeight: container.scrollHeight,
		clientHeight: container.clientHeight,
		distanceFromBottom: distanceFromBottom(container),
		anchor: findVisibleAnchor(container, messageSelector)
	};
}

export function getBatchScrollSnapshot(
	current: ScrollSnapshot | null,
	container: HTMLElement,
	messageSelector: string,
	thresholdPx = DEFAULT_BOTTOM_THRESHOLD
): ScrollSnapshot {
	return current ?? captureScrollSnapshot(container, messageSelector, thresholdPx);
}

export function getPinnedBatchScrollSnapshot(
	current: ScrollSnapshot | null,
	container: HTMLElement,
	messageSelector: string,
	preservePinnedIntent: boolean,
	thresholdPx = DEFAULT_BOTTOM_THRESHOLD
): ScrollSnapshot {
	const snapshot = getBatchScrollSnapshot(current, container, messageSelector, thresholdPx);
	return preservePinnedIntent && !snapshot.wasAtBottom ? { ...snapshot, wasAtBottom: true } : snapshot;
}

export function isUserScrollMovement(
	currentScrollTop: number,
	intent: ScrollIntentSnapshot | null,
	epsilonPx = 0
): boolean {
	if (!intent) return false;

	if (intent.direction === 'up') return currentScrollTop < intent.scrollTop - epsilonPx;
	if (intent.direction === 'down') return currentScrollTop > intent.scrollTop + epsilonPx;
	return Math.abs(currentScrollTop - intent.scrollTop) > epsilonPx;
}

export function isUserScrollPauseIntent(
	currentScrollTop: number,
	intent: ScrollIntentSnapshot | null,
	epsilonPx = 0
): boolean {
	if (!intent || intent.direction === 'down') return false;
	return currentScrollTop < intent.scrollTop - epsilonPx;
}

export function restoreScrollAfterRender(
	container: HTMLElement,
	snapshot: ScrollSnapshot,
	messageSelector: string,
	thresholdPx = DEFAULT_BOTTOM_THRESHOLD
): ScrollRestoreResult {
	if (snapshot.wasAtBottom) {
		scrollToBottom(container);
		return { pinned: true, anchored: false };
	}

	if (snapshot.anchor) {
		const anchor = findAnchorByKey(container, messageSelector, snapshot.anchor.key);
		if (anchor) {
			const containerTop = container.getBoundingClientRect().top;
			const currentTopOffset = anchor.getBoundingClientRect().top - containerTop;
			setScrollTopClamped(container, container.scrollTop + currentTopOffset - snapshot.anchor.topOffset);

			return { pinned: isAtBottom(container, thresholdPx), anchored: true };
		}
	}

	setScrollTopClamped(container, snapshot.scrollTop);
	return { pinned: isAtBottom(container, thresholdPx), anchored: false };
}

export function refreshScrollStateAfterScroll(
	container: HTMLElement,
	pendingSnapshot: ScrollSnapshot | null,
	restoreQueued: boolean,
	unreadMessageCount: number,
	messageSelector: string,
	thresholdPx = DEFAULT_BOTTOM_THRESHOLD,
	options: ScrollStateRefreshOptions = {}
): ScrollStateRefreshResult {
	if (restoreQueued && pendingSnapshot && !options.userInitiated) {
		return {
			pinned: pendingSnapshot.wasAtBottom,
			pendingSnapshot,
			unreadMessageCount: pendingSnapshot.wasAtBottom ? 0 : unreadMessageCount,
			deferred: true
		};
	}

	if (options.preservePinnedIntent && !options.userInitiated) {
		return {
			pinned: true,
			pendingSnapshot,
			unreadMessageCount: 0,
			deferred: true
		};
	}

	const pinned =
		options.userInitiated && options.preservePinnedIntent
			? distanceFromBottom(container) === 0
			: isAtBottom(container, thresholdPx);
	if (pinned) {
		return {
			pinned: true,
			pendingSnapshot: null,
			unreadMessageCount: 0,
			deferred: false
		};
	}

	return {
		pinned: false,
		pendingSnapshot: pendingSnapshot
			? captureScrollSnapshot(container, messageSelector, thresholdPx)
			: pendingSnapshot,
		unreadMessageCount,
		deferred: false
	};
}

function setScrollTopClamped(container: HTMLElement, scrollTop: number): void {
	container.scrollTop = Math.max(0, Math.min(scrollTop, maxScrollTop(container)));
}

function findVisibleAnchor(container: HTMLElement, messageSelector: string): ScrollAnchor | null {
	const containerRect = container.getBoundingClientRect();
	const messages = Array.from(container.querySelectorAll<HTMLElement>(messageSelector));

	for (const message of messages) {
		const key = message.dataset.chatMessageIndex;
		if (!key) continue;

		const rect = message.getBoundingClientRect();
		if (rect.bottom > containerRect.top && rect.top < containerRect.bottom) {
			return {
				key,
				topOffset: rect.top - containerRect.top
			};
		}
	}

	return null;
}

function findAnchorByKey(
	container: HTMLElement,
	messageSelector: string,
	key: string
): HTMLElement | null {
	const messages = Array.from(container.querySelectorAll<HTMLElement>(messageSelector));
	return messages.find((message) => message.dataset.chatMessageIndex === key) ?? null;
}
