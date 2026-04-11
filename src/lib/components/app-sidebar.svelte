<script lang="ts">
	import { onMount, type ComponentProps } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import * as Avatar from '$lib/components/ui/avatar/index.ts';
	import NavUser from '$lib/components/nav-user.svelte';
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { commands, type AuthState, type Broadcaster } from '$lib/bindings.ts';
	import { SyncedState } from 'tauri-svelte-synced-store';
	import Logger from '$utils/log';

	let followed_channels: Broadcaster[] = $state([]);

	let authState = new SyncedState<AuthState>('auth_state', {
		phase: 'unauthorized',
		device_code: '',
		token: null
	});

	let user = $derived({
		name: authState.obj.token?.login ?? '',
		provider: 'Twitch',
		avatar: authState.obj.token?.profile_image_url ?? ''
	});

	async function logout() {
		let result = await commands.logout();
		if (result.status == 'ok') {
			goto('/');
		} else {
			Logger.error('logout failed', result.error);
		}
	}

	onMount(async () => {
		let channels = await commands.getFollowedChannels();
		if (channels.status == 'ok') {
			Logger.debug(channels.data);
			followed_channels = channels.data;
		} else {
			Logger.error('failure', channels.error);
		}
	});

	$inspect(page.url.pathname);

	let {
		ref = $bindable(null),
		collapsible = 'icon',
		...restProps
	}: ComponentProps<typeof Sidebar.Root> = $props();
</script>

<Sidebar.Root bind:ref {collapsible} {...restProps}>
	<Sidebar.Content class="overscroll-none">
		<Sidebar.Group>
			<Sidebar.GroupLabel>Following</Sidebar.GroupLabel>
			<Sidebar.GroupContent>
				<Sidebar.Menu>
					{#each followed_channels as item (item.id)}
						<Sidebar.MenuItem
							isActive={`/app/chat/${item.login}` == page.url.pathname}
							class={`/app/chat/${item.login}` == page.url.pathname ? '' : ''}
						>
							<Sidebar.MenuButton size="lg" class="p-2">
								{#snippet child({ props })}
									<a href={`/app/chat/${item.login}`} {...props}>
										<Avatar.Root class="size-8">
											<Avatar.Image src={item.profile_image_url} alt={item.display_name} />
											<Avatar.Fallback>{item.display_name}</Avatar.Fallback>
										</Avatar.Root>
										<span>{item.display_name}</span>
									</a>
								{/snippet}
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
					{/each}
				</Sidebar.Menu>
			</Sidebar.GroupContent>
		</Sidebar.Group>
	</Sidebar.Content>
	<Sidebar.Footer>
		<Separator class="" />
		<NavUser {user} onlogout={logout} />
	</Sidebar.Footer>
	<Sidebar.Rail />
</Sidebar.Root>
