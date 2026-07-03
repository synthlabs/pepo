import type { ChannelStatus } from '$lib/bindings';

export type ChannelHeader = {
	text: string;
	viewerCount: number | null;
};

export function channelHeader(
	routeLogin: string | null | undefined,
	status: ChannelStatus | null | undefined
): ChannelHeader | null {
	const fallbackName = routeLogin?.trim() ?? '';
	if (!fallbackName) return null;

	const channelName = status?.display_name?.trim() || fallbackName;
	if (status?.stream) {
		return {
			text: `${channelName}: ${status.stream.title}`,
			viewerCount: status.stream.viewer_count
		};
	}

	if (status) {
		return {
			text: `${channelName} is offline.`,
			viewerCount: null
		};
	}

	return {
		text: fallbackName,
		viewerCount: null
	};
}
