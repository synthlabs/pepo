const CHAT_BADGE_GAP_PX = 4;

export function chatBadgePlaceholderWidth(badgeCount: number, badgeSizePx: number): string {
	const count = positiveInteger(badgeCount, 0);
	if (count === 0) return '0px';

	const sizePx = positiveInteger(badgeSizePx, 1);
	return `${count * sizePx + (count - 1) * CHAT_BADGE_GAP_PX}px`;
}

function positiveInteger(value: number, fallback: number): number {
	return Number.isFinite(value) && value > 0 ? Math.floor(value) : fallback;
}
