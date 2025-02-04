<script lang="ts">
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { onMount } from 'svelte';
	import { commands } from '$lib/bindings.ts';

	let banner = $state('');

	onMount(async () => {
		banner = await commands.greet('pog');
	});

	const msgs = Array.from({ length: 55 }).map(
		(_, i, a) => `12:3${i % 10}pm twitch_user${i % 6}: bingo bang, bazinga`
	);

	const evenOddClass = (x: number): string => {
		if (x % 2 === 0) {
			return 'background-color: #040a18';
		}
		return 'background-color: #0f1421';
	};
</script>

<div class="flex h-full w-full flex-col flex-nowrap">
	<div class="flex-grow overflow-y-auto overflow-x-hidden">
		{banner}
		{#each msgs as msg, index}
			<div class="p-2 text-sm" style={evenOddClass(index)}>
				{msg}
			</div>
			<Separator class="" />
		{/each}
	</div>
	<div class="">input</div>
</div>
