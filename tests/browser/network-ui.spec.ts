import assert from 'node:assert/strict';
import { test } from '../support/browser/fixtures';

const cases = [
	...[1280, 800].flatMap((width) =>
		['dark', 'light'].flatMap((theme) =>
			[true, false].map((open) => ({ width, theme, open, view: open ? 'settings' : 'chat' }))
		)
	),
	{ width: 1280, theme: 'dark', open: true, view: 'drawer' },
	{ width: 800, theme: 'light', open: false, view: 'drawer' }
];

for (const { width, theme, open, view } of cases) {
	test(`network UI (${width}px, ${theme}, ${open ? 'expanded' : 'collapsed'}, ${view})`, async ({
		app
	}) => {
		await app.viewport(width, 800);
		await app.page.goto(`/?harness=network-ui&theme=${theme}&open=${open}&view=${view}`, {
			waitUntil: 'domcontentloaded'
		});
		const result = await app.waitFor<{ verdict: string; details: string }>(
			`(() => {
				const node = document.querySelector('[data-network-check]');
				const verdict = node?.getAttribute('data-network-check');
				if (!verdict || verdict === 'running') return null;
				return { verdict, details: node?.textContent ?? '' };
			})()`,
			30000
		);
		assert.equal(result.verdict, 'passed', result.details);
	});
}
