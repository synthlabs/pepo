<script lang="ts">
	import { goto } from '$app/navigation';

	let inputStr = '';
	function submitForm() {
		goto(`/chat/${inputStr}`);
	}
</script>

<div class="flex flex-col flex-grow w-full items-center justify-center">
	<form class="w-full flex" on:submit|preventDefault={submitForm}>
		<input
			type="text"
			bind:value={inputStr}
			class="input input-bordered m-auto w-1/2"
			placeholder="Where would you like to go?"
		/>
	</form>
</div>
