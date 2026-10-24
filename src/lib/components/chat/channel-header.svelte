<script lang="ts">
	import BadgeCheck from '@lucide/svelte/icons/badge-check';
	import Clock from '@lucide/svelte/icons/clock';
	import SquarePlay from '@lucide/svelte/icons/square-play';
	import Users from '@lucide/svelte/icons/users';
	import { Button } from '$lib/components/ui/button';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { fitHeaderMetadata, streamUptime, type ChannelHeader } from '$lib/chat/channel-header';

	let { header }: { header: ChannelHeader | null } = $props();
	let container = $state<HTMLDivElement>();
	let now = $state(Date.now());
	let uptime = $derived(streamUptime(header?.startedAt ?? null, now));
	let visibility = $state({ tagCount: 0, showCategory: false, showUptime: false });
	let showLanguage = $derived(Boolean(header?.language) && visibility.tagCount > 0);
	let showTags = $derived(
		Boolean(header?.tags.length) && visibility.tagCount > (header?.language ? 1 : 0)
	);

	$effect(() => {
		const startedAt = header?.startedAt;
		if (!startedAt || streamUptime(startedAt, Date.now()) === null) return;
		now = Date.now();
		const timer = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(timer);
	});

	$effect(() => {
		if (!header || !container) return;
		const node = container;
		const measures = Array.from(node.querySelectorAll<HTMLElement>('[data-header-measure]'));
		const measure = () => {
			const width = (name: string) =>
				node.querySelector(`[data-header-measure="${name}"]`)?.getBoundingClientRect().width ?? 0;
			const availableWidth = Math.max(0, node.getBoundingClientRect().width - 48 - 24);
			visibility = fitHeaderMetadata({
				availableWidth,
				requiredWidth:
					Math.min(width('identity'), availableWidth * 0.4) +
					Math.min(width('title'), 128) +
					width('viewers') +
					24,
				tagWidths: measures
					.filter((el) => ['language', 'tags'].includes(el.dataset.headerMeasure ?? ''))
					.map((el) => el.getBoundingClientRect().width),
				categoryWidth: width('category'),
				uptimeWidth: width('uptime')
			});
		};
		const observer = new ResizeObserver(measure);
		observer.observe(node);
		for (const element of measures) observer.observe(element);
		measure();
		return () => observer.disconnect();
	});
</script>

{#snippet identity(data: ChannelHeader)}
	<span class="min-w-0 truncate text-base font-semibold" title={data.channelName}>
		{data.channelName}
	</span>
	{#if data.isPartner}
		<span
			class="text-primary flex shrink-0"
			role="img"
			aria-label="Twitch partner"
			title="Twitch partner"
		>
			<BadgeCheck class="size-3.5" aria-hidden="true" />
		</span>
	{/if}
{/snippet}

{#snippet tagPill(tag: string)}
	<span
		class="bg-muted text-muted-foreground block max-w-32 truncate rounded-full px-2 py-1 text-xs leading-none"
		title={tag}>{tag}</span
	>
{/snippet}

{#snippet viewers(count: number)}
	<Users class="size-3.5 shrink-0" aria-hidden="true" />
	<span>{count.toLocaleString()}</span>
{/snippet}

{#snippet duration(value: string)}
	<Clock class="size-3 shrink-0" aria-hidden="true" />
	<span>{value}</span>
{/snippet}

{#if header}
	<div
		bind:this={container}
		class="relative flex h-full min-w-0 flex-1 overflow-hidden"
		data-channel-header
	>
		<div
			class="flex h-full w-12 shrink-0 items-center justify-center border-r"
			title="Stream video coming soon"
		>
			<Button
				variant="ghost"
				size="icon"
				class="size-8 [&_svg]:size-5"
				disabled
				aria-label="Stream video coming soon"
			>
				<SquarePlay aria-hidden="true" />
			</Button>
		</div>
		<div class="flex min-w-0 flex-1 items-center gap-3 px-3">
			<div class="flex max-w-[40%] min-w-0 shrink-0 items-center gap-1.5" data-header-identity>
				{@render identity(header)}
			</div>
			<span
				class="min-w-0 flex-1 truncate text-sm font-normal"
				class:text-muted-foreground={header.state === 'offline'}
				title={header.title ?? undefined}
				data-header-title
			>
				{header.state === 'offline' ? 'Offline' : (header.title ?? '')}
			</span>
			{#if header.viewerCount !== null}
				<div class="flex shrink-0 items-center gap-3 text-xs">
					{#if visibility.showCategory && header.category}
						<span
							class="text-primary max-w-40 truncate font-medium"
							title={header.category}
							data-header-category>{header.category}</span
						>
					{/if}
					{#if showLanguage || showTags}
						<div class="flex items-center gap-1" data-header-tags>
							{#if showLanguage && header.language}
								<span data-header-language>{@render tagPill(header.language)}</span>
							{/if}
							{#if showTags}
								<Tooltip.Provider delayDuration={0}>
									<Tooltip.Root>
										<Tooltip.Trigger
											class="bg-muted text-muted-foreground focus-visible:ring-ring cursor-default rounded-full px-2 py-1 text-xs leading-none focus-visible:ring-2 focus-visible:outline-hidden"
											aria-label="Show stream tags"
											data-header-tags-trigger>Tags</Tooltip.Trigger
										>
										<Tooltip.Content
											side="bottom"
											class="flex max-w-[min(20rem,90vw)] flex-wrap gap-1 p-2"
											data-header-tags-tooltip
										>
											{#each header.tags as tag, index (index)}
												<span
													class="bg-muted text-muted-foreground max-w-full rounded-full px-1.5 py-0.5 text-xs leading-snug wrap-anywhere"
													>{tag}</span
												>
											{/each}
										</Tooltip.Content>
									</Tooltip.Root>
								</Tooltip.Provider>
							{/if}
						</div>
					{/if}
					<span
						class="flex items-center gap-1 font-medium text-red-600 tabular-nums dark:text-red-400"
						aria-label={`${header.viewerCount.toLocaleString()} viewers`}
						data-header-viewers
					>
						{@render viewers(header.viewerCount)}
					</span>
					{#if visibility.showUptime && uptime}
						<span
							class="text-muted-foreground flex items-center gap-1 tabular-nums"
							aria-label={`Stream uptime ${uptime}`}
							data-header-uptime
						>
							{@render duration(uptime)}
						</span>
					{/if}
				</div>
			{/if}
		</div>
		<!-- Intrinsic measurements stay independent of the visible metadata selection. -->
		<div
			class="pointer-events-none invisible absolute top-0 left-0 flex w-max items-center gap-3 text-xs whitespace-nowrap"
			aria-hidden="true"
			inert
		>
			<div class="flex items-center gap-1.5" data-header-measure="identity">
				{@render identity(header)}
			</div>
			<span class="text-sm font-normal" data-header-measure="title"
				>{header.state === 'offline' ? 'Offline' : (header.title ?? '')}</span
			>
			{#if header.viewerCount !== null}
				{#if header.category}
					<span class="max-w-40 truncate font-medium" data-header-measure="category"
						>{header.category}</span
					>
				{/if}
				{#if header.language}
					<div class="shrink-0" data-header-measure="language">
						{@render tagPill(header.language)}
					</div>
				{/if}
				{#if header.tags.length}
					<div class="shrink-0" data-header-measure="tags">{@render tagPill('Tags')}</div>
				{/if}
				<span class="flex items-center gap-1 font-medium tabular-nums" data-header-measure="viewers"
					>{@render viewers(header.viewerCount)}</span
				>
				{#if uptime}
					<span class="flex items-center gap-1 tabular-nums" data-header-measure="uptime"
						>{@render duration(uptime)}</span
					>
				{/if}
			{/if}
		</div>
	</div>
{/if}
