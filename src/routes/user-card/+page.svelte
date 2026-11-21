<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { commands, type UserCardState, type Result } from '$lib/bindings';
	import { UserCardSession } from '$lib/chat/user-card-session';
	import UserCard from '$lib/components/chat/user-card.svelte';
	import { Button } from '$lib/components/ui/button';
	import { getNormalizedAppSettings, appSettings } from '$lib/stores/settings.svelte';
	import { appErrors } from '$lib/errors/controller';

	let cardState = $state<UserCardState | null>(null);
	let error = $state<string | null>(null);
	let closeOnUnfocus = $derived(getNormalizedAppSettings().chat.user_card_close_on_unfocus);
	let session: UserCardSession;

	function unwrap<T>(result: Result<T, string>): T {
		if (result.status === 'error') throw result.error;
		return result.data;
	}

	async function perform(action: () => Promise<unknown>) {
		try {
			await action();
		} catch (error) {
			appErrors.capture(error, {
				source: 'user-card.action',
				title: 'Could not complete user-card action'
			});
		}
	}

	function close() {
		void perform(() => getCurrentWindow().close());
	}

	async function start() {
		error = null;
		try {
			await session.start();
		} catch (failure) {
			error = 'Could not load this user card.';
			appErrors.capture(failure, { source: 'user-card.load', title: error });
			await perform(async () => unwrap(await commands.readyUserCard()));
		}
	}

	onMount(() => {
		const window = getCurrentWindow();
		session = new UserCardSession(
			{
				listen: (receive) =>
					window.listen<UserCardState>('user-card-state', ({ payload }) => receive(payload)),
				snapshot: async () => unwrap(await commands.getUserCardState()),
				ready: async () => {
					await appSettings.initialized;
					await tick();
					unwrap(await commands.readyUserCard());
				}
			},
			(value) => {
				cardState = value;
			}
		);
		void start();
		return () => session.stop();
	});
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape') {
			event.preventDefault();
			close();
		}
	}}
/>

<svelte:head
	><title>{cardState ? `${cardState.user.display_name} — Pepo` : 'User card — Pepo'}</title
	></svelte:head
>

<div class="h-dvh min-h-0 overflow-hidden">
	{#if cardState}
		<UserCard
			state={cardState}
			{closeOnUnfocus}
			onclose={close}
			onpin={(pinned) => {
				void perform(async () => unwrap(await commands.setUserCardPinned(pinned)));
			}}
			onretry={() => {
				void perform(async () => unwrap(await commands.refreshUserCard()));
			}}
			onprofile={() => {
				void perform(() => openUrl(`https://www.twitch.tv/${cardState!.user.login}`));
			}}
		/>
	{:else if error}
		<div class="flex h-full flex-col items-start justify-center gap-3 p-5">
			<p role="alert" class="text-sm">{error}</p>
			<div class="flex gap-2">
				<Button
					size="sm"
					onclick={() => {
						void start();
					}}>Retry</Button
				><Button size="sm" variant="outline" onclick={close}>Close</Button>
			</div>
		</div>
	{/if}
</div>
