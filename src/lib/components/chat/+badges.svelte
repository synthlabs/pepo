<script lang="ts">
	import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';

	import Badge from '$lib/components/chat/+badge.svelte';

	export let message: TwitchPrivateMessage | null = null;
	let badges: Map<string, string> = new Map<string, string>();

	if (message) {
		badges = message.userInfo.badges;
	}
</script>

<span class="inline-flex whitespace-nowrap">
	{#each [...badges] as badge}
		<Badge {badge} />
	{/each}
</span>
