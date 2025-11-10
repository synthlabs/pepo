<script lang="ts">
	import type { BadgeRef } from '$lib/bindings';
	import Badge from '$lib/components/chat/+badge.svelte';

	interface Props {
		badges: BadgeRef[];
	}

	let { badges }: Props = $props();
</script>

<span class="inline-flex items-center gap-1 align-middle whitespace-nowrap">
	{#each badges as badge}
		<Badge badge_ref={badge} />
	{/each}
</span>
