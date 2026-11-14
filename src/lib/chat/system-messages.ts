import { commands, type ChatRoomInfo, type SystemMessageInput } from '$lib/bindings';

export type SystemMessageDraft = Pick<SystemMessageInput, 'segments'> &
	Partial<Pick<SystemMessageInput, 'message_id' | 'ts' | 'icon'>>;

export function postSystemMessage(room: ChatRoomInfo, draft: SystemMessageDraft) {
	return commands.postSystemMessage(room, {
		message_id: draft.message_id ?? crypto.randomUUID(),
		ts: draft.ts ?? new Date().toISOString(),
		segments: draft.segments,
		icon: draft.icon ?? null
	});
}

export function systemMessageLink(url: string): string | undefined {
	try {
		const parsed = new URL(url);
		return parsed.protocol === 'https:' || parsed.protocol === 'http:' ? parsed.href : undefined;
	} catch {
		return undefined;
	}
}
