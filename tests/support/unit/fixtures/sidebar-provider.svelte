<script lang="ts">
	import * as Sidebar from '$lib/components/ui/sidebar/index';

	let { open, onOpenChange }: { open?: boolean; onOpenChange?: (open: boolean) => void } = $props();
</script>

<Sidebar.Provider {open} {onOpenChange}>
	<Sidebar.Root collapsible="icon">
		<span>Channels</span>
		<Sidebar.Rail />
	</Sidebar.Root>
	<Sidebar.Trigger />
</Sidebar.Provider>
