<script lang="ts">
	import '../app.css';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import { Separator } from '$lib/components/ui/separator/index.ts';

	let { children } = $props();
</script>

<Sidebar.Provider>
	<AppSidebar></AppSidebar>
	<main class="w-full">
		<header class="flex h-16 shrink-0 items-center gap-2 border-b px-4">
			<Sidebar.Trigger class="-ml-1" />
			<Separator orientation="vertical" class="mr-2 h-4" />
		</header>
		<div class="flex flex-1 flex-col">
			{@render children?.()}
		</div>
	</main>
</Sidebar.Provider>
