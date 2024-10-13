<script lang="ts">
	import { page } from '$app/stores';

	import Logger from '$lib/logger/log';

	import Chat from '$lib/components/chat/+chat.svelte';

	Logger.debug($page);
</script>

{#key $page.params.channel}
	<Chat name={$page.params.channel} />
{/key}
