<script lang="ts">
	import { slide } from 'svelte/transition';
	import { cubicOut, quadInOut } from 'svelte/easing';
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { onDestroy, onMount, tick } from 'svelte';
	import {
		commands,
		type ChannelInfo,
		type ChannelMessage,
		type UserToken
	} from '$lib/bindings.ts';
	import { type UnlistenFn, listen } from '@tauri-apps/api/event';
	import { cn } from '$lib/utils';
	import { page } from '$app/state';
	import Badges from '$lib/components/chat/+badges.svelte';
	import Logger from '$utils/log';
	import Emote from '$lib/components/chat/+emote.svelte';

	const AUTOSCROLL_BUFFER = 200; // the amount you can scroll up and still not disable auto scroll
	const CHAT_MESSAGE_LIMIT = 10000;

	let banner = $state({} as UserToken);
	let chatDIV = $state<HTMLDivElement>();
	let scrolledAmount = $state(window.innerHeight);
	let autoscroll: boolean = $state(true);
	let isScrolled = $derived(
		chatDIV ? scrolledAmount < chatDIV.scrollHeight - AUTOSCROLL_BUFFER : false
	);
	let forceAutoscrollDebounce: boolean = $state(true);
	let showSeparator: boolean = $state(false);
	let channel_name: string = $derived(page.params.id ?? '');
	let msgs: ChannelMessage[] = $state([]);
	let chatInput = $state('');
	let hasInput = $derived(chatInput.length > 0);
	let errorState = $state({ active: false, msg: '' });
	let channelInfo = $state({} as ChannelInfo);

	let un_sub: UnlistenFn;

	$inspect(banner);
	$inspect(isScrolled);

	onMount(async () => {
		Logger.info('joining channel:', channel_name);
		let result = await commands.joinChat(channel_name);
		Logger.debug(result);
		if (result.status !== 'ok') {
			Logger.error('RESULT NOT OK');
			return;
		}

		channelInfo = result.data;

		Logger.debug('subbing to chat messages');
		un_sub = await listen<ChannelMessage>(`chat_message:${channel_name}`, (event) => {
			msgs.push(event.payload);
			if (msgs.length > CHAT_MESSAGE_LIMIT) msgs.shift();
			processAutoscroll();
		});

		scrollToBottom();
	});

	onDestroy(async () => {
		Logger.info('unsubbing from channel', channel_name);
		if (un_sub) {
			un_sub();
		}
		await commands.leaveChat(channel_name).then(Logger.debug);
	});

	const refreshScrollAmount = () => {
		scrolledAmount = chatDIV ? chatDIV.offsetHeight + chatDIV.scrollTop : 0;
	};

	const shouldScroll = (): boolean => {
		if (forceAutoscrollDebounce) {
			autoscroll = true;
		} else {
			refreshScrollAmount();
			// determine whether we should auto-scroll
			// once the DOM is updated...
			// if the scroll amount matches the tail minus a buffer amount then autoscroll
			autoscroll = !!chatDIV && scrolledAmount > chatDIV.scrollHeight - AUTOSCROLL_BUFFER;
		}
		return autoscroll;
	};

	const processAutoscroll = async () => {
		await tick();

		if (shouldScroll()) {
			if (chatDIV) {
				chatDIV.scrollTo({ top: chatDIV.scrollHeight, left: 0, behavior: 'auto' });
			}
		}
	};

	const forceAutoscrollDebounceFn = () => {
		forceAutoscrollDebounce = true;
		Logger.debug('autoscroll debounce START');
		setTimeout(() => {
			Logger.debug('autoscroll debounce FINISH');
			forceAutoscrollDebounce = false;
		}, 2000);
	};

	const scrollToBottom = () => {
		forceAutoscrollDebounceFn();
		processAutoscroll();
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
</script>

<div class="flex h-full w-full flex-col flex-nowrap">
	<div
		class="grow overflow-x-hidden overflow-y-auto"
		bind:this={chatDIV}
		onscroll={refreshScrollAmount}
	>
		{#each msgs as msg}
			<div
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
				{#each msg.fragments as fragment}
					{#if 'Text' in fragment}
						{fragment.Text.text}
					{:else if 'Emote' in fragment}
						<Emote emote={fragment.Emote.emote} />
					{/if}
				{/each}
			</div>
			{#if showSeparator}
				<Separator class="" />
			{/if}
		{/each}
	</div>
	{#if isScrolled}
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="bg-primary cursor-pointer text-center" onclick={scrollToBottom}>
			More Messages Below
		</div>
	{/if}
	{#if errorState.active}
		<div
			transition:slide={{ easing: quadInOut, duration: 250 }}
			class="cursor-none bg-red-950 text-center"
		>
			{errorState.msg}
		</div>
	{/if}
	<div class="relative border-t">
		<form onsubmit={submitForm}>
			<input
				bind:value={chatInput}
				type="text"
				class="bg-background placeholder:text-muted-foreground h-full w-full p-3 text-sm outline-hidden focus:border-none focus:ring-0 disabled:cursor-not-allowed disabled:opacity-50"
				placeholder="Send message as sir_xin"
			/>
			<!-- svelte-ignore a11y_consider_explicit_label -->
			<button type="submit" class="absolute inset-y-0 right-0 flex cursor-pointer items-center p-3">
				<svg
					xmlns="http://www.w3.org/2000/svg"
					viewBox="0 0 24 24"
					fill="none"
					stroke-width="1.5"
					stroke-linecap="round"
					stroke-linejoin="round"
					class="h-5 w-5 fill-none stroke-slate-100"
				>
					<circle cx="12" cy="12" r="10" />
					<path d="M8 14s1.5 2 4 2 4-2 4-2" />
					<line x1="9" x2="9.01" y1="9" y2="9" />
					<line x1="15" x2="15.01" y1="9" y2="9" />
				</svg>
			</button>
		</form>
	</div>
</div>
