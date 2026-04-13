<script lang="ts">
	import type { Emote } from '$lib/bindings';
	import * as Tooltip from '$lib/components/ui/tooltip';

	interface Props {
		emotes: Emote[];
		selectedIndex: number;
		onselect: (emote: Emote) => void;
		visible: boolean;
	}

	let { emotes, selectedIndex, onselect, visible }: Props = $props();

	let itemRefs: HTMLButtonElement[] = $state([]);

	$effect(() => {
		if (visible && itemRefs[selectedIndex]) {
			itemRefs[selectedIndex].scrollIntoView({ block: 'nearest' });
		}
	});
</script>

{#if visible && emotes.length > 0}
	<div
		class="absolute bottom-full left-0 right-0 z-50 max-h-48 overflow-y-auto border rounded-lg bg-popover p-2 shadow-md"
	>
		<div class="grid grid-cols-8 gap-1">
			{#each emotes as emote, i}
				<Tooltip.Root>
					<Tooltip.Trigger>
						<button
							bind:this={itemRefs[i]}
							class="rounded p-1 hover:bg-accent {i === selectedIndex
								? 'bg-accent'
								: ''}"
							onclick={() => onselect(emote)}
							type="button"
						>
							<img class="inline h-7 min-w-7" src={emote.url} alt={emote.name} />
						</button>
					</Tooltip.Trigger>
					<Tooltip.Content>
						<p class="font-semibold">{emote.name}</p>
						<p class="text-xs text-muted-foreground">
							{emote.provider} · {emote.scope}
						</p>
					</Tooltip.Content>
				</Tooltip.Root>
			{/each}
		</div>
	</div>
{/if}
