<script lang="ts">
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { commands, type EngineStatus } from '$lib/bindings';
	let status = $state<EngineStatus | null>(null);
	let error = $state('');
	onMount(() => {
		if (!('__TAURI_INTERNALS__' in window)) return;
		let closed = false;
		let received = false;
		let unlisten: (() => void) | undefined;
		void (async () => {
			try {
				const cleanup = await listen<EngineStatus>('chat_engine_status', (event) => {
					if (!closed) {
						received = true;
						status = event.payload;
					}
				});
				if (closed) {
					cleanup();
					return;
				}
				unlisten = cleanup;
				const initial = await commands.getChatEngineStatus();
				if (!closed && !received) status = initial;
			} catch (cause) {
				if (!closed) error = String(cause);
			}
		})();
		return () => {
			closed = true;
			unlisten?.();
		};
	});
</script>

{#if status}
	<div class="text-muted-foreground mt-2 text-xs" role="status">
		<p>
			{status.joined} joined · {status.eligible} eligible · {status.lingering} opened rooms retained
		</p>
		{#if !status.connected}<p>Connecting to chat…</p>{/if}
		{#if status.pending > 0}<p>{status.pending} rooms waiting to join</p>{/if}
		{#if status.limited > 0}<p>{status.limited} rooms excluded by the connection limit</p>{/if}
		{#each status.failures as failure (failure)}<p>{failure}</p>{/each}
	</div>
{:else if error}
	<p class="text-destructive mt-2 text-xs" role="status">Collection status unavailable: {error}</p>
{/if}
