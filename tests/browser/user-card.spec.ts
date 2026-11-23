import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { test } from '../support/browser/fixtures';

async function sendKey(
	cdp: (method: string, params?: Record<string, unknown>) => Promise<unknown>,
	key: string,
	code: string,
	windowsVirtualKeyCode: number,
	modifiers = 0,
	text?: string
) {
	await cdp('Input.dispatchKeyEvent', {
		type: 'rawKeyDown',
		key,
		code,
		windowsVirtualKeyCode,
		modifiers
	});
	if (text) {
		await cdp('Input.dispatchKeyEvent', {
			type: 'char',
			text,
			key,
			code,
			windowsVirtualKeyCode,
			modifiers
		});
	}
	await cdp('Input.dispatchKeyEvent', {
		type: 'keyUp',
		key,
		code,
		windowsVirtualKeyCode,
		modifiers
	});
}

for (const theme of ['dark', 'light'] as const) {
	for (const chrome of ['compact', 'native'] as const) {
		for (const width of [320, 360, 420] as const) {
			test(`user-card route (${theme}, ${chrome}, ${width}px)`, async ({ app }) => {
				const { cdp, evaluate, waitFor } = app;
				const send = cdp as (method: string, params?: Record<string, unknown>) => Promise<unknown>;
				const resize = (w: number, h: number) => app.viewport(w, h);

				await resize(width, 240);
				await app.page.goto(`/?harness=user-card&chrome=${chrome}`, {
					waitUntil: 'domcontentloaded'
				});
				await waitFor('window.cardNative?.ready && !!document.querySelector("[data-user-card]")');
				await evaluate(
					`document.documentElement.classList.toggle('dark', ${theme === 'dark'}); cardNative.update({ chrome: ${JSON.stringify(chrome)} });`
				);
				await waitFor(`!!document.querySelector('[aria-label="Pin user card"]')`);

				const contentHeight = await evaluate<number>('cardNative.heights[0]');
				const initialHeight = Math.max(120, contentHeight);
				assert(
					initialHeight >= 120 && initialHeight < 160,
					'initial height fits the empty-badge card'
				);
				await resize(width, initialHeight);

				assert.equal(
					await evaluate(
						`document.querySelector('[aria-label="Window controls"]').getBoundingClientRect().height`
					),
					32
				);
				assert.equal(
					await evaluate(`getComputedStyle(document.querySelector('header')).cursor`),
					'move'
				);
				assert.equal(
					await evaluate(
						`getComputedStyle(document.querySelector('[aria-label="Pin user card"]')).cursor`
					),
					'pointer'
				);
				assert.equal(
					await evaluate(`document.querySelector('[data-user-card-channel]').textContent.trim()`),
					'#room'
				);

				const longChannel = 'a_channel_name_long_enough_to_truncate_in_the_toolbar'.repeat(2);
				await evaluate(`cardNative.update({ channel_login: ${JSON.stringify(longChannel)} })`);
				await waitFor(
					`document.querySelector('[data-user-card-channel]').title === ${JSON.stringify('#' + longChannel)}`
				);

				const toolbarLayout = await evaluate<{
					text: string;
					title: string;
					scrollWidth: number;
					clientWidth: number;
					buttons: { width: number; height: number; right: number }[];
				}>(`(() => {
					const channel = document.querySelector('header [data-user-card-channel]');
					const buttons = [...document.querySelectorAll('header button')];
					return { text: channel.textContent.trim(), title: channel.title,
						scrollWidth: channel.scrollWidth, clientWidth: channel.clientWidth,
						buttons: buttons.map(button => {
							const rect = button.getBoundingClientRect();
							return { width: rect.width, height: rect.height, right: rect.right };
						}) };
				})()`);
				assert.equal(toolbarLayout.text, toolbarLayout.title);
				assert(
					toolbarLayout.scrollWidth > toolbarLayout.clientWidth,
					JSON.stringify(toolbarLayout)
				);
				assert(
					toolbarLayout.buttons.every(
						(button) => button.width >= 28 && button.height >= 28 && button.right <= width
					),
					JSON.stringify(toolbarLayout)
				);

				await evaluate(`cardNative.update({ channel_login: 'room' })`);
				assert.equal(
					await evaluate(`!!document.querySelector('[aria-label="Channel badges"]')`),
					false
				);
				assert.equal(
					await evaluate(`document.body.textContent.includes('Badges not observed')`),
					false
				);
				await evaluate('cardNative.update({ badges: [] })');
				assert.equal(
					await evaluate(`document.body.textContent.includes('No badges observed')`),
					false
				);

				assert.deepEqual(
					await evaluate(
						`Array.from(document.querySelectorAll('header button'), button => button.getAttribute('aria-label'))`
					),
					['Open in Twitch', 'Pin user card', ...(chrome === 'compact' ? ['Close user card'] : [])]
				);
				assert.equal(await evaluate(`!!document.querySelector('[data-user-card-footer]')`), false);

				for (const [label, tooltip, preceding, modifiers] of [
					['Open in Twitch', 'Open in Twitch', 'Pin user card', 8],
					['Pin user card', 'Keep this card open', 'Open in Twitch', 0]
				] as const) {
					const selector = `[aria-label="${label}"]`;
					await evaluate(
						`window.dispatchEvent(new Event('blur')); document.activeElement.blur(); document.querySelector('${selector}').focus()`
					);
					await delay(200);
					assert.equal(
						await evaluate(`!!document.querySelector('[role="tooltip"]')`),
						false,
						`${label}: automatic focus does not open tooltip`
					);
					await evaluate(
						`window.dispatchEvent(new Event('blur')); window.dispatchEvent(new Event('focus')); document.activeElement.dispatchEvent(new FocusEvent('focus'))`
					);
					await delay(200);
					assert.equal(
						await evaluate(`!!document.querySelector('[role="tooltip"]')`),
						false,
						`${label}: restored focus does not open tooltip`
					);
					await evaluate(`document.querySelector('[aria-label="${preceding}"]').focus()`);
					await sendKey(send, 'Tab', 'Tab', 9, modifiers);
					await waitFor(
						`document.querySelector('[role="tooltip"]')?.textContent === ${JSON.stringify(tooltip)}`
					);
					assert.equal(await evaluate(`document.activeElement.getAttribute('aria-label')`), label);
					await evaluate(`window.dispatchEvent(new Event('blur')); document.activeElement.blur()`);
					await waitFor(`!document.querySelector('[role="tooltip"]')`);

					const point = await evaluate<{ x: number; y: number }>(
						`(() => { const rect = document.querySelector('${selector}').getBoundingClientRect(); return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }; })()`
					);
					await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point });
					await waitFor(
						`document.querySelector('[role="tooltip"]')?.textContent === ${JSON.stringify(tooltip)}`
					);
					await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 5, y: 80 });
					await evaluate(`window.dispatchEvent(new Event('blur'))`);
					await waitFor(`!document.querySelector('[role="tooltip"]')`);
				}

				await evaluate(`document.querySelector('[aria-label="Open in Twitch"]').focus()`);
				await sendKey(send, 'Tab', 'Tab', 9);
				await waitFor(`!!document.querySelector('[role="tooltip"]')`);
				await sendKey(send, 'Enter', 'Enter', 13, 0, '\r');
				await waitFor(
					`document.querySelector('[aria-label="Unpin user card"]')?.getAttribute('aria-pressed') === 'true'`
				);
				await sendKey(send, ' ', 'Space', 32, 0, ' ');
				await waitFor(`!!document.querySelector('[aria-label="Pin user card"]')`);
				await sendKey(send, 'Enter', 'Enter', 13, 0, '\r');
				await waitFor(`!!document.querySelector('[aria-label="Unpin user card"]')`);
				await evaluate(`window.dispatchEvent(new Event('blur')); document.activeElement.blur()`);
				await waitFor(`!document.querySelector('[role="tooltip"]')`);

				const detailsTop = await evaluate<number>(
					`document.querySelector('[data-user-card-details]').getBoundingClientRect().top`
				);
				await resize(width, 400);
				assert.equal(
					await evaluate(
						`document.querySelector('[data-user-card-details]').getBoundingClientRect().top`
					),
					detailsTop,
					'growing the window leaves details below the controls'
				);

				await evaluate(`cardNative.update({ profile_status: 'loading' })`);
				await waitFor(`document.body.textContent.includes('Loading profile')`);
				await resize(width, 120);
				assert.equal(
					await evaluate(
						`(() => { const node = document.querySelector('[data-user-card-scroll]'); node.scrollTop = node.scrollHeight; return node.scrollHeight > node.clientHeight; })()`
					),
					true
				);
				assert.equal(
					await evaluate(`document.querySelector('header').getBoundingClientRect().top`),
					0,
					'window controls remain visible while scrolling'
				);
				assert.equal(
					await evaluate(
						`document.querySelector('[aria-label="Open in Twitch"]').getBoundingClientRect().bottom <= 32`
					),
					true,
					'Twitch button stays visible above the scrolling details'
				);

				await resize(width, initialHeight);
				await evaluate(`cardNative.update({ profile_status: 'ready' })`);
				await waitFor(`!document.querySelector('[role="status"]')`);
				await evaluate(`document.querySelector('[data-user-card-scroll]').scrollTop = 0`);

				await evaluate(`cardNative.update({ profile_status: 'unavailable' });`);
				await waitFor(`document.body.textContent.includes('unavailable')`);
				await evaluate(
					`Array.from(document.querySelectorAll('button')).find(button => button.textContent === 'Retry').click()`
				);
				assert.equal(await evaluate('cardNative.retries'), 1);

				await evaluate(
					`cardNative.update({ profile_status: 'ready', user: { user_id: '1', login: 'maya', display_name: '마야 — A longer display name that wraps', color: null }, badges: ['broadcaster', 'moderator', 'vip', 'subscriber', 'partner'].map(set_id => ({ set_id, id: '1', info: '', badge: { set_id: '', id: '', image_url_1x: '', image_url_2x: '', image_url_4x: '', title: '', description: '' } })) });`
				);
				await delay(50);
				assert.equal(
					await evaluate(`document.querySelector('[aria-label="Channel badges"]').children.length`),
					5
				);
				assert.deepEqual(
					await evaluate('cardNative.heights'),
					[contentHeight],
					'profile and badge updates do not refit an open window'
				);
				assert.equal(await evaluate('innerHeight'), initialHeight);
				assert.equal(
					await evaluate('document.documentElement.scrollWidth <= innerWidth'),
					true,
					'no horizontal overflow'
				);
				assert.equal(
					await evaluate(`!!document.querySelector('[aria-label="Close user card"]')`),
					chrome === 'compact'
				);

				await evaluate(
					`cardNative.update({ user: { user_id: '1', login: 'maya', display_name: 'Maya', color: null } }); document.querySelector('[data-user-card-scroll]').scrollTop = 0`
				);
				await waitFor(`document.querySelector('h1').textContent.trim() === 'Maya'`);
				const profileHeight = await evaluate<number>(
					`Math.max(120, Math.ceil(document.querySelector('header').getBoundingClientRect().height + document.querySelector('[data-user-card-details]').getBoundingClientRect().height))`
				);
				await resize(width, profileHeight);

				await evaluate('cardNative.policy(false)');
				await waitFor(`!document.querySelector('[aria-label="Unpin user card"]')`);
				assert.deepEqual(
					await evaluate(
						`Array.from(document.querySelectorAll('header button'), button => button.getAttribute('aria-label'))`
					),
					['Open in Twitch', ...(chrome === 'compact' ? ['Close user card'] : [])]
				);
				await evaluate(
					`document.querySelector('[aria-label="Open in Twitch"]').click(); document.querySelector('[aria-label="Open in Twitch"]').focus()`
				);
				await sendKey(send, 'Enter', 'Enter', 13, 0, '\r');
				await sendKey(send, ' ', 'Space', 32, 0, ' ');
				assert.deepEqual(await evaluate('cardNative.urls'), [
					'https://www.twitch.tv/maya',
					'https://www.twitch.tv/maya',
					'https://www.twitch.tv/maya'
				]);
				await send('Input.dispatchKeyEvent', {
					type: 'keyDown',
					key: 'Escape',
					code: 'Escape',
					windowsVirtualKeyCode: 27
				});
				await waitFor('cardNative.closes === 1');
				assert.deepEqual(await evaluate('cardNative.errors'), []);
			});
		}
	}
}
