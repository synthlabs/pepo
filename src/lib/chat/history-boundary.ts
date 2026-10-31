export interface HistoryBoundary {
	cursor: number;
	hasHistory: boolean;
}

export function showDividerAfter(
	messages: readonly { index: number }[],
	position: number,
	boundary: HistoryBoundary | null,
	enabled: boolean
): boolean {
	if (!enabled || !boundary?.hasHistory) return false;
	const message = messages[position];
	const next = messages[position + 1];
	return (
		message !== undefined &&
		message.index <= boundary.cursor &&
		(next === undefined || next.index > boundary.cursor)
	);
}
