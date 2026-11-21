<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { SystemMessage, ChatSettings } from '$lib/bindings';
	import Username from './username.svelte';
	import Info from '@lucide/svelte/icons/info';
	import Shield from '@lucide/svelte/icons/shield';
	import Clock from '@lucide/svelte/icons/clock';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import CircleX from '@lucide/svelte/icons/circle-x';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import Logger from '$utils/log';
	import { systemMessageLink } from '$lib/chat/system-messages';
	import { cn } from '$lib/utils';

	let {
		message,
		centered = false,
		showBackground = false,
		timestamp,
		settings
	}: {
		message: SystemMessage;
		centered?: boolean;
		showBackground?: boolean;
		timestamp?: Snippet;
		settings?: ChatSettings;
	} = $props();
	const icons = {
		info: Info,
		shield: Shield,
		clock: Clock,
		success: CircleCheck,
		warning: TriangleAlert,
		error: CircleX
	};

	function openLink(event: MouseEvent, url: string) {
		event.preventDefault();
		void openUrl(url).catch((error) => Logger.error('Could not open system message link', error));
	}
</script>

<div
	class={cn(
		'text-foreground/75 w-full px-2 py-1 text-sm',
		showBackground && 'bg-content-system',
		centered ? 'flex items-center gap-3 text-center' : 'block'
	)}
	data-chat-message-index={message.index}
	data-chat-system-message
	role="group"
	aria-label="System message"
>
	{#if centered}
		<span aria-hidden="true" data-chat-system-message-line class="bg-border h-px min-w-6 flex-1"
		></span>
	{/if}
	<div data-chat-system-message-content class="min-w-0 text-wrap wrap-anywhere">
		{#if !centered}{@render timestamp?.()}{/if}
		{#if message.icon}
			{@const Icon = icons[message.icon]}
			<span class="inline-grid h-[1lh] w-5 place-items-center align-top">
				<Icon class="size-4" aria-hidden="true" />
			</span>
		{/if}
		{#each message.segments as segment, i (i)}
			{#if segment.type === 'text'}{segment.text}{:else if segment.type === 'emphasis'}<strong
					>{segment.text}</strong
				>{:else if segment.type === 'user'}<Username
					userId={segment.user_id}
					login={segment.login}
					displayName={segment.display_name}
					kind="system"
					channelId={message.broadcaster_user_id}
					channelLogin={message.broadcaster_user_login}
					{settings}
				/>{:else if segment.type === 'link'}
				{@const url = systemMessageLink(segment.url)}
				{#if url}<a
						href={url}
						rel="external"
						class="focus-visible:outline-foreground underline underline-offset-2 focus-visible:rounded-sm focus-visible:outline-2 focus-visible:outline-offset-2"
						onclick={(event) => openLink(event, url)}>{segment.text}</a
					>{:else}{segment.text}{/if}
			{/if}
		{/each}
	</div>
	{#if centered}
		<span aria-hidden="true" data-chat-system-message-line class="bg-border h-px min-w-6 flex-1"
		></span>
	{/if}
</div>
