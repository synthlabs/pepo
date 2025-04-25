<script lang="ts">
	import '../app.css';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { isTauriMobile } from '$lib/tauri';
	import { cn } from '$lib/utils';
	import { onMount } from 'svelte';
	import { commands } from '$lib/bindings.ts';
	import { goto } from '$app/navigation';

	let { children } = $props();

	onMount(async () => {});
</script>

<Sidebar.Provider>
	<AppSidebar collapsible="icon"></AppSidebar>
	<main class="flex max-h-dvh w-full flex-col flex-nowrap">
		<header class="flex h-12 shrink-0 items-center gap-2 border-b px-4">
			<Sidebar.Trigger class="-ml-1" />

			{#if !isTauriMobile}
				<Separator orientation="vertical" class="mr-2 h-4" />
			{/if}
		</header>
		<div class={cn('flex w-full flex-grow overflow-hidden', isTauriMobile && 'mb-10')}>
			{@render children?.()}
		</div>
	</main>
</Sidebar.Provider>
