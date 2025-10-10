<script lang="ts">
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { onDestroy, onMount, tick } from 'svelte';
	import { commands, type ChannelMessage, type UserToken } from '$lib/bindings.ts';
	import { type UnlistenFn, listen } from '@tauri-apps/api/event';
	import { cn } from '$lib/utils';
	import { page } from '$app/state';

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
	let channel_name = $derived(page.params.id);
	let msgs: string[] = $state([]);

	let un_sub: UnlistenFn;

	$inspect(banner);
	$inspect(isScrolled);

	$effect(() => {
		console.log('joining channel:', channel_name);
		commands.joinChat(channel_name).then((result) => {
			if (result.status == 'ok') {
				console.log(result.data);
			}
		});
	});

	onMount(async () => {
		console.log('subbing to chat messages');
		un_sub = await listen<ChannelMessage>(`chat_message:${channel_name}`, (event) => {
			const msg = JSON.parse(event.payload.payload);
			msgs.push(
				`[${event.payload.ts.substring(11, 19)}] ${msg.chatter_user_name}: ${msg.message.text}`
			);
			if (msgs.length > CHAT_MESSAGE_LIMIT) msgs.shift();
			processAutoscroll();
		});

		scrollToBottom();
	});

	onDestroy(async () => {
		console.log('unsubbing');
		if (un_sub) {
			un_sub();
		}
		commands.leaveChat(channel_name).then(console.log);
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
		console.log('autoscroll debounce START');
		setTimeout(() => {
			console.log('autoscroll debounce FINISH');
			forceAutoscrollDebounce = false;
		}, 2000);
	};

	const scrollToBottom = () => {
		forceAutoscrollDebounceFn();
		processAutoscroll();
	};
</script>

<div class="flex h-full w-full flex-col flex-nowrap">
	<div
		class="grow overflow-x-hidden overflow-y-auto"
		bind:this={chatDIV}
		onscroll={refreshScrollAmount}
	>
		{#each msgs as msg, index}
			<div
				class={cn(
					'px-2 py-1 text-sm text-wrap break-words',
					index % 2 === 0 ? 'bg-content-primary' : 'bg-content-secondary'
				)}
			>
				{msg}
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
	<div class="relative border-t">
		<input
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
	</div>
</div>
