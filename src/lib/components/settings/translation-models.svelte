<script lang="ts">
	import {
		ModelManagerPanel,
		defaultModelManagerCopy as text,
		modelInstallationProgress
	} from '$utils/translate/ui';
	import type { InstallationDto } from '$utils/translate';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import Ellipsis from '@lucide/svelte/icons/ellipsis';
	import type { TranslationSession } from '$lib/translation/session.svelte';
	let { session, target }: { session: TranslationSession; target: string } = $props();

	function installationLabel(operation: InstallationDto): string {
		if (!operation.finished)
			return ['completed', 'cancelled', 'failed'].includes(operation.progress.phase)
				? text.awaitingResult
				: text.installPhase(operation.progress.phase);
		if (operation.result) return text.installationComplete;
		return operation.error?.code === 'cancelled'
			? text.installationCancelled
			: text.installationFailed;
	}
</script>

<div class="translation-models" data-translation-models>
	<ModelManagerPanel
		controller={session.controller}
		copy={{ refreshModels: 'Refresh status' }}
		class="text-sm"
	>
		{#snippet header(snapshot)}
			<header class="flex flex-wrap items-start justify-between gap-3">
				<div>
					<h3 id="translation-models" tabindex="-1" class="text-base font-medium outline-none">
						Language models
					</h3>
					<p class="text-muted-foreground mt-1 text-sm">
						Models stored on this device. Target: {text.language(target)}.
					</p>
				</div>
				{#if !snapshot.connected}<Button
						variant="outline"
						size="sm"
						disabled={session.connecting}
						onclick={() => void session.connect()}
						>{session.connecting ? 'Connecting…' : 'Retry connection'}</Button
					>{/if}
			</header>
		{/snippet}
		{#snippet action(control)}
			<Button
				variant={control.kind === 'remove' ? 'destructive' : 'outline'}
				size="sm"
				disabled={control.disabled}
				title={control.title}
				onclick={control.onclick}
				data-model-action={control.kind}>{control.label}</Button
			>
		{/snippet}
		{#snippet modelRow(row, controls)}
			<div class="min-w-0">
				<div
					class="flex min-w-0 flex-col gap-3 @min-[560px]/settings:flex-row @min-[560px]/settings:items-start @min-[560px]/settings:justify-between"
				>
					<div class="min-w-0 flex-1">
						<p class="font-medium">
							{row.pair ? text.pair(row.pair, text.language) : 'Unmanaged model data'}
						</p>
						<p class="text-muted-foreground mt-1 text-xs">
							{row.local ? text.localState(row.local.state) : 'Available to download'}{row.inUse
								? ' · In use'
								: ''}
						</p>
						{#if row.model}<p class="text-muted-foreground mt-1 text-xs">
								{#if !row.local}{text.bytes(row.model.downloadBytes)} download ·
								{/if}{text.bytes(row.model.installedBytes)} on disk
							</p>{/if}
						{#if row.local?.matchesCurrentCatalog === false}<p class="mt-2 text-xs">
								{text.differentCatalog}
							</p>{/if}
						{#if row.local?.state === 'unmanaged'}<p class="mt-2 text-xs">{text.unmanaged}</p>{/if}
					</div>
					<div
						class="flex flex-wrap gap-2 @min-[560px]/settings:max-w-[48%] @min-[560px]/settings:justify-end"
					>
						{#each controls.filter( (control) => ['install', 'verify', 'remove'].includes(control.kind) ) as control (control.kind)}<Button
								variant={control.kind === 'remove' ? 'ghost' : 'outline'}
								size="sm"
								disabled={control.disabled}
								title={control.title}
								onclick={control.onclick}
								data-model-action={control.kind}>{control.label}</Button
							>{/each}
						{#if controls.some((control) => !['install', 'verify', 'remove'].includes(control.kind))}
							<DropdownMenu.Root>
								<DropdownMenu.Trigger
									class="hover:bg-accent focus-visible:ring-ring inline-flex size-8 items-center justify-center rounded-md outline-none focus-visible:ring-2"
									aria-label={`More actions for ${row.pair ? text.pair(row.pair, text.language) : 'model'}`}
									data-model-more
								>
									<Ellipsis class="size-4" />
								</DropdownMenu.Trigger>
								<DropdownMenu.Content align="end">
									{#each controls.filter((control) => !['install', 'verify', 'remove'].includes(control.kind)) as control (control.kind)}
										<DropdownMenu.Item
											disabled={control.disabled}
											title={control.title}
											onclick={control.onclick}
											data-model-action={control.kind}>{control.label}</DropdownMenu.Item
										>
									{/each}
								</DropdownMenu.Content>
							</DropdownMenu.Root>
						{/if}
					</div>
				</div>
				<details class="text-muted-foreground mt-3 border-t pt-3 text-xs" data-model-details>
					<summary class="cursor-pointer">Model details</summary>
					<dl class="mt-3 grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2">
						{#if row.model}
							<dt>Model</dt>
							<dd class="break-all">{row.model.id}</dd>
							<dt>Version</dt>
							<dd>{row.model.version}</dd>
							<dt>Download</dt>
							<dd>{text.bytes(row.model.downloadBytes)}</dd>
							<dt>Catalog digest</dt>
							<dd class="font-mono break-all">{row.model.catalogSha256}</dd>
						{/if}
						{#if row.local}<dt>Directory</dt>
							<dd class="break-all">{row.local.directory}</dd>{/if}
					</dl>
					{#if row.local?.detail}<pre class="mt-3 whitespace-pre-wrap">{row.local.detail}</pre>{/if}
				</details>
			</div>
		{/snippet}
		{#snippet progress(operation, cancelControl)}
			{#if !operation.finished || !operation.result || operation.result.cleanupWarnings.length > 0}
				<div class="grid gap-3 rounded-lg border p-4" data-model-installation={operation.id}>
					<div class="flex flex-wrap items-center justify-between gap-3">
						<div>
							<p class="font-medium">{text.pair(operation.model.pair, text.language)}</p>
							<p class="text-muted-foreground mt-1 text-sm" role="status">
								{installationLabel(operation)}
							</p>
						</div>
						{#if cancelControl}<Button
								variant="outline"
								size="sm"
								disabled={cancelControl.disabled}
								onclick={cancelControl.onclick}
								data-model-action="cancel">{cancelControl.label}</Button
							>{/if}
					</div>
					<progress
						class="h-2 w-full"
						aria-label={text.progressLabel}
						value={modelInstallationProgress(operation)}
						max="100"
					></progress>
					{#if cancelControl}<p class="text-muted-foreground text-xs">{text.cancelHelp}</p>{/if}
					<details class="text-muted-foreground text-xs" data-model-progress-details>
						<summary class="cursor-pointer">Details</summary>
						<dl class="mt-2 grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1">
							<dt>Model</dt>
							<dd class="break-all">{operation.model.id}</dd>
							<dt>Current file</dt>
							<dd class="break-all">{operation.progress.filename ?? '—'}</dd>
							<dt>File</dt>
							<dd>{operation.progress.fileIndex} / {operation.progress.fileCount}</dd>
							<dt>Attempt</dt>
							<dd>{operation.progress.attempt} / {operation.progress.maxAttempts}</dd>
							<dt>Current file download</dt>
							<dd>
								{text.bytes(operation.progress.fileDownloadBytes)} / {text.bytes(
									operation.progress.fileDownloadTotal
								)}
							</dd>
							<dt>Current file decompression</dt>
							<dd>
								{text.bytes(operation.progress.fileInstalledBytes)} / {text.bytes(
									operation.progress.fileInstalledTotal
								)}
							</dd>
							<dt>Downloaded</dt>
							<dd>
								{text.bytes(operation.progress.completedDownloadBytes)} / {text.bytes(
									operation.progress.downloadTotal
								)}
							</dd>
							<dt>Decompressed</dt>
							<dd>
								{text.bytes(operation.progress.completedInstalledBytes)} / {text.bytes(
									operation.progress.installedTotal
								)}
							</dd>
							<dt>Reused</dt>
							<dd>{text.bytes(operation.progress.reusedBytes)}</dd>
							<dt>Network including retries</dt>
							<dd>{text.bytes(operation.progress.networkBytes)}</dd>
						</dl>
						{#if operation.progress.detail}<pre
								class="mt-2 overflow-auto whitespace-pre-wrap">{operation.progress
									.detail}</pre>{/if}
					</details>
					{#if operation.result && operation.result.cleanupWarnings.length > 0}<details>
							<summary>{text.cleanupWarnings}</summary>
							<ul class="mt-2 text-xs">
								{#each operation.result.cleanupWarnings as warning, index (index)}<li>
										{warning}
									</li>{/each}
							</ul>
						</details>{/if}
				</div>
			{/if}
		{/snippet}
	</ModelManagerPanel>
</div>

<style>
	.translation-models :global([data-model-section]) {
		gap: 0;
	}
	.translation-models :global([data-model-section] > h3) {
		margin-bottom: 0.75rem;
	}
	.translation-models :global([data-model-row]) {
		border-radius: 0;
		padding: 1rem;
	}
	.translation-models :global([data-model-row] + [data-model-row]) {
		border-top: 0;
	}
	.translation-models :global([data-model-row]:nth-child(2)) {
		border-radius: 0.5rem 0.5rem 0 0;
	}
	.translation-models :global([data-model-row]:last-child) {
		border-bottom-left-radius: 0.5rem;
		border-bottom-right-radius: 0.5rem;
	}
	@container settings (min-width: 800px) {
		.translation-models :global([data-model-manager]) {
			grid-template-columns: minmax(0, 1fr) auto;
		}
		.translation-models :global([data-model-manager] > *) {
			grid-column: 1 / -1;
		}
		.translation-models :global([data-model-manager] > header) {
			grid-column: 1;
			grid-row: 1;
		}
		.translation-models :global([data-model-manager] > header + div) {
			grid-column: 2;
			grid-row: 1;
			align-self: start;
		}
	}
</style>
