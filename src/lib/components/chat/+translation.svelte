<script lang="ts">
	import { quadInOut } from 'svelte/easing';
	import { slide } from 'svelte/transition';
	import type { ChannelMessageTranslation } from '$lib/bindings';

	interface Props {
		translation: ChannelMessageTranslation | null;
	}

	let { translation }: Props = $props();
	let source = $derived(translation?.source_language.trim().toUpperCase() ?? '');
	let target = $derived(translation?.target_language.trim().toUpperCase() ?? '');
	let text = $derived(translation?.translated_text.trim() ?? '');
</script>

{#if text}
	<div
		transition:slide={{ easing: quadInOut, duration: 40 }}
		class="text-muted-foreground mt-0.5 flex min-w-0 gap-2 pl-16 text-[0.8125rem] leading-snug sm:pl-28"
	>
		<span class="text-primary shrink-0 font-mono text-xs font-semibold whitespace-nowrap">
			{source} -> {target}
		</span>
		<span class="min-w-0 text-wrap">{text}</span>
	</div>
{/if}
