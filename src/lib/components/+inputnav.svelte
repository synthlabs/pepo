<script lang="ts">
	import { beforeUpdate, afterUpdate } from 'svelte';
	import { goto } from '$app/navigation';
	import { Sanitize } from '$lib/store/channels';
	import { createEventDispatcher } from 'svelte';

	let dispatch = createEventDispatcher();

	let inputStr = '';

	export const reset = () => {
		inputStr = '';
	};

	function submitForm(event: SubmitEvent) {
		let target = event.target as HTMLFormElement;
		goto(`/chat/${Sanitize(inputStr)}`);
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
