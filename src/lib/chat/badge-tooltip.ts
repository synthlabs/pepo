import type { BadgeRef } from '$lib/bindings';

export type BadgeTooltipContent = {
	name: string;
	description: string;
	detail: string;
};

const fallbackBadgeNames: Record<string, string> = {
	subscriber: 'Subscriber',
	moderator: 'Moderator',
	partner: 'Partner',
	broadcaster: 'Broadcaster',
	vip: 'VIP'
};

function firstPresent(...values: Array<string | undefined>): string {
	return values.map((value) => value?.trim() ?? '').find(Boolean) ?? '';
}

export function badgeTooltipContent(badgeRef: BadgeRef): BadgeTooltipContent {
	const name = firstPresent(
		badgeRef.badge.title,
		fallbackBadgeNames[badgeRef.set_id],
		badgeRef.badge.set_id,
		badgeRef.set_id
	);

	const description = firstPresent(badgeRef.badge.description);
	const info = badgeRef.info.trim();

	return {
		name,
		description: description === name ? '' : description,
		detail: badgeRef.set_id === 'subscriber' && info ? `Months subscribed: ${info}` : ''
	};
}
