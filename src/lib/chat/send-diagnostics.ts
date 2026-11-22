import type {
	ChatSendDiagnostics,
	ChatSendKeyEvent,
	Emote,
	EmoteCompletionDiagnostics
} from '$lib/bindings';

interface DiagnosticLogger {
	info: (...values: unknown[]) => void;
	warn: (...values: unknown[]) => void;
}

type PickerState = Pick<ChatSendKeyEvent, 'picker_visible' | 'result_count' | 'selected_index'>;

export function messageMetrics(message: string) {
	return {
		utf8_bytes: new TextEncoder().encode(message).length,
		utf16_length: message.length,
		scalar_length: Array.from(message).length
	};
}

export function unusualCodepoints(message: string) {
	return Array.from(message).flatMap((character, index) => {
		const code = character.codePointAt(0)!;
		return code < 0x21 || code > 0x7e
			? [{ index, codepoint: `U+${code.toString(16).toUpperCase().padStart(4, '0')}` }]
			: [];
	});
}

export class ComposerDiagnostics {
	private keys: (Omit<ChatSendKeyEvent, 'age_ms'> & { time: number })[] = [];
	private completion: EmoteCompletionDiagnostics | null = null;
	private completedDraft = '';

	constructor(private readonly now: () => number = () => performance.now()) {}

	reset() {
		this.keys = [];
		this.completion = null;
		this.completedDraft = '';
	}

	inputChanged(value: string) {
		if (!value) this.reset();
		else if (this.completion && value !== this.completedDraft) {
			this.completion.edited_since_selection = true;
		}
	}

	key(event: KeyboardEvent, picker: PickerState) {
		if (!['Enter', 'Tab', 'Escape'].includes(event.key)) return;
		this.keys.push({
			key: event.key,
			shift: event.shiftKey,
			repeat: event.repeat,
			is_composing: event.isComposing,
			...picker,
			selected_index: Number.isFinite(picker.selected_index) ? picker.selected_index : null,
			time: this.now()
		});
		this.keys = this.keys.slice(-16);
	}

	inserted(emote: Emote, mode: 'colon' | 'picker', method: 'keyboard' | 'mouse', draft: string) {
		this.completion = {
			mode,
			method,
			emote_id: emote.id,
			emote_name: emote.name,
			provider: emote.provider,
			scope: emote.scope,
			edited_since_selection: false
		};
		this.completedDraft = draft;
	}

	begin(message: string, channel: string, broadcasterId: string, logger: DiagnosticLogger) {
		const now = this.now();
		const diagnostics: ChatSendDiagnostics = {
			attempt_id: crypto.randomUUID(),
			channel_login: channel,
			last_completion: this.completion
				? {
						...this.completion,
						edited_since_selection:
							this.completion.edited_since_selection || message !== this.completedDraft
					}
				: null,
			key_events: this.keys.map(({ time, ...key }) => ({ ...key, age_ms: Math.max(0, now - time) }))
		};
		this.reset();
		return new ChatSendAttempt(message, broadcasterId, diagnostics, logger, this.now);
	}
}

class ChatSendAttempt {
	private readonly started: number;

	constructor(
		private readonly message: string,
		private readonly broadcasterId: string,
		readonly diagnostics: ChatSendDiagnostics,
		private readonly logger: DiagnosticLogger,
		private readonly now: () => number
	) {
		this.started = now();
		this.log('info', 'frontend_attempt', this.record());
	}

	private record() {
		return {
			attempt_id: this.diagnostics.attempt_id,
			channel_login: this.diagnostics.channel_login,
			broadcaster_id: this.broadcasterId,
			...messageMetrics(this.message),
			elapsed_ms: Math.max(0, this.now() - this.started)
		};
	}

	private log(level: 'info' | 'warn', event: string, record: unknown) {
		try {
			this.logger[level](`chat.send.${event}`, JSON.stringify(record));
		} catch {
			// Diagnostics must not interrupt sending or error presentation.
		}
	}

	sent() {
		this.log('info', 'frontend_result', { ...this.record(), is_sent: true });
	}

	failed(error: unknown, stage: 'returned' | 'invoke') {
		const kind = error && typeof error === 'object' && 'kind' in error ? error.kind : null;
		this.log('warn', 'frontend_failure', {
			...this.record(),
			stage,
			kind: typeof kind === 'string' ? kind : 'unexpected',
			message: this.message,
			unusual_codepoints: unusualCodepoints(this.message),
			composer: this.diagnostics
		});
	}
}
