import type { ChatSendError, ChatSendErrorKind } from '$lib/bindings';
import { errorDetails } from '$utils/inbound';
import { appErrors } from '$lib/errors/controller';

const KINDS: ReadonlySet<string> = new Set<ChatSendErrorKind>([
	'authentication',
	'permission_denied',
	'rate_limited',
	'rejected',
	'connection',
	'timeout',
	'unexpected'
]);
const UNEXPECTED_MESSAGE = 'Could not send this message.';

// Invoke-level failures reject with a plain string even though the binding types them.
export function isChatSendError(value: unknown): value is ChatSendError {
	if (!value || typeof value !== 'object') return false;
	const { kind, message, details } = value as Record<string, unknown>;
	return (
		typeof kind === 'string' &&
		KINDS.has(kind) &&
		typeof message === 'string' &&
		typeof details === 'string'
	);
}

export function captureChatSendError(channel: string, error: unknown): string | null {
	const sendError: ChatSendError = isChatSendError(error)
		? error
		: { kind: 'unexpected', message: UNEXPECTED_MESSAGE, details: errorDetails(error) };
	const technical = sendError.kind === 'unexpected';
	appErrors.capture(
		{ channel, ...sendError },
		{
			source: 'chat.send',
			title: channel ? `Could not send a message to #${channel}` : 'Could not send a message'
		},
		technical
	);
	return technical ? null : sendError.message;
}
