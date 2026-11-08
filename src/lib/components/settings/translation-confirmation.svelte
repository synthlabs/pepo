<script lang="ts">
	import { AlertDialog } from 'bits-ui';
	import { buttonVariants } from '$lib/components/ui/button';
	import type { TranslationSession } from '$lib/translation/session.svelte';
	import { defaultModelManagerCopy as text } from '$utils/translate/ui';
	let { session }: { session: TranslationSession } = $props();
	const request = $derived(session.confirmation);
	const setup = $derived(session.setupTarget !== null && request !== null && 'plan' in request);
	const verb = $derived(
		request
			? {
					install: 'Download',
					repair: 'Repair',
					replace: 'Replace',
					remove: 'Remove',
					clear_downloads: 'Clear downloads for'
				}[request.action]
			: ''
	);
</script>

<AlertDialog.Root
	open={request !== null}
	onOpenChange={(open) => {
		if (!open) session.respond(false);
	}}
>
	<AlertDialog.Portal>
		<AlertDialog.Overlay class="fixed inset-0 z-50 bg-black/50" />
		<AlertDialog.Content
			class="bg-background fixed top-1/2 left-1/2 z-50 max-h-[90dvh] w-[calc(100%-2rem)] max-w-lg -translate-x-1/2 -translate-y-1/2 overflow-auto rounded-lg border p-6 shadow-lg"
		>
			{#if request}
				<AlertDialog.Title class="text-lg font-semibold"
					>{setup
						? 'Download translation support?'
						: `${verb} ${text.pair(request.pair, text.language)}?`}</AlertDialog.Title
				>
				<AlertDialog.Description class="text-muted-foreground mt-2 text-sm">
					{#if 'plan' in request}
						{#if setup}Pepo needs a language model to translate {text.language(request.pair.source)} messages
							into {text.language(request.pair.target)}.
						{:else}Download this model to your device for local translation.{/if}
					{:else if request.action === 'remove'}Remove the installed model from this device. Your
						translation preferences will be kept.
					{:else}Delete retained download files for this direction. The installed model will be
						kept.{/if}
				</AlertDialog.Description>
				{#if 'plan' in request}
					<p class="text-muted-foreground mt-3 text-sm">Translations run on this device.</p>
					<dl
						class="mt-4 grid grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-3 text-sm"
						data-reviewed-model
					>
						<dt>Download</dt>
						<dd class="text-right tabular-nums">{text.bytes(request.plan.model.downloadBytes)}</dd>
						<dt>Space on disk</dt>
						<dd class="text-right tabular-nums">{text.bytes(request.plan.model.installedBytes)}</dd>
					</dl>
				{:else if request.installed}<p class="mt-3 text-sm break-all">
						{request.installed.id} · Version {request.installed.version}
					</p>{/if}
				{#if request.interruptsTranslation}<p class="mt-4 text-sm" role="note">
						Queued translations will be cancelled and active work will finish before this change.
						Translation will pause during the change, then its previous running state will be
						restored.
					</p>{/if}
				<div class="mt-6 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
					<AlertDialog.Cancel class={buttonVariants({ variant: 'outline', size: 'sm' })}
						>Cancel</AlertDialog.Cancel
					>
					<AlertDialog.Action
						class={buttonVariants({
							variant:
								request.action === 'remove' || request.action === 'clear_downloads'
									? 'destructive'
									: 'default',
							size: 'sm'
						})}
						onclick={() => session.respond(true)}
						>{setup
							? session.setupIntent === 'enable'
								? 'Download & enable'
								: 'Download & change language'
							: verb}</AlertDialog.Action
					>
				</div>
			{/if}
		</AlertDialog.Content>
	</AlertDialog.Portal>
</AlertDialog.Root>
