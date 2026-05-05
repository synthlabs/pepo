<script lang="ts">
	import { slide } from 'svelte/transition';
	import { quadInOut } from 'svelte/easing';
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { onDestroy, onMount, tick } from 'svelte';
	import { commands, type ChannelInfo, type ChannelMessage } from '$lib/bindings.ts';
	import { type UnlistenFn, listen } from '@tauri-apps/api/event';
	import { cn } from '$lib/utils';
	import { page } from '$app/state';
	import Badges from '$lib/components/chat/+badges.svelte';
	import Logger from '$utils/log';
	import Emote from '$lib/components/chat/+emote.svelte';
	import EmotePicker from '$lib/components/chat/+emote-picker.svelte';
	import Smile from '@lucide/svelte/icons/smile';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import type { Emote as EmoteType } from '$lib/bindings.ts';
	import { parseColonMacro } from '$lib/chat/colon-macro';
	import {
		DEFAULT_BOTTOM_THRESHOLD,
		getBatchScrollSnapshot,
		refreshScrollStateAfterScroll,
		restoreScrollAfterRender,
		scrollToBottom as scrollElementToBottom,
		type ScrollSnapshot
	} from '$lib/chat/autoscroll';

	const CHAT_MESSAGE_LIMIT = 500;
	const CHAT_MESSAGE_SELECTOR = '[data-chat-message-index]';
	const AUTOSCROLL_THRESHOLD = DEFAULT_BOTTOM_THRESHOLD;

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

	// Emote picker state
	let emotePickerVisible = $state(false);
	let emoteResults: EmoteType[] = $state([]);
	let selectedEmoteIndex = $state(0);
	let pickerOpenedByButton = $state(false);
	let dismissedQuery = $state('');
	let emoteSearchQuery = $state('');
	let searchDebounceTimer: ReturnType<typeof setTimeout> | undefined;

	let un_sub: UnlistenFn;
	let pendingScrollSnapshot: ScrollSnapshot | null = null;
	let scrollFlushQueued = false;
	let scrollFrame: number | undefined;
	let resizeScrollFrame: number | undefined;
	let resizeObserver: ResizeObserver | undefined;
	let destroyed = false;

	onMount(async () => {
		if (messageListDIV && typeof ResizeObserver !== 'undefined') {
			resizeObserver = new ResizeObserver(() => {
				queuePinnedScrollToBottom();
			});
			resizeObserver.observe(messageListDIV);
		}

		Logger.info('joining channel:', channel_name);
		let result = await commands.joinChat(channel_name);
		Logger.debug(result);
		if (result.status !== 'ok') {
			Logger.error('failed to join channel:', result.error);
			showMessageError(`Failed to join ${channel_name}: ${result.error}`);
			return;
		}

		channelInfo = result.data;

		Logger.debug('subbing to chat messages');
		un_sub = await listen<ChannelMessage>(`chat_message:${channel_name}`, (event) => {
			addMessage(event.payload);
		});

		jumpToBottom();
	});

	onDestroy(async () => {
		destroyed = true;
		resizeObserver?.disconnect();
		if (scrollFrame !== undefined) cancelAnimationFrame(scrollFrame);
		if (resizeScrollFrame !== undefined) cancelAnimationFrame(resizeScrollFrame);

		Logger.info('unsubbing from channel', channel_name);
		if (un_sub) {
			un_sub();
		}
		await commands.leaveChat(channel_name).then(Logger.debug);
	});

	const addMessage = (message: ChannelMessage) => {
		if (chatDIV) {
			pendingScrollSnapshot = getBatchScrollSnapshot(
				pendingScrollSnapshot,
				chatDIV,
				CHAT_MESSAGE_SELECTOR,
				AUTOSCROLL_THRESHOLD
			);
		}

		const wasPinned = pendingScrollSnapshot?.wasAtBottom ?? autoScrollPinned;

		msgs.push(message);
		if (msgs.length > CHAT_MESSAGE_LIMIT) msgs.shift();
		if (!wasPinned) unreadMessageCount += 1;

		queueScrollRestore();
	};

	const refreshScrollState = () => {
		if (!chatDIV) return;

		const scrollState = refreshScrollStateAfterScroll(
			chatDIV,
			pendingScrollSnapshot,
			scrollFlushQueued,
			unreadMessageCount,
			CHAT_MESSAGE_SELECTOR,
			AUTOSCROLL_THRESHOLD
		);

		autoScrollPinned = scrollState.pinned;
		pendingScrollSnapshot = scrollState.pendingSnapshot;
		unreadMessageCount = scrollState.unreadMessageCount;
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
				AUTOSCROLL_THRESHOLD
			);
			autoScrollPinned = result.pinned;
			if (autoScrollPinned) unreadMessageCount = 0;
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

	const jumpToBottom = () => {
		if (!chatDIV) return;

		pendingScrollSnapshot = null;
		scrollElementToBottom(chatDIV);
		autoScrollPinned = true;
		unreadMessageCount = 0;
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

	let colonMatch = $derived(parseColonMacro(chatInput));

	// React to colon match changes
	$effect(() => {
		if (pickerOpenedByButton) return;
		// only run the search if it's not an input that was dismissed by the user
		if (colonMatch && colonMatch !== dismissedQuery) {
			clearTimeout(searchDebounceTimer);
			const query = colonMatch;
			searchDebounceTimer = setTimeout(async () => {
				const result = await commands.searchEmotes(query, channelInfo.broadcaster_id, null);
				if (result.status === 'ok' && colonMatch === query) {
					emoteResults = result.data;
					selectedEmoteIndex = 0;
					emotePickerVisible = emoteResults.length > 0;
				}
			}, 75);
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
			const result = await commands.searchEmotes(query, channelInfo.broadcaster_id, 50);
			if (result.status === 'ok' && emoteSearchQuery === query && pickerOpenedByButton) {
				emoteResults = result.data;
				selectedEmoteIndex = 0;
			}
		}, 75);
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
		const result = await commands.searchEmotes('', channelInfo.broadcaster_id, 50);
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
			class="grow overflow-x-hidden overflow-y-auto"
			bind:this={chatDIV}
			onscroll={refreshScrollState}
		>
			<div bind:this={messageListDIV}>
				{#each msgs as msg (msg.index)}
					<div
						data-chat-message-index={msg.index}
						class={cn(
							'inline-block w-full px-2 py-1 text-sm text-wrap',
							msg.index % 2 === 0 ? 'bg-content-primary' : 'bg-content-secondary'
						)}
					>
						<span class="text-xs whitespace-nowrap text-gray-500"
							>{new Date(msg.ts).toLocaleTimeString('en', { timeStyle: 'short' })}</span
						>
						<Badges badges={msg.badges} />
						<span class="whitespace-nowrap" style="color: {msg.color}; font-weight: 700;"
							>{msg.chatter_user_name}</span
						>:
						{#each msg.fragments as fragment, i (i)}
							{#if 'Text' in fragment}
								{fragment.Text.text}
							{:else if 'Emote' in fragment && fragment.Emote !== undefined && fragment.Emote.emote !== undefined}
								<Emote emote={fragment.Emote.emote} />
							{/if}
						{/each}
					</div>
					{#if showSeparator}
						<Separator class="" />
					{/if}
				{/each}
			</div>
		</div>
		{#if showJumpToBottom}
			<button class="bg-primary w-full cursor-pointer text-center" type="button" onclick={jumpToBottom}>
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
			/>
			<form onsubmit={submitForm} class="flex items-center">
				<input
					bind:value={chatInput}
					onkeydown={handleKeydown}
					type="text"
					class="bg-background placeholder:text-muted-foreground h-full flex-1 p-3 text-sm outline-hidden focus:border-none focus:ring-0 disabled:cursor-not-allowed disabled:opacity-50"
					placeholder="Send message as sir_xin"
				/>
				<button
					type="button"
					class="text-muted-foreground hover:text-foreground p-2"
					onclick={toggleEmotePicker}
				>
					<Smile class="h-5 w-5" />
				</button>
			</form>
		</div>
	</div>
</Tooltip.Provider>
