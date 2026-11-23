<script lang="ts">
	import Root from '../../../../../src/routes/+layout.svelte';
	import Chat from '../../../../../src/routes/app/chat/[id]/+page.svelte';
	import { page, native } from './native.svelte';
	import { appErrors } from '$lib/errors/controller';
	import { getNormalizedAppSettings, updateAppSettings } from '$lib/stores/settings.svelte';

	const pause = appErrors.pause.bind(appErrors);
	const resume = appErrors.resume.bind(appErrors);
	appErrors.pause = (id, reason) => {
		native.pauses.push(['pause', reason]);
		pause(id, reason);
	};
	appErrors.resume = (id, reason) => {
		native.pauses.push(['resume', reason]);
		resume(id, reason);
	};
	native.errors = appErrors;
	native.settings = getNormalizedAppSettings;
	native.updateSettings = updateAppSettings;
</script>

<Root>
	<main class="flex h-screen w-full">
		{#key page.params.id}
			<Chat />
		{/key}
	</main>
</Root>
