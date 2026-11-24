<script lang="ts">
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import WifiOff from '@lucide/svelte/icons/wifi-off';
	import type { NetworkHealthSnapshot, NetworkIssue } from '$lib/bindings';
	import { commands } from '$lib/bindings';
	import { networkHealth } from '$lib/network/health';
	import { appErrors } from '$lib/errors/controller';
	import { Button } from '$lib/components/ui/button';
	import * as Sheet from '$lib/components/ui/sheet';
	import { NetworkStatusController } from '$lib/network/status.svelte';

	let {
		controller = new NetworkStatusController(),
		showButton = true
	}: {
		controller?: NetworkStatusController;
		showButton?: boolean;
	} = $props();
	let issues = $derived(controller.issues);
	let retryError = $state('');
	let now = $state(Date.now());
	const title = $derived(
		issues.some((issue) => issue.feature === 'Chat')
			? 'Reconnecting to chat'
			: issues.some((issue) => issue.core)
				? 'Checking Twitch session'
				: issues.some((issue) => !issue.retryable)
					? 'Some features are unavailable'
					: 'Some features are catching up'
	);

	onMount(() => {
		const unsubscribe = networkHealth.subscribe((value) => {
			controller.issues = value;
		});
		let disposed = false;
		let unlisten: (() => void) | undefined;
		const timer = setInterval(() => {
			now = Date.now();
		}, 1000);
		if ('__TAURI_INTERNALS__' in window)
			void (async () => {
				try {
					const stop = await listen<NetworkHealthSnapshot>('network_status', (event) =>
						networkHealth.receive(event.payload)
					);
					if (disposed) {
						stop();
						return;
					}
					unlisten = stop;
					const snapshot = await commands.getNetworkStatus();
					if (!disposed) networkHealth.receive(snapshot);
				} catch (error) {
					appErrors.capture(
						error,
						{ source: 'network.status', title: 'Connection status unavailable' },
						false
					);
				}
			})();
		return () => {
			disposed = true;
			unsubscribe();
			unlisten?.();
			clearInterval(timer);
		};
	});

	async function retry(issue: NetworkIssue) {
		retryError = '';
		try {
			await commands.retryNetworkOperation(issue.id);
		} catch {
			retryError = 'Could not start the retry. Try again.';
		}
	}
	function report(issue: NetworkIssue) {
		const incident = appErrors.capture(
			{
				feature: issue.feature,
				category: issue.category,
				message: issue.message,
				failures: issue.failures,
				affected: issue.affected
			},
			{ source: 'network', title: `${issue.feature} connection problem` },
			false
		);
		controller.closeForReport();
		appErrors.openReport(incident);
	}
</script>

{#if showButton && issues.length}
	<Button
		variant="ghost"
		size="sm"
		class="text-muted-foreground mx-3 ml-auto max-w-full min-w-0 shrink-0 gap-2"
		onclick={(event) => controller.show(event.currentTarget)}
		aria-label={`${title}. Open connection details`}
		data-network-status
	>
		<WifiOff class="size-4 shrink-0" />
		<span class="max-w-44 truncate text-xs sm:max-w-none" role="status">{title}</span>
	</Button>
{/if}

<Sheet.Root bind:open={controller.open}>
	<Sheet.Content
		data-network-drawer
		class="flex w-full flex-col gap-6 overflow-y-auto p-6 sm:max-w-lg"
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			controller.restoreFocus();
		}}
	>
		<Sheet.Header>
			<Sheet.Title>Connection details</Sheet.Title>
			<Sheet.Description>
				See what is waiting for a connection and which saved data is available.
			</Sheet.Description>
		</Sheet.Header>
		{#if !issues.length}
			<p class="text-sm" role="status">Everything has caught up.</p>
		{:else}
			<ul class="divide-y" data-network-issues>
				{#each issues as issue (issue.id)}
					<li class="space-y-2 py-4 first:pt-0">
						<h3 class="text-sm font-semibold">{issue.feature}</h3>
						<p class="text-muted-foreground text-sm">
							{issue.core
								? issue.message
								: issue.cached
									? 'Using saved data while a refresh is unavailable.'
									: issue.retryable
										? 'Waiting for data to load.'
										: 'This refresh needs attention. Try again or inspect the details.'}
						</p>
						<p class="text-muted-foreground text-xs">
							{issue.affected} affected {issue.affected === 1 ? 'operation' : 'operations'} · {issue.failures}
							failed {issue.failures === 1 ? 'attempt' : 'attempts'}
						</p>
						{#if issue.retry_at !== null}<p class="text-muted-foreground text-xs">
								Next retry {issue.retry_at > now
									? `in ${Math.ceil((issue.retry_at - now) / 1000)} seconds`
									: 'soon'}.
							</p>{/if}
						<details class="text-xs">
							<summary class="text-muted-foreground cursor-pointer py-1">
								Diagnostic details
							</summary>
							<p class="mt-2 wrap-anywhere whitespace-pre-wrap">{issue.message}</p>
							<p class="text-muted-foreground mt-2">
								Last failure: {new Date(issue.last_failure_at).toLocaleTimeString()}
							</p>
						</details>
						<div class="flex flex-wrap gap-2 pt-1">
							{#if issue.can_retry}<Button variant="outline" size="sm" onclick={() => retry(issue)}>
									Retry now
								</Button>{/if}
							<Button variant="ghost" size="sm" onclick={() => report(issue)}>
								Report problem
							</Button>
						</div>
					</li>
				{/each}
			</ul>
		{/if}
		{#if retryError}<p class="text-destructive text-sm" role="alert">{retryError}</p>{/if}
	</Sheet.Content>
</Sheet.Root>
