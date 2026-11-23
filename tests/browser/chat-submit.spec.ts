import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { test } from '../support/browser/fixtures';

test('chat submit flows, emote completion, diagnostics, inline notices, and error reporting', async ({
	app
}) => {
	const { cdp, evaluate, waitFor } = app;
	const send = cdp as (method: string, params?: Record<string, unknown>) => Promise<unknown>;
	const run = evaluate;
	const wait = (expression: string, timeoutMs = 5000) => waitFor(expression, timeoutMs);

	await send('Network.enable');
	await send('Network.setBlockedURLs', {
		urls: ['https://*', 'http://api.twitch.tv/*', 'wss://*']
	});
	await app.page.goto('/?harness=chat-submit', { waitUntil: 'domcontentloaded' });

	const input = 'document.querySelector(\'[aria-label="Chat message"]\')';
	const form = `${input}.form`;
	const notice = '[data-error-notice]';
	const counter = '[data-chat-counter]';
	const inline = '[data-chat-input-error]';

	const clickText = async (selector: string, text: string) => {
		await run(
			`Array.from(document.querySelectorAll(${JSON.stringify(selector)})).find(node => node.textContent.trim() === ${JSON.stringify(text)}).click()`
		);
		await delay(40);
	};
	const type = async (text: string) => {
		await run(
			`(() => { const input = ${input}; input.focus(); input.value = ${JSON.stringify(text)}; input.dispatchEvent(new Event('input', { bubbles: true })); })()`
		);
	};
	const submit = async (text: string) => {
		const index = await run<number>('chatNative.calls.length');
		await type(text);
		await run(`${form}.requestSubmit()`);
		await wait(`chatNative.calls.length === ${index + 1}`);
		assert.equal(
			await run(`new Promise(resolve => requestAnimationFrame(() => resolve(${input}.value)))`),
			'',
			'clears before backend settles'
		);
		return index;
	};
	const settle = async (index: number, error?: Record<string, unknown>) => {
		await run(
			`chatNative.resolve(${index}, ${JSON.stringify(error ? { status: 'error', error } : { status: 'ok', data: null })})`
		);
		await wait(`${form}.getAttribute('aria-busy') === 'false'`);
	};
	const failure = (
		kind: string,
		message = `${kind} guidance`,
		details = 'private diagnostics'
	) => ({
		kind,
		message,
		details
	});

	await wait(`${input} && chatNative.joins.length === 1`);
	let index = await submit('first message');
	assert.deepEqual(await run('chatNative.calls[0]'), {
		broadcasterId: 'maya-id',
		message: 'first message',
		diagnostics: {
			attempt_id: await run('chatNative.calls[0].diagnostics.attempt_id'),
			channel_login: 'maya',
			last_completion: null,
			key_events: []
		}
	});
	await type('next draft');
	await run(`${form}.requestSubmit()`);
	assert.equal(await run('chatNative.calls.length'), 1);
	assert.equal(await run(`${input}.value`), 'next draft');
	await wait(
		`document.querySelector('${inline}')?.textContent.includes('previous message is still sending')`
	);
	await settle(index);
	assert.equal(await run(`${input}.value`), 'next draft', 'success preserves subsequent typing');

	for (const kind of [
		'authentication',
		'permission_denied',
		'rate_limited',
		'rejected',
		'connection',
		'timeout'
	]) {
		index = await submit(`${kind} message`);
		await type('new draft');
		const debugBefore = await run<number>('chatNative.debug.length');
		await settle(index, failure(kind));
		await wait(`document.querySelector('${inline}')?.textContent.includes('${kind} guidance')`);
		assert.equal(await run(`${input}.value`), 'new draft');
		assert.equal(
			await run('chatNative.debug.length'),
			debugBefore,
			'returned error never logs success'
		);
		assert.equal(await run(`document.querySelectorAll('${notice}').length`), 0);
	}

	await wait(`!document.querySelector('${inline}')`, 8000);
	index = await submit('timer replacement');
	await type('blocked draft');
	await run(`${form}.requestSubmit()`);
	await wait(
		`document.querySelector('${inline}')?.textContent.includes('previous message is still sending')`
	);
	await delay(2000);
	const replacedAt = await run<number>(`(${form}.requestSubmit(), performance.now())`);
	await settle(index);
	await wait(`!document.querySelector('${inline}')`, 8000);
	const removedAt = await run<number>('performance.now()');
	assert(
		removedAt - replacedAt >= 4500,
		`inline bar expired ${Math.round(removedAt - replacedAt)}ms after the replacing submit`
	);

	index = await submit('old view pending');
	await run(`chatNative.navigate('other')`);
	await wait(
		`chatNative.joins.at(-1) === 'other' && ${form}.getAttribute('aria-busy') === 'false'`
	);
	const other = await submit('new view send');
	const logsBeforeLate = await run<number>('chatNative.logs.length');
	await run(
		`chatNative.resolve(${index}, { status: 'error', error: ${JSON.stringify(failure('authentication'))} })`
	);
	await wait(`chatNative.logs.length > ${logsBeforeLate}`);
	assert.equal(
		await run(`!!document.querySelector('${inline}')`),
		false,
		'no late actionable feedback in another view'
	);
	await settle(other);
	assert.equal(await run(`${input}.value`), '', 'failed text is not restored');

	index = await submit('late technical failure');
	await run(`chatNative.navigate('third')`);
	await wait(`chatNative.joins.at(-1) === 'third'`);
	await run(
		`chatNative.resolve(${index}, { status: 'error', error: ${JSON.stringify(failure('unexpected', 'not user guidance', 'late technical details'))} })`
	);
	await wait(`document.querySelector('${notice}')?.textContent.includes('#other')`);
	assert.equal(await run(`!!document.querySelector('${inline}')`), false);
	assert.equal(
		await run(`document.querySelector('${notice}').textContent.includes('late technical details')`),
		false
	);
	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);

	for (let repeat = 0; repeat < 2; repeat++) {
		index = await submit(`technical failure ${repeat}`);
		await settle(
			index,
			failure('unexpected', 'not user guidance', `selected technical details ${repeat}`)
		);
	}
	await wait(`document.querySelectorAll('${notice}').length === 2`);
	const box = await run<{ x: number; y: number }>(
		`(() => { const box = document.querySelector('${notice}').getBoundingClientRect(); return { x: box.x + 8, y: box.y + 8 }; })()`
	);
	await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...box });
	await wait(`chatNative.pauses.some(event => event[0] === 'pause' && event[1] === 'hover')`);
	await run(`document.querySelector('${notice} button').focus()`);
	await wait(`chatNative.pauses.some(event => event[0] === 'pause' && event[1] === 'focus')`);
	await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 });
	await wait(`chatNative.pauses.some(event => event[0] === 'resume' && event[1] === 'hover')`);
	await clickText(`${notice} button`, 'Report this');
	await wait(`!!document.querySelector('[data-inbound-wizard]')`);
	index = await submit('another failure during report');
	await settle(index, failure('unexpected', 'not user guidance', 'newest technical details'));
	assert.equal(
		await run(`document.querySelectorAll('${notice}').length`),
		0,
		'offers pause during reporting'
	);
	await wait(`!document.querySelector('[data-inbound-action="next"]')?.disabled`);
	for (let step = 0; step < 10; step++) {
		if (await run(`!!document.querySelector('[data-inbound-action="send"]')`)) break;
		await wait(`document.querySelector('[data-inbound-action="next"]')?.disabled === false`);
		await clickText('[data-inbound-wizard] button', 'Next');
	}
	await run('chatNative.failReport = true');
	await clickText('[data-inbound-wizard] button', 'Send');
	await wait(`!!document.querySelector('[data-inbound-action="retry"]')`);
	await run('chatNative.failReport = false');
	await clickText('[data-inbound-wizard] button', 'Try again');
	await wait(`!!document.querySelector('[data-inbound-action="send"]')`);
	await clickText('[data-inbound-wizard] button', 'Send');
	await wait(`!!document.querySelector('[data-inbound-action="close"]')`);
	const report = await run<{ error: { message: string } }>('chatNative.reports.at(-1)');
	assert.match(report.error.message, /selected technical details/);
	assert.doesNotMatch(report.error.message, /newest technical details|wrong latest native error/);
	const closedAt = await run<number>(
		`(Array.from(document.querySelectorAll('[data-inbound-wizard] button')).find(node => node.textContent.trim() === 'Done').click(), performance.now())`
	);
	await wait(`!!document.querySelector('${notice}')`);
	await type('focus outside notification');
	await wait(`chatNative.pauses.some(event => event[0] === 'resume' && event[1] === 'focus')`);
	await wait(`!document.querySelector('${notice}')`, 15000);
	const expiredAt = await run<number>('performance.now()');
	assert(
		expiredAt - closedAt >= 8000,
		`offers expired ${Math.round(expiredAt - closedAt)}ms after the report closed`
	);

	index = await submit('thrown invocation');
	await type('draft survives exception');
	await run(`chatNative.reject(${index}, 'invocation exploded')`);
	await wait(
		`${form}.getAttribute('aria-busy') === 'false' && !!document.querySelector('${notice}')`
	);
	assert.equal(await run(`${input}.value`), 'draft survives exception');
	assert.equal(await run(`!!document.querySelector('${inline}')`), false);
	assert.equal(
		await run(
			`Array.from(document.querySelectorAll('button')).some(button => button.textContent.trim() === 'Report this')`
		),
		true
	);
	assert.equal(
		await run(
			'chatNative.logs.every(log => log.options?.keyValues?.["inbound.handled"] === "true")'
		),
		true
	);
	assert.equal(
		await run('chatNative.logs.some(log => log.message.includes("report submission failure"))'),
		true
	);
	assert.equal(
		await run('chatNative.logs.some(log => log.message.includes("invocation exploded"))'),
		true
	);
	assert.equal(await run(`document.querySelectorAll('${notice}').length`), 1, 'one card');
	assert.deepEqual(app.errors, [], 'no uncaught browser errors');

	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);
	index = await submit('string rejection');
	await run(
		`chatNative.resolve(${index}, { status: 'error', error: 'invalid args broadcasterId for command send_chat_message: missing required key broadcasterId' })`
	);
	await wait(
		`${form}.getAttribute('aria-busy') === 'false' && !!document.querySelector('${notice}')`
	);
	assert.equal(await run(`!!document.querySelector('${inline}')`), false, 'no inline guidance');
	assert.equal(
		await run(`document.querySelector('${notice}').textContent.includes('#third')`),
		true,
		'card names the channel'
	);
	assert.equal(
		await run(
			'chatNative.logs.some(log => log.message.includes("missing required key broadcasterId"))'
		),
		true
	);
	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);

	assert.equal(await run(`${input}.maxLength`), 500);
	await type('a'.repeat(399));
	assert.equal(await run(`!!document.querySelector('${counter}')`), false, 'counter hidden');
	await type('a'.repeat(400));
	await wait(`document.querySelector('${counter}')?.textContent === '400/500'`);
	assert.equal(
		await run(`document.querySelector('${counter}').classList.contains('text-destructive')`),
		false
	);
	await type('a'.repeat(500));
	await wait(`document.querySelector('${counter}')?.classList.contains('text-destructive')`);
	await type('a'.repeat(496) + ' :ka');
	await wait(`!!document.querySelector('img[alt="Kappa"]')`);
	const keydown = `${input}.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }))`;
	const callsBeforeEmote = await run<number>('chatNative.calls.length');
	await run(keydown);
	await wait(
		`document.querySelector('${inline}')?.textContent.includes('cannot exceed 500 characters')`
	);
	assert.equal(await run(`${input}.value`), 'a'.repeat(496) + ' :ka', 'refused insertion');
	await wait(`!document.querySelector('img[alt="Kappa"]')`);
	await type('a'.repeat(10) + ' :kap');
	await wait(`!!document.querySelector('img[alt="Kappa"]')`);
	await run(keydown);
	await wait(`${input}.value === ${JSON.stringify('a'.repeat(10) + ' Kappa ')}`);
	assert.equal(await run('chatNative.calls.length'), callsBeforeEmote, 'Enter picked, not sent');
	await type('');

	const pressKey = async (key: 'Enter' | 'Tab' | 'Escape', shift = false) => {
		const codes = { Enter: 13, Tab: 9, Escape: 27 };
		const params = {
			key,
			code: key,
			windowsVirtualKeyCode: codes[key],
			nativeVirtualKeyCode: codes[key],
			modifiers: shift ? 8 : 0
		};
		await send('Input.dispatchKeyEvent', {
			...params,
			type: 'keyDown',
			...(key === 'Enter' ? { text: '\r', unmodifiedText: '\r' } : {})
		});
		await send('Input.dispatchKeyEvent', { ...params, type: 'keyUp' });
	};
	const assertSentPayload = async (callIndex: number, message: string) => {
		const call = await run<{
			broadcasterId: string;
			message: string;
			diagnostics: {
				channel_login: string;
				attempt_id: string;
				last_completion: Record<string, unknown> | null;
				key_events: { key: string; shift?: boolean; picker_visible: boolean }[];
			};
		}>(`chatNative.calls[${callIndex}]`);
		assert.equal(call.broadcasterId, 'third-id');
		assert.equal(call.message, message);
		assert.equal(call.diagnostics.channel_login, 'third');
		assert.match(call.diagnostics.attempt_id, /^[0-9a-f-]{36}$/);
		return call;
	};

	const fixtures = ['Twitch', 'BTTV', 'FFZ', '7TV'].map((provider, i) => ({
		provider,
		name: ['Kappa', 'Kapow', 'KapCat', 'KaDance'][i]
	}));
	await run(
		`chatNative.fixtures = ${JSON.stringify(fixtures)}.map((emote, i) => ({ ...chatNative.emote, ...emote, id: String(i) }))`
	);
	for (let target = 0; target < fixtures.length; target++) {
		const name = fixtures[target].name;
		index = await submit(name);
		await assertSentPayload(index, name);
		await settle(index);
		index = await submit(name + ' ');
		const manual = await assertSentPayload(index, name + ' ');
		await settle(index);
		await type(':ka');
		await wait(`!!document.querySelector('img[alt="Kappa"]')`);
		const before = await run<number>('chatNative.calls.length');
		for (let step = 0; step < (target || fixtures.length); step++) await pressKey('Tab');
		await pressKey('Enter');
		assert.equal(await run('chatNative.calls.length'), before, 'first Enter only inserts');
		await pressKey('Enter');
		await wait(`chatNative.calls.length === ${before + 1}`);
		const completed = await assertSentPayload(before, name + ' ');
		assert.equal(completed.message, manual.message);
		assert.deepEqual(completed.diagnostics.last_completion, {
			mode: 'colon',
			method: 'keyboard',
			emote_id: String(target),
			emote_name: name,
			provider: fixtures[target].provider,
			scope: 'Global',
			edited_since_selection: false
		});
		assert.deepEqual(
			completed.diagnostics.key_events.slice(-2).map((event) => [event.key, event.picker_visible]),
			[
				['Enter', true],
				['Enter', false]
			]
		);
		await settle(before);
		assert.equal(await run('chatNative.calls.length'), before + 1, 'one send');
	}

	await type('');
	await run(`${form}.querySelector('button[type="button"]').click()`);
	await wait(`!!document.querySelector('input[placeholder="Search emotes..."]')`);
	await run(`document.querySelector('img[alt="Kappa"]').closest('button').click()`);
	await wait(`${input}.value === 'Kappa '`);
	await run(`${input}.focus()`);
	await pressKey('Enter');
	index = await run<number>('chatNative.calls.length - 1');
	assert.equal(
		(await assertSentPayload(index, 'Kappa ')).diagnostics.last_completion?.mode,
		'picker'
	);
	await settle(index);

	await type(':ka');
	await wait(`!!document.querySelector('img[alt="Kappa"]')`);
	await pressKey('Tab', true);
	await pressKey('Enter');
	await pressKey('Enter');
	index = await run<number>('chatNative.calls.length - 1');
	const backwards = await assertSentPayload(index, 'KaDance ');
	assert(backwards.diagnostics.key_events.some((event) => event.key === 'Tab' && event.shift));
	await settle(index);

	await type(':ka');
	await wait(`!!document.querySelector('img[alt="Kappa"]')`);
	await run(`document.querySelector('img[alt="Kappa"]').closest('button').click()`);
	await wait(`${input}.value === 'Kappa '`);
	await run(`${input}.focus()`);
	await pressKey('Enter');
	index = await run<number>('chatNative.calls.length - 1');
	assert.equal(
		(await assertSentPayload(index, 'Kappa ')).diagnostics.last_completion?.method,
		'mouse'
	);
	await settle(index);

	await type(':ka');
	await wait(`!!document.querySelector('img[alt="Kappa"]')`);
	await pressKey('Enter');
	await type('Kappa \u200b🥳');
	await pressKey('Enter');
	index = await run<number>('chatNative.calls.length - 1');
	const failedCall = await assertSentPayload(index, 'Kappa \u200b🥳');
	assert.equal(failedCall.diagnostics.last_completion?.edited_since_selection, true);
	await type('next draft while pending');
	await settle(
		index,
		failure('rejected', 'Your message could not be sent, please try again later.', 'MsgRejected')
	);
	const failedRecord = await run<{
		level: string;
		record: {
			message: string;
			composer: { last_completion: { emote_name: string } };
			unusual_codepoints: { codepoint: string }[];
		};
	}>(
		`chatNative.diagnostics.find(entry => entry.event === 'chat.send.frontend_failure' && entry.record.attempt_id === ${JSON.stringify(failedCall.diagnostics.attempt_id)})`
	);
	assert.equal(failedRecord.level, 'warn');
	assert.equal(failedRecord.record.message, failedCall.message);
	assert.equal(failedRecord.record.composer.last_completion.emote_name, 'Kappa');
	assert(failedRecord.record.unusual_codepoints.some((point) => point.codepoint === 'U+200B'));
	assert.equal(await run(`${input}.value`), 'next draft while pending');
	assert.equal(await run(`document.querySelectorAll('${notice}').length`), 0);
	assert(
		await run(
			`chatNative.logs.some(entry => entry.message.includes(${JSON.stringify(failedCall.diagnostics.attempt_id)}))`
		)
	);
	index = await submit('next independent draft');
	assert.equal(
		(await run<{ diagnostics: { last_completion: unknown } }>(`chatNative.calls[${index}]`))
			.diagnostics.last_completion,
		null
	);
	await settle(index);

	await type(':ka');
	await wait(`!!document.querySelector('img[alt="Kappa"]')`);
	await pressKey('Enter');
	await type('');
	index = await submit('cleared draft has no completion');
	assert.equal(
		(await run<{ diagnostics: { last_completion: unknown } }>(`chatNative.calls[${index}]`))
			.diagnostics.last_completion,
		null
	);
	await settle(index);

	await type('a'.repeat(496) + ' :ka');
	await wait(`!!document.querySelector('img[alt="Kappa"]')`);
	await pressKey('Enter');
	await pressKey('Enter');
	index = await run<number>('chatNative.calls.length - 1');
	const refused = await assertSentPayload(index, 'a'.repeat(496) + ' :ka');
	assert.equal(refused.diagnostics.last_completion, null, 'refused insertion is not a completion');
	await settle(index);

	const sentIds = await run<string[]>('chatNative.calls.map(call => call.diagnostics.attempt_id)');
	assert.equal(new Set(sentIds).size, sentIds.length);
	const successRecords = await run<{ record: Record<string, unknown> }[]>(
		"chatNative.diagnostics.filter(entry => entry.level === 'info')"
	);
	assert(
		successRecords.every((entry) => !('message' in entry.record) && !('composer' in entry.record))
	);
	const invokeFailure = await run<{ record: { attempt_id: string } }>(
		"chatNative.diagnostics.find(entry => entry.event === 'chat.send.frontend_failure' && entry.record.stage === 'invoke')"
	);
	assert(invokeFailure);
	assert(
		await run(
			`chatNative.logs.some(entry => entry.message.includes(${JSON.stringify(invokeFailure.record.attempt_id)}))`
		)
	);
	await run('delete chatNative.fixtures');

	await run("chatNative.navigate('notice-preview')");
	await wait("chatNative.joins.at(-1) === 'notice-preview'");
	await send('Emulation.setEmulatedMedia', {
		features: [{ name: 'prefers-reduced-motion', value: 'no-preference' }]
	});
	await run("chatNative.roomError('Message cannot be empty')");
	await wait(
		`document.querySelector('${inline}')?.getAnimations().some(animation => animation.effect.getTiming().duration === 250)`
	);
	await wait(`!document.querySelector('${inline}')`, 8000);
	await send('Emulation.setEmulatedMedia', {
		features: [{ name: 'prefers-reduced-motion', value: 'reduce' }]
	});
	await run("chatNative.roomError('Message cannot be empty')");
	await wait(`!!document.querySelector('${inline}')`);
	assert.equal(
		await run(
			`document.querySelector('${inline}').getAnimations().filter(animation => animation.effect.getTiming().duration > 0).length`
		),
		0
	);

	const inspectNotice = `(() => {
		const notice = document.querySelector('[data-chat-input-error]');
		const message = notice.querySelector('span');
		const icon = notice.querySelector('svg');
		const input = document.querySelector('[aria-label="Chat message"]');
		const rect = notice.getBoundingClientRect();
		const textRect = message.getBoundingClientRect();
		const inputRect = input.getBoundingClientRect();
		const style = getComputedStyle(notice);
		const context = document.createElement('canvas').getContext('2d');
		const luminance = (color) => {
			context.clearRect(0, 0, 1, 1);
			context.fillStyle = color;
			context.fillRect(0, 0, 1, 1);
			const rgb = [...context.getImageData(0, 0, 1, 1).data].slice(0, 3).map((channel) => {
				const value = channel / 255;
				return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
			});
			return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
		};
		const foreground = luminance(style.color);
		const background = luminance(style.backgroundColor);
		return {
			text: message.textContent,
			role: notice.getAttribute('role'),
			iconHidden: icon.getAttribute('aria-hidden'),
			focused: document.activeElement === input,
			contrast: (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05),
			lines: textRect.height / parseFloat(style.lineHeight),
			fits:
				textRect.right <= rect.right &&
				textRect.left >= rect.left &&
				notice.scrollWidth <= notice.clientWidth &&
				rect.right <= innerWidth &&
				inputRect.bottom <= innerHeight &&
				inputRect.top >= rect.bottom - 1 &&
				document.documentElement.scrollWidth <= innerWidth,
			iconAligned:
				Math.abs(icon.getBoundingClientRect().top - textRect.top - parseFloat(style.lineHeight) / 10) < 1
		};
	})()`;
	const messages = [
		'Message cannot be empty',
		'Your previous message is still sending.\nPlease wait before submitting another.',
		'Connection failed: ' + 'x'.repeat(100)
	];
	for (const theme of ['light', 'dark']) {
		for (const width of [900, 320]) {
			for (const fontSize of [16, 32]) {
				await app.viewport(width, 600);
				await run(
					`document.documentElement.classList.toggle('dark', ${theme === 'dark'}); document.documentElement.style.fontSize = '${fontSize}px'`
				);
				for (const [messageIndex, message] of messages.entries()) {
					await type('draft stays focused');
					await run(`chatNative.roomError(${JSON.stringify(message)})`);
					await wait(
						`document.querySelector('${inline}')?.textContent.includes(${JSON.stringify(message)})`
					);
					const state = await run<{
						text: string;
						role: string;
						iconHidden: string;
						focused: boolean;
						contrast: number;
						lines: number;
						fits: boolean;
						iconAligned: boolean;
					}>(inspectNotice);
					const label = `${theme} ${width}px font ${fontSize}px message ${messageIndex}`;
					assert.equal(state.text, message, label);
					assert.equal(state.role, 'alert', label);
					assert.equal(state.iconHidden, 'true', label);
					assert(
						state.focused && state.fits && state.iconAligned,
						`${label}: ${JSON.stringify(state)}`
					);
					assert(state.contrast >= 4.5, `${label}: contrast ${state.contrast}`);
					if (messageIndex === 1 || (messageIndex === 2 && (width === 320 || fontSize === 32))) {
						assert(state.lines > 1, `${label}: message did not wrap`);
					}
				}
			}
		}
	}
	await app.viewport(1200, 800);
	await run(
		"document.documentElement.style.fontSize = ''; document.documentElement.classList.remove('dark')"
	);
	await send('Emulation.setEmulatedMedia', { features: [] });
	await type(':ka');
	await wait('!!document.querySelector(\'img[alt="Kappa"]\')');
	assert(
		await run(
			`document.querySelector('img[alt="Kappa"]').getBoundingClientRect().bottom <= ${input}.getBoundingClientRect().top`
		),
		'picker remains above the input while an error is visible'
	);
	await run(keydown);
	await wait(`${input}.value === 'Kappa '`);

	await run(`chatNative.logError('native failure 42')`);
	await wait(
		`document.querySelector('${notice}')?.textContent.includes('Error detected') && document.querySelector('${notice}').textContent.includes('native failure 42')`
	);
	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);
	await run(`setTimeout(() => { throw new Error('window boom'); }, 0)`);
	await wait(`document.querySelector('${notice}')?.textContent.includes('window boom')`);
	assert.equal(
		await run(
			'chatNative.logs.some(log => log.message.includes("window boom") && log.options?.keyValues?.["inbound.handled"] === "true")'
		),
		true,
		'window error logged as handled'
	);
	await clickText(`${notice} button`, 'Dismiss');
	await wait(`!document.querySelector('${notice}')`);
	await run('chatNative.updateSettings({ diagnostics: { capture_window_errors: false } })');
	await wait('chatNative.settings().diagnostics.capture_window_errors === false');
	await run(`setTimeout(() => { throw new Error('window silent'); }, 0)`);
	await run(`chatNative.logError('native after toggle')`);
	await wait(`document.querySelector('${notice}')?.textContent.includes('native after toggle')`);
	assert.equal(await run(`document.body.textContent.includes('window silent')`), false);
	assert.equal(await run(`document.querySelectorAll('${notice}').length`), 1);
	// Clear expected thrown window errors before fixture teardown checks app.errors
	const unexpected = app.errors.filter((text) => !/window (boom|silent)/.test(text));
	app.errors.length = 0;
	app.errors.push(...unexpected);
});
