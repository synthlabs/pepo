<script lang="ts">
	import { Separator } from '$lib/components/ui/separator/index.ts';

	const tags = Array.from({ length: 25 }).map((_, i, a) => `v1.2.0-beta.${a.length - i}`);
</script>

<div class="flex h-full w-full flex-col flex-nowrap">
	<div class="flex-grow overflow-y-auto overflow-x-hidden">
		<h4 class="mb-4 text-sm font-medium leading-none">Tags</h4>
		{#each tags as tag}
			<div class="text-sm">
				{tag}
			</div>
			<Separator class="my-2" />
		{/each}
	</div>
	<div class="">input</div>
</div>
