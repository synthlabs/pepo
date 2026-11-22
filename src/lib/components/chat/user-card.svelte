<script lang="ts">
	import { onMount } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
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
		state: cardState,
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

	let header = $state<HTMLElement>();
	let details = $state<HTMLDivElement>();
	type Action = 'profile' | 'pin';
	let tooltipOpen = $state({ profile: false, pin: false });
	let keyboardFocus = false;
	const tooltipIntent = {
		profile: { keyboardFocus: false, hovered: false },
		pin: { keyboardFocus: false, hovered: false }
	};

	export function measureContentHeight(): number {
		return Math.ceil(
			(header?.getBoundingClientRect().height ?? 0) + (details?.getBoundingClientRect().height ?? 0)
		);
	}

	function clearTooltips() {
		keyboardFocus = false;
		for (const action of ['profile', 'pin'] as const) {
			tooltipIntent[action].keyboardFocus = false;
			tooltipIntent[action].hovered = false;
			tooltipOpen[action] = false;
		}
	}

	function setTooltipOpen(action: Action, open: boolean) {
		const intent = tooltipIntent[action];
		tooltipOpen[action] = open && (intent.hovered || intent.keyboardFocus);
	}

	onMount(() => {
		const keydown = (event: KeyboardEvent) => {
			keyboardFocus = event.key === 'Tab' && !event.altKey && !event.ctrlKey && !event.metaKey;
		};
		const pointerdown = () => {
			keyboardFocus = false;
			tooltipIntent.profile.keyboardFocus = false;
			tooltipIntent.pin.keyboardFocus = false;
		};
		document.addEventListener('keydown', keydown, true);
		document.addEventListener('pointerdown', pointerdown, true);
		return () => {
			document.removeEventListener('keydown', keydown, true);
			document.removeEventListener('pointerdown', pointerdown, true);
		};
	});
</script>

<svelte:window onblur={clearTooltips} />

{#snippet actionButton(
	action: Action,
	label: string,
	tooltip: string,
	onclick: () => void,
	pressed?: boolean
)}
	<Tooltip.Root bind:open={() => tooltipOpen[action], (open) => setTooltipOpen(action, open)}>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				{@const triggerProps = props as HTMLAttributes<HTMLElement>}
				<Button
					{...props}
					variant="ghost"
					size="icon"
					class="size-6 focus-visible:ring-offset-0"
					data-tauri-drag-region="false"
					aria-label={label}
					aria-pressed={pressed}
					onfocus={(event) => {
						const intent = tooltipIntent[action];
						intent.keyboardFocus = keyboardFocus;
						keyboardFocus = false;
						if (intent.keyboardFocus) triggerProps.onfocus?.call(event.currentTarget, event);
					}}
					onblur={(event) => {
						tooltipIntent[action].keyboardFocus = false;
						triggerProps.onblur?.call(event.currentTarget, event);
					}}
					onpointerenter={(event) => {
						tooltipIntent[action].hovered = event.pointerType !== 'touch';
						triggerProps.onpointerenter?.call(event.currentTarget, event);
					}}
					onpointermove={(event) => {
						tooltipIntent[action].hovered = event.pointerType !== 'touch';
						triggerProps.onpointermove?.call(event.currentTarget, event);
					}}
					onpointerleave={(event) => {
						tooltipIntent[action].hovered = false;
						triggerProps.onpointerleave?.call(event.currentTarget, event);
					}}
					onclick={(event) => {
						triggerProps.onclick?.call(event.currentTarget, event);
						onclick();
					}}
				>
					{#if action === 'profile'}
						<ExternalLink class="size-3.5" aria-hidden="true" />
					{:else if cardState.pinned}
						<PinOff class="size-3.5" aria-hidden="true" />
					{:else}
						<Pin class="size-3.5" aria-hidden="true" />
					{/if}
				</Button>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content role="tooltip">{tooltip}</Tooltip.Content>
	</Tooltip.Root>
{/snippet}

<Tooltip.Provider delayDuration={150}>
	<section
		class="bg-background text-foreground flex h-full min-h-0 flex-col"
		aria-label="User profile"
		data-user-card
	>
		<header
			bind:this={header}
			class="flex h-7 shrink-0 cursor-move items-center justify-end gap-1 px-1 select-none"
			data-tauri-drag-region="deep"
			aria-label="Window controls"
		>
			{@render actionButton('profile', 'Open in Twitch', 'Open in Twitch', onprofile)}
			{#if closeOnUnfocus}
				{@render actionButton(
					'pin',
					cardState.pinned ? 'Unpin user card' : 'Pin user card',
					cardState.pinned ? 'Unpin to close on unfocus' : 'Keep this card open',
					() => onpin(!cardState.pinned),
					cardState.pinned
				)}
			{/if}
			{#if cardState.chrome === 'compact'}
				<Button
					variant="ghost"
					size="icon"
					class="size-6 focus-visible:ring-offset-0"
					data-tauri-drag-region="false"
					aria-label="Close user card"
					onclick={onclose}><X class="size-4" /></Button
				>
			{/if}
		</header>
		<div class="min-h-0 flex-1 overflow-y-auto" data-user-card-scroll>
			<div bind:this={details} class="flex flex-col gap-3 px-5 pb-4" data-user-card-details>
				<div class="flex items-center gap-3">
					<Avatar.Root class="size-14 shrink-0">
						<Avatar.Image src={cardState.profile_image_url ?? undefined} alt="" />
						<Avatar.Fallback
							><UserRound
								class="text-muted-foreground size-7"
								aria-hidden="true"
							/></Avatar.Fallback
						>
					</Avatar.Root>
					<div class="min-w-0">
						<h1 class="text-lg leading-snug font-semibold wrap-anywhere">
							{cardState.user.display_name || cardState.user.login}
						</h1>
						<p class="text-muted-foreground mt-0.5 text-sm wrap-anywhere">
							@{cardState.user.login}
						</p>
					</div>
				</div>
				<div class="space-y-2">
					<p class="text-muted-foreground text-xs wrap-anywhere" data-user-card-channel>
						#{cardState.channel_login}
					</p>
					{#if cardState.badges?.length}
						<div class="flex flex-wrap items-center gap-1.5" aria-label="Channel badges">
							{#each cardState.badges as badge (`${badge.set_id}:${badge.id}`)}<Badge
									badge_ref={badge}
									sizePx={20}
								/>{/each}
						</div>
					{/if}
				</div>
				{#if cardState.profile_status === 'loading'}
					<p class="text-muted-foreground text-xs" role="status">Loading profile…</p>
				{:else if cardState.profile_status === 'error' || cardState.profile_status === 'unavailable'}
					<div class="space-y-1">
						<p class="text-muted-foreground text-xs" role="status">
							{cardState.profile_status === 'unavailable'
								? 'This Twitch profile is unavailable.'
								: (cardState.error ?? 'Could not load this profile.')}
						</p>
						<Button variant="link" size="sm" class="h-6 px-0 text-xs" onclick={onretry}
							>Retry</Button
						>
					</div>
				{/if}
			</div>
		</div>
	</section>
</Tooltip.Provider>
