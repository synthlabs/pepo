<script lang="ts">
	import * as Sidebar from './ui/sidebar/index';
	import SearchForm from './search-form.svelte';

	let {
		open = $bindable(true),
		value = $bindable(''),
		onsubmit
	}: { open?: boolean; value?: string; onsubmit?: (channel: string) => void } = $props();
</script>

<Sidebar.Provider bind:open>
	<Sidebar.Trigger />
	<Sidebar.Root collapsible="icon">
		<Sidebar.Header>
			<SearchForm bind:value {onsubmit} />
		</Sidebar.Header>
		<Sidebar.Rail />
	</Sidebar.Root>
	<button type="button" data-outside>Outside</button>
	<output data-query>{value}</output>
</Sidebar.Provider>
