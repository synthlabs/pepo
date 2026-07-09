<script lang="ts">
	import type { Emote } from '$lib/bindings';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { Input } from '$lib/components/ui/input';
	import { gridTemplateColumns, px } from '$lib/settings';
	import EmoteTooltip from './+emote-tooltip.svelte';

	interface Props {
		emotes: Emote[];
		selectedIndex: number;
		onselect: (emote: Emote) => void;
		visible: boolean;
		showSearch?: boolean;
		searchQuery?: string;
		onSearchKeydown?: (e: KeyboardEvent) => void;
		columns?: number;
		maxHeightPx?: number;
		emoteSizePx?: number;
	}

	let {
		emotes,
		selectedIndex,
		onselect,
		visible,
		showSearch = false,
		searchQuery = $bindable(''),
		onSearchKeydown,
		columns = 8,
		maxHeightPx = 192,
		emoteSizePx = 28
	}: Props = $props();

	let itemRefs: HTMLButtonElement[] = $state([]);

	$effect(() => {
		if (visible && itemRefs[selectedIndex]) {
			itemRefs[selectedIndex].scrollIntoView({ block: 'nearest' });
		}
	});
</script>

{#if visible && (showSearch || emotes.length > 0)}
	<div
		class="bg-popover absolute right-0 bottom-full left-0 z-50 overflow-y-auto rounded-lg border p-2 shadow-md"
		style="max-height: {px(maxHeightPx)};"
	>
		{#if showSearch}
			<Input
				bind:value={searchQuery}
				onkeydown={onSearchKeydown}
				placeholder="Search emotes..."
				class="mb-2 h-8"
				autofocus
			/>
		{/if}
		{#if emotes.length > 0}
			<div class="grid gap-1" style="grid-template-columns: {gridTemplateColumns(columns)};">
				{#each emotes as emote, i (`${emote.provider}:${emote.id}`)}
					<Tooltip.Root>
						<Tooltip.Trigger>
							<button
								bind:this={itemRefs[i]}
								class="hover:bg-accent cursor-pointer rounded p-1 {i === selectedIndex
									? 'bg-accent'
									: ''}"
								onclick={() => onselect(emote)}
								type="button"
							>
								<img
									class="inline max-w-none"
									style="height: {px(emoteSizePx)}; min-width: {px(emoteSizePx)};"
									src={emote.url}
									alt={emote.name}
								/>
							</button>
						</Tooltip.Trigger>
						<Tooltip.Content class="p-2">
							<EmoteTooltip {emote} showTier={false} />
						</Tooltip.Content>
					</Tooltip.Root>
				{/each}
			</div>
		{:else}
			<div class="text-muted-foreground px-1 py-2 text-center text-sm">No emotes match</div>
		{/if}
	</div>
{/if}
