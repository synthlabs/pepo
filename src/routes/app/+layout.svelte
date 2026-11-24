<script lang="ts">
	import { onDestroy, setContext, untrack } from 'svelte';
	import { TranslationSession, TRANSLATION_CONTEXT } from '$lib/translation/session.svelte';
	import TranslationConfirmation from '$lib/components/settings/translation-confirmation.svelte';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import { isTauriMobile } from '$lib/tauri';
	import { cn } from '$lib/utils';
	import { page } from '$app/state';
	import { SyncedState } from 'tauri-svelte-synced-store';
	import type { AuthState, ChannelCache } from '$lib/bindings.ts';
	import ChannelHeader from '$lib/components/chat/channel-header.svelte';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import { Button } from '$lib/components/ui/button';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { hasUsableAuth } from '$lib/auth';
	import { getNormalizedAppSettings, settingsState } from '$lib/stores/settings.svelte';
	import SettingsStatus from '$lib/components/settings/settings-status.svelte';
	import NetworkStatus from '$lib/components/network-status.svelte';
	import NetworkStatusIcon from '$lib/components/network-status-icon.svelte';
	import { NETWORK_STATUS_CONTEXT, NetworkStatusController } from '$lib/network/status.svelte';
	import { InternalRoot } from '$internal';
	import { channelHeader } from '$lib/chat/channel-header';
	import { userDirectory } from '$lib/chat/users.svelte';

	let { children } = $props();
	let normalizedAppSettings = $derived(getNormalizedAppSettings());
	const translation = setContext(TRANSLATION_CONTEXT, new TranslationSession());
	const network = setContext(NETWORK_STATUS_CONTEXT, new NetworkStatusController());
	$effect(() => {
		const ready = settingsState.ready;
		const enabled = normalizedAppSettings.translation.enabled;
		untrack(() => translation.start(ready, enabled));
	});
	onDestroy(() => {
		userDirectory.clear();
		void translation.dispose();
	});

	let authState = new SyncedState<AuthState>('auth_state');

	$effect(() => {
		if (authState.ready && !hasUsableAuth(authState.obj)) {
			goto(resolve('/'));
		}
	});

	let channelCache = new SyncedState<ChannelCache>('channel_cache', { channels: {} });
	onDestroy(() => {
		void authState.close();
		void channelCache.close();
	});
	let channelStatus = $derived(
		page.params.id ? (channelCache.obj.channels[page.params.id] ?? null) : null
	);
	let header = $derived(channelHeader(page.params.id, channelStatus));
	let isSettings = $derived(page.url.pathname === resolve('/app/settings'));

	function goBack() {
		const canGoBack = window.navigation?.canGoBack ?? window.history.length > 1;
		if (canGoBack) window.history.back();
		else void goto(resolve('/app'), { replaceState: true });
	}
</script>

<Sidebar.Provider class="h-dvh min-h-0 overflow-hidden">
	<AppSidebar collapsible="icon"></AppSidebar>
	<main class="flex h-full min-h-0 w-full max-w-full min-w-0 flex-col flex-nowrap overflow-hidden">
		<header class="flex h-12 min-w-0 shrink-0 items-center overflow-hidden border-b">
			<Sidebar.Trigger class="mx-2 shrink-0 md:hidden" />
			{#if isSettings}
				<div
					class="flex h-full w-12 shrink-0 items-center justify-center border-r"
					data-settings-back-slot
				>
					<Tooltip.Provider delayDuration={0}>
						<Tooltip.Root>
							<Tooltip.Trigger onclick={goBack}>
								{#snippet child({ props })}
									<Button
										{...props}
										variant="ghost"
										size="icon"
										class="size-8 [&_svg]:size-5"
										aria-label="Back"
										data-settings-back
									>
										<ArrowLeft aria-hidden="true" />
									</Button>
								{/snippet}
							</Tooltip.Trigger>
							<Tooltip.Content data-settings-back-tooltip>Back</Tooltip.Content>
						</Tooltip.Root>
					</Tooltip.Provider>
				</div>
				<div class="flex items-center gap-2 px-4">
					<h1 class="text-base font-semibold">Settings</h1>
					<SettingsStatus />
					<NetworkStatusIcon />
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
	<NetworkStatus controller={network} showButton={false} />
	<InternalRoot />
	<TranslationConfirmation session={translation} />
</Sidebar.Provider>
