<script lang="ts">
	import type { Emote } from '$lib/bindings';

	interface Props {
		emote: Emote;
		showTier?: boolean;
	}

	let { emote, showTier = true }: Props = $props();
</script>

<div
	class="flex flex-wrap items-center justify-center gap-3"
	style="max-width: min(92vw, 320px);"
>
	<div class="flex max-h-[72px] max-w-[112px] shrink-0 items-center justify-center overflow-hidden">
		<img
			class="block h-auto max-h-[72px] w-auto max-w-[112px] object-contain"
			src={emote.url}
			alt={emote.name}
		/>
	</div>
	<div class="flex min-w-0 max-w-[240px] flex-1 basis-48 flex-col gap-0.5 text-left">
		<p class="break-words font-semibold leading-snug [overflow-wrap:anywhere]">{emote.name}</p>
		<p class="break-words text-xs text-muted-foreground [overflow-wrap:anywhere]">
			{emote.provider} · {emote.scope}
		</p>
		{#if showTier && emote.tier}
			<p class="break-words text-xs text-muted-foreground [overflow-wrap:anywhere]">
				Tier: {emote.tier}
			</p>
		{/if}
	</div>
</div>
