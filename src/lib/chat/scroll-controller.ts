import {
	beginManualScrollInteraction,
	captureScrollSnapshot,
	getPinnedBatchScrollSnapshot,
	isAtBottom,
	isManualScrollInteractionActive,
	releaseManualScrollInteraction,
	refreshScrollStateAfterScroll,
	restoreScrollAfterRender,
	scrollToBottom as scrollElementToBottom,
	shouldOwnManualScroll,
	type ManualScrollInteraction,
	type ManualScrollSource,
	type ScrollIntentDirection,
	type ScrollSnapshot
} from './autoscroll';

const MANUAL_SCROLL_SETTLE_MS = 250;
const PAUSED_REFLOW_SETTLE_MS = 250;
const SCROLL_RESTORE_FALLBACK_MS = 50;

interface ChatScrollOptions {
	messageSelector: string;
	afterRender: () => Promise<void>;
	getThresholdPx: () => number;
	onPinnedChange: (pinned: boolean) => void;
	onClearUnread: () => void;
	onHistoryUpdate: () => void;
	onResume: () => void;
}

export class ChatScrollController {
	private viewport: HTMLElement | undefined;
	private isPinned = true;
	private disposed = false;
	private resumingToBottom = false;
	private restoreGeneration = 0;
	private pinnedRenderPending = false;
	private pendingScrollSnapshot: ScrollSnapshot | null = null;
	private pausedReflowSnapshot: ScrollSnapshot | null = null;
	private scrollFlushQueued = false;
	private scrollFrame: number | undefined;
	private scrollFallbackTimer: ReturnType<typeof setTimeout> | undefined;
	private pinnedBottomFrame: number | undefined;
	private pinnedBottomTimer: ReturnType<typeof setTimeout> | undefined;
	private pausedReflowFrame: number | undefined;
	private pausedReflowTimer: ReturnType<typeof setTimeout> | undefined;
	private resizeObserver: ResizeObserver | undefined;
	private manualScrollInteraction: ManualScrollInteraction | null = null;
	private manualScrollSnapshot: ScrollSnapshot | null = null;
	private manualScrollTimer: ReturnType<typeof setTimeout> | undefined;
	private touchStartY: number | undefined;

	constructor(private options: ChatScrollOptions) {}

	get pinned(): boolean {
		return this.isPinned;
	}

	get interacting(): boolean {
		return this.manualScrollInteraction !== null;
	}

	attach(viewport: HTMLElement | undefined, content: HTMLElement | undefined): void {
		if (this.disposed) return;
		this.viewport = viewport;
		if (content && typeof ResizeObserver !== 'undefined') {
			this.resizeObserver = new ResizeObserver(this.queueResizeScrollRestore);
			this.resizeObserver.observe(content);
			if (viewport) this.resizeObserver.observe(viewport);
		}
		document.addEventListener('visibilitychange', this.handleViewportWake);
		window.addEventListener('resize', this.handleViewportWake);
		window.addEventListener('pointercancel', this.handleScrollbarPointerEnd);
		window.addEventListener('pointerup', this.handleScrollbarPointerEnd);
	}

	beforeContentChange(kind: 'append' | 'trim' | 'translation' | 'username'): boolean {
		const shouldRestore =
			!this.disposed &&
			!this.interacting &&
			(kind === 'translation' || kind === 'username' || !this.resumingToBottom) &&
			(kind !== 'trim' || this.viewport !== undefined);
		if (shouldRestore && this.viewport && !this.pinned) {
			this.pendingScrollSnapshot = getPinnedBatchScrollSnapshot(
				this.pendingScrollSnapshot,
				this.viewport,
				this.options.messageSelector,
				false,
				this.options.getThresholdPx()
			);
		}
		return shouldRestore;
	}

	afterContentChange(): void {
		if (this.pinned) void this.queuePinnedBottomAfterRender();
		else this.queueScrollRestore();
	}

	beforeReset(): boolean {
		this.cancelQueuedScrollRestore();
		this.cancelPinnedBottomFlush();
		if (!this.disposed && this.viewport && !this.pinned) {
			this.pendingScrollSnapshot = captureScrollSnapshot(
				this.viewport,
				this.options.messageSelector,
				this.options.getThresholdPx()
			);
		}
		return this.pinned;
	}

	afterReset(wasPinned: boolean): void {
		if (this.disposed) return;
		if (wasPinned) this.resumeToBottom();
		else this.queueScrollRestore();
	}

	dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.resizeObserver?.disconnect();
		document.removeEventListener('visibilitychange', this.handleViewportWake);
		window.removeEventListener('resize', this.handleViewportWake);
		window.removeEventListener('pointercancel', this.handleScrollbarPointerEnd);
		window.removeEventListener('pointerup', this.handleScrollbarPointerEnd);
		this.cancelQueuedScrollRestore();
		this.cancelPinnedBottomFlush();
		this.cancelManualScrollInteraction();
		this.viewport = undefined;
	}

	private setPinned(pinned: boolean): void {
		this.isPinned = pinned;
		this.options.onPinnedChange(pinned);
	}

	refreshScrollState = () => {
		if (!this.viewport) return;
		if (this.interacting) {
			this.refreshManualScrollState();
			return;
		}

		const wasPinned = this.pinned;
		const hadQueuedRestore = this.scrollFlushQueued && this.pendingScrollSnapshot !== null;
		const scrollState = refreshScrollStateAfterScroll(
			this.viewport,
			this.pendingScrollSnapshot,
			this.scrollFlushQueued,
			0,
			this.options.messageSelector,
			this.options.getThresholdPx(),
			{ preservePinnedIntent: wasPinned }
		);

		this.setPinned(scrollState.pinned);
		this.pendingScrollSnapshot = scrollState.pendingSnapshot;
		if (scrollState.pinned) this.options.onClearUnread();
		if (wasPinned && scrollState.deferred && !hadQueuedRestore) this.queuePinnedScrollToBottom();
	};

	private captureManualScrollSnapshot = () => {
		if (!this.viewport) return null;

		return {
			...captureScrollSnapshot(
				this.viewport,
				this.options.messageSelector,
				this.options.getThresholdPx()
			),
			wasAtBottom: false
		};
	};

	private beginUserScrollInteraction = (
		source: ManualScrollSource,
		direction: ScrollIntentDirection
	) => {
		if (!this.viewport || !shouldOwnManualScroll(this.pinned, source, direction)) return;

		this.manualScrollInteraction = beginManualScrollInteraction(
			this.manualScrollInteraction,
			source,
			direction,
			performance.now(),
			MANUAL_SCROLL_SETTLE_MS
		);
		this.manualScrollSnapshot ??= this.captureManualScrollSnapshot();
		this.setPinned(false);
		this.pendingScrollSnapshot = null;
		this.cancelQueuedScrollRestore();
		this.cancelPinnedBottomFlush();
		this.clearPausedReflowSnapshot();
		this.options.onHistoryUpdate();
		this.scheduleManualScrollSettle();
	};

	private refreshManualScrollState = () => {
		if (!this.viewport || !this.manualScrollInteraction) return;

		const previousScrollTop = this.manualScrollSnapshot?.scrollTop ?? this.viewport.scrollTop;
		const moved = this.viewport.scrollTop !== previousScrollTop;
		const movedDown = this.viewport.scrollTop > previousScrollTop;
		if (moved) this.manualScrollInteraction = { ...this.manualScrollInteraction, moved: true };
		if (
			isAtBottom(this.viewport, 0) &&
			!this.manualScrollInteraction.scrollbarHeld &&
			(this.manualScrollInteraction.direction === 'down' || movedDown)
		) {
			this.finishManualScrollInteraction(true);
			return;
		}

		this.setPinned(false);
		this.pendingScrollSnapshot = null;
		this.manualScrollSnapshot = this.captureManualScrollSnapshot();
	};

	handleScrollbarPointerIntent = (event: PointerEvent) => {
		if (!this.viewport || !this.isScrollbarPointerEvent(this.viewport, event)) return;

		this.beginUserScrollInteraction('scrollbar', 'unknown');
	};

	private handleScrollbarPointerEnd = () => {
		if (!this.manualScrollInteraction?.scrollbarHeld) return;
		const pinToBottom =
			this.viewport !== undefined &&
			this.manualScrollInteraction.moved &&
			isAtBottom(this.viewport, 0);

		this.manualScrollInteraction = releaseManualScrollInteraction(
			this.manualScrollInteraction,
			performance.now(),
			MANUAL_SCROLL_SETTLE_MS
		);
		if (pinToBottom) {
			this.finishManualScrollInteraction(true);
			return;
		}
		this.scheduleManualScrollSettle();
	};

	handleWheelIntent = (event: WheelEvent) => {
		const direction = this.scrollDirectionFromWheel(event);
		if (direction !== 'unknown') this.beginUserScrollInteraction('wheel', direction);
	};

	handleTouchStartIntent = (event: TouchEvent) => {
		this.touchStartY = event.touches[0]?.clientY;
	};

	handleTouchMoveIntent = (event: TouchEvent) => {
		if (this.touchStartY === undefined) return;

		const touchY = event.touches[0]?.clientY;
		if (touchY === undefined) return;

		const deltaY = touchY - this.touchStartY;
		if (Math.abs(deltaY) < 1) return;

		this.touchStartY = touchY;
		this.beginUserScrollInteraction('touch', deltaY > 0 ? 'up' : 'down');
	};

	handleTouchEndIntent = () => {
		this.touchStartY = undefined;
	};

	private scrollDirectionFromWheel = (event: WheelEvent): ScrollIntentDirection => {
		if (event.deltaY < 0) return 'up';
		if (event.deltaY > 0) return 'down';
		return 'unknown';
	};

	private scheduleManualScrollSettle = () => {
		if (this.manualScrollTimer !== undefined) clearTimeout(this.manualScrollTimer);
		this.manualScrollTimer = undefined;
		if (!this.manualScrollInteraction || this.manualScrollInteraction.scrollbarHeld) return;

		const remaining = Math.max(0, this.manualScrollInteraction.activeUntil - performance.now());
		this.manualScrollTimer = setTimeout(this.settleManualScrollInteraction, remaining + 1);
	};

	private settleManualScrollInteraction = () => {
		this.manualScrollTimer = undefined;
		if (!this.manualScrollInteraction) return;
		if (isManualScrollInteractionActive(this.manualScrollInteraction, performance.now())) {
			this.scheduleManualScrollSettle();
			return;
		}

		this.finishManualScrollInteraction(false);
	};

	private finishManualScrollInteraction = (pinToBottom: boolean) => {
		if (this.manualScrollTimer !== undefined) clearTimeout(this.manualScrollTimer);
		this.manualScrollTimer = undefined;
		this.manualScrollInteraction = null;

		const snapshot = this.manualScrollSnapshot;
		this.manualScrollSnapshot = null;
		this.pendingScrollSnapshot = null;
		if (!this.viewport) return;

		if (pinToBottom) {
			this.resumeToBottom();
			return;
		}

		if (snapshot) {
			restoreScrollAfterRender(
				this.viewport,
				snapshot,
				this.options.messageSelector,
				this.options.getThresholdPx()
			);
		}
		this.setPinned(false);
		this.rememberPausedReflowSnapshot();
		this.options.onHistoryUpdate();
	};

	private cancelManualScrollInteraction = () => {
		if (this.manualScrollTimer !== undefined) clearTimeout(this.manualScrollTimer);
		this.manualScrollTimer = undefined;
		this.manualScrollInteraction = null;
		this.manualScrollSnapshot = null;
	};

	private queueScrollRestore = () => {
		if (this.disposed || this.scrollFlushQueued || this.interacting) return;

		this.scrollFlushQueued = true;
		void this.restoreQueuedScroll();
	};

	private restoreQueuedScroll = async () => {
		const generation = this.restoreGeneration;
		await this.options.afterRender();
		if (generation !== this.restoreGeneration) return;
		if (this.disposed || this.interacting) {
			this.scrollFlushQueued = false;
			this.pendingScrollSnapshot = null;
			return;
		}

		this.scheduleQueuedScrollFlush();
	};

	private scheduleQueuedScrollFlush = () => {
		if (this.interacting) return;
		if (this.scrollFrame === undefined) {
			this.scrollFrame = requestAnimationFrame(this.flushQueuedScrollRestore);
		}
		if (this.scrollFallbackTimer === undefined) {
			this.scrollFallbackTimer = setTimeout(
				this.flushQueuedScrollRestore,
				SCROLL_RESTORE_FALLBACK_MS
			);
		}
	};

	private flushQueuedScrollRestore = () => {
		if (this.scrollFrame !== undefined) {
			cancelAnimationFrame(this.scrollFrame);
			this.scrollFrame = undefined;
		}
		if (this.scrollFallbackTimer !== undefined) {
			clearTimeout(this.scrollFallbackTimer);
			this.scrollFallbackTimer = undefined;
		}

		if (this.disposed || this.interacting) {
			this.scrollFlushQueued = false;
			this.pendingScrollSnapshot = null;
			return;
		}

		this.scrollFlushQueued = false;

		const snapshot = this.pendingScrollSnapshot;
		this.pendingScrollSnapshot = null;
		if (!this.viewport || !snapshot) return;

		const result = restoreScrollAfterRender(
			this.viewport,
			snapshot,
			this.options.messageSelector,
			this.options.getThresholdPx()
		);
		this.setPinned(snapshot.wasAtBottom && result.pinned);
		if (this.pinned) {
			this.options.onClearUnread();
			this.clearPausedReflowSnapshot();
		} else {
			this.rememberPausedReflowSnapshot();
		}
	};

	private cancelQueuedScrollRestore = () => {
		this.restoreGeneration += 1;
		this.pinnedRenderPending = false;
		if (this.scrollFrame !== undefined) {
			cancelAnimationFrame(this.scrollFrame);
			this.scrollFrame = undefined;
		}
		if (this.scrollFallbackTimer !== undefined) {
			clearTimeout(this.scrollFallbackTimer);
			this.scrollFallbackTimer = undefined;
		}

		this.scrollFlushQueued = false;
		this.pendingScrollSnapshot = null;
		this.clearPausedReflowSnapshot();
	};

	private applyPinnedBottomState = () => {
		if (!this.viewport) return;

		scrollElementToBottom(this.viewport);
		this.setPinned(true);
		this.options.onClearUnread();
		this.clearPausedReflowSnapshot();
	};

	resumeToBottom = () => {
		if (this.disposed) return;
		this.cancelManualScrollInteraction();
		this.cancelQueuedScrollRestore();
		this.cancelPinnedBottomFlush();

		this.setPinned(true);
		this.resumingToBottom = true;
		try {
			this.options.onResume();
		} finally {
			this.resumingToBottom = false;
		}
		this.applyPinnedBottomState();
		void this.queuePinnedBottomAfterRender();
	};

	private queuePinnedBottomAfterRender = async () => {
		if (this.disposed || !this.pinned || this.interacting || this.pinnedRenderPending) return;
		this.cancelPinnedBottomFlush();
		this.pinnedRenderPending = true;
		const generation = this.restoreGeneration;
		await this.options.afterRender();
		if (generation !== this.restoreGeneration) return;
		this.pinnedRenderPending = false;
		if (this.disposed || !this.pinned || this.interacting) return;

		this.queuePinnedScrollToBottom();
	};

	private queuePinnedScrollToBottom = () => {
		if (
			this.disposed ||
			!this.pinned ||
			!this.viewport ||
			this.interacting ||
			this.pinnedRenderPending
		)
			return;

		if (!this.resizeObserver && this.pinnedBottomFrame === undefined) {
			this.pinnedBottomFrame = requestAnimationFrame(this.flushPinnedBottomScroll);
		}
		if (this.pinnedBottomTimer === undefined) {
			this.pinnedBottomTimer = setTimeout(this.flushPinnedBottomScroll, SCROLL_RESTORE_FALLBACK_MS);
		}
	};

	private flushPinnedBottomScroll = () => {
		this.cancelPinnedBottomFlush();
		if (this.disposed || !this.pinned || !this.viewport || this.interacting) return;

		this.applyPinnedBottomState();
	};

	private cancelPinnedBottomFlush = () => {
		if (this.pinnedBottomFrame !== undefined) {
			cancelAnimationFrame(this.pinnedBottomFrame);
			this.pinnedBottomFrame = undefined;
		}
		if (this.pinnedBottomTimer !== undefined) {
			clearTimeout(this.pinnedBottomTimer);
			this.pinnedBottomTimer = undefined;
		}
	};

	private handleViewportWake = () => {
		if (this.disposed || !this.pinned || this.interacting) return;
		if (this.pendingScrollSnapshot) {
			if (!this.scrollFlushQueued) this.queueScrollRestore();
			return;
		}

		this.queuePinnedScrollToBottom();
	};

	private queueResizeScrollRestore = () => {
		if (this.disposed || !this.viewport || this.interacting) return;
		if (this.pinned) {
			// Resize observations arrive after layout.
			this.flushPinnedBottomScroll();
			return;
		}

		if (this.pendingScrollSnapshot) {
			this.queueScrollRestore();
			return;
		}

		this.queuePausedReflowRestore();
	};

	private queuePausedReflowRestore = () => {
		if (
			!this.viewport ||
			!this.pausedReflowSnapshot ||
			this.pausedReflowFrame !== undefined ||
			this.interacting
		)
			return;

		this.pausedReflowFrame = requestAnimationFrame(() => {
			this.pausedReflowFrame = undefined;
			if (!this.viewport || !this.pausedReflowSnapshot || this.pinned || this.interacting) return;

			const snapshot = this.pausedReflowSnapshot;
			const result = restoreScrollAfterRender(
				this.viewport,
				snapshot,
				this.options.messageSelector,
				this.options.getThresholdPx()
			);
			this.setPinned(snapshot.wasAtBottom && result.pinned);
			if (this.pinned) {
				this.options.onClearUnread();
				this.clearPausedReflowSnapshot();
				return;
			}

			this.rememberPausedReflowSnapshot();
		});
	};

	private rememberPausedReflowSnapshot = () => {
		if (!this.viewport) return;

		const snapshot = captureScrollSnapshot(
			this.viewport,
			this.options.messageSelector,
			this.options.getThresholdPx()
		);
		this.pausedReflowSnapshot = this.pinned ? snapshot : { ...snapshot, wasAtBottom: false };
		if (this.pausedReflowTimer !== undefined) clearTimeout(this.pausedReflowTimer);
		this.pausedReflowTimer = setTimeout(() => {
			this.pausedReflowTimer = undefined;
			this.pausedReflowSnapshot = null;
		}, PAUSED_REFLOW_SETTLE_MS);
	};

	private clearPausedReflowSnapshot = () => {
		this.pausedReflowSnapshot = null;
		if (this.pausedReflowFrame !== undefined) {
			cancelAnimationFrame(this.pausedReflowFrame);
			this.pausedReflowFrame = undefined;
		}
		if (this.pausedReflowTimer !== undefined) {
			clearTimeout(this.pausedReflowTimer);
			this.pausedReflowTimer = undefined;
		}
	};

	private isScrollbarPointerEvent = (container: HTMLElement, event: PointerEvent) => {
		const rect = container.getBoundingClientRect();
		const verticalScrollbarWidth = container.offsetWidth - container.clientWidth;
		const horizontalScrollbarHeight = container.offsetHeight - container.clientHeight;

		return (
			(verticalScrollbarWidth > 0 && event.clientX >= rect.right - verticalScrollbarWidth) ||
			(horizontalScrollbarHeight > 0 && event.clientY >= rect.bottom - horizontalScrollbarHeight)
		);
	};
}
