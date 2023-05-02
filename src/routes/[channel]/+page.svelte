<script lang="ts">
	import { page } from '$app/stores';

	import Logger from '$lib/logger/log';

	import Chat from '$lib/components/chat/+chat.svelte';

	Logger.debug($page);
</script>

<Chat channel={$page.params.channel} />
