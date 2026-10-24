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
	<div
		class="absolute top-2 right-2 flex size-8 items-center justify-center group-data-[collapsible=icon]:top-1 group-data-[collapsible=icon]:right-0 group-data-[collapsible=icon]:left-0 group-data-[collapsible=icon]:h-10 group-data-[collapsible=icon]:w-auto"
	>
		<Sidebar.Trigger
			class="group-data-[collapsible=icon]:hover:bg-sidebar-accent group-data-[collapsible=icon]:hover:text-sidebar-accent-foreground group-data-[collapsible=icon]:active:bg-sidebar-accent group-data-[collapsible=icon]:active:text-sidebar-accent-foreground group-data-[collapsible=icon]:focus-visible:ring-sidebar-ring size-8 group-data-[collapsible=icon]:h-10 group-data-[collapsible=icon]:w-full group-data-[collapsible=icon]:rounded-lg group-data-[collapsible=icon]:focus-visible:ring-offset-0"
		/>
	</div>
</Sidebar.Header>
