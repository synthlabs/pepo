<script lang="ts">
	import { onMount, type ComponentProps } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import * as Avatar from '$lib/components/ui/avatar/index.ts';
	import NavUser from '$lib/components/nav-user.svelte';
	import SearchForm from '$lib/components/search-form.svelte';
	import { Separator } from '$lib/components/ui/separator/index.ts';
	import { commands, type AuthState, type Broadcaster, type ChannelInfo } from '$lib/bindings.ts';
	import { SyncedState } from 'tauri-svelte-synced-store';
	import Logger from '$utils/log';

	let followed_channels: Broadcaster[] = $state([]);
	let extra_channel: ChannelInfo | null = $state(null);
	let search = $state('');
	let filtered_channels = $derived(
		search
			? followed_channels.filter((c) => c.login.includes(search.toLowerCase()))
			: followed_channels
	);
	let current_channel = $derived(page.url.pathname.match(/^\/app\/chat\/(.+)$/)?.[1] ?? null);
	let is_extra = $derived(
		current_channel && !followed_channels.some((f) => f.login === current_channel)
	);

	async function joinChannel(login: string) {
		if (!followed_channels.some((f) => f.login === login)) {
			const result = await commands.getChannelInfo(login);
			if (result.status === 'ok') {
				extra_channel = result.data;
			}
		}
		goto(`/app/chat/${login}`);
	}

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

{#snippet channelItem(login: string, displayName: string, imageUrl: string | undefined)}
	<Sidebar.MenuItem isActive={`/app/chat/${login}` === page.url.pathname}>
		<Sidebar.MenuButton size="lg" class="p-2">
			{#snippet child({ props })}
				<a href={`/app/chat/${login}`} {...props}>
					<Avatar.Root class="size-8">
						<Avatar.Image src={imageUrl} alt={displayName} />
						<Avatar.Fallback>{displayName}</Avatar.Fallback>
					</Avatar.Root>
					<span>{displayName}</span>
				</a>
			{/snippet}
		</Sidebar.MenuButton>
	</Sidebar.MenuItem>
{/snippet}

<Sidebar.Root bind:ref {collapsible} {...restProps}>
	<Sidebar.Header>
		<SearchForm bind:value={search} onsubmit={joinChannel} />
	</Sidebar.Header>
	<Sidebar.Content class="overscroll-none">
		{#if is_extra && extra_channel}
			<Sidebar.Group>
				<Sidebar.GroupContent>
					<Sidebar.Menu
						class="px-2 py-1 transition-[padding] duration-200 ease-linear group-data-[collapsible=icon]:px-0 group-data-[collapsible=icon]:py-0"
					>
						{@render channelItem(extra_channel.broadcaster_login, extra_channel.broadcaster_name, extra_channel.profile_image_url)}
					</Sidebar.Menu>
				</Sidebar.GroupContent>
			</Sidebar.Group>
		{/if}
		<Sidebar.Group>
			<Sidebar.GroupLabel>Following</Sidebar.GroupLabel>
			<Sidebar.GroupContent>
				<Sidebar.Menu
					class="px-2 py-1 transition-[padding] duration-200 ease-linear group-data-[collapsible=icon]:px-0 group-data-[collapsible=icon]:py-0"
				>
					{#each filtered_channels as item (item.id)}
						{@render channelItem(item.login, item.display_name, item.profile_image_url)}
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
