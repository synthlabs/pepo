<script lang="ts">
	import { getContext } from 'svelte';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { NETWORK_STATUS_CONTEXT, type NetworkStatusController } from '$lib/network/status.svelte';
	import NetworkTooltip from './network-tooltip.svelte';
	const network = getContext<NetworkStatusController | undefined>(NETWORK_STATUS_CONTEXT);
</script>

{#if network?.issues.length}
	<Tooltip.Provider delayDuration={200}>
		<Tooltip.Root>
			<Tooltip.Trigger
				aria-label={network.summary.text}
				class="focus-visible:ring-ring inline-flex size-7 shrink-0 cursor-default items-center justify-center rounded-md outline-none focus-visible:ring-2"
				data-network-header-status
			>
				{#if network.summary.urgent}<TriangleAlert
						class="text-destructive size-4"
					/>{:else}<RefreshCw class="size-4 text-amber-600 dark:text-amber-400" />{/if}
			</Tooltip.Trigger>
			<Tooltip.Content class="max-w-xs" align="start">
				<NetworkTooltip summary={network.summary} />
			</Tooltip.Content>
		</Tooltip.Root>
	</Tooltip.Provider>
{/if}
