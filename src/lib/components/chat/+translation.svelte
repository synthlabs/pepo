<script lang="ts">
	import type { ChannelMessageTranslation, ChatTranslationLayout } from '$lib/bindings';

	interface Props {
		translation: ChannelMessageTranslation | null;
		authorName: string;
		layout: ChatTranslationLayout;
	}

	let { translation, authorName, layout }: Props = $props();
	let source = $derived(translation?.source_language.trim().toUpperCase() ?? '');
	let target = $derived(translation?.target_language.trim().toUpperCase() ?? '');
	let text = $derived(translation?.translated_text.trim() ?? '');
</script>

{#if text}
	{#if layout === 'message_text'}
		<span class="relative inline-block align-baseline whitespace-nowrap">
			<span aria-hidden="true" class="invisible">
				<span class="font-bold">{authorName}</span>:&#32;
			</span>
			<span
				class="text-primary absolute top-0 right-2 font-mono text-[0.75rem] font-semibold whitespace-nowrap"
			>
				{source} -> {target}
			</span>
		</span>
		<span
			class="text-muted-foreground align-baseline text-[0.75rem] leading-snug text-wrap wrap-anywhere"
			>{text}</span
		>
	{:else}
		<span
			class="text-muted-foreground align-baseline text-[0.75rem] leading-snug text-wrap wrap-anywhere"
		>
			<span class="text-primary font-mono text-[0.75rem] font-semibold whitespace-nowrap">
				{source} -> {target}
			</span>&#32;
			<span>{text}</span>
		</span>
	{/if}
{/if}
