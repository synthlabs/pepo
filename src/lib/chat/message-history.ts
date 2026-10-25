import type { ChatSettings } from '$lib/bindings';

export type MessageHistoryState = Pick<
	ChatSettings,
	| 'message_limit'
	| 'pause_message_gc_while_paused'
	| 'batch_live_messages'
	| 'message_batch_interval_ms'
	| 'message_gc_interval_ms'
> & {
	paused: boolean;
	interacting: boolean;
	message_count: number;
};

interface MessageHistoryCallbacks<T> {
	getState: () => MessageHistoryState;
	append: (messages: T[]) => void;
	trim: (count: number) => void;
}

export class MessageHistoryScheduler<T> {
	private pending: T[] = [];
	private messageTimer: ReturnType<typeof setTimeout> | undefined;
	private gcTimer: ReturnType<typeof setTimeout> | undefined;
	private messageInterval = 0;
	private gcInterval = 0;
	private disposed = false;

	constructor(private callbacks: MessageHistoryCallbacks<T>) {}

	enqueue(message: T): void {
		if (this.disposed) return;
		this.pending.push(message);
		this.update();
	}

	update(): void {
		if (this.disposed) return;
		const state = this.callbacks.getState();
		if (!state.paused && !state.batch_live_messages) {
			this.flushMessages();
		} else if (this.pending.length > 0) {
			if (this.messageInterval !== state.message_batch_interval_ms) this.cancelMessageTimer();
			if (this.messageTimer === undefined) {
				this.messageInterval = state.message_batch_interval_ms;
				this.messageTimer = setTimeout(() => this.flushMessages(), this.messageInterval);
			}
		}

		this.scheduleGarbageCollection();
	}

	flushMessages(): void {
		this.cancelMessageTimer();
		if (this.disposed || this.pending.length === 0) return;
		const batch = this.pending;
		this.pending = [];
		this.callbacks.append(batch);
		this.scheduleGarbageCollection();
	}

	resume(): void {
		if (this.disposed) return;
		this.flushMessages();
		this.collectGarbage();
	}

	reset(): void {
		this.cancelMessageTimer();
		this.cancelGcTimer();
		this.pending = [];
	}

	dispose(): void {
		this.disposed = true;
		this.cancelMessageTimer();
		this.cancelGcTimer();
		this.pending = [];
	}

	private canCollect(state: MessageHistoryState): boolean {
		return (
			state.message_count > state.message_limit &&
			!state.interacting &&
			(!state.paused || !state.pause_message_gc_while_paused)
		);
	}

	private scheduleGarbageCollection(): void {
		if (this.disposed) return;
		const state = this.callbacks.getState();
		if (!this.canCollect(state)) {
			this.cancelGcTimer();
			return;
		}
		if (this.gcInterval !== state.message_gc_interval_ms) this.cancelGcTimer();
		if (this.gcTimer === undefined) {
			this.gcInterval = state.message_gc_interval_ms;
			this.gcTimer = setTimeout(() => this.collectGarbage(), this.gcInterval);
		}
	}

	private collectGarbage(): void {
		this.cancelGcTimer();
		if (this.disposed) return;
		const state = this.callbacks.getState();
		if (this.canCollect(state)) this.callbacks.trim(state.message_count - state.message_limit);
	}

	private cancelMessageTimer(): void {
		if (this.messageTimer !== undefined) clearTimeout(this.messageTimer);
		this.messageTimer = undefined;
	}

	private cancelGcTimer(): void {
		if (this.gcTimer !== undefined) clearTimeout(this.gcTimer);
		this.gcTimer = undefined;
	}
}
