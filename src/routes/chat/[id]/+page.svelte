<script lang="ts">
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { onMount } from 'svelte';
	import { commands, type UserToken } from '$lib/bindings.ts';
	import type { UIEventHandler } from 'svelte/elements';

	let banner = $state({} as UserToken);
	let chatDIV = $state<HTMLDivElement>();
	let scrolledAmount = $state(0);
	let isScrolled = $derived(scrolledAmount > 0);

	$inspect(banner);
	$inspect(isScrolled);

	onMount(async () => {
		if (chatDIV) {
			scrollToBottom('instant');
		}
		// let result = await commands.login();
		// if (result.status == 'ok') {
		// 	banner = result.data;
		// } else {
		// 	console.log('failure', result.error);
		// }
	});

	const msgs = Array.from({ length: 55 }).map(
		(_, i, a) => `12:3${i % 10}pm twitch_user${i % 6}: bingo bang, bazinga`
	);

	const evenOddClass = (x: number): string => {
		if (x % 2 === 0) {
			return 'background-color: #040a18';
		}
		return 'background-color: #0f1421';
	};

	const scrollToBottom = async (behavior: ScrollBehavior | undefined = 'smooth') => {
		if (chatDIV) {
			chatDIV.scroll({ top: chatDIV.scrollHeight, behavior });
		}
	};

	const scroll = (e: UIEvent & { currentTarget: EventTarget & HTMLDivElement }): any => {
		if (chatDIV) {
			let scrollArea = chatDIV.scrollHeight - chatDIV.offsetHeight;
			scrolledAmount = Math.max(scrollArea - chatDIV.scrollTop, 0);
		}
	};
</script>

<div class="flex h-full w-full flex-col flex-nowrap">
	<div class="flex-grow overflow-y-auto overflow-x-hidden" bind:this={chatDIV} onscroll={scroll}>
		{banner}
		{#each msgs as msg, index}
			<div class="p-2 text-sm" style={evenOddClass(index)}>
				{msg}
			</div>
			<Separator class="" />
		{/each}
	</div>
	{#if isScrolled}
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="cursor-pointer text-center"
			onclick={() => scrollToBottom('smooth')}
			style="background-color: hsl(262 83.3% 57.8% / 0.25);"
		>
			More Messages Below
		</div>
	{/if}
	<div class="relative border-t">
		<input
			type="text"
			class="h-full w-full bg-background p-3 text-sm outline-none placeholder:text-muted-foreground focus:border-none focus:ring-0 disabled:cursor-not-allowed disabled:opacity-50"
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
