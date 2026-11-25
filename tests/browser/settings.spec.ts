import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { test } from '../support/browser/fixtures';

const cases = [
	{ name: 'expanded-dark', width: 1200, open: true, theme: 'dark' },
	{ name: 'collapsed-dark', width: 1200, open: false, theme: 'dark' },
	{ name: 'expanded-light', width: 1200, open: true, theme: 'light' },
	{ name: 'collapsed-light', width: 1200, open: false, theme: 'light' },
	{ name: 'narrow-expanded', width: 820, open: true, theme: 'dark' },
	{ name: 'mobile-dark', width: 375, open: true, theme: 'dark' },
	{ name: 'mobile-light', width: 320, open: false, theme: 'light' }
] as const;

for (const scenario of cases) {
	test(`settings page, navigation, translation models, and sidebar state (${scenario.name})`, async ({
		app
	}) => {
		const { cdp, evaluate, waitFor } = app;
		const send = cdp as (method: string, params?: Record<string, unknown>) => Promise<unknown>;

		const click = async (selector: string) => {
			await evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
			await evaluate('new Promise(requestAnimationFrame)');
		};
		const pressKey = async (key: string, code = key) => {
			const virtualKeyCode = { Enter: 13, Space: 32, Tab: 9 }[code];
			for (const type of ['keyDown', 'keyUp']) {
				await send('Input.dispatchKeyEvent', {
					type,
					key,
					code,
					text: type === 'keyDown' ? { Enter: '\r', Space: ' ' }[code] : undefined,
					windowsVirtualKeyCode: virtualKeyCode,
					nativeVirtualKeyCode: virtualKeyCode
				});
			}
		};
		const openSettings = async () => {
			await evaluate(
				`document.querySelector('[data-sidebar="footer"] [data-sidebar="menu-button"]').focus()`
			);
			await pressKey('Enter');
			await waitFor(
				`Array.from(document.querySelectorAll('[role="menuitem"]')).some(item => item.textContent.trim() === 'Settings')`
			);
			await evaluate(
				`Array.from(document.querySelectorAll('[role="menuitem"]')).find(item => item.textContent.trim() === 'Settings').focus()`
			);
			await pressKey('Enter');
			await waitFor(
				`location.pathname === '/app/settings' && !!document.querySelector('[data-settings-page]')`
			);
			await waitFor(`getComputedStyle(document.body).pointerEvents !== 'none'`);
		};
		const search = async (value: string) => {
			await evaluate(
				`(() => { const input = document.querySelector('[aria-label="Search settings"]'); input.value = ${JSON.stringify(value)}; input.dispatchEvent(new Event('input', { bubbles: true })); })()`
			);
			await evaluate('new Promise(requestAnimationFrame)');
		};

		await app.viewport(scenario.width, 800);
		const params = new URLSearchParams({
			harness: 'settings',
			open: String(scenario.open),
			theme: scenario.theme
		});
		await app.page.goto(`/app/settings?${params}`, { waitUntil: 'domcontentloaded' });
		await waitFor(`document.querySelector('#setting-chat-show_badges')?.disabled === false`);
		await app.waitForAnimations(
			'[data-sidebar="panel"], [data-sidebar="header"], [data-sidebar-search]'
		);

		const layout = await evaluate<{
			overflow: boolean;
			shellWidth: number;
			direction: string;
			dark: boolean;
			sidebar: number | undefined;
			rows: number;
		}>(`(() => {
			const shell = document.querySelector('[data-settings-page]'), nav = document.querySelector('[aria-label="Settings categories"]');
			return { overflow: document.documentElement.scrollWidth > innerWidth, shellWidth: shell.clientWidth,
				direction: getComputedStyle(nav).flexDirection, dark: document.documentElement.classList.contains('dark'),
				sidebar: document.querySelector('[data-sidebar="panel"]')?.getBoundingClientRect().width,
				rows: document.querySelectorAll('[data-setting-row]').length };
		})()`);
		assert.equal(layout.overflow, false, scenario.name + ' page overflow');
		assert.equal(layout.rows, 7, scenario.name + ' Chat essentials');
		assert.equal(
			layout.direction,
			layout.shellWidth < 640 ? 'row' : 'column',
			scenario.name + ' content-width navigation'
		);
		assert.equal(layout.dark, scenario.theme === 'dark', scenario.name + ' theme');
		if (scenario.width >= 768) {
			assert.equal(
				layout.sidebar,
				scenario.open ? 256 : 48,
				scenario.name + ' retained sidebar width'
			);
		}

		assert.deepEqual(
			await evaluate(`(() => {
				const slot = document.querySelector('[data-settings-back-slot]');
				const button = document.querySelector('[data-settings-back]');
				const icon = button.querySelector('svg');
				const header = slot.closest('header');
				return {
					slotWidth: slot.getBoundingClientRect().width,
					slotHeight: slot.getBoundingClientRect().height,
					headerHeight: header.clientHeight,
					divider: getComputedStyle(slot).borderRightWidth,
					buttonWidth: button.getBoundingClientRect().width,
					iconWidth: icon.getBoundingClientRect().width,
					label: button.ariaLabel,
					titleFollows: slot.nextElementSibling.querySelector('h1')?.textContent,
					saveStatus: !!header.querySelector('[data-settings-status]')
				};
			})()`),
			{
				slotWidth: 48,
				slotHeight: 47,
				headerHeight: 47,
				divider: '1px',
				buttonWidth: 32,
				iconWidth: 20,
				label: 'Back',
				titleFollows: 'Settings',
				saveStatus: true
			},
			scenario.name + ' back button matches video slot'
		);

		const backCenter = await evaluate<{ x: number; y: number }>(`(() => {
			const r = document.querySelector('[data-settings-back]').getBoundingClientRect();
			return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
		})()`);
		await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...backCenter });
		await waitFor(`!!document.querySelector('[data-settings-back-tooltip][data-state$="open"]')`);
		assert.equal(
			await evaluate(`document.querySelector('[data-settings-back-tooltip]').textContent`),
			'Back',
			scenario.name + ' back hover tooltip'
		);
		await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 100 });
		await waitFor(`!document.querySelector('[data-settings-back-tooltip][data-state$="open"]')`);
		await pressKey('Tab');
		await evaluate(`document.querySelector('[data-settings-back]').focus()`);
		await waitFor(`!!document.querySelector('[data-settings-back-tooltip][data-state$="open"]')`);
		await send('Page.resetNavigationHistory');
		assert.equal(await evaluate('history.length'), 1, 'direct settings entry has no history');
		assert.equal(
			await evaluate('window.navigation?.canGoBack ?? false'),
			false,
			'direct settings entry cannot go back'
		);
		if (scenario.name === 'collapsed-light') {
			await evaluate(
				`void Object.defineProperty(window, 'navigation', { value: undefined, configurable: true })`
			);
		}
		await pressKey('Enter');
		await waitFor(
			`location.pathname === '/app' && !!document.querySelector('img[alt="The project logo"]')`
		);
		assert.equal(
			await evaluate('history.length'),
			1,
			scenario.name + ' Home fallback replaces Settings'
		);
		assert.equal(await evaluate(`!!document.querySelector('[data-settings-back]')`), false);

		await app.page.goto(`/app/settings?${params}`, { waitUntil: 'domcontentloaded' });
		await waitFor(`document.querySelector('#setting-chat-show_badges')?.disabled === false`);

		await search('connector');
		assert.equal(await evaluate(`!!document.querySelector('#setting-translation-layout')`), true);
		await search('');
		assert.equal(
			await evaluate(`window.translationNative.calls.includes('status')`),
			false,
			'disabled startup does not connect model management'
		);
		await click('[aria-label="Settings categories"] button:nth-child(3)');
		assert.deepEqual(
			await evaluate(
				`Array.from(document.querySelectorAll('[data-setting-row]'), row => row.dataset.settingRow)`
			),
			['translation.enabled', 'translation.target_language', 'translation.layout']
		);
		assert.equal(await evaluate(`!!document.querySelector('[data-model-manager]')`), false);
		assert.equal(await evaluate(`window.translationNative.calls.includes('status')`), false);
		assert.equal(await evaluate(`document.documentElement.scrollWidth > innerWidth`), false);

		if (scenario.name === 'expanded-dark') {
			await evaluate(`window.translationNative.emit({ phase: 'unavailable' })`);
			await click('#setting-translation-enabled');
			await waitFor(`!!document.querySelector('[data-translation-setup-error]')`);
			assert.equal(
				await evaluate(
					`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked')`
				),
				'false'
			);
			await evaluate(`window.translationNative.emit({ phase: 'ready' })`);
		}

		await click('#setting-translation-enabled');
		await waitFor(`!!document.querySelector('[data-reviewed-model]')`);
		assert.equal(
			await evaluate(
				`document.getElementById(document.querySelector('[role="alertdialog"]').getAttribute('aria-describedby')).textContent.replace(/\\s+/g, ' ').trim()`
			),
			'Pepo needs a small language model to translate messages into English.'
		);
		assert.equal(await evaluate(`window.translationNative.calls.includes('start_install')`), false);
		assert.equal(
			await evaluate(
				`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked')`
			),
			'false'
		);
		assert.equal(
			await evaluate(
				`document.querySelector('[role="alertdialog"]').contains(document.activeElement)`
			),
			true
		);
		await evaluate(
			`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Cancel').click()`
		);
		await waitFor(`!document.querySelector('[role="alertdialog"]')`);
		assert.equal(await evaluate(`window.translationNative.calls.includes('start_install')`), false);

		await click('#setting-translation-enabled');
		await waitFor(`!!document.querySelector('[data-reviewed-model]')`);
		await evaluate(
			`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Download & enable').click()`
		);
		await waitFor(`!!document.querySelector('[data-translation-setup]')`);
		assert.equal(
			await evaluate(
				`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked')`
			),
			'false'
		);

		if (scenario.name === 'expanded-dark') {
			await click('[data-translation-cancel]');
			await waitFor(`!document.querySelector('[data-translation-setup]')`);
			assert.equal(
				await evaluate(
					`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked')`
				),
				'false'
			);
			await click('#setting-translation-enabled');
			await waitFor(`!!document.querySelector('[data-reviewed-model]')`);
			await evaluate(
				`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Download & enable').click()`
			);
			await waitFor(`!!document.querySelector('[data-translation-setup]')`);
		}

		await evaluate(`window.translationNative.finish()`);
		await waitFor(
			`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked') === 'true'`
		);

		if (scenario.name === 'expanded-dark') {
			await evaluate(
				`(() => { const select = document.querySelector('#setting-translation-target_language'); select.value = 'rus'; select.dispatchEvent(new Event('change', { bubbles: true })); })()`
			);
			await waitFor(`!!document.querySelector('[data-reviewed-model]')`);
			assert.equal(
				await evaluate(
					`document.getElementById(document.querySelector('[role="alertdialog"]').getAttribute('aria-describedby')).textContent.replace(/\\s+/g, ' ').trim()`
				),
				'Pepo needs a small language model to translate messages into Russian.'
			);
			await evaluate(
				`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Cancel').click()`
			);
			await waitFor(`!document.querySelector('[role="alertdialog"]')`);
			assert.equal(
				await evaluate(`document.querySelector('#setting-translation-target_language').value`),
				'eng'
			);
		}

		await click('#setting-translation-enabled');
		await waitFor(`window.settingsNative.writes.at(-1)?.translation.enabled === false`);
		await evaluate(`
			window.preparationFlashes = [];
			window.preparationObserver = new MutationObserver(records => {
				for (const record of records) {
					for (const node of record.addedNodes) {
						if (node.textContent?.includes('Preparing translation…')) window.preparationFlashes.push(node.textContent);
					}
				}
			});
			window.preparationObserver.observe(document.body, { childList: true, subtree: true });
		`);
		for (let attempt = 0; attempt < 3; attempt++) {
			if (attempt === 1) {
				await click('[aria-label="Settings categories"] button:first-child');
				await click('[aria-label="Settings categories"] button:nth-child(3)');
			}
			await click('#setting-translation-enabled');
			await waitFor(
				`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked') === 'true'`
			);
			await click('#setting-translation-enabled');
			await waitFor(
				`document.querySelector('#setting-translation-enabled').getAttribute('aria-checked') === 'false'`
			);
		}
		await delay(350);
		assert.deepEqual(
			await evaluate(`window.preparationFlashes`),
			[],
			scenario.name + ' preparation flash'
		);
		await evaluate(`window.preparationObserver.disconnect()`);

		for (const activation of ['pointer', 'keyboard'] as const) {
			if (activation === 'pointer') {
				const point = await evaluate<{ x: number; y: number }>(`(() => {
					const button = document.querySelector('[data-open-translation-models]');
					button.scrollIntoView({ block: 'nearest' });
					const rect = button.getBoundingClientRect();
					return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
				})()`);
				for (const type of ['mousePressed', 'mouseReleased']) {
					await send('Input.dispatchMouseEvent', { type, ...point, button: 'left', clickCount: 1 });
				}
			} else {
				await evaluate(`document.querySelector('[data-open-translation-models]').focus()`);
				await pressKey('Enter');
			}
			await waitFor(`document.querySelectorAll('[data-model-row]').length === 2`);
			const position = await evaluate(`(() => {
				const content = document.querySelector('[data-settings-content]');
				const bounds = content.getBoundingClientRect();
				const visible = selector => {
					const rect = document.querySelector(selector).getBoundingClientRect();
					return rect.top >= bounds.top && rect.bottom <= bounds.bottom;
				};
				return {
					scrollTop: content.scrollTop,
					overflows: content.scrollHeight > content.clientHeight,
					focus: document.activeElement.id,
					headingVisible: visible('#settings-category-title'),
					firstSettingVisible: visible('[data-setting-row="translation.translate_history"]')
				};
			})()`);
			assert.deepEqual(
				position,
				{
					scrollTop: 0,
					overflows: true,
					focus: 'settings-category-title',
					headingVisible: true,
					firstSettingVisible: true
				},
				`${scenario.name} Advanced opens at the top via ${activation}`
			);
			await evaluate(`document.querySelector('[data-settings-content]').scrollTop = 200`);
			await click('[data-translation-back]');
			assert.equal(
				await evaluate(`document.activeElement.hasAttribute('data-open-translation-models')`),
				true
			);
		}

		await search('models');
		await click('[data-open-translation-models]');
		await waitFor(`document.querySelectorAll('[data-model-row]').length === 2`);
		for (const field of [
			'translate_history',
			'detection_policy',
			'queue_policy',
			'queue_capacity',
			'max_in_flight'
		]) {
			assert.equal(
				await evaluate(`!!document.querySelector('#setting-translation-${field}')`),
				true
			);
		}
		await evaluate(`(() => {
			const input = document.querySelector('#setting-translation-queue_capacity');
			input.value = '257'; input.dispatchEvent(new Event('input', { bubbles: true }));
			input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
		})()`);
		await waitFor(
			`document.querySelector('#setting-translation-queue_capacity').getAttribute('aria-invalid') === 'true'`
		);
		await evaluate(`(() => {
			const input = document.querySelector('#setting-translation-queue_capacity');
			input.value = '32'; input.dispatchEvent(new Event('input', { bubbles: true }));
			input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
			const select = document.querySelector('#setting-translation-detection_policy');
			select.value = 'best_guess'; select.dispatchEvent(new Event('change', { bubbles: true }));
		})()`);
		await waitFor(
			`JSON.parse(localStorage.getItem('settings-check')).translation.queue_capacity === 32 && JSON.parse(localStorage.getItem('settings-check')).translation.detection_policy === 'best_guess'`
		);
		assert.equal(await evaluate(`!!document.querySelector('#setting-translation-layout')`), false);
		assert.equal(
			await evaluate(`document.activeElement.id`),
			'translation-models',
			'model search focuses its heading'
		);
		assert.equal(
			await evaluate(`document.documentElement.scrollWidth > innerWidth`),
			false,
			scenario.name + ' model list overflow'
		);
		assert.equal(
			await evaluate(
				`Array.from(document.querySelectorAll('[aria-label="Settings categories"] button')).every(button => { const r = button.getBoundingClientRect(); return r.left >= 0 && r.right <= innerWidth; })`
			),
			true,
			scenario.name + ' every category remains visible'
		);
		assert.equal(await evaluate(`!!document.querySelector('#setting-translation-enabled')`), false);

		await evaluate(`document.querySelector('[data-settings-content]').scrollTop = 0`);
		await click('[data-model-row="eng:rus"] [data-model-action="install"]');
		await waitFor(`!!document.querySelector('[data-reviewed-model]')`);
		assert.equal(
			await evaluate(
				`document.querySelector('[data-model-installation]')?.textContent.includes('English → Russian') ?? false`
			),
			false,
			'download requires consent'
		);
		assert.equal(
			await evaluate(
				`document.querySelector('[role="alertdialog"]').contains(document.activeElement)`
			),
			true,
			'confirmation receives focus'
		);
		await evaluate(
			`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Download').focus()`
		);
		await pressKey('Enter');
		await waitFor(`!!document.querySelector('[data-model-installation]')`);
		assert.equal(
			await evaluate(`document.querySelector('[data-model-progress-details]').open`),
			false
		);
		assert.equal(await evaluate(`document.querySelector('progress').value`), 25);
		await click('[aria-label="Settings categories"] button:first-child');
		assert.equal(
			await evaluate(`!!document.querySelector('[data-model-installation]')`),
			false,
			'progress stays in Translation settings'
		);
		await search('models');
		await click('[data-open-translation-models]');
		await waitFor(`!!document.querySelector('[data-model-installation]')`);
		await evaluate(`window.translationNative.finish()`);
		await waitFor(`document.querySelector('[data-model-notice="installed"]') !== null`);
		assert.equal(
			await evaluate(`window.translationNative.calls.includes('verify_model')`),
			false,
			'opening settings does not verify models'
		);

		await click('[aria-label="Settings categories"] button:first-child');
		await click('[aria-controls="settings-more-chat"]');
		assert.equal(
			await evaluate(
				`document.querySelector('#setting-chat-moderation_notices_enabled').getAttribute('aria-checked')`
			),
			'true'
		);
		assert.deepEqual(
			await evaluate(
				`Array.from(document.querySelector('#setting-chat-join_mode').options, option => option.value)`
			),
			['all', 'recent', 'open']
		);
		await evaluate(
			`document.querySelector('#settings-more-chat').scrollIntoView({ block: 'start' })`
		);
		await evaluate('new Promise(requestAnimationFrame)');
		assert.equal(
			await evaluate(`document.documentElement.scrollWidth > innerWidth`),
			false,
			scenario.name + ' moderation settings overflow'
		);
		await click('#setting-chat-moderation_notices_enabled');
		await waitFor(`window.settingsNative.writes.at(-1)?.chat.moderation_notices_enabled === false`);
		await click('#setting-chat-moderation_notices_enabled');
		await waitFor(`window.settingsNative.writes.at(-1)?.chat.moderation_notices_enabled === true`);
		await click('[aria-controls="settings-more-chat"]');

		if (scenario.name === 'collapsed-dark' || scenario.name === 'expanded-light') {
			await app.page.goto(`/app/chat/maya?${params}`, { waitUntil: 'domcontentloaded' });
			await waitFor(`!!document.querySelector('[data-chat-fixture]')`);
			await waitFor(
				`document.querySelector('[data-sidebar="panel"]').getBoundingClientRect().width === ${scenario.open ? 256 : 48}`
			);
			const chatUrl = await evaluate<string>('location.href');
			const videoSlot = await evaluate(`(() => {
				const r = document.querySelector('[aria-label="Stream video coming soon"]').parentElement.getBoundingClientRect();
				return { x: r.x, width: r.width };
			})()`);
			await openSettings();
			assert.equal(await evaluate('location.pathname'), '/app/settings', 'profile menu navigation');
			assert.deepEqual(
				await evaluate(`(() => {
					const r = document.querySelector('[data-settings-back-slot]').getBoundingClientRect();
					return { x: r.x, width: r.width };
				})()`),
				videoSlot,
				scenario.name + ' back and video slots align'
			);
			const historyLength = await evaluate<number>('history.length');
			for (const action of ['click', 'Enter', 'Space'] as const) {
				if (action === 'click') {
					await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...backCenter });
					for (const type of ['mousePressed', 'mouseReleased']) {
						await send('Input.dispatchMouseEvent', {
							type,
							...backCenter,
							button: 'left',
							clickCount: 1
						});
					}
				} else {
					await evaluate(`document.querySelector('[data-settings-back]').focus()`);
					await pressKey(action === 'Space' ? ' ' : action, action);
				}
				await waitFor(
					`location.href === ${JSON.stringify(chatUrl)} && !!document.querySelector('[data-chat-fixture]')`
				);
				assert.equal(
					await evaluate('history.length'),
					historyLength,
					action + ' traverses existing history'
				);
				assert.equal(await evaluate(`!!document.querySelector('[data-settings-back]')`), false);
				await evaluate('history.forward()');
				await waitFor(
					`location.pathname === '/app/settings' && !!document.querySelector('[data-settings-back]')`
				);
			}
			await app.page.goto(`/app?${params}`, { waitUntil: 'domcontentloaded' });
			await waitFor(`!!document.querySelector('img[alt="The project logo"]')`);
			await openSettings();
			await click('[data-settings-back]');
			await waitFor(
				`location.pathname === '/app' && !!document.querySelector('img[alt="The project logo"]')`
			);
			await evaluate('history.forward()');
			await waitFor(
				`location.pathname === '/app/settings' && !!document.querySelector('[data-settings-back]')`
			);
		}

		if (scenario.width >= 768) {
			// Wait for the 200ms header search transition after navigating back from /app.
			await app.waitForAnimations(
				'[data-sidebar="panel"], [data-sidebar="header"], [data-sidebar-search]'
			);
			const inputWidth = await evaluate<number>(
				`document.querySelector('[data-sidebar-search] input')?.getBoundingClientRect().width`
			);
			for (const source of ['settings', 'chat'] as const) {
				if (source === 'chat') {
					await click('[data-sidebar="content"] a[href="/app/chat/maya"]');
					await waitFor(`!!document.querySelector('[data-chat-fixture]')`);
				}
				await waitFor(`!!document.querySelector('[data-sidebar-home]')`);
				await evaluate(
					`document.querySelector('[data-sidebar-search] [data-popover-trigger]')?.click()`
				);
				await waitFor(`!!document.querySelector('[data-sidebar-search] input, #search-floating')`);
				await evaluate(`(() => {
					window.homeSidebar = document.querySelector('[data-sidebar="sidebar"]');
					const input = document.querySelector('[data-sidebar-search] input, #search-floating');
					input.value = 'maya'; input.dispatchEvent(new Event('input', { bubbles: true })); input.focus();
				})()`);
				if (scenario.open) {
					await waitFor(`!document.querySelector('[data-sidebar-home]')`);
					await evaluate(
						`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
					);
				}
				await waitFor(`!!document.querySelector('[data-sidebar-home]')`);
				await app.waitForAnimations(
					'[data-sidebar="panel"], [data-sidebar="header"], [data-sidebar-search]'
				);
				const keyboard = source === 'chat' ? scenario.open : !scenario.open;
				if (keyboard) {
					if (!scenario.open) {
						for (const type of ['keyDown', 'keyUp']) {
							await send('Input.dispatchKeyEvent', { type, key: 'Escape', code: 'Escape' });
						}
						await waitFor(`!document.querySelector('#search-floating')`);
					}
					await evaluate(`document.querySelector('[data-sidebar-home]').focus()`);
					await waitFor(`document.activeElement === document.querySelector('[data-sidebar-home]')`);
					await pressKey('Enter');
				} else {
					const center = await evaluate<{ x: number; y: number }>(`(() => {
						const r = document.querySelector('[data-sidebar-home]').getBoundingClientRect();
						return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
					})()`);
					await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...center });
					await send('Input.dispatchMouseEvent', {
						type: 'mousePressed',
						...center,
						button: 'left',
						clickCount: 1
					});
					// Hold >10ms so bits-ui's DismissibleLayer debounce fires before click unmounts Home.
					await delay(30);
					await send('Input.dispatchMouseEvent', {
						type: 'mouseReleased',
						...center,
						button: 'left',
						clickCount: 1
					});
				}
				await waitFor(
					`location.pathname === '/app' && !!document.querySelector('img[alt="The project logo"]')`
				);
				assert.equal(
					await evaluate(`!!document.querySelector('[data-sidebar-home]')`),
					false,
					scenario.name + ' Home hidden on home page'
				);
				assert.equal(
					await evaluate(
						`window.homeSidebar === document.querySelector('[data-sidebar="sidebar"]')`
					),
					true,
					scenario.name + ' navigation preserves sidebar instance'
				);
				await app.waitForAnimations(
					'[data-sidebar="panel"], [data-sidebar="header"], [data-sidebar-search]'
				);
				assert.equal(
					await evaluate(
						`document.querySelector('[data-sidebar="header"]').getBoundingClientRect().height`
					),
					scenario.open ? 48 : 96,
					scenario.name + ' no empty Home row'
				);
				if (scenario.open) {
					assert.equal(
						await evaluate(
							`document.querySelector('[data-sidebar-search] input').getBoundingClientRect().width`
						),
						inputWidth + 40,
						scenario.name + ' no empty Home column'
					);
				} else {
					await evaluate(
						`document.querySelector('[data-sidebar-search] [data-popover-trigger]').click()`
					);
					await waitFor(`!!document.querySelector('#search-floating')`);
				}
				assert.equal(
					await evaluate(
						`document.querySelector('[data-sidebar-search] input, #search-floating').value`
					),
					'',
					scenario.name + ' ' + source + ' to Home clears search'
				);
				if (!scenario.open) {
					for (const type of ['keyDown', 'keyUp']) {
						await send('Input.dispatchKeyEvent', { type, key: 'Escape', code: 'Escape' });
					}
					await waitFor(`!document.querySelector('#search-floating')`);
				}
				await waitFor(`document.querySelector('img[alt="The project logo"]').naturalWidth > 0`);
				await evaluate('history.back()');
				await waitFor(
					`location.pathname === '/app/${source === 'settings' ? 'settings' : 'chat/maya'}' && !!document.querySelector('[data-sidebar-home]')`
				);
			}
			await app.page.goto(`/app?${params}`, { waitUntil: 'domcontentloaded' });
			await waitFor(
				`!!document.querySelector('img[alt="The project logo"]') && !!document.querySelector('[data-sidebar-search]')`
			);
			assert.equal(
				await evaluate(`!!document.querySelector('[data-sidebar-home]')`),
				false,
				scenario.name + ' Home hidden on direct entry'
			);
		}
	});
}

test('settings persistence, save status, provider order, and sidebar local state (expanded-dark)', async ({
	app
}) => {
	const { cdp, evaluate, waitFor } = app;
	const send = cdp as (method: string, params?: Record<string, unknown>) => Promise<unknown>;

	const click = async (selector: string) => {
		await evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
		await evaluate('new Promise(requestAnimationFrame)');
	};
	const pressKey = async (key: string, code = key) => {
		const virtualKeyCode = { Enter: 13, Space: 32, Tab: 9 }[code];
		for (const type of ['keyDown', 'keyUp']) {
			await send('Input.dispatchKeyEvent', {
				type,
				key,
				code,
				text: type === 'keyDown' ? { Enter: '\r', Space: ' ' }[code] : undefined,
				windowsVirtualKeyCode: virtualKeyCode,
				nativeVirtualKeyCode: virtualKeyCode
			});
		}
	};
	const search = async (value: string) => {
		await evaluate(
			`(() => { const input = document.querySelector('[aria-label="Search settings"]'); input.value = ${JSON.stringify(value)}; input.dispatchEvent(new Event('input', { bubbles: true })); })()`
		);
		await evaluate('new Promise(requestAnimationFrame)');
	};

	await app.viewport(1200, 800);
	await app.page.goto('/app/settings?harness=settings&open=true&theme=dark', {
		waitUntil: 'domcontentloaded'
	});
	await waitFor(`document.querySelector('#setting-chat-show_badges')?.disabled === false`);

	await evaluate(`window.settingsNative.hold = true`);
	await click('#setting-chat-show_badges');
	await waitFor(`/Saving/.test(document.querySelector('[data-settings-status]').ariaLabel)`);
	await evaluate(`window.settingsNative.release()`);
	await waitFor(`document.querySelector('[data-settings-status]').ariaLabel === 'Settings saved'`);
	await evaluate(`window.settingsNative.fail = true`);
	await click('#setting-chat-show_badges');
	await waitFor(
		`document.querySelector('[data-settings-status]').ariaLabel.includes('Save failed')`
	);
	assert.equal(
		await evaluate(
			`document.querySelector('#setting-chat-show_badges').getAttribute('aria-checked')`
		),
		'false'
	);
	await evaluate(`document.querySelector('[data-settings-status]').focus()`);
	await waitFor(`!!document.querySelector('[data-settings-status-tooltip]')`);
	await evaluate(`window.settingsNative.fail = false; window.settingsNative.delay = 0`);
	await click('[aria-label="Reset settings"] > button');
	await waitFor(`!!document.querySelector('[role="alertdialog"]')`);
	await evaluate(
		`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent.trim() === 'Cancel').click()`
	);
	await search('emote size');
	await evaluate(
		`(() => { const input = document.querySelector('#setting-emotes-inline_emote_px'); input.value = '36'; input.dispatchEvent(new Event('input', { bubbles: true })); input.focus(); })()`
	);
	await pressKey('Enter');
	await waitFor(`window.settingsNative.writes.at(-1)?.emotes.inline_emote_px === 36`);
	await waitFor(`document.querySelector('[data-settings-status]').ariaLabel === 'Settings saved'`);
	await app.page.reload({ waitUntil: 'domcontentloaded' });
	await waitFor(`document.querySelector('#setting-chat-show_badges')?.disabled === false`);
	await search('emote size');
	assert.equal(
		await evaluate(`document.querySelector('#setting-emotes-inline_emote_px').value`),
		'36',
		'saved mock-native value survives reload'
	);
	await click('[aria-label="Settings categories"] button:nth-child(2)');
	await click('[aria-controls="settings-more-emotes"]');
	await click('[aria-label="Move BTTV up"]');
	await waitFor(
		`document.querySelector('[data-provider-priority]')?.dataset.providerPriority === 'bttv'`
	);
	await search('');
	await click('[aria-label="Settings categories"] button:first-child');

	await evaluate(`(() => {
		const saved = JSON.parse(localStorage.getItem('settings-check'));
		saved.layout = { sidebar_open: false };
		localStorage.setItem('settings-check', JSON.stringify(saved));
		localStorage.removeItem('sidebar:state');
		document.cookie = 'sidebar:state=false; path=/';
	})()`);
	await app.page.goto('/app/settings', { waitUntil: 'domcontentloaded' });
	await waitFor(`document.querySelector('#setting-chat-show_badges')?.disabled === false`);
	await waitFor(`document.querySelector('[data-collapsible]')?.dataset.state === 'expanded'`);
	assert.equal(
		await evaluate(`localStorage.getItem('sidebar:state')`),
		null,
		'legacy settings and cookie do not seed local sidebar preference'
	);
	const snapshot = () =>
		evaluate(
			`({ writes: window.settingsNative.writes.length, status: document.querySelector('[data-settings-status]').ariaLabel })`
		);
	const before = await snapshot();
	await click('[data-sidebar="header"] [data-sidebar="trigger"]');
	await waitFor(`document.querySelector('[data-collapsible]')?.dataset.state === 'collapsed'`);
	assert.equal(await evaluate(`localStorage.getItem('sidebar:state')`), 'false');
	assert.deepEqual(
		await snapshot(),
		before,
		'collapse does not sync settings or change save status'
	);
	assert.equal(
		await evaluate(`document.cookie.includes('sidebar:state=false')`),
		true,
		'collapse does not replace the old cookie'
	);
	for (const route of ['settings', 'chat/maya', 'settings']) {
		await app.page.goto(`/app/${route}`, { waitUntil: 'domcontentloaded' });
		await waitFor(`document.querySelector('[data-collapsible]')?.dataset.state === 'collapsed'`);
	}
	await click('[aria-label="Settings categories"] button:last-child');
	assert.equal(await evaluate(`!!document.querySelector('#setting-layout-sidebar_open')`), false);
	await search('sidebar');
	assert.equal(await evaluate(`document.querySelectorAll('[data-setting-row]').length`), 0);
	await search('');
	for (const reset of ['app', 'all'] as const) {
		if (reset === 'app') await click('[aria-label="Reset settings"] > button');
		else {
			await click('[aria-label="More reset actions"]');
			await click('[role="menuitem"]');
		}
		await evaluate(
			`Array.from(document.querySelectorAll('[role="alertdialog"] button')).find(button => button.textContent === 'Reset to defaults').click()`
		);
		await waitFor(
			`document.querySelector('[data-settings-status]').ariaLabel === 'Settings saved'`
		);
		await click('[data-alert-dialog-cancel]');
		assert.equal(
			await evaluate(`localStorage.getItem('sidebar:state')`),
			'false',
			reset + ' reset preserves sidebar preference'
		);
		assert.equal(
			await evaluate(`document.querySelector('[data-collapsible]').dataset.state`),
			'collapsed'
		);
	}
	await evaluate(`window.settingsNative.hold = true`);
	await evaluate(
		`Array.from(document.querySelectorAll('[data-setting-row="appearance.theme"] button')).find(button => button.textContent === 'Light').click()`
	);
	await waitFor(
		`document.querySelector('[data-settings-status]').ariaLabel === 'Saving settings…'`
	);
	const pending = await snapshot();
	await click('[data-sidebar="header"] [data-sidebar="trigger"]');
	assert.deepEqual(await snapshot(), pending, 'expand does not enqueue a settings save');
	assert.equal(await evaluate(`localStorage.getItem('sidebar:state')`), 'true');
	await evaluate(`window.settingsNative.release()`);
	await waitFor(`document.querySelector('[data-settings-status]').ariaLabel === 'Settings saved'`);
	await evaluate(`window.settingsNative.delay = 0; window.settingsNative.fail = true`);
	await evaluate(
		`Array.from(document.querySelectorAll('[data-setting-row="appearance.theme"] button')).find(button => button.textContent === 'Dark').click()`
	);
	await waitFor(
		`document.querySelector('[data-settings-status]').ariaLabel === 'Save failed. Previous value restored.'`
	);
	const failed = await snapshot();
	await click('[data-sidebar="header"] [data-sidebar="trigger"]');
	assert.deepEqual(await snapshot(), failed, 'collapse leaves an existing save failure alone');
	assert.equal(await evaluate(`localStorage.getItem('sidebar:state')`), 'false');
	assert.equal(
		await evaluate(`window.settingsNative.writes.every(value => !('layout' in value))`),
		true,
		'settings writes omit the retired layout section'
	);
	await app.page.reload({ waitUntil: 'domcontentloaded' });
	await waitFor(`document.querySelector('[data-collapsible]')?.dataset.state === 'collapsed'`);
	await click('[data-sidebar="footer"] [data-sidebar="menu-button"]');
	await waitFor(`!!document.querySelector('[role="menuitem"]')`);
	await evaluate(
		`Array.from(document.querySelectorAll('[role="menuitem"]')).find(item => item.textContent.trim() === 'Log out').click()`
	);
	await waitFor(`location.pathname === '/'`);
	assert.equal(
		await evaluate(`localStorage.getItem('sidebar:state')`),
		'false',
		'logout preserves the local sidebar preference'
	);
	await app.page.goto('/app/settings', { waitUntil: 'domcontentloaded' });
	await waitFor(`document.querySelector('[data-collapsible]')?.dataset.state === 'collapsed'`);
});
