<script lang="ts">
	import type { Badge as BadgeType } from '$lib/bindings';
	import Badge from '$lib/components/chat/+badge.svelte';

	interface Props {
		badges: BadgeType[];
	}

	let { badges }: Props = $props();
</script>

<span class="inline-flex items-center gap-1 align-middle whitespace-nowrap">
	{#each badges as badge}
		<Badge {badge} />
	{/each}
</span>
