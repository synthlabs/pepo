<script lang="ts">
	import { fade, slide } from 'svelte/transition';
	import { quadInOut } from 'svelte/easing';
	import HistoryDivider from '$lib/components/chat/history-divider.svelte';
	import { openRoomFeed } from '$lib/chat/feed';
	import { showDividerAfter, type HistoryBoundary } from '$lib/chat/history-boundary';
	import { onDestroy, onMount, tick } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		commands,
		type ChatRoomInfo,
		type ChannelMessage,
		type ChatTranslationLayout,
		type ChannelMessageTranslationUpdate
	} from '$lib/bindings.ts';
	import { cn } from '$lib/utils';
	import { page } from '$app/state';
	import Badges from '$lib/components/chat/+badges.svelte';
	import Logger from '$utils/log';
	import Emote from '$lib/components/chat/+emote.svelte';
	import EmotePicker from '$lib/components/chat/+emote-picker.svelte';
	import Translation from '$lib/components/chat/+translation.svelte';
	import { Button } from '$lib/components/ui/button';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import Smile from '@lucide/svelte/icons/smile';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import type { Emote as EmoteType } from '$lib/bindings.ts';
	import { parseColonMacro } from '$lib/chat/colon-macro';
	import { captureChatSendError } from '$lib/chat/send-errors';
	import { MessageHistoryScheduler } from '$lib/chat/message-history';
	import { measureHistoryOriginSpacer } from '$lib/chat/history-origin';
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
	} from '$lib/chat/autoscroll';
	import {
		applyTranslationUpdate,
		attachPendingTranslation,
		type PendingTranslations
	} from '$lib/chat/translation';
	import {
		chatBadgePlaceholderWidth,
		translationHasBadgePlaceholder,
		translationHasTimestampPlaceholder
	} from '$lib/chat/message-layout';
	import { formatTimestamp } from '$lib/settings';
	import { authState } from '$lib/stores/auth.svelte';
	import { getNormalizedAppSettings } from '$lib/stores/settings.svelte';

	const CHAT_MESSAGE_SELECTOR = '[data-chat-message-index]';
	const MANUAL_SCROLL_SETTLE_MS = 250;
	const PAUSED_REFLOW_SETTLE_MS = 250;
	const SCROLL_RESTORE_FALLBACK_MS = 50;
	const MAX_MESSAGE_LENGTH = 500;
	const COUNTER_THRESHOLD = 400;

	let chatDIV = $state<HTMLDivElement>();
	let messageListDIV = $state<HTMLDivElement>();
	let historySpacerDIV = $state<HTMLDivElement>();
	let historySpacerHeight = $state(0);
	let autoScrollPinned = $state(true);
	let unreadMessageCount = $state(0);
	let showJumpToBottom = $derived(!autoScrollPinned);
	let jumpToBottomText = $derived(
		unreadMessageCount > 0
			? `chat paused: ${unreadMessageCount} new message${unreadMessageCount === 1 ? '' : 's'}`
			: 'chat paused'
	);
	let jumpToBottomLabel = $derived(
		unreadMessageCount > 0
			? `Jump to ${unreadMessageCount} new message${unreadMessageCount === 1 ? '' : 's'} below`
			: 'Jump to more messages below'
	);
	let historyBoundary: HistoryBoundary | null = $state(null);
	let initialHistoryLoaded = false;
	let channel_name: string = $derived(page.params.id ?? '');
	let msgs: ChannelMessage[] = $state([]);
	let chatInput = $state('');
	let hasInput = $derived(chatInput.length > 0);
	let showCounter = $derived(chatInput.length >= COUNTER_THRESHOLD);
	let errorState = $state({ active: false, msg: '' });
	let sendPending = $state(false);
	let messageErrorTimer: ReturnType<typeof setTimeout> | undefined;
	let channelInfo = $state({} as ChatRoomInfo);
	let username = $derived(
		authState.obj.phase === 'authorized' && authState.obj.token ? authState.obj.token.login : null
	);
	let normalizedAppSettings = $derived(getNormalizedAppSettings());
	let chatSettings = $derived(normalizedAppSettings.chat);
	let emoteSettings = $derived(normalizedAppSettings.emotes);

	// Emote picker state
	let emotePickerVisible = $state(false);
	let emoteResults: EmoteType[] = $state([]);
	let selectedEmoteIndex = $state(0);
	let pickerOpenedByButton = $state(false);
	let dismissedQuery = $state('');
	let emoteSearchQuery = $state('');
	let searchDebounceTimer: ReturnType<typeof setTimeout> | undefined;

	const pendingTranslations: PendingTranslations = new SvelteMap();
	let roomFeed: ReturnType<typeof openRoomFeed> | undefined;
	let pendingScrollSnapshot: ScrollSnapshot | null = null;
	let pausedReflowSnapshot: ScrollSnapshot | null = null;
	let scrollFlushQueued = false;
	let scrollFrame: number | undefined;
	let scrollFallbackTimer: ReturnType<typeof setTimeout> | undefined;
	let pinnedBottomFrame: number | undefined;
	let pinnedBottomTimer: ReturnType<typeof setTimeout> | undefined;
	let pausedReflowFrame: number | undefined;
	let pausedReflowTimer: ReturnType<typeof setTimeout> | undefined;
	let resizeObserver: ResizeObserver | undefined;
	let manualScrollInteraction: ManualScrollInteraction | null = null;
	let manualScrollSnapshot: ScrollSnapshot | null = null;
	let manualScrollTimer: ReturnType<typeof setTimeout> | undefined;
	let touchStartY: number | undefined;
	let destroyed = false;
	let resumingToBottom = false;
	const messageHistory = new MessageHistoryScheduler<ChannelMessage>({
		getState: () => ({
			...chatSettings,
			paused: !autoScrollPinned,
			interacting: manualScrollOwnsViewport(),
			message_count: msgs.length
		}),
		append: (messages) => appendMessages(messages),
		trim: (count) => trimMessageHistory(count)
	});

	onMount(async () => {
		if (messageListDIV && typeof ResizeObserver !== 'undefined') {
			resizeObserver = new ResizeObserver(() => {
				queueResizeScrollRestore();
			});
			resizeObserver.observe(messageListDIV);
		}

		document.addEventListener('visibilitychange', handleViewportWake);
		window.addEventListener('resize', handleViewportWake);
		window.addEventListener('pointercancel', handleScrollbarPointerEnd);
		window.addEventListener('pointerup', handleScrollbarPointerEnd);

		roomFeed = openRoomFeed(channel_name, (frame) => {
			switch (frame.event.type) {
				case 'snapshot':
				case 'reset': {
					messageHistory.reset();
					pendingTranslations.clear();
					if (!initialHistoryLoaded) {
						historyBoundary = { cursor: frame.cursor, hasHistory: frame.event.messages.length > 0 };
						initialHistoryLoaded = true;
					}
					const wasPinned = autoScrollPinned;
					if (chatDIV && !wasPinned) {
						pendingScrollSnapshot = captureScrollSnapshot(
							chatDIV,
							CHAT_MESSAGE_SELECTOR,
							chatSettings.autoscroll_threshold_px
						);
					}
					msgs = frame.event.messages;
					if (wasPinned) pinToBottomNowAndAfterRender();
					else queueScrollRestore();
					break;
				}
				case 'message':
					addMessage(frame.event.message);
					break;
				case 'translation':
					applyTranslation(frame.event.update);
					break;
				case 'unavailable':
					showMessageError(frame.event.error);
					break;
			}
		});
		try {
			const info = await roomFeed.ready;
			if (!destroyed) channelInfo = info;
		} catch (error) {
			if (!destroyed) showMessageError(String(error));
		}
	});

	onDestroy(() => {
		destroyed = true;
		clearTimeout(messageErrorTimer);
		messageHistory.dispose();
		resizeObserver?.disconnect();
		document.removeEventListener('visibilitychange', handleViewportWake);
		window.removeEventListener('resize', handleViewportWake);
		window.removeEventListener('pointercancel', handleScrollbarPointerEnd);
		window.removeEventListener('pointerup', handleScrollbarPointerEnd);
		if (scrollFrame !== undefined) cancelAnimationFrame(scrollFrame);
		if (scrollFallbackTimer !== undefined) clearTimeout(scrollFallbackTimer);
		if (pinnedBottomFrame !== undefined) cancelAnimationFrame(pinnedBottomFrame);
		if (pinnedBottomTimer !== undefined) clearTimeout(pinnedBottomTimer);
		if (pausedReflowFrame !== undefined) cancelAnimationFrame(pausedReflowFrame);
		if (pausedReflowTimer !== undefined) clearTimeout(pausedReflowTimer);
		if (manualScrollTimer !== undefined) clearTimeout(manualScrollTimer);

		roomFeed?.dispose();
	});

	$effect(() => {
		messageHistory.update();
	});

	const addMessage = (message: ChannelMessage) => {
		if (!autoScrollPinned || manualScrollOwnsViewport()) unreadMessageCount += 1;
		messageHistory.enqueue(message);
	};

	const appendMessages = (messages: ChannelMessage[]) => {
		const manualScrollActive = manualScrollOwnsViewport();
		if (chatDIV && !manualScrollActive && !resumingToBottom) {
			pendingScrollSnapshot = getPinnedBatchScrollSnapshot(
				pendingScrollSnapshot,
				chatDIV,
				CHAT_MESSAGE_SELECTOR,
				autoScrollPinned,
				chatSettings.autoscroll_threshold_px
			);
		}

		msgs.push(...messages.map((message) => attachPendingTranslation(message, pendingTranslations)));

		if (!manualScrollActive && !resumingToBottom) queueScrollRestore();
	};

	const trimMessageHistory = (count: number) => {
		const container = chatDIV;
		const shouldRestore = container && !manualScrollOwnsViewport() && !resumingToBottom;
		if (shouldRestore) {
			pendingScrollSnapshot = getPinnedBatchScrollSnapshot(
				pendingScrollSnapshot,
				container,
				CHAT_MESSAGE_SELECTOR,
				autoScrollPinned,
				chatSettings.autoscroll_threshold_px
			);
		}
		historySpacerHeight = measureHistoryOriginSpacer(
			historySpacerDIV,
			messageListDIV?.querySelectorAll<HTMLElement>(CHAT_MESSAGE_SELECTOR) ?? [],
			new Set(msgs.slice(0, count).map((message) => String(message.index))),
			window.devicePixelRatio
		);
		msgs.splice(0, count);
		if (shouldRestore) queueScrollRestore();
	};

	const applyTranslation = (update: ChannelMessageTranslationUpdate) => {
		const result = applyTranslationUpdate(msgs, update, pendingTranslations);
		if (!result.changed) return;

		const manualScrollActive = manualScrollOwnsViewport();
		if (chatDIV && !manualScrollActive) {
			pendingScrollSnapshot = getPinnedBatchScrollSnapshot(
				pendingScrollSnapshot,
				chatDIV,
				CHAT_MESSAGE_SELECTOR,
				autoScrollPinned,
				chatSettings.autoscroll_threshold_px
			);
		}

		msgs = result.messages;
		if (!manualScrollActive) queueScrollRestore();
	};

	const refreshScrollState = () => {
		if (!chatDIV) return;
		if (manualScrollOwnsViewport()) {
			refreshManualScrollState();
			return;
		}

		const wasPinned = autoScrollPinned;
		const hadQueuedRestore = scrollFlushQueued && pendingScrollSnapshot !== null;
		const scrollState = refreshScrollStateAfterScroll(
			chatDIV,
			pendingScrollSnapshot,
			scrollFlushQueued,
			unreadMessageCount,
			CHAT_MESSAGE_SELECTOR,
			chatSettings.autoscroll_threshold_px,
			{ preservePinnedIntent: wasPinned }
		);

		autoScrollPinned = scrollState.pinned;
		pendingScrollSnapshot = scrollState.pendingSnapshot;
		unreadMessageCount = scrollState.unreadMessageCount;
		if (wasPinned && scrollState.deferred && !hadQueuedRestore) queuePinnedScrollToBottom();
	};

	const manualScrollOwnsViewport = () => manualScrollInteraction !== null;

	const captureManualScrollSnapshot = () => {
		if (!chatDIV) return null;

		return {
			...captureScrollSnapshot(
				chatDIV,
				CHAT_MESSAGE_SELECTOR,
				chatSettings.autoscroll_threshold_px
			),
			wasAtBottom: false
		};
	};

	const beginUserScrollInteraction = (
		source: ManualScrollSource,
		direction: ScrollIntentDirection
	) => {
		if (!chatDIV || !shouldOwnManualScroll(autoScrollPinned, source, direction)) return;

		manualScrollInteraction = beginManualScrollInteraction(
			manualScrollInteraction,
			source,
			direction,
			performance.now(),
			MANUAL_SCROLL_SETTLE_MS
		);
		manualScrollSnapshot ??= captureManualScrollSnapshot();
		autoScrollPinned = false;
		pendingScrollSnapshot = null;
		cancelQueuedScrollRestore();
		cancelPinnedBottomFlush();
		clearPausedReflowSnapshot();
		messageHistory.update();
		scheduleManualScrollSettle();
	};

	const refreshManualScrollState = () => {
		if (!chatDIV || !manualScrollInteraction) return;

		const previousScrollTop = manualScrollSnapshot?.scrollTop ?? chatDIV.scrollTop;
		const moved = chatDIV.scrollTop !== previousScrollTop;
		const movedDown = chatDIV.scrollTop > previousScrollTop;
		if (moved) manualScrollInteraction = { ...manualScrollInteraction, moved: true };
		if (
			isAtBottom(chatDIV, 0) &&
			!manualScrollInteraction.scrollbarHeld &&
			(manualScrollInteraction.direction === 'down' || movedDown)
		) {
			finishManualScrollInteraction(true);
			return;
		}

		autoScrollPinned = false;
		pendingScrollSnapshot = null;
		manualScrollSnapshot = captureManualScrollSnapshot();
	};

	const handleScrollbarPointerIntent = (event: PointerEvent) => {
		if (!chatDIV || !isScrollbarPointerEvent(chatDIV, event)) return;

		beginUserScrollInteraction('scrollbar', 'unknown');
	};

	const handleScrollbarPointerEnd = () => {
		if (!manualScrollInteraction?.scrollbarHeld) return;
		const pinToBottom =
			chatDIV !== undefined && manualScrollInteraction.moved && isAtBottom(chatDIV, 0);

		manualScrollInteraction = releaseManualScrollInteraction(
			manualScrollInteraction,
			performance.now(),
			MANUAL_SCROLL_SETTLE_MS
		);
		if (pinToBottom) {
			finishManualScrollInteraction(true);
			return;
		}
		scheduleManualScrollSettle();
	};

	const handleWheelIntent = (event: WheelEvent) => {
		const direction = scrollDirectionFromWheel(event);
		if (direction !== 'unknown') beginUserScrollInteraction('wheel', direction);
	};

	const handleTouchStartIntent = (event: TouchEvent) => {
		touchStartY = event.touches[0]?.clientY;
	};

	const handleTouchMoveIntent = (event: TouchEvent) => {
		if (touchStartY === undefined) return;

		const touchY = event.touches[0]?.clientY;
		if (touchY === undefined) return;

		const deltaY = touchY - touchStartY;
		if (Math.abs(deltaY) < 1) return;

		touchStartY = touchY;
		beginUserScrollInteraction('touch', deltaY > 0 ? 'up' : 'down');
	};

	const handleTouchEndIntent = () => {
		touchStartY = undefined;
	};

	const scrollDirectionFromWheel = (event: WheelEvent): ScrollIntentDirection => {
		if (event.deltaY < 0) return 'up';
		if (event.deltaY > 0) return 'down';
		return 'unknown';
	};

	const scheduleManualScrollSettle = () => {
		if (manualScrollTimer !== undefined) clearTimeout(manualScrollTimer);
		manualScrollTimer = undefined;
		if (!manualScrollInteraction || manualScrollInteraction.scrollbarHeld) return;

		const remaining = Math.max(0, manualScrollInteraction.activeUntil - performance.now());
		manualScrollTimer = setTimeout(settleManualScrollInteraction, remaining + 1);
	};

	const settleManualScrollInteraction = () => {
		manualScrollTimer = undefined;
		if (!manualScrollInteraction) return;
		if (isManualScrollInteractionActive(manualScrollInteraction, performance.now())) {
			scheduleManualScrollSettle();
			return;
		}

		finishManualScrollInteraction(false);
	};

	const finishManualScrollInteraction = (pinToBottom: boolean) => {
		if (manualScrollTimer !== undefined) clearTimeout(manualScrollTimer);
		manualScrollTimer = undefined;
		manualScrollInteraction = null;

		const snapshot = manualScrollSnapshot;
		manualScrollSnapshot = null;
		pendingScrollSnapshot = null;
		if (!chatDIV) return;

		if (pinToBottom) {
			pinToBottomNowAndAfterRender();
			return;
		}

		if (snapshot) {
			restoreScrollAfterRender(
				chatDIV,
				snapshot,
				CHAT_MESSAGE_SELECTOR,
				chatSettings.autoscroll_threshold_px
			);
		}
		autoScrollPinned = false;
		rememberPausedReflowSnapshot();
		messageHistory.update();
	};

	const cancelManualScrollInteraction = () => {
		if (manualScrollTimer !== undefined) clearTimeout(manualScrollTimer);
		manualScrollTimer = undefined;
		manualScrollInteraction = null;
		manualScrollSnapshot = null;
	};

	const queueScrollRestore = () => {
		if (scrollFlushQueued || manualScrollOwnsViewport()) return;

		scrollFlushQueued = true;
		void restoreQueuedScroll();
	};

	const restoreQueuedScroll = async () => {
		await tick();
		if (destroyed || manualScrollOwnsViewport()) {
			scrollFlushQueued = false;
			pendingScrollSnapshot = null;
			return;
		}

		scheduleQueuedScrollFlush();
	};

	const scheduleQueuedScrollFlush = () => {
		if (manualScrollOwnsViewport()) return;
		if (scrollFrame === undefined) {
			scrollFrame = requestAnimationFrame(flushQueuedScrollRestore);
		}
		if (scrollFallbackTimer === undefined) {
			scrollFallbackTimer = setTimeout(flushQueuedScrollRestore, SCROLL_RESTORE_FALLBACK_MS);
		}
	};

	const flushQueuedScrollRestore = () => {
		if (scrollFrame !== undefined) {
			cancelAnimationFrame(scrollFrame);
			scrollFrame = undefined;
		}
		if (scrollFallbackTimer !== undefined) {
			clearTimeout(scrollFallbackTimer);
			scrollFallbackTimer = undefined;
		}

		if (destroyed || manualScrollOwnsViewport()) {
			scrollFlushQueued = false;
			pendingScrollSnapshot = null;
			return;
		}

		scrollFlushQueued = false;

		const snapshot = pendingScrollSnapshot;
		pendingScrollSnapshot = null;
		if (!chatDIV || !snapshot) return;

		const result = restoreScrollAfterRender(
			chatDIV,
			snapshot,
			CHAT_MESSAGE_SELECTOR,
			chatSettings.autoscroll_threshold_px
		);
		autoScrollPinned = snapshot.wasAtBottom && result.pinned;
		if (autoScrollPinned) {
			unreadMessageCount = 0;
			clearPausedReflowSnapshot();
		} else {
			rememberPausedReflowSnapshot();
		}
	};

	const cancelQueuedScrollRestore = () => {
		if (scrollFrame !== undefined) {
			cancelAnimationFrame(scrollFrame);
			scrollFrame = undefined;
		}
		if (scrollFallbackTimer !== undefined) {
			clearTimeout(scrollFallbackTimer);
			scrollFallbackTimer = undefined;
		}

		scrollFlushQueued = false;
		pendingScrollSnapshot = null;
		clearPausedReflowSnapshot();
	};

	const applyPinnedBottomState = () => {
		if (!chatDIV) return;

		scrollElementToBottom(chatDIV);
		autoScrollPinned = true;
		unreadMessageCount = 0;
		clearPausedReflowSnapshot();
	};

	const pinToBottomNowAndAfterRender = () => {
		cancelManualScrollInteraction();
		cancelQueuedScrollRestore();
		cancelPinnedBottomFlush();

		autoScrollPinned = true;
		resumingToBottom = true;
		try {
			messageHistory.resume();
		} finally {
			resumingToBottom = false;
		}
		applyPinnedBottomState();
		void queuePinnedBottomAfterRender();
	};

	const queuePinnedBottomAfterRender = async () => {
		await tick();
		if (destroyed || !autoScrollPinned || manualScrollOwnsViewport()) return;

		queuePinnedScrollToBottom();
	};

	const queuePinnedScrollToBottom = () => {
		if (!autoScrollPinned || !chatDIV || manualScrollOwnsViewport()) return;

		if (pinnedBottomFrame === undefined) {
			pinnedBottomFrame = requestAnimationFrame(flushPinnedBottomScroll);
		}
		if (pinnedBottomTimer === undefined) {
			pinnedBottomTimer = setTimeout(flushPinnedBottomScroll, SCROLL_RESTORE_FALLBACK_MS);
		}
	};

	const flushPinnedBottomScroll = () => {
		cancelPinnedBottomFlush();
		if (destroyed || !autoScrollPinned || !chatDIV || manualScrollOwnsViewport()) return;

		applyPinnedBottomState();
	};

	const cancelPinnedBottomFlush = () => {
		if (pinnedBottomFrame !== undefined) {
			cancelAnimationFrame(pinnedBottomFrame);
			pinnedBottomFrame = undefined;
		}
		if (pinnedBottomTimer !== undefined) {
			clearTimeout(pinnedBottomTimer);
			pinnedBottomTimer = undefined;
		}
	};

	const handleViewportWake = () => {
		if (destroyed || !autoScrollPinned || manualScrollOwnsViewport()) return;
		if (pendingScrollSnapshot) {
			if (!scrollFlushQueued) queueScrollRestore();
			return;
		}

		queuePinnedScrollToBottom();
	};

	const queueResizeScrollRestore = () => {
		if (!chatDIV || manualScrollOwnsViewport()) return;
		if (autoScrollPinned) {
			queuePinnedScrollToBottom();
			return;
		}

		if (pendingScrollSnapshot) {
			queueScrollRestore();
			return;
		}

		queuePausedReflowRestore();
	};

	const queuePausedReflowRestore = () => {
		if (
			!chatDIV ||
			!pausedReflowSnapshot ||
			pausedReflowFrame !== undefined ||
			manualScrollOwnsViewport()
		)
			return;

		pausedReflowFrame = requestAnimationFrame(() => {
			pausedReflowFrame = undefined;
			if (!chatDIV || !pausedReflowSnapshot || autoScrollPinned || manualScrollOwnsViewport())
				return;

			const snapshot = pausedReflowSnapshot;
			const result = restoreScrollAfterRender(
				chatDIV,
				snapshot,
				CHAT_MESSAGE_SELECTOR,
				chatSettings.autoscroll_threshold_px
			);
			autoScrollPinned = snapshot.wasAtBottom && result.pinned;
			if (autoScrollPinned) {
				unreadMessageCount = 0;
				clearPausedReflowSnapshot();
				return;
			}

			rememberPausedReflowSnapshot();
		});
	};

	const rememberPausedReflowSnapshot = () => {
		if (!chatDIV) return;

		const snapshot = captureScrollSnapshot(
			chatDIV,
			CHAT_MESSAGE_SELECTOR,
			chatSettings.autoscroll_threshold_px
		);
		pausedReflowSnapshot = autoScrollPinned ? snapshot : { ...snapshot, wasAtBottom: false };
		if (pausedReflowTimer !== undefined) clearTimeout(pausedReflowTimer);
		pausedReflowTimer = setTimeout(() => {
			pausedReflowTimer = undefined;
			pausedReflowSnapshot = null;
		}, PAUSED_REFLOW_SETTLE_MS);
	};

	const clearPausedReflowSnapshot = () => {
		pausedReflowSnapshot = null;
		if (pausedReflowFrame !== undefined) {
			cancelAnimationFrame(pausedReflowFrame);
			pausedReflowFrame = undefined;
		}
		if (pausedReflowTimer !== undefined) {
			clearTimeout(pausedReflowTimer);
			pausedReflowTimer = undefined;
		}
	};

	const isScrollbarPointerEvent = (container: HTMLElement, event: PointerEvent) => {
		const rect = container.getBoundingClientRect();
		const verticalScrollbarWidth = container.offsetWidth - container.clientWidth;
		const horizontalScrollbarHeight = container.offsetHeight - container.clientHeight;

		return (
			(verticalScrollbarWidth > 0 && event.clientX >= rect.right - verticalScrollbarWidth) ||
			(horizontalScrollbarHeight > 0 && event.clientY >= rect.bottom - horizontalScrollbarHeight)
		);
	};

	const jumpToBottom = () => {
		if (!chatDIV) return;

		pinToBottomNowAndAfterRender();
	};

	const submitForm = async (event: SubmitEvent) => {
		event.preventDefault();
		if (sendPending) {
			showMessageError(
				'Your previous message is still sending. Please wait before submitting another.'
			);
			return;
		}
		if (!hasInput) {
			showMessageError('Message cannot be empty');
			return;
		}

		const message = chatInput;
		const channel = channel_name;
		const broadcasterId = channelInfo.broadcaster_id;
		sendPending = true;
		chatInput = '';
		try {
			const result = await commands.sendChatMessage(broadcasterId, message);
			if (result.status === 'error') {
				const feedback = captureChatSendError(channel, result.error);
				if (feedback && !destroyed) showMessageError(feedback);
			} else {
				Logger.debug('message sent');
			}
		} catch (error) {
			captureChatSendError(channel, error);
		} finally {
			if (!destroyed) sendPending = false;
		}
	};

	const showMessageError = (msg: string, timeout = 5000) => {
		if (destroyed) return;
		clearTimeout(messageErrorTimer);
		errorState.msg = msg;
		errorState.active = true;
		messageErrorTimer = setTimeout(() => {
			errorState.active = false;
			errorState.msg = '';
		}, timeout);
	};

	let colonMatch = $derived(
		emoteSettings.autocomplete_enabled
			? parseColonMacro(chatInput, emoteSettings.autocomplete_min_chars)
			: null
	);

	// React to colon match changes
	$effect(() => {
		if (pickerOpenedByButton) return;
		// only run the search if it's not an input that was dismissed by the user
		if (colonMatch && colonMatch !== dismissedQuery) {
			clearTimeout(searchDebounceTimer);
			const query = colonMatch;
			searchDebounceTimer = setTimeout(async () => {
				const result = await commands.searchEmotes(
					query,
					channelInfo.broadcaster_id,
					emoteSettings.autocomplete_result_limit
				);
				if (result.status === 'ok' && colonMatch === query) {
					emoteResults = result.data;
					selectedEmoteIndex = 0;
					emotePickerVisible = emoteResults.length > 0;
				}
			}, emoteSettings.search_debounce_ms);
		} else if (!colonMatch) {
			emotePickerVisible = false;
			emoteResults = [];
		}
	});

	// Reset dismissed query when user types past it
	$effect(() => {
		if (!colonMatch || (dismissedQuery && !colonMatch.startsWith(dismissedQuery))) {
			dismissedQuery = '';
		}
	});

	// Button-mode search: debounced query into searchEmotes
	$effect(() => {
		if (!pickerOpenedByButton) return;
		const query = emoteSearchQuery;
		clearTimeout(searchDebounceTimer);
		searchDebounceTimer = setTimeout(async () => {
			const result = await commands.searchEmotes(
				query,
				channelInfo.broadcaster_id,
				emoteSettings.picker_result_limit
			);
			if (result.status === 'ok' && emoteSearchQuery === query && pickerOpenedByButton) {
				emoteResults = result.data;
				selectedEmoteIndex = 0;
			}
		}, emoteSettings.search_debounce_ms);
	});

	const insertEmote = (emote: EmoteType) => {
		const next =
			!pickerOpenedByButton && colonMatch
				? chatInput.substring(0, chatInput.lastIndexOf(':')) + emote.name + ' '
				: chatInput + (chatInput.endsWith(' ') || chatInput === '' ? '' : ' ') + emote.name + ' ';
		emotePickerVisible = false;
		pickerOpenedByButton = false;
		emoteSearchQuery = '';
		selectedEmoteIndex = 0;
		if (next.length > MAX_MESSAGE_LENGTH) {
			showMessageError(`Message cannot exceed ${MAX_MESSAGE_LENGTH} characters`);
			return;
		}
		chatInput = next;
	};

	const toggleEmotePicker = async () => {
		if (emotePickerVisible && pickerOpenedByButton) {
			emotePickerVisible = false;
			pickerOpenedByButton = false;
			emoteSearchQuery = '';
			return;
		}

		pickerOpenedByButton = true;
		emoteSearchQuery = '';
		const result = await commands.searchEmotes(
			'',
			channelInfo.broadcaster_id,
			emoteSettings.picker_result_limit
		);
		if (result.status === 'ok') {
			emoteResults = result.data;
			selectedEmoteIndex = 0;
			emotePickerVisible = true;
		}
	};

	const handleKeydown = (event: KeyboardEvent) => {
		if (!emotePickerVisible) return;

		if (event.key === 'Tab') {
			event.preventDefault();
			if (event.shiftKey) {
				selectedEmoteIndex = (selectedEmoteIndex - 1 + emoteResults.length) % emoteResults.length;
			} else {
				selectedEmoteIndex = (selectedEmoteIndex + 1) % emoteResults.length;
			}
		} else if (event.key === 'Enter') {
			event.preventDefault();
			insertEmote(emoteResults[selectedEmoteIndex]);
		} else if (event.key === 'Escape') {
			event.preventDefault();
			if (colonMatch) dismissedQuery = colonMatch;
			emotePickerVisible = false;
			pickerOpenedByButton = false;
			emoteSearchQuery = '';
		}
	};
</script>

{#snippet timestampCell(msg: ChannelMessage, invisible = false)}
	{#if chatSettings.show_timestamps}
		<span
			aria-hidden={invisible ? 'true' : undefined}
			class={cn(
				'text-xs whitespace-nowrap text-gray-500',
				invisible && 'pointer-events-none invisible'
			)}
		>
			{formatTimestamp(msg.ts, normalizedAppSettings)}
		</span>
	{/if}
{/snippet}

{#snippet badgeCell(msg: ChannelMessage, invisible = false)}
	{#if chatSettings.show_badges}
		{#if invisible}
			<span
				aria-hidden="true"
				class="pointer-events-none invisible inline-block"
				style="width: {chatBadgePlaceholderWidth(msg.badges.length, emoteSettings.inline_badge_px)}"
			></span>
		{:else}
			<Badges badges={msg.badges} sizePx={emoteSettings.inline_badge_px} />
		{/if}
	{/if}
{/snippet}

{#snippet translationPrefix(msg: ChannelMessage, layout: ChatTranslationLayout)}
	{#if translationHasTimestampPlaceholder(layout)}
		{@render timestampCell(msg, true)}
		{#if chatSettings.show_timestamps}
			<span aria-hidden="true"> </span>
		{/if}
	{/if}
	{#if translationHasBadgePlaceholder(layout)}
		{@render badgeCell(msg, true)}
		{#if chatSettings.show_badges && msg.badges.length > 0}
			<span aria-hidden="true"> </span>
		{/if}
	{/if}
{/snippet}

<Tooltip.Provider delayDuration={200}>
	<div class="flex h-full min-h-0 w-full flex-col flex-nowrap overflow-hidden">
		<div class="relative min-h-0 grow">
			<div
				class="h-full overflow-x-hidden overflow-y-auto [overflow-anchor:none]"
				aria-label="Chat messages"
				bind:this={chatDIV}
				onpointerdown={handleScrollbarPointerIntent}
				role="region"
				onscroll={refreshScrollState}
				ontouchcancel={handleTouchEndIntent}
				ontouchend={handleTouchEndIntent}
				ontouchmove={handleTouchMoveIntent}
				ontouchstart={handleTouchStartIntent}
				onwheel={handleWheelIntent}
			>
				<div bind:this={messageListDIV}>
					<div
						aria-hidden="true"
						data-chat-history-spacer
						bind:this={historySpacerDIV}
						style="height: {historySpacerHeight}px;"
					></div>
					{#each msgs as msg, messagePosition (msg.index)}
						<div
							data-chat-message-index={msg.index}
							class={cn(
								'block w-full px-2 py-1 text-sm',
								chatSettings.alternate_backgrounds &&
									(msg.channel_sequence % 2 === 0 ? 'bg-content-primary' : 'bg-content-secondary')
							)}
						>
							<div class="min-w-0 text-wrap wrap-anywhere">
								{@render timestampCell(msg)}
								{#if chatSettings.show_timestamps}
									<span aria-hidden="true"> </span>
								{/if}
								{@render badgeCell(msg)}
								{#if chatSettings.show_badges && msg.badges.length > 0}
									<span aria-hidden="true"> </span>
								{/if}
								<span class="whitespace-nowrap">
									<span style="color: {msg.color}; font-weight: 700;">{msg.chatter_user_name}</span
									>:&#32;
								</span>
								{#each msg.fragments as fragment, i (i)}
									{#if 'Text' in fragment}
										{fragment.Text.text}
									{:else if 'Emote' in fragment && fragment.Emote !== undefined && fragment.Emote.emote !== undefined}
										{#if chatSettings.show_emotes}
											<Emote emote={fragment.Emote.emote} sizePx={emoteSettings.inline_emote_px} />
										{:else}
											{fragment.Emote.emote.name}
										{/if}
									{/if}
								{/each}
							</div>
							{#if msg.translation}
								<div
									transition:slide={{ easing: quadInOut, duration: 40 }}
									class="min-w-0 text-wrap wrap-anywhere"
								>
									{@render translationPrefix(msg, chatSettings.translation_layout)}
									<Translation
										translation={msg.translation}
										authorName={msg.chatter_user_name}
										layout={chatSettings.translation_layout}
									/>
								</div>
							{/if}
						</div>
						{#if showDividerAfter(msgs, messagePosition, historyBoundary, chatSettings.show_history_divider)}
							<HistoryDivider />
						{/if}
					{/each}
				</div>
			</div>
			{#if showJumpToBottom}
				<div
					transition:fade={{ duration: 120 }}
					class="pointer-events-none absolute inset-x-0 bottom-2 z-10 flex justify-center"
				>
					<Button
						variant="outline"
						size="sm"
						class="bg-background/90 text-muted-foreground hover:text-foreground pointer-events-auto h-7 w-56 rounded-full pr-4 pl-3 text-xs tabular-nums shadow-sm backdrop-blur-sm"
						aria-label={jumpToBottomLabel}
						onclick={jumpToBottom}
					>
						<ArrowDown />
						{jumpToBottomText}
					</Button>
				</div>
			{/if}
		</div>
		{#if errorState.active}
			<div
				transition:slide={{ easing: quadInOut, duration: 250 }}
				role="alert"
				data-chat-input-error
				class="shrink-0 cursor-not-allowed bg-red-950 text-center"
			>
				{errorState.msg}
			</div>
		{/if}
		<div class="relative shrink-0 border-t">
			<EmotePicker
				emotes={emoteResults}
				selectedIndex={selectedEmoteIndex}
				onselect={insertEmote}
				visible={emotePickerVisible}
				showSearch={pickerOpenedByButton}
				bind:searchQuery={emoteSearchQuery}
				onSearchKeydown={handleKeydown}
				columns={emoteSettings.picker_columns}
				maxHeightPx={emoteSettings.picker_max_height_px}
				emoteSizePx={emoteSettings.inline_emote_px}
			/>
			<form onsubmit={submitForm} aria-busy={sendPending} class="flex items-center">
				<input
					bind:value={chatInput}
					aria-label="Chat message"
					maxlength={MAX_MESSAGE_LENGTH}
					onkeydown={handleKeydown}
					type="text"
					class="bg-background placeholder:text-muted-foreground h-full min-w-0 flex-1 p-3 text-sm outline-hidden focus:border-none focus:ring-0 disabled:cursor-not-allowed disabled:opacity-50"
					placeholder={username ? `Send message as ${username}` : 'Sign in to chat'}
				/>
				{#if showCounter}
					<span
						aria-live="polite"
						data-chat-counter
						class={cn(
							'shrink-0 px-2 text-xs tabular-nums',
							chatInput.length >= MAX_MESSAGE_LENGTH ? 'text-destructive' : 'text-muted-foreground'
						)}>{chatInput.length}/{MAX_MESSAGE_LENGTH}</span
					>
				{/if}
				<button
					type="button"
					class="text-muted-foreground hover:text-foreground cursor-pointer p-2"
					onclick={toggleEmotePicker}
				>
					<Smile class="h-5 w-5" />
				</button>
			</form>
		</div>
	</div>
</Tooltip.Provider>
