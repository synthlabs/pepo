<script lang="ts">
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import { isTauriMobile } from '$lib/tauri';
	import { cn } from '$lib/utils';
	import { page } from '$app/state';
	import { SyncedState } from 'tauri-svelte-synced-store';
	import type { AuthState, ChannelCache } from '$lib/bindings.ts';
	import ChannelHeader from '$lib/components/chat/channel-header.svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { hasUsableAuth } from '$lib/auth';
	import { getNormalizedAppSettings, updateAppSettings } from '$lib/stores/settings.svelte';
	import SettingsStatus from '$lib/components/settings/settings-status.svelte';
	import { InternalRoot } from '$internal';
	import { channelHeader } from '$lib/chat/channel-header';

	let { children } = $props();
	let normalizedAppSettings = $derived(getNormalizedAppSettings());

	let authState = new SyncedState<AuthState>('auth_state');

	$effect(() => {
		if (authState.ready && !hasUsableAuth(authState.obj)) {
			goto(resolve('/'));
		}
	});

	let channelCache = new SyncedState<ChannelCache>('channel_cache', { channels: {} });
	let channelStatus = $derived(
		page.params.id ? (channelCache.obj.channels[page.params.id] ?? null) : null
	);
	let header = $derived(channelHeader(page.params.id, channelStatus));
	let isSettings = $derived(page.url.pathname === resolve('/app/settings'));
</script>

<Sidebar.Provider
	class="h-dvh min-h-0 overflow-hidden"
	open={normalizedAppSettings.layout.sidebar_open}
	onOpenChange={(open) => {
		void updateAppSettings({ layout: { sidebar_open: open } });
	}}
>
	<AppSidebar collapsible="icon"></AppSidebar>
	<main class="flex h-full min-h-0 w-full max-w-full min-w-0 flex-col flex-nowrap overflow-hidden">
		<header class="flex h-12 min-w-0 shrink-0 items-center overflow-hidden border-b">
			<Sidebar.Trigger class="mx-2 shrink-0 md:hidden" />
			{#if isSettings}
				<div class="flex items-center gap-2 px-4">
					<h1 class="text-base font-semibold">Settings</h1>
					<SettingsStatus />
				</div>
			{:else}
				<ChannelHeader {header} />
			{/if}
		</header>
		<div class={cn('flex min-h-0 w-full grow overflow-hidden', isTauriMobile && 'mb-10')}>
			{#key page.params.id}
				{@render children?.()}
			{/key}
		</div>
	</main>
	<InternalRoot />
</Sidebar.Provider>
