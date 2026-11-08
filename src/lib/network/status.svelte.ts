import type { NetworkIssue } from '$lib/bindings';
import { summarizeNetworkIssues } from './summary';

export const NETWORK_STATUS_CONTEXT = Symbol('network-status');

export class NetworkStatusController {
	issues: NetworkIssue[] = $state.raw([]);
	open = $state(false);
	summary = $derived(summarizeNetworkIssues(this.issues));
	private opener: HTMLElement | null = null;

	show(opener: HTMLElement) {
		this.opener = opener;
		this.open = true;
	}
	closeForReport() {
		this.opener = null;
		this.open = false;
	}
	restoreFocus() {
		if (this.opener?.isConnected) this.opener.focus();
		this.opener = null;
	}
}
