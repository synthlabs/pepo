<script lang="ts">
	import { SyncedState } from 'tauri-svelte-synced-store';
	import { CircleCheckBig } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { commands, type AuthState } from '$lib/bindings';
	import { cn } from '$lib/utils.js';
	import Logger from '$lib/logger/log';

	let authState = new SyncedState<AuthState>('auth_state', {
		phase: 'unauthorized',
		device_code: '',
		token: null
	});

	let loading: boolean = $derived(
		authState.obj.phase === 'waitingForDeviceCode' || authState.obj.phase === 'waitingForAuth'
	);

	$inspect(loading);

	async function login() {
		Logger.debug('login hook');
		authState.obj.phase = 'waitingForDeviceCode';
		await authState.sync();

		let result = await commands.login();
		if (result.status == 'ok') {
			goto('/app');
		} else {
			Logger.error('failure', result.error);
		}
	}
</script>

<div class="bg-sidebar flex h-screen w-full flex-col items-center justify-center">
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<button
		class={cn(
			'text-muted-foreground flex flex-col items-center justify-center gap-2 rounded-lg px-8 py-2 text-center hover:underline',
			loading ? '' : 'hover:bg-secondary hover:cursor-pointer'
		)}
		onclick={login}
		disabled={!!loading}
	>
		<img class="flex size-28" alt="The project logo" src="/pepo.png" />
		<span class="flex flex-col items-center justify-center">
			{#if authState.obj.phase === 'waitingForDeviceCode'}
				<span class="loading text-primary w-12"></span>
			{:else if authState.obj.phase === 'waitingForAuth'}
				<span
					class="bg-muted-foreground text-accent w-3xs rounded-lg px-4 py-2 text-center font-mono text-2xl font-bold tracking-wider hover:cursor-text"
					>{authState.obj.device_code}</span
				>
			{:else if authState.obj.phase === 'authorized'}
				<CircleCheckBig class="text-primary" />
			{:else}
				<span class="pb-2"> Login </span>
			{/if}
		</span>
	</button>
	{#if authState.obj.phase === 'waitingForAuth'}
		<span class="text-muted-foreground flex p-2 text-center text-xs hover:cursor-text">
			or goto <br />
			https://www.twitch.tv/activate?device-code={authState.obj.device_code}
		</span>
	{/if}
</div>

<style>
	.loading {
		pointer-events: none;
		aspect-ratio: 1;
		vertical-align: middle;
		background-color: currentColor;
		display: inline-block;
		-webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' style='shape-rendering:auto;' width='200px' height='200px' viewBox='0 0 100 100' preserveAspectRatio='xMidYMid'%3E%3Cpath fill='none' stroke='black' stroke-width='10' stroke-dasharray='205.271 51.318' d='M24.3 30C11.4 30 5 43.3 5 50s6.4 20 19.3 20c19.3 0 32.1-40 51.4-40C88.6 30 95 43.3 95 50s-6.4 20-19.3 20C56.4 70 43.6 30 24.3 30z' stroke-linecap='round' style='transform:scale(0.8);transform-origin:50px 50px'%3E%3Canimate attributeName='stroke-dashoffset' repeatCount='indefinite' dur='2s' keyTimes='0;1' values='0;256.589'/%3E%3C/path%3E%3C/svg%3E");
		mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' style='shape-rendering:auto;' width='200px' height='200px' viewBox='0 0 100 100' preserveAspectRatio='xMidYMid'%3E%3Cpath fill='none' stroke='black' stroke-width='10' stroke-dasharray='205.271 51.318' d='M24.3 30C11.4 30 5 43.3 5 50s6.4 20 19.3 20c19.3 0 32.1-40 51.4-40C88.6 30 95 43.3 95 50s-6.4 20-19.3 20C56.4 70 43.6 30 24.3 30z' stroke-linecap='round' style='transform:scale(0.8);transform-origin:50px 50px'%3E%3Canimate attributeName='stroke-dashoffset' repeatCount='indefinite' dur='2s' keyTimes='0;1' values='0;256.589'/%3E%3C/path%3E%3C/svg%3E");
		-webkit-mask-position: 50%;
		mask-position: 50%;
		-webkit-mask-size: 100%;
		mask-size: 100%;
		-webkit-mask-repeat: no-repeat;
		mask-repeat: no-repeat;
	}
</style>
