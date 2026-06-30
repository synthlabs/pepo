<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quadInOut } from 'svelte/easing';
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { onDestroy, onMount, tick } from 'svelte';
	import {
		commands,
		type ChannelInfo,
		type ChannelMessage,
		type ChannelMessageTranslationUpdate
	} from '$lib/bindings.ts';
	import { type UnlistenFn, listen } from '@tauri-apps/api/event';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { cn } from '$lib/utils';
	import { page } from '$app/state';
	import Badges from '$lib/components/chat/+badges.svelte';
	import Logger from '$utils/log';
	import Emote from '$lib/components/chat/+emote.svelte';
	import EmotePicker from '$lib/components/chat/+emote-picker.svelte';
	import Translation from '$lib/components/chat/+translation.svelte';
	import Smile from '@lucide/svelte/icons/smile';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import type { Emote as EmoteType } from '$lib/bindings.ts';
	import { parseColonMacro } from '$lib/chat/colon-macro';
	import {
		captureScrollSnapshot,
		capturePinnedIntent,
		getBatchScrollSnapshot,
		refreshScrollStateAfterScroll,
		restoreScrollAfterRender,
		scrollToBottom as scrollElementToBottom,
		type ScrollSnapshot
	} from '$lib/chat/autoscroll';
	import {
		applyTranslationUpdate,
		attachPendingTranslation,
		type PendingTranslations
	} from '$lib/chat/translation';
	import { formatTimestamp } from '$lib/settings';
	import { authState } from '$lib/stores/auth.svelte';
	import { getNormalizedAppSettings } from '$lib/stores/settings.svelte';

	const CHAT_MESSAGE_SELECTOR = '[data-chat-message-index]';
	const USER_SCROLL_INTENT_MS = 250;
	const PAUSED_REFLOW_SETTLE_MS = 250;

	let chatDIV = $state<HTMLDivElement>();
	let messageListDIV = $state<HTMLDivElement>();
	let autoScrollPinned = $state(true);
	let unreadMessageCount = $state(0);
	let showJumpToBottom = $derived(!autoScrollPinned);
	let jumpToBottomLabel = $derived(
		unreadMessageCount > 0
			? `${unreadMessageCount} New Message${unreadMessageCount === 1 ? '' : 's'} Below`
			: 'More Messages Below'
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
	let focus_un_sub: UnlistenFn | undefined;
	let pendingScrollSnapshot: ScrollSnapshot | null = null;
	let pausedReflowSnapshot: ScrollSnapshot | null = null;
	let scrollFlushQueued = false;
	let scrollFrame: number | undefined;
	let resizeScrollFrame: number | undefined;
	let pausedReflowFrame: number | undefined;
	let focusRestoreFrame: number | undefined;
	let pausedReflowTimer: ReturnType<typeof setTimeout> | undefined;
	let resizeObserver: ResizeObserver | undefined;
	let pinnedBeforeFocusLoss = true;
	let userScrollIntentUntil = 0;
	let destroyed = false;

	onMount(async () => {
		if (messageListDIV && typeof ResizeObserver !== 'undefined') {
			resizeObserver = new ResizeObserver(() => {
				queueResizeScrollRestore();
			});
			resizeObserver.observe(messageListDIV);
		}

		focus_un_sub = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
			handleWindowFocusChanged(focused);
		});

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
		if (scrollFrame !== undefined) cancelAnimationFrame(scrollFrame);
		if (resizeScrollFrame !== undefined) cancelAnimationFrame(resizeScrollFrame);
		if (pausedReflowFrame !== undefined) cancelAnimationFrame(pausedReflowFrame);
		if (focusRestoreFrame !== undefined) cancelAnimationFrame(focusRestoreFrame);
		if (pausedReflowTimer !== undefined) clearTimeout(pausedReflowTimer);

		Logger.info('unsubbing from channel', channel_name);
		if (un_sub) {
			un_sub();
		}
		if (translation_un_sub) {
			translation_un_sub();
		}
		if (focus_un_sub) {
			focus_un_sub();
		}
		await commands.leaveChat(channel_name).then(Logger.debug);
	});

	$effect(() => {
		while (msgs.length > chatSettings.message_limit) msgs.shift();
	});

	const addMessage = (message: ChannelMessage) => {
		if (chatDIV) {
			pendingScrollSnapshot = getBatchScrollSnapshot(
				pendingScrollSnapshot,
				chatDIV,
				CHAT_MESSAGE_SELECTOR,
				chatSettings.autoscroll_threshold_px
			);
		}

		const wasPinned = pendingScrollSnapshot?.wasAtBottom ?? autoScrollPinned;

		msgs.push(attachPendingTranslation(message, pendingTranslations));
		if (msgs.length > chatSettings.message_limit) msgs.shift();
		if (!wasPinned) unreadMessageCount += 1;

		queueScrollRestore();
	};

	const applyTranslation = (update: ChannelMessageTranslationUpdate) => {
		const result = applyTranslationUpdate(msgs, update, pendingTranslations);
		if (!result.changed) return;

		if (chatDIV) {
			pendingScrollSnapshot = getBatchScrollSnapshot(
				pendingScrollSnapshot,
				chatDIV,
				CHAT_MESSAGE_SELECTOR,
				chatSettings.autoscroll_threshold_px
			);
		}

		msgs = result.messages;
		queueScrollRestore();
	};

	const refreshScrollState = () => {
		if (!chatDIV) return;

		const userInitiated = consumeUserScrollIntent();
		const wasPinned = autoScrollPinned;
		const hadQueuedRestore = scrollFlushQueued && pendingScrollSnapshot !== null;
		const scrollState = refreshScrollStateAfterScroll(
			chatDIV,
			pendingScrollSnapshot,
			scrollFlushQueued,
			unreadMessageCount,
			CHAT_MESSAGE_SELECTOR,
			chatSettings.autoscroll_threshold_px,
			{ userInitiated, preservePinnedIntent: wasPinned }
		);

		autoScrollPinned = scrollState.pinned;
		pendingScrollSnapshot = scrollState.pendingSnapshot;
		unreadMessageCount = scrollState.unreadMessageCount;
		if (userInitiated) cancelQueuedScrollRestore();
		else if (wasPinned && scrollState.deferred && !hadQueuedRestore) queuePinnedScrollToBottom();
	};

	const markUserScrollIntent = () => {
		userScrollIntentUntil = performance.now() + USER_SCROLL_INTENT_MS;
		cancelQueuedScrollRestore();
	};

	const handleScrollbarPointerIntent = (event: PointerEvent) => {
		if (!chatDIV || !isScrollbarPointerEvent(chatDIV, event)) return;

		markUserScrollIntent();
	};

	const consumeUserScrollIntent = () => {
		if (performance.now() > userScrollIntentUntil) return false;

		userScrollIntentUntil = 0;
		return true;
	};

	const handleWindowFocusChanged = (focused: boolean) => {
		if (focused) {
			if (pinnedBeforeFocusLoss) forcePinnedScrollToBottom();
			return;
		}

		pinnedBeforeFocusLoss = capturePinnedIntent(
			chatDIV,
			pendingScrollSnapshot,
			autoScrollPinned,
			chatSettings.autoscroll_threshold_px
		);
	};

	const queueScrollRestore = () => {
		if (scrollFlushQueued) return;

		scrollFlushQueued = true;
		void restoreQueuedScroll();
	};

	const restoreQueuedScroll = async () => {
		await tick();
		if (destroyed) {
			scrollFlushQueued = false;
			return;
		}

		scrollFrame = requestAnimationFrame(() => {
			scrollFrame = undefined;
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
			autoScrollPinned = result.pinned;
			if (autoScrollPinned) {
				unreadMessageCount = 0;
				clearPausedReflowSnapshot();
			} else {
				rememberPausedReflowSnapshot();
			}
		});
	};

	const cancelQueuedScrollRestore = () => {
		if (scrollFrame !== undefined) {
			cancelAnimationFrame(scrollFrame);
			scrollFrame = undefined;
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
	};

	const forcePinnedScrollToBottom = () => {
		cancelQueuedScrollRestore();
		if (focusRestoreFrame !== undefined) {
			cancelAnimationFrame(focusRestoreFrame);
			focusRestoreFrame = undefined;
		}

		applyPinnedBottomState();
		void restorePinnedBottomAfterRender();
	};

	const restorePinnedBottomAfterRender = async () => {
		await tick();
		if (destroyed) return;

		focusRestoreFrame = requestAnimationFrame(() => {
			focusRestoreFrame = undefined;
			if (destroyed) return;

			applyPinnedBottomState();
		});
	};

	const queuePinnedScrollToBottom = () => {
		if (!autoScrollPinned || !chatDIV || resizeScrollFrame !== undefined) return;

		resizeScrollFrame = requestAnimationFrame(() => {
			resizeScrollFrame = undefined;
			if (!autoScrollPinned || !chatDIV) return;

			scrollElementToBottom(chatDIV);
			refreshScrollState();
		});
	};

	const queueResizeScrollRestore = () => {
		if (!chatDIV) return;
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
		if (!chatDIV || !pausedReflowSnapshot || pausedReflowFrame !== undefined) return;

		pausedReflowFrame = requestAnimationFrame(() => {
			pausedReflowFrame = undefined;
			if (!chatDIV || !pausedReflowSnapshot || autoScrollPinned) return;

			const result = restoreScrollAfterRender(
				chatDIV,
				pausedReflowSnapshot,
				CHAT_MESSAGE_SELECTOR,
				chatSettings.autoscroll_threshold_px
			);
			autoScrollPinned = result.pinned;
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

		pausedReflowSnapshot = captureScrollSnapshot(
			chatDIV,
			CHAT_MESSAGE_SELECTOR,
			chatSettings.autoscroll_threshold_px
		);
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

		forcePinnedScrollToBottom();
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

<Tooltip.Provider delayDuration={200}>
	<div class="flex h-full w-full flex-col flex-nowrap">
		<div
			class="grow overflow-x-hidden overflow-y-auto [overflow-anchor:none]"
			aria-label="Chat messages"
			bind:this={chatDIV}
			onpointerdown={handleScrollbarPointerIntent}
			role="region"
			onscroll={refreshScrollState}
			ontouchstart={markUserScrollIntent}
			onwheel={markUserScrollIntent}
		>
			<div bind:this={messageListDIV}>
				{#each msgs as msg (msg.index)}
					<div
						data-chat-message-index={msg.index}
						class={cn(
							'inline-block w-full px-2 py-1 text-sm text-wrap',
							chatSettings.alternate_backgrounds &&
								(msg.index % 2 === 0 ? 'bg-content-primary' : 'bg-content-secondary')
						)}
					>
						{#if chatSettings.show_timestamps}
							<span class="text-xs whitespace-nowrap text-gray-500">
								{formatTimestamp(msg.ts, normalizedAppSettings)}
							</span>
						{/if}
						{#if chatSettings.show_badges}
							<Badges badges={msg.badges} sizePx={emoteSettings.inline_badge_px} />
						{/if}
						<span class="whitespace-nowrap" style="color: {msg.color}; font-weight: 700;"
							>{msg.chatter_user_name}</span
						>:
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
						<Translation translation={msg.translation} />
					</div>
					{#if showSeparator}
						<Separator class="" />
					{/if}
				{/each}
			</div>
		</div>
		{#if showJumpToBottom}
			<button
				class="bg-primary w-full cursor-pointer text-center"
				type="button"
				onclick={jumpToBottom}
			>
				{jumpToBottomLabel}
			</button>
		{/if}
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
