<script lang="ts">
	import { onMount } from 'svelte';
	import { commands } from '$lib/bindings.ts';
	import { goto } from '$app/navigation';

	onMount(async () => {
		let result = await commands.login();
		if (result.status == 'ok') {
			goto('/app');
		} else {
			console.log('failure', result.error);
		}
	});
</script>

<div class="flex h-screen w-full flex-col items-center justify-center">
	<div class="flex">
		<img style="height: 96px; width: 96px;" alt="The project logo" src="/pepo.png" />
	</div>
	<div class="flex">pepo &trade;</div>
</div>
