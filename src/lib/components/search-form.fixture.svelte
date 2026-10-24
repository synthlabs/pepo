<script lang="ts">
	import * as Sidebar from './ui/sidebar/index';
	import SidebarControls from './sidebar-controls.svelte';

	let {
		open = $bindable(true),
		value = $bindable(''),
		onsubmit
	}: { open?: boolean; value?: string; onsubmit?: (channel: string) => void } = $props();
</script>

<Sidebar.Provider bind:open>
	<Sidebar.Root collapsible="icon">
		<SidebarControls bind:value {onsubmit} />
		<Sidebar.Rail />
	</Sidebar.Root>
	<Sidebar.Trigger class="md:hidden" />
	<button type="button" data-outside>Outside</button>
	<output data-query>{value}</output>
</Sidebar.Provider>
