<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import { page } from '$app/state';
	import { checkForAppUpdates } from '$utils/updater';

	let { children } = $props();

	$inspect(page.params.id);

	onMount(async () => {
		await checkForAppUpdates();
	});
</script>

<div class="h-full w-full">
	{@render children?.()}
</div>
