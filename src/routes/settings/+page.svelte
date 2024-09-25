<script lang="ts">
	import Logger from '$lib/logger/log';
	import { client } from '$lib/store/runes/apiclient.svelte';
	import { TwitchToken } from '$lib/store/runes/token.svelte';
	import { currentUser } from '$lib/store/runes/user.svelte';
	import ShowBadge from '$resources/show.svelte';
	import HideBadge from '$resources/hide.svelte';
	import { GlobalBadgeCache } from '$lib/store/badges';

	let token = new TwitchToken();

	let inputStr = '';

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

	const submitForm = async (event: SubmitEvent) => {
		let target = event.target as HTMLFormElement;

		if (!inputStr) {
			setError('no input');
			return;
		}

		const { username, user_id, client_id, oauth_token } = Object.fromEntries(
			inputStr
				.replace(/;\s*$/, '')
				.split(';')
				.map((pair) => {
					const [k, v] = pair.split('=');
					if (!v) {
						return ['_unknown', pair];
					}
					return [k, v];
				})
		);

		Logger.debug(username, user_id, client_id, oauth_token);
		token.client_id = client_id;
		token.token = oauth_token;

		let valid = await token.validate();

		if (valid) {
			success = true;
			error = false;

			Logger.debug('token validated');
			client.token = token;

			GlobalBadgeCache.UseClient(client.api);

			let u = await client.api.users.getUserById(user_id);
			if (u) {
				Logger.debug('got user');
				currentUser.fromHelix(u);
			} else {
				setError('an error occured while getting your twitch user');
			}
		} else {
			setError('invalid token');
		}
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
			<!-- svelte-ignore a11y-no-static-element-interactions -->
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
			>
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
				/></svg
			>
			<span> Saved! </span>
		</div>
	{/if}
</div>
