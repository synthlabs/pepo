<script lang="ts">
	import { beforeUpdate, afterUpdate } from 'svelte';
	import { goto } from '$app/navigation';
	import { createEventDispatcher } from 'svelte';
	import { channels as channelCache, Sanitize } from '$lib/store/channels';
	import Logger from '$lib/logger/log';

	let dispatch = createEventDispatcher();

	let inputStr = '';

	export const reset = () => {
		inputStr = '';
	};

	// TODO: clean this up 🤮
	function submitForm(event: SubmitEvent) {
		let target = event.target as HTMLFormElement;

		const chan = Sanitize(inputStr);
		goto(`/chat/${chan}`);

		target.reset();
		dispatch('inputNavSubmitted', inputStr);
	}
</script>

<div class="flex flex-col flex-grow items-center justify-center">
	<form class="w-full" method="dialog" on:submit|preventDefault={submitForm}>
		<input
			type="text"
			bind:value={inputStr}
			class="input input-bordered m-auto w-full input-sm"
			placeholder="Go to channel..."
		/>
	</form>
</div>
