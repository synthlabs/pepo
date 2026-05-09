<script lang="ts">
	import type { Emote } from '$lib/bindings';
	import * as Tooltip from '$lib/components/ui/tooltip';

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
	<Tooltip.Content>
		<p class="font-semibold">{emote.name}</p>
		<p class="text-xs text-muted-foreground">{emote.provider} · {emote.scope}</p>
	</Tooltip.Content>
</Tooltip.Root>
