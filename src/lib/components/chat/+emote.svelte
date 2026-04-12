<script lang="ts">
	import type { Emote } from '$lib/bindings';
	import * as Tooltip from '$lib/components/ui/tooltip';

	interface Props {
		emote: Emote;
	}

	let { emote }: Props = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		<img class="inline h-7 min-w-7" src={emote.url} alt={emote.name} />
	</Tooltip.Trigger>
	<Tooltip.Content>
		<p class="font-semibold">{emote.name}</p>
		<p class="text-xs text-muted-foreground">{emote.provider} · {emote.scope}</p>
	</Tooltip.Content>
</Tooltip.Root>
