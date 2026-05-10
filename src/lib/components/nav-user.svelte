<script lang="ts">
	import * as Avatar from '$lib/components/ui/avatar/index.js';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import * as Sheet from '$lib/components/ui/sheet/index.js';
	import * as Sidebar from '$lib/components/ui/sidebar/index.js';
	import { useSidebar } from '$lib/components/ui/sidebar/index.js';
	import Bell from '@lucide/svelte/icons/bell';
	import Bug from '@lucide/svelte/icons/bug';
	import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
	import LogOut from '@lucide/svelte/icons/log-out';
	import { ReportWizard } from '$utils/inbound';

	interface Props {
		user: { name: string; provider: string; avatar: string };
		onlogout: () => void;
	}

	let { user, onlogout }: Props = $props();
	const sidebar = useSidebar();
	let reportOpen = $state(false);
</script>

<Sidebar.Menu
	class="px-2 py-1 transition-[padding] duration-200 ease-linear group-data-[collapsible=icon]:px-0 group-data-[collapsible=icon]:py-0"
>
	<Sidebar.MenuItem>
		<DropdownMenu.Root>
			<DropdownMenu.Trigger>
				{#snippet child({ props })}
					<Sidebar.MenuButton
						size="sm"
						class="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
						{...props}
					>
						<Avatar.Root class="size-8">
							<Avatar.Image src={user.avatar} alt={user.name} />
							<Avatar.Fallback>{user.name}</Avatar.Fallback>
						</Avatar.Root>
						<div class="grid flex-1 text-left text-sm leading-tight">
							<span class="truncate font-semibold">{user.name}</span>
							<span class="text-muted-foreground truncate text-xs">{user.provider}</span>
						</div>
						<ChevronsUpDown class="ml-auto size-4 group-data-[collapsible=icon]:hidden" />
					</Sidebar.MenuButton>
				{/snippet}
			</DropdownMenu.Trigger>
			<DropdownMenu.Content
				class="w-(--bits-dropdown-menu-anchor-width) min-w-56 rounded-lg"
				side={sidebar.isMobile ? 'bottom' : 'right'}
				align="end"
				sideOffset={10}
			>
				<DropdownMenu.Label class="p-0 font-normal">
					<div class="flex items-center gap-2 px-1 py-1.5 text-left text-sm">
						<Avatar.Root class="h-8 w-8">
							<Avatar.Image src={user.avatar} alt={user.name} />
							<Avatar.Fallback>{user.name}</Avatar.Fallback>
						</Avatar.Root>
						<div class="grid flex-1 text-left text-sm leading-tight">
							<span class="truncate font-semibold">{user.name}</span>
							<span class="truncate text-xs">{user.provider}</span>
						</div>
					</div>
				</DropdownMenu.Label>
				<DropdownMenu.Separator />
					<DropdownMenu.Group>
						<DropdownMenu.Item>
							<Bell />
							Notifications
						</DropdownMenu.Item>
						<DropdownMenu.Item onclick={() => (reportOpen = true)}>
							<Bug />
							Report a bug
						</DropdownMenu.Item>
					</DropdownMenu.Group>
				<DropdownMenu.Separator />
				<DropdownMenu.Item onclick={onlogout}>
					<LogOut />
					Log out
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
		</Sidebar.MenuItem>
	</Sidebar.Menu>

<Sheet.Root bind:open={reportOpen}>
	<Sheet.Content class="w-full sm:max-w-xl">
		<ReportWizard onclose={() => (reportOpen = false)} />
	</Sheet.Content>
</Sheet.Root>
