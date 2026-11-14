<script lang="ts">
	import { fade, slide } from 'svelte/transition';
	import { quadInOut } from 'svelte/easing';
	import HistoryDivider from '$lib/components/chat/history-divider.svelte';
	import { openRoomFeed } from '$lib/chat/feed';
	import { showDividerAfter, type HistoryBoundary } from '$lib/chat/history-boundary';
	import { onDestroy, onMount, tick } from 'svelte';
	import { MediaQuery, SvelteMap, SvelteSet } from 'svelte/reactivity';
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
	import CircleAlert from '@lucide/svelte/icons/circle-alert';
	import Smile from '@lucide/svelte/icons/smile';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import type { Emote as EmoteType } from '$lib/bindings.ts';
	import { parseColonMacro } from '$lib/chat/colon-macro';
	import { captureChatSendError } from '$lib/chat/send-errors';
	import { MessageHistoryScheduler } from '$lib/chat/message-history';
	import { measureHistoryOriginSpacer } from '$lib/chat/history-origin';
	import { ChatScrollController } from '$lib/chat/scroll-controller';
	import {
		applyTranslationUpdate,
		attachPendingTranslation,
		HistoryTranslationBuffer,
		isRowOutsideViewport,
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
	const MAX_MESSAGE_LENGTH = 500;
	const COUNTER_THRESHOLD = 400;
	const reducedMotion = new MediaQuery('(prefers-reduced-motion: reduce)');

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
	let timestampLocale = $derived(chatSettings.timestamp_locale);
	let timestampStyle = $derived(chatSettings.timestamp_style);
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
	const historyTranslations = new HistoryTranslationBuffer();
	let historyTranslationObserver: IntersectionObserver | undefined;
	const observedHistoryRows = new SvelteSet<HTMLElement>();
	let historyTranslationRenderQueued = false;
	let historyTranslationFrame: number | undefined;
	let historyTranslationTimer: ReturnType<typeof setTimeout> | undefined;
	let roomFeed: ReturnType<typeof openRoomFeed> | undefined;
	let destroyed = false;
	const messageHistory = new MessageHistoryScheduler<ChannelMessage>({
		getState: () => ({
			...chatSettings,
			paused: !autoScrollPinned,
			interacting: scrollController.interacting,
			message_count: msgs.length
		}),
		append: (messages) => appendMessages(messages),
		trim: (count) => trimMessageHistory(count)
	});

	const scrollController = new ChatScrollController({
		messageSelector: CHAT_MESSAGE_SELECTOR,
		afterRender: tick,
		getThresholdPx: () => chatSettings.autoscroll_threshold_px,
		onPinnedChange: (pinned) => {
			autoScrollPinned = pinned;
		},
		onClearUnread: () => {
			unreadMessageCount = 0;
		},
		onHistoryUpdate: () => {
			messageHistory.update();
			scheduleHistoryTranslations();
		},
		onResume: () => {
			messageHistory.resume();
			scheduleHistoryTranslations();
		}
	});

	onMount(async () => {
		scrollController.attach(chatDIV, messageListDIV);
		if (typeof IntersectionObserver !== 'undefined') {
			historyTranslationObserver = new IntersectionObserver(scheduleHistoryTranslations, {
				root: chatDIV
			});
		}

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
					const wasPinned = scrollController.beforeReset();
					msgs = historyTranslations.reset(msgs, frame.event.messages, frame.cursor);
					scrollController.afterReset(wasPinned);
					scheduleHistoryTranslations();
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
		if (historyTranslationFrame !== undefined) cancelAnimationFrame(historyTranslationFrame);
		clearTimeout(historyTranslationTimer);
		historyTranslationObserver?.disconnect();
		observedHistoryRows.clear();
		historyTranslations.clear();
		messageHistory.dispose();
		scrollController.dispose();

		roomFeed?.dispose();
	});

	$effect(() => {
		messageHistory.update();
	});

	const addMessage = (message: ChannelMessage) => {
		if (!autoScrollPinned || scrollController.interacting) unreadMessageCount += 1;
		messageHistory.enqueue(message);
	};

	const appendMessages = (messages: ChannelMessage[]) => {
		const shouldRestore = scrollController.beforeContentChange('append');

		msgs.push(...messages.map((message) => attachPendingTranslation(message, pendingTranslations)));

		if (shouldRestore) scrollController.afterContentChange();
		scheduleHistoryTranslations();
	};

	const trimMessageHistory = (count: number) => {
		const shouldRestore = scrollController.beforeContentChange('trim');
		historySpacerHeight = measureHistoryOriginSpacer(
			historySpacerDIV,
			messageListDIV?.querySelectorAll<HTMLElement>(CHAT_MESSAGE_SELECTOR) ?? [],
			new Set(msgs.slice(0, count).map((message) => String(message.index))),
			window.devicePixelRatio
		);
		msgs.splice(0, count);
		historyTranslations.retain(msgs);
		if (shouldRestore) scrollController.afterContentChange();
		scheduleHistoryTranslations();
	};

	const applyTranslation = (update: ChannelMessageTranslationUpdate) => {
		const message = msgs.find((message) => message.message_id === update.message_id);
		if (message && historyTranslations.isHistorical(message)) {
			historyTranslations.defer(message, update.translation);
			scheduleHistoryTranslations();
			return;
		}
		const result = applyTranslationUpdate(msgs, update, pendingTranslations);
		if (!result.changed) return;

		const shouldRestore = scrollController.beforeContentChange('translation');

		msgs = result.messages;
		if (shouldRestore) scrollController.afterContentChange();
	};

	const observeHistoryTranslations = (rows: HTMLElement[]) => {
		const pendingIndexes = new Set(
			msgs
				.filter((message) => historyTranslations.has(message.message_id))
				.map((message) => String(message.index))
		);
		const pendingRows = new Set(
			rows.filter((row) => pendingIndexes.has(row.dataset.chatMessageIndex ?? ''))
		);
		for (const row of observedHistoryRows) {
			if (pendingRows.has(row)) continue;
			historyTranslationObserver?.unobserve(row);
			observedHistoryRows.delete(row);
		}
		for (const row of pendingRows) {
			if (observedHistoryRows.has(row)) continue;
			historyTranslationObserver?.observe(row);
			observedHistoryRows.add(row);
		}
		return pendingRows;
	};

	const flushHistoryTranslations = () => {
		historyTranslationTimer = undefined;
		if (destroyed || !chatDIV || !messageListDIV) return;
		const rows = [...messageListDIV.querySelectorAll<HTMLElement>(CHAT_MESSAGE_SELECTOR)];
		const pendingRows = observeHistoryTranslations(rows);
		if (scrollController.interacting) return;

		const viewport = chatDIV.getBoundingClientRect();
		const offscreen = new Set(
			[...pendingRows]
				.filter((row) => isRowOutsideViewport(row.getBoundingClientRect(), viewport))
				.map((row) => row.dataset.chatMessageIndex)
		);
		const result = historyTranslations.flush(msgs, (message) =>
			offscreen.has(String(message.index))
		);
		if (result.changed) {
			const shouldRestore = scrollController.beforeContentChange('translation');
			msgs = result.messages;
			if (shouldRestore) scrollController.afterContentChange();
		}
		observeHistoryTranslations(rows);
	};

	const scheduleHistoryTranslations = () => {
		if (destroyed) return;
		if (historyTranslations.size === 0) {
			historyTranslationObserver?.disconnect();
			observedHistoryRows.clear();
			return;
		}
		if (
			historyTranslationRenderQueued ||
			historyTranslationFrame !== undefined ||
			historyTranslationTimer !== undefined
		)
			return;
		historyTranslationRenderQueued = true;
		void tick().then(() => {
			historyTranslationRenderQueued = false;
			if (destroyed) return;
			historyTranslationFrame = requestAnimationFrame(() => {
				historyTranslationFrame = undefined;
				// Read settled geometry, then let scroll restoration precede the next paint.
				historyTranslationTimer = setTimeout(flushHistoryTranslations, 0);
			});
		});
	};

	const handleChatScroll = () => {
		scrollController.refreshScrollState();
		scheduleHistoryTranslations();
	};

	const jumpToBottom = () => {
		if (!chatDIV) return;

		scrollController.resumeToBottom();
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
	{@const timestamp = msg.ts}
	{#if chatSettings.show_timestamps}
		<!-- Keep distinct formatter inputs observable when their text matches. -->
		{@const formattedTimestamp = {
			text: formatTimestamp(timestamp, timestampLocale, timestampStyle)
		}}
		<span
			aria-hidden={invisible ? 'true' : undefined}
			class={cn(
				'text-xs whitespace-nowrap text-gray-500',
				invisible && 'pointer-events-none invisible'
			)}
		>
			{formattedTimestamp.text}
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
				onpointerdown={scrollController.handleScrollbarPointerIntent}
				role="region"
				onscroll={handleChatScroll}
				ontouchcancel={scrollController.handleTouchEndIntent}
				ontouchend={scrollController.handleTouchEndIntent}
				ontouchmove={scrollController.handleTouchMoveIntent}
				ontouchstart={scrollController.handleTouchStartIntent}
				onwheel={scrollController.handleWheelIntent}
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
									data-chat-translation
									transition:slide={{
										easing: quadInOut,
										duration: historyTranslations.isHistorical(msg) ? 0 : 40
									}}
									class="min-w-0 text-wrap wrap-anywhere"
								>
									{@render translationPrefix(msg, normalizedAppSettings.translation.layout)}
									<Translation
										translation={msg.translation}
										authorName={msg.chatter_user_name}
										layout={normalizedAppSettings.translation.layout}
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
				transition:slide={{ easing: quadInOut, duration: reducedMotion.current ? 0 : 250 }}
				role="alert"
				data-chat-input-error
				class="bg-background flex shrink-0 items-start gap-2 px-3 py-2 text-left text-sm leading-5 text-red-700 dark:text-red-400"
			>
				<CircleAlert class="mt-0.5 size-4 shrink-0" aria-hidden="true" />
				<span class="min-w-0 wrap-anywhere whitespace-pre-wrap">{errorState.msg}</span>
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
