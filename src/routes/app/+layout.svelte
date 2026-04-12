<script lang="ts">
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { isTauriMobile } from '$lib/tauri';
	import { cn } from '$lib/utils';
	import { page } from '$app/state';
	import { SyncedState } from 'tauri-svelte-synced-store';
	import type { ChannelCache } from '$lib/bindings.ts';
	import Users from '@lucide/svelte/icons/users';
	import * as Tooltip from '$lib/components/ui/tooltip/index.ts';

	let { children } = $props();

	let channelCache = new SyncedState<ChannelCache>('channel_cache', { channels: {} });
	let channelStatus = $derived(
		page.params.id ? (channelCache.obj.channels[page.params.id] ?? null) : null
	);
</script>

<Sidebar.Provider>
	<AppSidebar collapsible="icon"></AppSidebar>
	<main class="flex max-h-dvh w-full max-w-full min-w-0 flex-col flex-nowrap">
		<header class="flex h-12 min-w-0 shrink-0 items-center gap-2 overflow-hidden border-b px-4">
			<Sidebar.Trigger class="-ml-1" />

			{#if !isTauriMobile}
				<Separator orientation="vertical" class="mr-2 h-4" />
			{/if}

			{#if channelStatus?.stream}
				<Tooltip.Provider>
					<Tooltip.Root>
						<Tooltip.Trigger
							class="text-muted-foreground min-w-0 flex-1 cursor-default truncate text-left text-sm"
						>
							{channelStatus.stream.title}
						</Tooltip.Trigger>
						<Tooltip.Content>
							{channelStatus.stream.title}
						</Tooltip.Content>
					</Tooltip.Root>
				</Tooltip.Provider>
				<span class="text-muted-foreground flex shrink-0 items-center gap-1 text-sm">
					<Users class="size-3.5" />
					{channelStatus.stream.viewer_count.toLocaleString()}
				</span>
			{/if}
		</header>
		<div class={cn('flex w-full grow overflow-hidden', isTauriMobile && 'mb-10')}>
			{#key page.params.id}
				{@render children?.()}
			{/key}
		</div>
	</main>
</Sidebar.Provider>
