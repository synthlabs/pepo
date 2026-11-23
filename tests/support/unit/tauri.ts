type EventListener<T = unknown> = (event: { payload: T }) => void;
type InvokeHandler = (args: Record<string, unknown> | undefined) => unknown;

export interface TauriUnitMock {
	calls: { command: string; args: Record<string, unknown> | undefined }[];
	listeners: Map<string, Set<EventListener>>;
	handle: (command: string, handler: InvokeHandler) => void;
	emit: <T>(event: string, payload: T) => void;
	reset: () => void;
	listen: <T = unknown>(event: string, listener: EventListener<T>) => Promise<() => void>;
	invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
}

export function createTauriUnitMock(
	initialHandlers: Record<string, InvokeHandler> = {}
): TauriUnitMock {
	const handlers = new Map<string, InvokeHandler>(Object.entries(initialHandlers));
	const listeners = new Map<string, Set<EventListener>>();
	const calls: { command: string; args: Record<string, unknown> | undefined }[] = [];

	return {
		calls,
		listeners,
		handle(command, handler) {
			handlers.set(command, handler);
		},
		emit(event, payload) {
			for (const listener of listeners.get(event) ?? []) {
				listener({ payload: structuredClone(payload) });
			}
		},
		reset() {
			calls.length = 0;
			listeners.clear();
		},
		async listen(event, listener) {
			const group = listeners.get(event) ?? new Set();
			group.add(listener as EventListener);
			listeners.set(event, group);
			return () => {
				group.delete(listener as EventListener);
			};
		},
		async invoke(command, args) {
			calls.push({ command, args });
			const handler = handlers.get(command);
			if (!handler) throw new Error(`Unexpected command ${command}`);
			return handler(args);
		}
	};
}
