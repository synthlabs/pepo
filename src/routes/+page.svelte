<script lang="ts">
	import { Separator } from '$lib/components/ui/separator/index.ts';

	const tags = Array.from({ length: 55 }).map((_, i, a) => `v1.2.0-beta.${a.length - i}`);

	const evenOddClass = (x: number): string => {
		if (x % 2 === 0) {
			return 'background-color: #040a18';
		}
		return 'background-color: #0f1421';
	};
</script>

<div class="flex h-full w-full flex-col flex-nowrap">
	<div class="flex-grow overflow-y-auto overflow-x-hidden">
		<h4 class="mb-4 text-sm font-medium leading-none">Tags</h4>
		{#each tags as tag, index}
			<div class="py-2 text-sm" style={evenOddClass(index)}>
				{tag}
			</div>
			<Separator class="" />
		{/each}
	</div>
	<div class="">input</div>
</div>
