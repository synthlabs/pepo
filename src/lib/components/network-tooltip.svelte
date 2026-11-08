<script lang="ts">
	import type { NetworkSummary } from '$lib/network/summary';
	let { summary }: { summary: NetworkSummary } = $props();
</script>

<div class="space-y-2 text-xs" data-network-tooltip>
	{#each summary.groups as group (group.heading)}
		<div>
			<p>{group.heading}</p>
			<ul class="mt-1 list-disc space-y-0.5 pl-4">
				{#each group.features as feature (feature)}<li class="wrap-anywhere">{feature}</li>{/each}
			</ul>
		</div>
	{/each}
</div>
