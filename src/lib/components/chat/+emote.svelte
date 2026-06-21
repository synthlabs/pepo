<script lang="ts">
	import type { Emote } from '$lib/bindings';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import EmoteTooltip from './+emote-tooltip.svelte';

	interface Props {
		emote: Emote;
		sizePx?: number;
	}

	let { emote, sizePx = 28 }: Props = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		<img
			class="inline max-w-none"
			style="height: {sizePx}px; min-width: {sizePx}px;"
			src={emote.url}
			alt={emote.name}
		/>
	</Tooltip.Trigger>
	<Tooltip.Content class="p-2">
		<EmoteTooltip {emote} />
	</Tooltip.Content>
</Tooltip.Root>
