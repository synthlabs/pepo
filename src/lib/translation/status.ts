import type { ModelManagerSnapshot } from '$utils/translate';

export function targetModelStatus(snapshot: ModelManagerSnapshot, target: string): string {
	if (!snapshot.connected) return 'Connect model management to check your target language.';
	if (snapshot.loading || !snapshot.service || snapshot.service.phase === 'initializing')
		return 'Checking model status…';
	if (!['ready', 'paused', 'releasing'].includes(snapshot.service.phase))
		return 'Model status is unavailable.';
	if (snapshot.errors.some((issue) => issue.action === 'inventory'))
		return 'Could not read model status. Refresh model status to try again.';
	if (snapshot.attention.some((row) => row.pair?.target === target))
		return 'A model for this target needs attention. See the model list below.';
	if (snapshot.installed.some((row) => row.pair?.target === target))
		return 'A model for this target is installed. Integrity has not been verified; use Verify to check it.';
	return 'No model is installed for this target. Download one below to prepare for translation.';
}
