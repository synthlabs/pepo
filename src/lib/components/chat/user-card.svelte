<script lang="ts">
	import type { UserCardState } from '$lib/bindings';
	import * as Avatar from '$lib/components/ui/avatar';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { Button } from '$lib/components/ui/button';
	import Badge from './+badge.svelte';
	import Pin from '@lucide/svelte/icons/pin';
	import PinOff from '@lucide/svelte/icons/pin-off';
	import X from '@lucide/svelte/icons/x';
	import ExternalLink from '@lucide/svelte/icons/external-link';
	import UserRound from '@lucide/svelte/icons/user-round';

	let {
		state,
		closeOnUnfocus,
		onpin,
		onclose,
		onretry,
		onprofile
	}: {
		state: UserCardState;
		closeOnUnfocus: boolean;
		onpin: (pinned: boolean) => void;
		onclose: () => void;
		onretry: () => void;
		onprofile: () => void;
	} = $props();
</script>

<Tooltip.Provider delayDuration={150}>
	<section
		class="bg-background text-foreground flex h-full min-h-0 flex-col"
		aria-label="User profile"
		data-user-card
	>
		{#if state.chrome === 'compact' || closeOnUnfocus}
			<header
				class="flex h-9 shrink-0 items-center justify-end gap-1 px-1"
				data-tauri-drag-region
				aria-label="Window controls"
			>
				{#if closeOnUnfocus}
					<Tooltip.Root>
						<Tooltip.Trigger>
							{#snippet child({ props })}
								<Button
									{...props}
									variant="ghost"
									size="icon"
									class="size-7"
									aria-label={state.pinned ? 'Unpin user card' : 'Pin user card'}
									aria-pressed={state.pinned}
									onclick={() => onpin(!state.pinned)}
								>
									{#if state.pinned}<PinOff class="size-3.5" />{:else}<Pin class="size-3.5" />{/if}
								</Button>
							{/snippet}
						</Tooltip.Trigger>
						<Tooltip.Content
							>{state.pinned ? 'Unpin to close on unfocus' : 'Keep this card open'}</Tooltip.Content
						>
					</Tooltip.Root>
				{/if}
				{#if state.chrome === 'compact'}
					<Button
						variant="ghost"
						size="icon"
						class="size-7"
						aria-label="Close user card"
						onclick={onclose}><X class="size-4" /></Button
					>
				{/if}
			</header>
		{/if}
		<div
			class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 pb-4"
			class:pt-4={state.chrome === 'native' && !closeOnUnfocus}
		>
			<div class="flex items-center gap-3">
				<Avatar.Root class="size-14 shrink-0">
					<Avatar.Image src={state.profile_image_url ?? undefined} alt="" />
					<Avatar.Fallback
						><UserRound class="text-muted-foreground size-7" aria-hidden="true" /></Avatar.Fallback
					>
				</Avatar.Root>
				<div class="min-w-0">
					<h1 class="text-lg leading-snug font-semibold wrap-anywhere">
						{state.user.display_name || state.user.login}
					</h1>
					<p class="text-muted-foreground mt-0.5 text-sm wrap-anywhere">@{state.user.login}</p>
				</div>
			</div>
			<div class="space-y-2">
				<p class="text-muted-foreground text-xs wrap-anywhere">In #{state.channel_login}</p>
				{#if state.badges === null}
					<p class="text-muted-foreground text-xs" data-user-card-badges-unknown>
						Badges not observed yet
					</p>
				{:else if state.badges.length === 0}
					<p class="text-muted-foreground text-xs">No badges observed</p>
				{:else}
					<div class="flex flex-wrap items-center gap-1.5" aria-label="Channel badges">
						{#each state.badges as badge (`${badge.set_id}:${badge.id}`)}<Badge
								badge_ref={badge}
								sizePx={20}
							/>{/each}
					</div>
				{/if}
			</div>
			{#if state.profile_status === 'loading'}
				<p class="text-muted-foreground text-xs" role="status">Loading profile…</p>
			{:else if state.profile_status === 'error' || state.profile_status === 'unavailable'}
				<div class="space-y-1">
					<p class="text-muted-foreground text-xs" role="status">
						{state.profile_status === 'unavailable'
							? 'This Twitch profile is unavailable.'
							: (state.error ?? 'Could not load this profile.')}
					</p>
					<Button variant="link" size="sm" class="h-6 px-0 text-xs" onclick={onretry}>Retry</Button>
				</div>
			{/if}
			<div class="mt-auto pt-1">
				<Button variant="outline" size="sm" class="h-8" onclick={onprofile}
					>View on Twitch <ExternalLink class="size-3.5" aria-hidden="true" /></Button
				>
			</div>
		</div>
	</section>
</Tooltip.Provider>
