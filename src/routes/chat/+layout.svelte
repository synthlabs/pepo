<script lang="ts">
	import Tabs from '$lib/components/+tabs.svelte';
</script>

<div class="flex flex-shrink-0 w-full">
	<Tabs />
</div>

<div class="flex flex-grow overflow-hidden w-full">
	<slot />
</div>
