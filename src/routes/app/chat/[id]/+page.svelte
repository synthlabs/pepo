<script lang="ts">
	import { fade, slide } from 'svelte/transition';
	import { quadInOut } from 'svelte/easing';
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { onDestroy, onMount, tick } from 'svelte';
	import {
		commands,
		type ChannelInfo,
		type ChannelMessage,
		type ChatTranslationLayout,
		type ChannelMessageTranslationUpdate
	} from '$lib/bindings.ts';
	import { type UnlistenFn, listen } from '@tauri-apps/api/event';
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

	let chatDIV = $state<HTMLDivElement>();
	let messageListDIV = $state<HTMLDivElement>();
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
	let showSeparator: boolean = $state(false);
	let channel_name: string = $derived(page.params.id ?? '');
	let msgs: ChannelMessage[] = $state([]);
	let chatInput = $state('');
	let hasInput = $derived(chatInput.length > 0);
	let errorState = $state({ active: false, msg: '' });
	let channelInfo = $state({} as ChannelInfo);
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

	const pendingTranslations: PendingTranslations = new Map();
	let un_sub: UnlistenFn | undefined;
	let translation_un_sub: UnlistenFn | undefined;
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

		Logger.debug('subbing to chat messages');
		un_sub = await listen<ChannelMessage>(`chat_message:${channel_name}`, (event) => {
			addMessage(event.payload);
		});
		translation_un_sub = await listen<ChannelMessageTranslationUpdate>(
			`chat_translation:${channel_name}`,
			(event) => {
				applyTranslation(event.payload);
			}
		);

		Logger.info('joining channel:', channel_name);
		let result = await commands.joinChat(channel_name);
		Logger.debug(result);
		if (result.status !== 'ok') {
			Logger.error('failed to join channel:', result.error);
			showMessageError(`Failed to join ${channel_name}: ${result.error}`);
			un_sub?.();
			translation_un_sub?.();
			un_sub = undefined;
			translation_un_sub = undefined;
			return;
		}

		channelInfo = result.data;

		jumpToBottom();
	});

	onDestroy(async () => {
		destroyed = true;
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

		Logger.info('unsubbing from channel', channel_name);
		if (un_sub) {
			un_sub();
		}
		if (translation_un_sub) {
			translation_un_sub();
		}
		await commands.leaveChat(channel_name).then(Logger.debug);
	});

	$effect(() => {
		while (msgs.length > chatSettings.message_limit) msgs.shift();
	});

	const addMessage = (message: ChannelMessage) => {
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

		const wasPinned = manualScrollActive
			? false
			: (pendingScrollSnapshot?.wasAtBottom ?? autoScrollPinned);

		msgs.push(attachPendingTranslation(message, pendingTranslations));
		if (msgs.length > chatSettings.message_limit) msgs.shift();
		if (!wasPinned) unreadMessageCount += 1;

		if (!manualScrollActive) queueScrollRestore();
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
			applyPinnedBottomState();
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

	const submitForm = (event: SubmitEvent) => {
		event.preventDefault();
		let target = event.target as HTMLFormElement;

		if (hasInput) {
			commands
				.sendChatMessage(channelInfo.broadcaster_id, chatInput)
				.then(() => Logger.debug('message sent'))
				.catch(Logger.error)
				.finally(() => {
					chatInput = '';
					if (event.target) {
						target.reset();
					}
				});
		} else {
			showMessageError('Message cannot be empty');
		}
	};

	const showMessageError = (msg: string, timeout = 5000) => {
		errorState.msg = msg;
		errorState.active = true;
		setTimeout(() => {
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
		if (!pickerOpenedByButton && colonMatch) {
			const colonIndex = chatInput.lastIndexOf(':');
			chatInput = chatInput.substring(0, colonIndex) + emote.name + ' ';
		} else {
			chatInput =
				chatInput + (chatInput.endsWith(' ') || chatInput === '' ? '' : ' ') + emote.name + ' ';
		}
		emotePickerVisible = false;
		pickerOpenedByButton = false;
		emoteSearchQuery = '';
		selectedEmoteIndex = 0;
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
	<div class="flex h-full w-full flex-col flex-nowrap">
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
					{#each msgs as msg (msg.index)}
						<div
							data-chat-message-index={msg.index}
							class={cn(
								'block w-full px-2 py-1 text-sm',
								chatSettings.alternate_backgrounds &&
									(msg.index % 2 === 0 ? 'bg-content-primary' : 'bg-content-secondary')
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
						{#if showSeparator}
							<Separator class="" />
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
				class=" cursor-not-allowed bg-red-950 text-center"
			>
				{errorState.msg}
			</div>
		{/if}
		<div class="relative border-t">
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
			<form onsubmit={submitForm} class="flex items-center">
				<input
					bind:value={chatInput}
					onkeydown={handleKeydown}
					type="text"
					class="bg-background placeholder:text-muted-foreground h-full flex-1 p-3 text-sm outline-hidden focus:border-none focus:ring-0 disabled:cursor-not-allowed disabled:opacity-50"
					placeholder={username ? `Send message as ${username}` : 'Sign in to chat'}
				/>
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
