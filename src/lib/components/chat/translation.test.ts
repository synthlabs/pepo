import { flushSync, mount, unmount } from 'svelte';
import { describe, expect, it } from 'vitest';
import type { ChatTranslationLayout } from '$lib/bindings';

import Translation from './+translation.svelte';

const translation = {
	source_language: 'ru',
	target_language: 'en',
	translated_text: 'Kirov Street'
};

describe('chat translation rendering', () => {
	it('renders the connector layout with compact translation content', async () => {
		const { component, target } = mountTranslation('connector');
		const connector = target.querySelector('[data-translation-connector]');

		expect(connector).not.toBeNull();
		expect(connector?.getAttribute('aria-hidden')).toBe('true');
		expect(connector?.classList).toContain('h-2.5');
		expect(connector?.classList).toContain('w-[18px]');
		expect(connector?.classList).toContain('-translate-y-0.5');
		expect(connector?.classList).toContain('rounded-bl-xl');
		expect(connector?.classList).toContain('border-b-2');
		expect(connector?.classList).toContain('border-l-2');
		expect(connector?.parentElement?.classList).toContain('pl-1');
		expect(connector?.parentElement?.classList).toContain('pr-2');
		expect(connector?.parentElement?.nextElementSibling?.classList).toContain('pr-1');
		expect(target.textContent?.replaceAll(/\s+/g, ' ').trim()).toBe('RU -> EN Kirov Street');

		await unmount(component);
	});

	it.each(['language_tag', 'message_text', 'timestamp_end'] as const)(
		'does not render the connector for %s',
		async (layout) => {
			const { component, target } = mountTranslation(layout);

			expect(target.querySelector('[data-translation-connector]')).toBeNull();

			await unmount(component);
		}
	);
});

function mountTranslation(layout: ChatTranslationLayout) {
	const target = document.createElement('div');
	const component = mount(Translation, {
		target,
		props: { translation, authorName: 'GrumpyGordon', layout }
	});
	flushSync();

	return { component, target };
}
