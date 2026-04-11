<script lang="ts">
	import { Label } from '$lib/components/ui/label/index.ts';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import SearchIcon from '@lucide/svelte/icons/search';

	let { value = $bindable('') }: { value?: string } = $props();
</script>

<form onsubmit={(e) => e.preventDefault()}>
	<Sidebar.Group class="py-0">
		<Sidebar.GroupContent class="relative">
			<Label for="search" class="sr-only">Search</Label>
			<Sidebar.Input id="search" placeholder="Filter..." class="min-w-0 ps-8 transition-opacity duration-200 ease-linear group-data-[collapsible=icon]:opacity-0" bind:value />
			<SearchIcon
				class="pointer-events-none absolute inset-s-2 top-1/2 size-4 -translate-y-1/2 opacity-50 select-none"
			/>
		</Sidebar.GroupContent>
	</Sidebar.Group>
</form>
