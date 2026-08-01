<script lang="ts">
	import { onMount } from 'svelte';
	import { commands } from './bindings';

	let commit = $state('unknown');
	let label = $derived(`internal * ${commit}`);

	onMount(() => {
		void loadBuildInfo();
	});

	async function loadBuildInfo() {
		try {
			const buildInfo = await commands.internalBuildInfo();
			commit = buildInfo.app_commit || 'unknown';
		} catch {
			commit = 'unknown';
		}
	}
</script>

<div
	class="text-muted-foreground/45 pointer-events-none fixed top-14 right-3 z-50 px-2 py-1 font-mono text-[8px] opacity-70 select-none"
	aria-label={label}
>
	{label}
</div>
