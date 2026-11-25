import assert from 'node:assert/strict';
import { test } from '../support/browser/fixtures';

interface SidebarHeaderCase {
	name: string;
	width: number;
	open?: boolean;
	theme?: string;
	'no-language'?: boolean;
	'no-tags'?: boolean;
	long?: boolean;
}

const cases: SidebarHeaderCase[] = [
	{ name: 'collapsed-dark', width: 1200, open: false },
	{ name: 'collapsed-light', width: 1200, open: false, theme: 'light' },
	{ name: 'expanded-dark', width: 1200 },
	{ name: 'expanded-light', width: 1200, theme: 'light' },
	{ name: 'fallback-tags', width: 1200, 'no-language': true },
	{ name: 'language-only', width: 1200, 'no-tags': true },
	{ name: 'no-metadata', width: 1200, 'no-language': true, 'no-tags': true },
	{ name: 'narrow-desktop', width: 820, long: true },
	{ name: 'mobile', width: 375, long: true },
	{ name: 'small-mobile', width: 320, long: true }
];

for (const scenario of cases) {
	test(`sidebar and channel header (${scenario.name})`, async ({ app }) => {
		const { cdp, evaluate, waitFor } = app;
		const send = cdp as (method: string, params?: Record<string, unknown>) => Promise<unknown>;

		await app.viewport(scenario.width, 600);
		const params = new URLSearchParams(
			Object.entries(scenario).map(([key, value]) => [key, String(value)])
		);
		params.set('harness', 'sidebar-header');
		await app.page.goto(`/?${params.toString()}`, { waitUntil: 'domcontentloaded' });
		await waitFor(
			'document.querySelector("[data-header-viewers]")?.textContent.trim() === "4,646"'
		);
		await app.waitForAnimations(
			'[data-sidebar="panel"], [data-sidebar="header"], [data-sidebar-search]'
		);

		const layout = await evaluate<{
			overflow: boolean;
			height: number;
			pills: number;
			label: string | undefined;
			trigger: boolean;
		}>(`({
			overflow: document.documentElement.scrollWidth > innerWidth,
			height: document.querySelector('[data-preview-header]').getBoundingClientRect().height,
			pills: document.querySelector('[data-header-tags]')?.children.length ?? 0,
			label: document.querySelector('[data-header-tags]')?.textContent.trim(),
			trigger: !!document.querySelector('[data-header-tags-trigger]')
		})`);

		assert.equal(layout.overflow, false, scenario.name + ' horizontal overflow');
		assert.equal(layout.height, 48, scenario.name + ' header height');
		if (scenario.width === 1200) {
			assert.equal(
				layout.pills,
				scenario['no-language'] && scenario['no-tags'] ? 0 : 1,
				scenario.name + ' one pill'
			);
			assert.equal(
				layout.label,
				scenario['no-language'] ? (scenario['no-tags'] ? undefined : 'Tags') : 'English'
			);
			assert.equal(layout.trigger, !scenario['no-tags'], scenario.name + ' tooltip availability');
		}

		if (scenario.open === false) {
			const styles: {
				width: number;
				height: number;
				left: number;
				railWidth: number;
				background: string;
				color: string;
				radius: string;
			}[] = [];
			for (const selector of [
				'[data-sidebar="header"] [data-sidebar="trigger"]',
				'[data-sidebar-home]',
				'[data-sidebar="header"] [data-popover-trigger]',
				'[data-sidebar="content"] [data-sidebar="menu-button"]',
				'[data-sidebar="footer"] [data-sidebar="menu-button"]'
			]) {
				assert(
					await evaluate(`!!document.querySelector(${JSON.stringify(selector)})`),
					`Missing ${selector}`
				);
				const center = await evaluate<{ x: number; y: number }>(
					`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`
				);
				await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...center });
				await waitFor(
					`getComputedStyle(document.querySelector(${JSON.stringify(selector)})).backgroundColor !== 'rgba(0, 0, 0, 0)'`
				);
				await app.waitForAnimations(selector);
				styles.push(
					await evaluate(`(() => {
						const el = document.querySelector(${JSON.stringify(selector)}), r = el.getBoundingClientRect(), s = getComputedStyle(el);
						const rail = document.querySelector('[data-sidebar="sidebar"]').getBoundingClientRect();
						return { width: r.width, height: r.height, left: r.left, railWidth: rail.width,
							background: s.backgroundColor, color: s.color, radius: s.borderRadius };
					})()`)
				);
			}
			for (const style of styles) {
				assert.equal(style.height, 40, scenario.name + ' 40px hover height');
				assert.equal(style.left, 0, scenario.name + ' full rail hover');
				assert.equal(style.width, style.railWidth, scenario.name + ' full rail hover width');
				assert.equal(style.background, styles[3].background, scenario.name + ' shared hover color');
				assert.notEqual(
					style.background,
					'rgba(0, 0, 0, 0)',
					scenario.name + ' visible hover background'
				);
				assert.equal(style.color, styles[3].color, scenario.name + ' shared hover foreground');
				assert.equal(style.radius, styles[3].radius, scenario.name + ' shared corner radius');
			}
			const centers = await evaluate<number[]>(
				`Array.from(document.querySelectorAll('[data-sidebar="header"] svg')).map(el => { const r = el.getBoundingClientRect(); return r.y + r.height / 2; })`
			);
			assert.deepEqual(
				centers.sort((a, b) => a - b),
				[24, 72, 120],
				scenario.name + ' collapse, Home, Search icon centers'
			);
		}

		if (scenario.width >= 768) {
			type Rect = {
				x: number;
				y: number;
				width: number;
				height: number;
				right: number;
				bottom: number;
			};
			const controls = await evaluate<{
				search: Rect;
				home: Rect;
				toggle: Rect;
				header: Rect;
				sidebar: Rect;
			}>(`(() => {
				const rect = selector => {
					const r = document.querySelector(selector).getBoundingClientRect();
					return { x: r.x, y: r.y, width: r.width, height: r.height, right: r.right, bottom: r.bottom };
				};
				return {
					search: rect('[data-sidebar-search] input, [data-sidebar-search] [data-popover-trigger]'),
					home: rect('[data-sidebar-home]'),
					toggle: rect('[data-sidebar="header"] [data-sidebar="trigger"]'),
					header: rect('[data-sidebar="header"]'),
					sidebar: rect('[data-sidebar="panel"]')
				};
			})()`);
			assert.equal(
				controls.sidebar.width,
				scenario.open === false ? 48 : 256,
				scenario.name + ' sidebar width'
			);
			assert.equal(
				controls.header.height,
				scenario.open === false ? 144 : 48,
				scenario.name + ' controls height'
			);
			if (scenario.open === false) {
				assert(controls.toggle.bottom <= controls.home.y, scenario.name + ' collapse above Home');
				assert(controls.home.bottom <= controls.search.y, scenario.name + ' Home above Search');
			} else {
				assert(controls.search.right <= controls.home.x, scenario.name + ' Search before Home');
				assert(controls.home.right <= controls.toggle.x, scenario.name + ' Home before collapse');
				assert.equal(controls.search.y, controls.home.y, scenario.name + ' aligned Home');
				assert.equal(controls.home.y, controls.toggle.y, scenario.name + ' aligned collapse');
			}

			await send('Input.dispatchMouseEvent', {
				type: 'mouseMoved',
				x: controls.home.x + controls.home.width / 2,
				y: controls.home.y + controls.home.height / 2
			});
			const homeTooltip = `Array.from(document.querySelectorAll('[data-tooltip-content]')).some(el => el.textContent.trim() === 'Home' && el.dataset.state !== 'closed' && el.getBoundingClientRect().width > 0)`;
			await waitFor(homeTooltip);
			await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: scenario.width - 5, y: 400 });
			await evaluate(
				`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
			);
			for (const type of ['keyDown', 'keyUp']) {
				await send('Input.dispatchKeyEvent', { type, key: 'Tab', code: 'Tab', modifiers: 8 });
			}
			await waitFor(`document.activeElement === document.querySelector('[data-sidebar-home]')`);
			await waitFor(homeTooltip);
			assert.equal(
				await evaluate(`getComputedStyle(document.activeElement).boxShadow !== 'none'`),
				true,
				scenario.name + ' Home keyboard focus ring'
			);
			await evaluate('document.activeElement.blur()');

			if (scenario.open === false) {
				await evaluate(
					`document.querySelector('[data-sidebar-search] [data-popover-trigger]').click()`
				);
				await waitFor(`document.activeElement === document.querySelector('#search-floating')`);
				assert.equal(
					await evaluate(`!!document.querySelector('[data-sidebar-home]')`),
					true,
					scenario.name + ' Home remains beside popover'
				);
				assert.equal(
					await evaluate(
						`document.querySelector('[data-sidebar="header"]').getBoundingClientRect().height`
					),
					144
				);
				for (const type of ['keyDown', 'keyUp']) {
					await send('Input.dispatchKeyEvent', { type, key: 'Escape', code: 'Escape' });
				}
				await waitFor(`!document.querySelector('#search-floating')`);
			} else {
				await evaluate(`(() => {
					const input = document.querySelector('[data-sidebar-search] input');
					input.value = 'maya'; input.dispatchEvent(new Event('input', { bubbles: true })); input.focus();
				})()`);
				await waitFor(`!document.querySelector('[data-sidebar-home]')`);
				await waitFor(
					`document.querySelector('[data-sidebar-search] input')?.getBoundingClientRect().width === ${controls.search.width + 40}`
				);
				assert.equal(
					await evaluate(
						`document.querySelector('[data-sidebar-search] input').getBoundingClientRect().width`
					),
					controls.search.width + 40,
					scenario.name + ' focused input expands'
				);
				for (const type of ['keyDown', 'keyUp']) {
					await send('Input.dispatchKeyEvent', { type, key: 'Tab', code: 'Tab' });
				}
				await waitFor(`document.activeElement?.textContent.trim() === 'Clear search'`);
				assert.equal(
					await evaluate(`!!document.querySelector('[data-sidebar-home]')`),
					false,
					scenario.name + ' clear button keeps Home hidden'
				);
				await evaluate('document.activeElement.click()');
				await waitFor(
					`document.activeElement === document.querySelector('[data-sidebar-search] input') && document.activeElement.value === ''`
				);
				assert.equal(
					await evaluate(`!!document.querySelector('[data-sidebar-home]')`),
					false,
					scenario.name + ' cleared input keeps focus layout'
				);
				await evaluate(
					`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
				);
				await waitFor(`!!document.querySelector('[data-sidebar-home]')`);
				await waitFor(
					`document.querySelector('[data-sidebar-search] input')?.getBoundingClientRect().width === ${controls.search.width}`
				);
				assert.equal(
					await evaluate(
						`document.querySelector('[data-sidebar-search] input').getBoundingClientRect().width`
					),
					controls.search.width,
					scenario.name + ' empty input shrinks on blur'
				);
				await evaluate(`(() => {
					const input = document.querySelector('[data-sidebar-search] input');
					input.focus(); input.value = 'maya'; input.dispatchEvent(new Event('input', { bubbles: true }));
				})()`);
				await waitFor(`!document.querySelector('[data-sidebar-home]')`);
				await evaluate(
					`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
				);
				await waitFor(`!!document.querySelector('[data-sidebar-home]')`);
				await waitFor(
					`document.querySelector('[data-sidebar-search] input')?.getBoundingClientRect().width === ${controls.search.width}`
				);
				assert.equal(
					await evaluate(
						`document.querySelector('[data-sidebar-search] input').getBoundingClientRect().width`
					),
					controls.search.width,
					scenario.name + ' filled input shrinks on blur'
				);
				assert.equal(
					await evaluate(`document.querySelector('[data-sidebar-search] input').value`),
					'maya',
					scenario.name + ' blur preserves query'
				);
			}
		}

		if (layout.trigger) {
			const center = await evaluate<{ x: number; y: number }>(
				`(() => { const r = document.querySelector('[data-header-tags-trigger]').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`
			);
			await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...center });
			const tooltip = await waitFor<{ tags: string[]; overflow: boolean }>(`(() => {
				const el = document.querySelector('[data-header-tags-tooltip]'); if (!el?.getBoundingClientRect().width) return null;
				return { tags: Array.from(el.children).map(tag => tag.textContent), overflow: Array.from(el.children).some(tag => tag.scrollWidth > tag.clientWidth + 1) };
			})()`);
			assert.deepEqual(tooltip.tags, [
				'Hair',
				'DropsEnabled',
				'AnExceptionallyLongTagThatShouldRemainFullyReadable'
			]);
			assert.equal(tooltip.overflow, false, scenario.name + ' wrapped full tag text');
			await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: scenario.width - 5, y: 400 });
			await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape' });
		}

		if (scenario.name === 'expanded-dark') {
			const result = await evaluate(`(async () => {
				const wait = (fn) => new Promise((resolve, reject) => {
					const deadline = performance.now() + 5000;
					const tick = () => {
						const value = fn();
						if (value) resolve(value);
						else if (performance.now() > deadline) reject(new Error('Timed out waiting for sidebar transition'));
						else requestAnimationFrame(tick);
					};
					tick();
				});
				const input = document.querySelector('[data-sidebar="header"] input');
				const toggle = document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]');
				const rect = node => node.getBoundingClientRect();
				const aligned = rect(input).y === rect(toggle).y && rect(input).height === rect(toggle).height;
				input.value = 'maya'; input.dispatchEvent(new Event('input', { bubbles: true })); input.focus();
				toggle.click();
				const collapsed = await wait(() => {
					const trigger = document.querySelector('[data-popover-trigger]');
					return trigger && document.activeElement === trigger ? trigger : null;
				});
				const collapsedFocus = document.activeElement === collapsed;
				toggle.click();
				const expanded = await wait(() => {
					const field = document.querySelector('[data-sidebar="header"] input');
					return field && document.activeElement === field ? field : null;
				});
				return {
					aligned,
					collapsedFocus,
					value: expanded.value,
					expandedFocus: document.activeElement === expanded
				};
			})()`);
			assert.deepEqual(
				result,
				{ aligned: true, collapsedFocus: true, value: 'maya', expandedFocus: true },
				'search query and focus survive both transitions'
			);
			await evaluate(
				`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
			);
			await waitFor(`!!document.querySelector('[data-sidebar-home]')`);
		}

		if (scenario.width < 768) {
			await evaluate(`document.querySelector('[data-mobile-opener]').click()`);
			await waitFor(`document.querySelector('[data-mobile="true"] input')`);
			assert.equal(
				await evaluate(`document.querySelector('[data-mobile="true"] input').readOnly`),
				false,
				'editable mobile search'
			);
			assert.equal(
				await evaluate(`!!document.querySelector('[data-popover-trigger]')`),
				false,
				'inline mobile search'
			);
		}
	});
}
