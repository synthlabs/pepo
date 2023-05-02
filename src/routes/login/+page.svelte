<script lang="ts">
	import Logger from '$lib/logger/log';
	import { TwitchToken, token } from '$lib/store/token';

	let inputStr = $token?.raw ?? '';

	let show_password = false;
	$: type = show_password ? 'text' : 'password';

	function onInput(event: any) {
		inputStr = event.target.value;
	}
	const submitForm = (event: SubmitEvent) => {
		let target = event.target as HTMLFormElement;

		if (!inputStr) {
			Logger.error('no input');
			return;
		}

		Logger.debug(inputStr, $token);

		let toke = new TwitchToken(inputStr);

		toke
			.validate()
			.then(() => {
				Logger.debug('token validated');
				token.set(toke);
			})
			.catch(Logger.error);
	};
</script>

<div class="flex flex-col h-full p-2">
	<form on:submit|preventDefault={submitForm}>
		<input
			{type}
			value={inputStr}
			on:input={onInput}
			class="w-full input input-bordered focus:input-primary hover:input-primary"
			placeholder="Paste chatterino string here 🥷"
		/>
	</form>

	<button class="btn" on:click={() => (show_password = !show_password)}>
		{show_password ? 'hide' : 'show'}
	</button>
</div>
