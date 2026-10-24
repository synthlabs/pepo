import type { ChannelStatus } from '$lib/bindings';

export type ChannelHeader = {
	channelName: string;
	isPartner: boolean;
	state: 'loading' | 'live' | 'offline';
	title: string | null;
	category: string | null;
	language: string | null;
	tags: string[];
	viewerCount: number | null;
	startedAt: string | null;
};

const languageNames = new Intl.DisplayNames(['en'], { type: 'language' });

function streamLanguage(language: string): string | null {
	const code = language.trim();
	let name = code;
	if (code) {
		try {
			name = languageNames.of(code) ?? code;
		} catch {
			// Keep unknown language codes readable instead of dropping the pill.
		}
	}
	return name || null;
}

export function channelHeader(
	routeLogin: string | null | undefined,
	status: ChannelStatus | null | undefined
): ChannelHeader | null {
	const fallbackName = routeLogin?.trim() ?? '';
	if (!fallbackName) return null;

	const stream = status?.stream;
	const language = streamLanguage(stream?.language ?? '');
	const languageTags = new Set([stream?.language.trim().toLowerCase(), language?.toLowerCase()]);
	return {
		channelName: status?.display_name?.trim() || fallbackName,
		isPartner: status?.is_partner ?? false,
		state: stream ? 'live' : status && !status.is_live ? 'offline' : 'loading',
		title: stream?.title ?? null,
		category: stream?.game_name.trim() || null,
		language,
		tags:
			stream?.tags.filter((tag) => tag.trim() && !languageTags.has(tag.trim().toLowerCase())) ?? [],
		viewerCount: stream?.viewer_count ?? null,
		startedAt: stream?.started_at ?? null
	};
}

export function streamUptime(startedAt: string | null, now: number): string | null {
	if (!startedAt) return null;
	const started = Date.parse(startedAt);
	if (!Number.isFinite(started)) return null;

	const seconds = Math.max(0, Math.floor((now - started) / 1000));
	const hours = Math.floor(seconds / 3600);
	const minutes = Math.floor((seconds % 3600) / 60);
	return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
}

export type HeaderMeasurements = {
	availableWidth: number;
	requiredWidth: number;
	tagWidths: number[];
	categoryWidth: number;
	uptimeWidth: number;
};

export function fitHeaderMetadata({
	availableWidth,
	requiredWidth,
	tagWidths,
	categoryWidth,
	uptimeWidth
}: HeaderMeasurements) {
	let tagCount = tagWidths.length;
	let showCategory = categoryWidth > 0;
	let showUptime = uptimeWidth > 0;
	const width = () =>
		requiredWidth +
		(showCategory ? categoryWidth + 12 : 0) +
		(showUptime ? uptimeWidth + 12 : 0) +
		(tagCount
			? tagWidths.slice(0, tagCount).reduce((sum, size) => sum + size, 0) + (tagCount - 1) * 4 + 12
			: 0);

	while (tagCount && width() > availableWidth) tagCount -= 1;
	if (width() > availableWidth) showCategory = false;
	if (width() > availableWidth) showUptime = false;
	return { tagCount, showCategory, showUptime };
}
