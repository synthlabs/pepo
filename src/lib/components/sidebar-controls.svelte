<script lang="ts">
	import * as Sidebar from '$lib/components/ui/sidebar';
	import SearchForm from '$lib/components/search-form.svelte';

	let {
		value = $bindable(''),
		onsubmit
	}: { value?: string; onsubmit?: (channel: string) => void } = $props();
</script>

<Sidebar.Header
	class="relative h-12 gap-0 p-0 transition-[height] duration-200 ease-linear group-data-[collapsible=icon]:h-24"
>
	<div
		class="absolute top-2 right-12 left-2 min-w-0 transition-[top,right] duration-200 ease-linear group-data-[collapsible=icon]:top-14 group-data-[collapsible=icon]:right-2"
	>
		<SearchForm bind:value {onsubmit} />
	</div>
	<div class="absolute top-2 right-2 flex size-8 items-center justify-center">
		<Sidebar.Trigger class="size-8" />
	</div>
</Sidebar.Header>
