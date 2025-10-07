<script lang="ts">
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import * as Avatar from '$lib/components/ui/avatar/index.ts';
	import NavUser from '$lib/components/nav-user.svelte';
	import { onMount, type ComponentProps } from 'svelte';
	import { page } from '$app/state';
	import { commands, type Broadcaster } from '$lib/bindings.ts';
	import { Separator } from '$lib/components/ui/separator/index.ts';

	let followed_channels: Broadcaster[] = $state([]);

	onMount(async () => {
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
		<NavUser {user} />
	</Sidebar.Footer>
	<Sidebar.Rail />
</Sidebar.Root>
