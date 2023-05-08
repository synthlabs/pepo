<script lang="ts">
	import Logger from '$lib/logger/log';
	import { TwitchToken, token } from '$lib/store/token';
	import ShowBadge from '$resources/show.svelte';
	import HideBadge from '$resources/hide.svelte';

	let inputStr = $token?.raw ?? '';

	let show_password = false;
	$: type = show_password ? 'text' : 'password';

	let success = false;
	let error = false;
	let errorReason = 'Something went wrong';

	function onInput(event: any) {
		inputStr = event.target.value;
	}

	function setError(e: string) {
		success = false;
		error = true;
		errorReason = e;
		Logger.error(errorReason);
	}

	const submitForm = (event: SubmitEvent) => {
		let target = event.target as HTMLFormElement;

		if (!inputStr) {
			setError('no input');
			return;
		}

		Logger.debug(inputStr, $token);

		let toke = new TwitchToken(inputStr);

		toke
			.validate()
			.then(() => {
				Logger.debug('token validated');
				token.set(toke);
				success = true;
				error = false;
			})
			.catch(setError);
	};
</script>

<div class="flex flex-col h-full p-2">
	<form class="w-full" on:submit|preventDefault={submitForm}>
		<div class="relative">
			<input
				{type}
				value={inputStr}
				on:input={onInput}
				class="w-full input input-bordered"
				class:input-error={error}
				style="padding-right: 3rem;"
				placeholder="Paste chatterino string here 🥷"
			/>

			<!-- svelte-ignore a11y-click-events-have-key-events -->
			<div
				class="absolute inset-y-0 right-0 flex items-center pr-3 cursor-pointer stroke-slate-400 hover:stroke-slate-200"
				on:click={() => (show_password = !show_password)}
			>
				{#if !show_password}
					<ShowBadge />
				{:else}
					<HideBadge />
				{/if}
			</div>
		</div>
		<div class="flex flex-row justify-end pt-1 gap-x-2">
			<!-- <button class="flex btn btn-sm">Cancel</button> -->
			<button type="submit" class="flex btn btn-sm">Save</button>
		</div>
	</form>
	{#if error}
		<div class="flex flex-row gap-x-2">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				class="stroke-error flex-shrink-0 h-6 w-6"
				fill="none"
				viewBox="0 0 24 24"
				><path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"
				/></svg
			>
			<span>
				{errorReason}
			</span>
		</div>
	{/if}

	{#if success}
		<div class="flex flex-row gap-x-2">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				class="stroke-success flex-shrink-0 h-6 w-6"
				fill="none"
				viewBox="0 0 24 24"
				><path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"
				/></svg
			>
			<span> Saved! </span>
		</div>
	{/if}
</div>
