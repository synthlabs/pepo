import type { UserCardState } from '$lib/bindings';

export interface UserCardApi {
	listen: (receive: (state: UserCardState) => void) => Promise<() => void>;
	snapshot: () => Promise<UserCardState>;
	ready: () => Promise<void>;
}

export class UserCardSession {
	private revision = -1;
	private generation = 0;
	private unlisten?: () => void;

	constructor(
		private api: UserCardApi,
		private publish: (state: UserCardState) => void
	) {}

	async start(): Promise<void> {
		this.stop();
		const generation = this.generation;
		const receive = (state: UserCardState) => {
			if (generation !== this.generation || state.revision < this.revision) return;
			this.revision = state.revision;
			this.publish(state);
		};
		const unlisten = await this.api.listen(receive);
		if (generation !== this.generation) {
			unlisten();
			return;
		}
		this.unlisten = unlisten;
		receive(await this.api.snapshot());
		if (generation === this.generation) await this.api.ready();
	}

	stop(): void {
		this.generation += 1;
		this.unlisten?.();
		this.unlisten = undefined;
	}
}
