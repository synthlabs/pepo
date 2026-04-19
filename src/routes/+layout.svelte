<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import { checkForAppUpdates } from '$utils/updater';

	let { children } = $props();

	onMount(async () => {
		await checkForAppUpdates('https://github.com/synthlabs/pepo/releases/latest');
	});
</script>

<div class="h-full w-full">
	{@render children?.()}
</div>
