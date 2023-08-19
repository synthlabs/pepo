<script lang="ts">
	import type { TwitchPrivateMessage } from '@twurple/chat/lib/commands/TwitchPrivateMessage';
	import Badge from '$lib/components/chat/+badge.svelte';
	import Logger from '$lib/logger/log';

	export let message: TwitchPrivateMessage | null = null;
	let badges: Map<string, string> = new Map<string, string>();

	$: if (badges) {
		badges.forEach((k, v) => {
			Logger.trace(k, v);
		});
	}

	if (message) {
		badges = message.userInfo.badges;
	}
</script>

<span class="inline-flex whitespace-nowrap items-center align-middle gap-1">
	{#each [...badges] as [id, version]}
		<Badge {id} {version} />
	{/each}
</span>
