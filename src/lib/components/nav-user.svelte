<script lang="ts">
	import * as Avatar from '$lib/components/ui/avatar/index.js';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import * as Sheet from '$lib/components/ui/sheet/index.js';
	import * as Sidebar from '$lib/components/ui/sidebar/index.js';
	import { useSidebar } from '$lib/components/ui/sidebar/index.js';
	import Bell from '@lucide/svelte/icons/bell';
	import Bug from '@lucide/svelte/icons/bug';
	import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import LogOut from '@lucide/svelte/icons/log-out';
	import Plus from '@lucide/svelte/icons/plus';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { ReportWizard } from '$utils/inbound';
	import TwitchIcon from '$lib/resources/twitch.svelte';
	import type { AuthPhase } from '$lib/bindings';

	interface NavAccount {
		id: string;
		login: string;
		avatar: string;
	}

	interface Props {
		accounts: NavAccount[];
		activeAccountId: string | null;
		authPhase: AuthPhase;
		onlogout: () => void;
		onswitchaccount?: (accountId: string) => void;
		onaddaccount?: () => void;
	}

	let {
		accounts,
		activeAccountId,
		authPhase,
		onlogout,
		onswitchaccount = () => {},
		onaddaccount = () => {}
	}: Props = $props();
	const sidebar = useSidebar();
	let reportOpen = $state(false);
	let activeAccount = $derived(
		accounts.find((account) => account.id === activeAccountId) ?? accounts[0] ?? null
	);
	let switchableAccounts = $derived(
		activeAccount ? accounts.filter((account) => account.id !== activeAccount.id) : accounts
	);
	let isSignedIn = $derived(authPhase === 'authorized' && !!activeAccount);
	// TODO: replace this frontend-only check with backend refresh/validation health.
	let authStatus = $derived(
		isSignedIn
			? {
					label: 'Signed in',
					class: 'text-emerald-500'
				}
			: {
					label: 'Reauthentication needed',
					class: 'text-destructive'
				}
	);
</script>

{#snippet providerBadge(sizeClass: string, iconSize: number, surfaceClass: string)}
	<span
		class={[
			'text-primary absolute -right-1 -bottom-1 grid place-items-center rounded-md shadow-sm',
			sizeClass,
			surfaceClass
		]}
	>
		<TwitchIcon size={iconSize} aria-hidden="true" />
	</span>
{/snippet}

<Sidebar.Menu
	class="px-2 py-1 transition-[padding] duration-200 ease-linear group-data-[collapsible=icon]:px-0 group-data-[collapsible=icon]:py-0"
>
	<Sidebar.MenuItem>
		<DropdownMenu.Root>
			<DropdownMenu.Trigger>
				{#snippet child({ props })}
					<Sidebar.MenuButton
						size="sm"
						class="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground mb-1"
						{...props}
					>
						<div class="relative shrink-0">
							<Avatar.Root class="ring-primary/55 size-8 ring-2">
								<Avatar.Image
									src={activeAccount?.avatar ?? ''}
									alt={activeAccount?.login ?? 'Account'}
								/>
								<Avatar.Fallback>{activeAccount?.login ?? '?'}</Avatar.Fallback>
							</Avatar.Root>
							{@render providerBadge('', 16, 'bg-sidebar ring-2 ring-sidebar')}
						</div>
						<div
							class="min-w-0 flex-1 pl-2 text-left text-sm leading-tight group-data-[collapsible=icon]:hidden"
						>
							<span class="truncate font-semibold">{activeAccount?.login ?? 'Account'}</span>
						</div>
						<ChevronsUpDown class="ml-auto size-4 group-data-[collapsible=icon]:hidden" />
					</Sidebar.MenuButton>
				{/snippet}
			</DropdownMenu.Trigger>
			<DropdownMenu.Content
				class="border-border/80 bg-popover w-68 rounded-lg p-1.5 shadow-xl"
				side={sidebar.isMobile ? 'bottom' : 'right'}
				align="end"
				sideOffset={10}
			>
				<DropdownMenu.Label class="p-0 font-normal">
					<div class="flex items-center gap-3 px-2 py-2.5 text-left">
						<div class="relative shrink-0">
							<Avatar.Root class="ring-primary/70 size-10 ring-2">
								<Avatar.Image
									src={activeAccount?.avatar ?? ''}
									alt={activeAccount?.login ?? 'Account'}
								/>
								<Avatar.Fallback>{activeAccount?.login ?? '?'}</Avatar.Fallback>
							</Avatar.Root>
							{@render providerBadge('', 16, 'bg-background ring-2 ring-popover')}
						</div>
						<div class="min-w-0 flex-1">
							<div class="flex min-w-0 items-center gap-1.5">
								<span class="truncate text-sm font-semibold"
									>{activeAccount?.login ?? 'Account'}</span
								>
							</div>
							<div class="text-muted-foreground mt-1 flex items-center gap-1.5 text-xs">
								{#if isSignedIn}
									<CircleCheck class={['size-3.5', authStatus.class]} />
								{:else}
									<TriangleAlert class={['size-3.5', authStatus.class]} />
								{/if}
								<span>{authStatus.label}</span>
							</div>
						</div>
					</div>
				</DropdownMenu.Label>
				<DropdownMenu.Separator class="my-2" />
				<DropdownMenu.Group>
					{#if switchableAccounts.length > 0}
						<DropdownMenu.GroupHeading
							class="text-muted-foreground px-2 pt-0 pb-1.5 text-[10px] font-semibold tracking-wider uppercase"
						>
							Switch to
						</DropdownMenu.GroupHeading>
						{#each switchableAccounts as account (account.id)}
							<DropdownMenu.Item
								class="my-0.5 gap-3 rounded-md px-2 py-2"
								onclick={() => onswitchaccount(account.id)}
							>
								<div class="relative shrink-0">
									<Avatar.Root class="size-8">
										<Avatar.Image src={account.avatar} alt={account.login} />
										<Avatar.Fallback>{account.login}</Avatar.Fallback>
									</Avatar.Root>
									{@render providerBadge('', 13, 'bg-background ring-2 ring-popover')}
								</div>
								<div class="min-w-0 flex-1">
									<div class="truncate text-sm font-semibold">{account.login}</div>
								</div>
							</DropdownMenu.Item>
						{/each}
					{/if}
					<DropdownMenu.Item
						disabled
						aria-label="Add an account coming soon"
						class="text-muted-foreground my-0.5 gap-3 rounded-md px-2 py-1.5 data-disabled:opacity-75"
						onclick={onaddaccount}
					>
						<span class="border-border/80 grid size-7 place-items-center rounded-full border">
							<Plus class="size-4" />
						</span>
						<span>Add an account</span>
					</DropdownMenu.Item>
				</DropdownMenu.Group>
				<DropdownMenu.Separator class="my-1" />
				<DropdownMenu.Group>
					<DropdownMenu.Item disabled class="rounded-md px-2 py-2 data-disabled:opacity-65">
						<Bell />
						Notifications
					</DropdownMenu.Item>
					<DropdownMenu.Item class="rounded-md px-2 py-2" onclick={() => (reportOpen = true)}>
						<Bug />
						Report a bug
					</DropdownMenu.Item>
				</DropdownMenu.Group>
				<DropdownMenu.Separator class="my-1" />
				{#if !isSignedIn}
					<DropdownMenu.Item
						class="text-destructive focus:text-destructive data-highlighted:text-destructive rounded-md px-2 py-2"
						onclick={onlogout}
					>
						<TriangleAlert />
						Reauthenticate
					</DropdownMenu.Item>
					<DropdownMenu.Separator class="my-1" />
				{/if}
				<DropdownMenu.Item
					class="text-red-600 focus:text-red-600 data-highlighted:text-red-600 dark:text-red-400 dark:focus:text-red-400 dark:data-highlighted:text-red-400 rounded-md px-2 py-2"
					onclick={onlogout}
				>
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
