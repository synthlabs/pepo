<script lang="ts">
	import { goto } from '$app/navigation';
	import { Sanitize } from '$lib/store/channels';

	let inputStr = '';
	function submitForm(event: SubmitEvent) {
		let target = event.target as HTMLFormElement;

		goto(`/chat/${Sanitize(inputStr)}`);
		inputStr = '';
		if (event.target) {
			target.reset();
		}
	}
</script>

<div class="flex flex-col flex-grow items-center justify-center">
	<form class="w-full flex" on:submit|preventDefault={submitForm}>
		<!-- TODO: escape should unfocus and clear this -->
		<input
			type="text"
			bind:value={inputStr}
			class="input input-bordered m-auto w-full input-sm"
			placeholder="Where would you like to go?"
		/>
	</form>
</div>
