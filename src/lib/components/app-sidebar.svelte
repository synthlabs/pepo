<script lang="ts">
	import Calendar from 'lucide-svelte/icons/calendar';
	import House from 'lucide-svelte/icons/house';
	import Inbox from 'lucide-svelte/icons/inbox';
	import Search from 'lucide-svelte/icons/search';
	import Settings from 'lucide-svelte/icons/settings';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import * as Avatar from '$lib/components/ui/avatar/index.ts';
	import NavUser from '$lib/components/nav-user.svelte';
	import { onMount, type ComponentProps } from 'svelte';
	import { page } from '$app/state';
	import { commands, type Broadcaster } from '$lib/bindings.ts';
	import { Separator } from '$lib/components/ui/separator/index.ts';

	let followed_channels: Broadcaster[] = $state([]);

	onMount(async () => {
		let result = await commands.login();
		if (result.status == 'ok') {
			console.log(result.data);
		} else {
			console.log('failure', result.error);
		}

		let channels = await commands.getFollowedChannels();
		if (channels.status == 'ok') {
			console.log(channels.data);
			followed_channels = channels.data;
		} else {
			console.log('failure', channels.error);
		}
	});

	$inspect(page.url.pathname);

	const user = {
		name: 'sir_xin',
		provider: 'Twitch',
		avatar:
			'https://static-cdn.jtvnw.net/jtv_user_pictures/07cc2a6a-b550-4ae3-abf3-c13d4f5c2d74-profile_image-300x300.png'
	};

	let {
		ref = $bindable(null),
		collapsible = 'icon',
		...restProps
	}: ComponentProps<typeof Sidebar.Root> = $props();
</script>

<Sidebar.Root bind:ref {collapsible} {...restProps}>
	<Sidebar.Content>
		<Sidebar.Group>
			<Sidebar.GroupLabel>Following</Sidebar.GroupLabel>
			<Sidebar.GroupContent>
				<Sidebar.Menu>
					{#each followed_channels as item (item.id)}
						<Sidebar.MenuItem
							isActive={`/chat/${item.login}` == page.url.pathname}
							class={`/chat/${item.login}` == page.url.pathname ? 'pl-1 pr-2' : 'px-2'}
						>
							<Sidebar.MenuButton size="lg">
								{#snippet child({ props })}
									<a href={`/chat/${item.login}`} {...props}>
										<Avatar.Root class="h-8 w-8">
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
		<NavUser {user} />
	</Sidebar.Footer>
	<Sidebar.Rail />
</Sidebar.Root>
