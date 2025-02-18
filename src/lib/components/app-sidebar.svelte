<script lang="ts">
	import Calendar from 'lucide-svelte/icons/calendar';
	import House from 'lucide-svelte/icons/house';
	import Inbox from 'lucide-svelte/icons/inbox';
	import Search from 'lucide-svelte/icons/search';
	import Settings from 'lucide-svelte/icons/settings';
	import * as Sidebar from '$lib/components/ui/sidebar/index.ts';
	import * as Avatar from '$lib/components/ui/avatar/index.ts';
	import NavUser from '$lib/components/nav-user.svelte';
	import type { ComponentProps } from 'svelte';

	// Menu items.
	const items = [
		{
			title: 'AlveusSanctuary',
			url: '#',
			icon: House,
			avatar:
				'https://static-cdn.jtvnw.net/jtv_user_pictures/4384f6c4-6608-48f4-b3a7-36d0eb6efbd3-profile_image-300x300.png'
		},
		{
			title: 'hanner',
			url: '#',
			icon: Inbox,
			avatar:
				'https://static-cdn.jtvnw.net/jtv_user_pictures/0712d245-f404-4487-972d-c9caec092b16-profile_image-300x300.png'
		},
		{
			title: 'SeanDaBlack',
			url: '#',
			icon: Calendar,
			avatar:
				'https://static-cdn.jtvnw.net/jtv_user_pictures/4bd8d660-5365-4f2f-ac71-b986710f413a-profile_image-300x300.png'
		},
		{
			title: 'EsfandTV',
			url: '#',
			icon: Search,
			avatar:
				'https://static-cdn.jtvnw.net/jtv_user_pictures/476ee93d-66a6-4e57-b3a9-db1ceb168ad8-profile_image-300x300.png'
		},
		{
			title: 'fanfan',
			url: '#',
			icon: Settings,
			avatar:
				'https://static-cdn.jtvnw.net/jtv_user_pictures/c65b6b35-0702-494e-9f81-0b2939f0adc2-profile_image-300x300.png'
		}
	];

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
					{#each items as item (item.title)}
						<Sidebar.MenuItem isActive={item.title == 'fanfan'}>
							<Sidebar.MenuButton size="lg">
								{#snippet child({ props })}
									<a href={item.url} {...props}>
										<Avatar.Root class="h-8 w-8 rounded-full">
											<Avatar.Image class="rounded-full " src={item.avatar} alt={item.title} />
											<Avatar.Fallback class="rounded-full">{item.title}</Avatar.Fallback>
										</Avatar.Root>
										<span>{item.title}</span>
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
		<NavUser {user} />
	</Sidebar.Footer>
	<Sidebar.Rail />
</Sidebar.Root>
