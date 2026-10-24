<script lang="ts">
	import ChannelHeader from './channel-header.svelte';
	import type { ChannelHeader as Header } from '$lib/chat/channel-header';

	let { header, nextHeader = null }: { header: Header | null; nextHeader?: Header | null } =
		$props();
</script>

<ChannelHeader {header} />
<button type="button" data-switch-channel onclick={() => (header = nextHeader)}
	>Switch channel</button
>
