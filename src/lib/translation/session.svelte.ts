import {
	createTranslationClient,
	ModelManagerController,
	type ModelManagerClient,
	type ModelConfirmationRequest,
	type ModelManagerSnapshot
} from '$utils/translate';
import { appErrors } from '$lib/errors/controller';

export const TRANSLATION_CONTEXT = Symbol('translation');
type SessionClient = ModelManagerClient & { dispose(): Promise<void> };

export class TranslationSession {
	readonly controller: ModelManagerController;
	snapshot: ModelManagerSnapshot = $state.raw(null!);
	confirmation: ModelConfirmationRequest | null = $state.raw(null);
	connecting = $state(false);
	setupTarget: string | null = $state(null);
	setupIntent: 'enable' | 'language' = $state('enable');
	setupError = $state('');
	private setupVersion = 0;
	private finishSetup?: () => void;
	private answer?: (accepted: boolean) => void;
	private connection?: Promise<void>;
	private disposal?: Promise<void>;
	private stop: () => void;
	private started = false;

	constructor(private client: SessionClient = createTranslationClient()) {
		this.controller = new ModelManagerController({
			client,
			confirm: (request) =>
				new Promise<boolean>((resolve) => {
					this.confirmation = request;
					this.answer = resolve;
				}),
			onerror: (error, context) =>
				appErrors.capture(
					{ code: error.code, message: error.message, details: error.details, ...context },
					{ source: 'translation.models', title: 'Translation model action failed' },
					true
				)
		});
		this.stop = this.controller.subscribe((snapshot) => {
			this.snapshot = snapshot;
		});
	}

	start(ready: boolean, enabled: boolean): void {
		if (!ready || !enabled || this.started) return;
		this.started = true;
		void this.connect();
	}

	connect(): Promise<void> {
		if (this.disposal) return Promise.resolve();
		if (this.connection) return this.connection;
		this.connecting = true;
		this.connection = this.controller
			.connect()
			.catch(() => {
				/* The controller reports connection failures. */
			})
			.finally(() => {
				this.connecting = false;
				this.connection = undefined;
			});
		return this.connection;
	}

	respond(accepted: boolean): void {
		const answer = this.answer;
		this.answer = undefined;
		this.confirmation = null;
		answer?.(accepted);
	}

	abandonSetup(): void {
		this.setupVersion++;
		this.setupError = '';
		if (this.setupTarget !== null) this.respond(false);
		this.finishSetup?.();
	}

	async prepareTarget(target: string, intent: 'enable' | 'language'): Promise<boolean> {
		if (
			this.disposal ||
			this.setupTarget !== null ||
			this.snapshot.pending ||
			(this.snapshot.installation && !this.snapshot.installation.finished)
		)
			return false;
		const version = ++this.setupVersion;
		this.setupTarget = target;
		this.setupIntent = intent;
		this.setupError = '';
		const current = () => !this.disposal && version === this.setupVersion;
		const failed = () => {
			if (current()) this.setupError = 'Could not prepare translation. Try again or open Advanced.';
			return false;
		};
		try {
			await this.connect();
			if (!current()) return false;
			if (!this.snapshot.connected || !(await this.controller.refreshModels())) return failed();
			if (!current()) return false;
			if (!['ready', 'paused'].includes(this.snapshot.service?.phase ?? '')) return failed();
			if (this.snapshot.attention.some((row) => row.pair?.target === target)) {
				this.setupError = 'A language model needs attention. Open Advanced to repair it.';
				return false;
			}
			const installed = this.snapshot.installed.filter((row) => row.pair?.target === target);
			const models =
				this.snapshot.service?.catalog.catalog?.models.filter(
					(model) => model.pair.target === target
				) ?? [];
			if (!models.length && !installed.length) {
				this.setupError =
					'No translation model is available for this language. Open Advanced to check language support.';
				return false;
			}
			for (const model of models) {
				if (installed.some((row) => row.pair?.source === model.pair.source)) continue;
				if (!current()) return false;
				const errors = this.snapshot.errors.length;
				if (!(await this.controller.install(model.pair))) {
					return this.snapshot.errors.length > errors ? failed() : false;
				}
				if (!current()) return false;
				const operation = this.snapshot.installation;
				if (!operation) return failed();
				if (!(await this.waitForSetup(operation.id))) {
					return this.snapshot.installation?.error?.code === 'cancelled' || !current()
						? false
						: failed();
				}
			}
			if (!current()) return false;
			if (this.snapshot.service?.phase === 'paused' && !(await this.controller.resume()))
				return failed();
			return current();
		} finally {
			this.setupTarget = null;
		}
	}

	private waitForSetup(id: string): Promise<boolean> {
		return new Promise((resolve) => {
			let timer: ReturnType<typeof setTimeout> | undefined;
			let done = false;
			let stop = () => {};
			const finish = (success: boolean) => {
				if (done) return;
				done = true;
				clearTimeout(timer);
				stop();
				this.finishSetup = undefined;
				resolve(success);
			};
			this.finishSetup = () => finish(false);
			stop = this.controller.subscribe((snapshot) => {
				if (this.disposal || snapshot.service?.phase === 'shutdown') finish(false);
				const operation = snapshot.installation;
				if (operation?.id === id && operation.finished && !snapshot.pending)
					finish(operation.result !== null);
			});
			const poll = async () => {
				if (done) return;
				if (!this.snapshot.pending && !(await this.controller.refreshModels())) finish(false);
				if (!done) timer = setTimeout(() => void poll(), 1000);
			};
			if (done) stop();
			else timer = setTimeout(() => void poll(), 1000);
		});
	}

	dispose(): Promise<void> {
		if (this.disposal) return this.disposal;
		this.abandonSetup();
		this.respond(false);
		this.disposal = (async () => {
			await this.controller.dispose();
			this.stop();
			await this.client.dispose();
		})();
		return this.disposal;
	}
}
