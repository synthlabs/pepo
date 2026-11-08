import type { NetworkHealthSnapshot, NetworkIssue } from '$lib/bindings';

export class NetworkHealth {
	private native: NetworkHealthSnapshot = { revision: 0, issues: [] };
	private external = new Map<string, NetworkIssue>();
	private listeners = new Set<(issues: NetworkIssue[]) => void>();
	subscribe(listener: (issues: NetworkIssue[]) => void) {
		this.listeners.add(listener);
		listener(this.issues());
		return () => {
			this.listeners.delete(listener);
		};
	}
	receive(snapshot: NetworkHealthSnapshot) {
		if (snapshot.revision < this.native.revision) return;
		this.native = snapshot;
		this.publish();
	}
	setExternal(
		id: string,
		feature: string,
		message: string | null,
		cached = false,
		options: { retryable?: boolean; retryAt?: number; newAttempt?: boolean } = {}
	) {
		if (message === null) {
			if (!this.external.delete(id)) return;
		} else {
			const previous = this.external.get(id);
			if (
				previous?.message === message &&
				previous.cached === cached &&
				previous.retryable === (options.retryable ?? true) &&
				previous.retry_at === (options.retryAt ?? null) &&
				!options.newAttempt
			)
				return;
			this.external.set(id, {
				id,
				feature,
				message,
				category: 'request',
				failures: (previous?.failures ?? 0) + 1,
				affected: 1,
				cached,
				last_failure_at: Date.now(),
				retry_at: options.retryAt ?? null,
				retryable: options.retryable ?? true,
				core: false,
				can_retry: false
			});
		}
		this.publish();
	}
	issues(): NetworkIssue[] {
		return [...this.native.issues, ...this.external.values()];
	}
	private publish() {
		const issues = this.issues();
		for (const listener of this.listeners) listener(issues);
	}
}

export const networkHealth = new NetworkHealth();
