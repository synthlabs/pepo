<script lang="ts">
	import { onMount } from 'svelte';
	import {
		ReportWizard,
		connectErrorCapture,
		logHandledError,
		type ErrorSnapshot
	} from '$utils/inbound';
	import { appErrors } from '$lib/errors/controller';
	import { getNormalizedAppSettings } from '$lib/stores/settings.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Sheet from '$lib/components/ui/sheet';

	let snapshot = $state.raw<ErrorSnapshot>({ notifications: [], report: null });
	let captureWindow = $derived(getNormalizedAppSettings().diagnostics.capture_window_errors);

	onMount(() => {
		const unsubscribe = appErrors.subscribe((value) => (snapshot = value));
		const stopTimers = appErrors.start();
		return () => {
			unsubscribe();
			stopTimers();
		};
	});

	// Reconnects when the setting changes; replayed native records are deduplicated by id.
	$effect(() => connectErrorCapture(appErrors, captureWindow ? window : undefined));
</script>

{#if snapshot.notifications.length}
	<div
		class="pointer-events-none fixed right-4 bottom-4 z-[100] flex w-[min(24rem,calc(100vw-2rem))] flex-col gap-2"
	>
		{#each snapshot.notifications as notice (notice.id)}
			{@const title = typeof notice.title === 'function' ? notice.title() : notice.title}
			<section
				class="bg-background pointer-events-auto rounded-md border p-4 shadow-lg"
				aria-label="Error"
				data-error-notice={notice.id}
				onpointerenter={() => appErrors.pause(notice.id, 'hover')}
				onpointerleave={() => appErrors.resume(notice.id, 'hover')}
				onfocusin={() => appErrors.pause(notice.id, 'focus')}
				onfocusout={(event) => {
					if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
						appErrors.resume(notice.id, 'focus');
					}
				}}
			>
				<p class="text-sm font-semibold" role="status">
					{title ?? 'Error detected'}
					{#if notice.count > 1}<span class="text-muted-foreground">({notice.count})</span>{/if}
				</p>
				{#if !title}
					<p class="text-muted-foreground mt-1 line-clamp-2 text-xs">{notice.error.message}</p>
				{/if}
				<div class="mt-3 flex justify-end gap-2">
					<Button size="sm" variant="outline" onclick={() => appErrors.dismiss(notice.id)}
						>Dismiss</Button
					>
					<Button size="sm" onclick={() => appErrors.openReport(notice)}>Report this</Button>
				</div>
			</section>
		{/each}
	</div>
{/if}

<Sheet.Root
	open={snapshot.report !== null}
	onOpenChange={(open) => {
		if (!open) appErrors.closeReport();
	}}
>
	<Sheet.Content class="w-full sm:max-w-xl">
		{#if snapshot.report}
			{#key snapshot.report.id}
				<ReportWizard
					prefilledError={snapshot.report.error}
					submitSnapshot={true}
					copy={{ errorHelp: 'The selected error.' }}
					onerror={(error) => logHandledError(error, 'app.report')}
					onclose={() => appErrors.closeReport()}
				/>
			{/key}
		{/if}
	</Sheet.Content>
</Sheet.Root>
