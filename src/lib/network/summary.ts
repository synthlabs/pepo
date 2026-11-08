import type { NetworkIssue } from '$lib/bindings';

const headings = [
	'Some features are still refreshing in the background:',
	'Reconnecting:',
	'Some features need attention:'
];

export function summarizeNetworkIssues(issues: NetworkIssue[]) {
	const features = new Map<string, number>();
	for (const issue of issues) {
		const severity = !issue.retryable ? 2 : issue.core ? 1 : 0;
		features.set(issue.feature, Math.max(features.get(issue.feature) ?? 0, severity));
	}
	const groups = [2, 1, 0].flatMap((severity) => {
		const names = [...features]
			.filter(([, level]) => level === severity)
			.map(([feature]) => feature)
			.sort((left, right) => left.localeCompare(right, 'en'));
		return names.length ? [{ heading: headings[severity], features: names }] : [];
	});
	return {
		groups,
		urgent: [...features.values()].some((severity) => severity > 0),
		text: groups.map((group) => `${group.heading} ${group.features.join(', ')}`).join(' ')
	};
}

export type NetworkSummary = ReturnType<typeof summarizeNetworkIssues>;
