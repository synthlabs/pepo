<script lang="ts">
	import { goto } from '$app/navigation';
	import Logger from '$lib/logger/log';

	let inputStr = '';
	function submitForm() {
		goto(`/chat/${inputStr}`);
	}
</script>

<div class="flex flex-col items-center justify-center w-full h-screen">
	<form class="w-full flex" on:submit|preventDefault={submitForm}>
		<input
			type="text"
			bind:value={inputStr}
			class="input input-bordered m-auto w-1/2"
			placeholder="Where would you like to go?"
		/>
	</form>
</div>
