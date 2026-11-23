<script lang="ts">
	import * as Sidebar from '$lib/components/ui/sidebar';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import ChannelHeader from '$lib/components/chat/channel-header.svelte';

	let {
		open = $bindable(true),
		language = 'English',
		tags = true,
		long = false
	}: {
		open?: boolean;
		language?: string | null;
		tags?: boolean;
		long?: boolean;
	} = $props();

	const header = $derived({
		channelName: 'Maya',
		isPartner: true,
		state: 'live' as const,
		title: long ? 'A very long stream title 🦊 '.repeat(20) : 'Original stream title 🦊',
		category: 'Terraria',
		language,
		tags: tags
			? ['Hair', 'DropsEnabled', 'AnExceptionallyLongTagThatShouldRemainFullyReadable']
			: [],
		viewerCount: 4646,
		startedAt: new Date(Date.now() - 10065000).toISOString()
	});
</script>

<Sidebar.Provider bind:open class="h-dvh min-h-0 overflow-hidden">
	<AppSidebar collapsible="icon" />
	<main class="flex h-full min-w-0 flex-1 flex-col overflow-hidden">
		<header
			class="flex h-12 min-w-0 shrink-0 items-center overflow-hidden border-b"
			data-preview-header
		>
			<Sidebar.Trigger class="mx-2 shrink-0 md:hidden" data-mobile-opener />
			<ChannelHeader {header} />
		</header>
	</main>
</Sidebar.Provider>
