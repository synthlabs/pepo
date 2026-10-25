const MAX_SPACER_PX = 10;
const PHYSICAL_PRECISION = 10_000;
const PHYSICAL_ROUNDING_BIAS = 1 / PHYSICAL_PRECISION;

export function getHistoryOriginSpacer(
	currentHeightPx: number,
	removedHeightsPx: readonly number[],
	devicePixelRatio: number
): number {
	if (!Number.isFinite(devicePixelRatio) || devicePixelRatio <= 0) return 0;

	for (let periodPx = 1; periodPx <= MAX_SPACER_PX; periodPx++) {
		const physicalPeriod = Math.round(periodPx * devicePixelRatio);
		if (
			physicalPeriod === 0 ||
			Math.abs(periodPx * devicePixelRatio - physicalPeriod) > PHYSICAL_ROUNDING_BIAS
		)
			continue;

		const removedHeightPx = removedHeightsPx.reduce((sum, height) => sum + height, 0);
		const physicalHeight = (currentHeightPx + removedHeightPx) * devicePixelRatio;
		const remainder =
			(Math.round((physicalHeight % physicalPeriod) * PHYSICAL_PRECISION) / PHYSICAL_PRECISION) %
			physicalPeriod;

		// Keep layout quantization from rounding an exact remainder down.
		return remainder === 0 ? 0 : (remainder + PHYSICAL_ROUNDING_BIAS) / devicePixelRatio;
	}

	return 0;
}

export function measureHistoryOriginSpacer(
	spacer: HTMLElement | undefined,
	messageRows: Iterable<HTMLElement>,
	removedKeys: ReadonlySet<string>,
	devicePixelRatio: number
): number {
	const rows = Array.from(messageRows);
	const removed = (row: HTMLElement) => removedKeys.has(row.dataset.chatMessageIndex ?? '');
	if (rows.every(removed)) return 0;

	return getHistoryOriginSpacer(
		spacer?.getBoundingClientRect().height ?? 0,
		rows.filter(removed).map((row) => row.getBoundingClientRect().height),
		devicePixelRatio
	);
}
