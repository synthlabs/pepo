import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { capturedBaseURL, test, waitForExpression } from '../support/browser/fixtures';

interface RowSnapshot {
	index: string | undefined;
	background: string;
	shade: 'primary' | 'secondary' | null;
}

const checkHistoricalTranslationsSource = `async function checkHistoricalTranslations() {
	const native = window.chatNative;
	const viewport = document.querySelector('[aria-label="Chat messages"]');
	const rows = () => [
		...viewport.querySelectorAll('[data-chat-message-index]:not([data-chat-system-message])')
	];
	const visibleRows = () => {
		const box = viewport.getBoundingClientRect();
		return rows().filter((row) => {
			const rect = row.getBoundingClientRect();
			return rect.bottom > box.top && rect.top < box.bottom;
		});
	};
	const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
	const settle = async () => {
		for (let i = 0; i < 2; i++) await new Promise(requestAnimationFrame);
		await wait(0);
		for (let i = 0; i < 2; i++) await new Promise(requestAnimationFrame);
	};
	const require = (condition, message) => {
		if (!condition) throw new Error(message);
	};
	const gap = () => Math.abs(viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop);
	const translated = (row) => !!row.querySelector('[data-chat-translation]');
	await document.fonts.ready;
	await settle();
	require(rows().length === 500 && gap() < 1, 'Restored history did not open at the bottom');
	const protectedRows = visibleRows();
	const baseline = protectedRows.map((row) => row.firstElementChild.getBoundingClientRect().top);
	const above = rows()
		.filter((row) => row.getBoundingClientRect().bottom < viewport.getBoundingClientRect().top)
		.slice(-40);
	let maxMovement = 0;
	let maxGap = 0;
	let samples = 0;
	let sampling = true;
	const sample = () => {
		maxMovement = Math.max(
			maxMovement,
			...protectedRows.map((row, i) =>
				Math.abs(row.firstElementChild.getBoundingClientRect().top - baseline[i])
			)
		);
		maxGap = Math.max(maxGap, gap());
		samples++;
	};
	const sampleFrame = () =>
		requestAnimationFrame(() => {
			setTimeout(() => {
				if (sampling) sample();
			}, 0);
			if (sampling) sampleFrame();
		});
	sampleFrame();
	try {
		for (const row of [...protectedRows, ...above]) {
			const id = row.dataset.chatMessageIndex;
			native.translate(
				'backlog',
				('Translated history ' + id + ' with text that wraps at narrow widths. ').repeat(3),
				id
			);
			await wait(12);
		}
		await settle();
		require(protectedRows.every((row) => !translated(row)), 'A visible historical translation was revealed');
		require(above.every(translated), 'Offscreen historical translations were not revealed');
		native.reset();
		await settle();
		require(protectedRows.every((row) => !translated(row)), 'A feed reset revealed protected translations');
		sample();
	} finally {
		sampling = false;
	}
	require(maxMovement < 1 && maxGap < 1, 'Historical catch-up moved visible originals: ' + JSON.stringify({ maxMovement, maxGap }));
	const protectedRow = protectedRows.at(-1);
	const protectedId = protectedRow.dataset.chatMessageIndex;
	viewport.dispatchEvent(new WheelEvent('wheel', { deltaY: -500, bubbles: true }));
	viewport.scrollTop -= viewport.clientHeight * 2;
	native.translate('backlog', 'translated-during-manual-scroll', protectedId);
	await wait(80);
	require(!translated(protectedRow), 'Historical translations changed during manual scrolling');
	const anchor = visibleRows()[0];
	const anchorTop = anchor.getBoundingClientRect().top;
	await wait(300);
	await settle();
	require(translated(protectedRow), 'A deferred translation was not revealed after leaving view');
	require(Math.abs(anchor.getBoundingClientRect().top - anchorTop) < 1, 'Offscreen reveal moved the paused anchor');
	const scrollingPast = visibleRows()[0];
	const replacement = 'paused-above-translation '.repeat(60).trim();
	native.translate('backlog', replacement, scrollingPast.dataset.chatMessageIndex);
	await settle();
	require(!scrollingPast.textContent.includes(replacement), 'A visible historical replacement was revealed');
	viewport.dispatchEvent(new WheelEvent('wheel', { deltaY: 500, bubbles: true }));
	viewport.scrollTop += viewport.clientHeight * 0.8;
	await wait(80);
	const pausedAnchor = visibleRows()[0];
	const pausedTop = pausedAnchor.getBoundingClientRect().top;
	let pausedMovement = 0;
	let pausedSampling = true;
	const samplePausedFrame = () =>
		requestAnimationFrame(() => {
			setTimeout(() => {
				if (pausedSampling)
					pausedMovement = Math.max(
						pausedMovement,
						Math.abs(pausedAnchor.getBoundingClientRect().top - pausedTop)
					);
			}, 0);
			if (pausedSampling) samplePausedFrame();
		});
	samplePausedFrame();
	await wait(350);
	await settle();
	pausedSampling = false;
	require(scrollingPast.textContent.includes(replacement), 'A translation above the paused viewport was not revealed');
	require(pausedMovement < 1, 'A translation above the paused viewport moved its anchor: ' + pausedMovement);
	const jump = document.querySelector('button[aria-label^="Jump to"]');
	require(!!jump, 'Offscreen reveal unexpectedly resumed chat');
	jump.click();
	await settle();
	require(translated(protectedRow) && gap() < 1, 'Scrolling back did not show the completed translation');
	native.push('backlog', 'live-after-backlog');
	await settle();
	native.translate('backlog', 'live-translation-after-backlog');
	await wait(50);
	await settle();
	require(rows().at(-1).textContent.includes('live-translation-after-backlog'), 'Live translation was deferred');
	require(gap() < 1, 'Live translation lost the bottom');
	const lastHistorical = rows().at(-2);
	const revisitId = lastHistorical.dataset.chatMessageIndex;
	native.translate('backlog', 'completed-before-revisit', revisitId);
	await settle();
	require(!lastHistorical.textContent.includes('completed-before-revisit'), 'A visible replacement was revealed');
	native.translate('backlog', 'completed-before-revisit', revisitId);
	native.go(null);
	await settle();
	native.go('backlog');
	await settle();
	const revisited = document.querySelector('[data-chat-message-index="' + revisitId + '"]');
	require(revisited?.textContent.includes('completed-before-revisit'), 'Completed translations were lost on navigation');
	return { maxMovement, maxGap, pausedMovement, samples, visibleProtected: protectedRows.length, offscreenTranslated: above.length };
}`;

const checkSystemMessagesSource = `async function checkSystemMessages() {
	const native = window.chatNative;
	const wait = (ms = 180) => new Promise((resolve) => setTimeout(resolve, ms));
	const flush = async () => {
		await wait(0);
		for (let i = 0; i < 2; i++) await new Promise(requestAnimationFrame);
	};
	const waitFor = async (predicate, timeout = 200) => {
		const deadline = performance.now() + timeout;
		while (!predicate() && performance.now() < deadline) {
			await new Promise(requestAnimationFrame);
		}
		return predicate();
	};
	const require = (condition, message) => { if (!condition) throw new Error(message); };
	await native.go(null);
	await flush();
	Object.assign(native.settings.obj.chat, {
		message_limit: 1000, alternate_backgrounds: true, show_timestamps: true, center_system_messages: false
	});
	await native.settings.sync();
	native.seed('notices', 80);
	await native.go('notices');
	await flush();
	const viewport = document.querySelector('[aria-label="Chat messages"]');
	const rows = () => [...viewport.querySelectorAll('[data-chat-message-index]')];
	const notices = () => [...viewport.querySelectorAll('[data-chat-system-message]')];
	const gap = () => Math.abs(viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop);
	const previous = rows().at(-1);
	const previousBackground = getComputedStyle(previous).backgroundColor;
	for (const icon of ['info', 'shield', 'clock', 'success', 'warning', 'error']) {
		native.system('notices', { icon, segments: [{ type: 'text', text: 'Synthetic ' + icon + ' notice' }] });
	}
	native.push('notices', 'chat-after-notices');
	await flush();
	require(notices().length === 6, 'Missing system icons');
	require(notices().every((row) => row.querySelector('svg[aria-hidden="true"]')), 'System icon is not decorative');
	require(!native.settings.obj.chat.show_system_message_background &&
		notices().every((row) => getComputedStyle(row).backgroundColor === 'rgba(0, 0, 0, 0)'),
		'System backgrounds are not off by default');
	native.settings.obj.chat.show_system_message_background = true;
	await native.settings.sync();
	await flush();
	require(getComputedStyle(rows().at(-1)).backgroundColor !== previousBackground, 'Notices consumed ordinary row alternation');
	const backgrounds = new Set([previousBackground, getComputedStyle(rows().at(-1)).backgroundColor]);
	require(!backgrounds.has(getComputedStyle(notices()[0]).backgroundColor), 'System background matches a chat shade');
	const enabledBackground = getComputedStyle(notices()[0]).backgroundColor;
	native.system('notices', {
		icon: 'shield',
		segments: [
			{ type: 'user', user_id: 'viewer', login: 'viewer', display_name: 'Viewer' },
			{ type: 'text', text: ' was timed out. ' },
			{ type: 'emphasis', text: 'Synthetic wrapping notice. ' },
			{ type: 'text', text: 'LongNotice'.repeat(45) },
			{ type: 'link', text: ' Details', url: 'https://example.com/notice' }
		]
	});
	await flush();
	const notice = notices().at(-1);
	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = 1;
	const context = canvas.getContext('2d', { willReadFrequently: true });
	const luminance = (row, value) => {
		context.clearRect(0, 0, 1, 1);
		context.fillStyle = '#fff';
		context.fillRect(0, 0, 1, 1);
		const ancestors = [];
		for (let element = row; element; element = element.parentElement) ancestors.unshift(element);
		for (const element of ancestors) {
			context.fillStyle = getComputedStyle(element).backgroundColor;
			context.fillRect(0, 0, 1, 1);
		}
		context.fillStyle = value;
		context.fillRect(0, 0, 1, 1);
		return [...context.getImageData(0, 0, 1, 1).data]
			.slice(0, 3)
			.map((v) => v / 255)
			.map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4))
			.reduce((sum, v, i) => sum + v * [0.2126, 0.7152, 0.0722][i], 0);
	};
	const link = notice.querySelector('a');
	link.focus();
	let contrast = Infinity;
	let focusContrast = Infinity;
	const checkNoticeContrast = () => {
		const style = getComputedStyle(notice);
		const background = luminance(notice, 'transparent');
		const foreground = luminance(notice, style.color);
		const ratio = (value) => (Math.max(value, background) + 0.05) / (Math.min(value, background) + 0.05);
		const textContrast = ratio(foreground);
		contrast = Math.min(contrast, textContrast);
		require(textContrast >= 4.5, 'System text contrast below 4.5');
		require(Math.abs(foreground - background) < Math.abs(luminance(notice, getComputedStyle(document.body).color) - background), 'System text is not muted');
		const focusStyle = getComputedStyle(link);
		const outlineContrast = ratio(luminance(notice, focusStyle.outlineColor));
		focusContrast = Math.min(focusContrast, outlineContrast);
		require(link.matches(':focus-visible') && parseFloat(focusStyle.outlineWidth) >= 2 && outlineContrast >= 3, 'System link keyboard focus is not sufficiently visible');
	};
	let maxIconCenterOffset = 0;
	const checkNoticeLayout = () => {
		const settings = native.settings.obj.chat;
		const divider = document.querySelector('[data-chat-history-divider]');
		const dividerLine = divider.querySelector('span');
		for (const row of notices()) {
			const rowStyle = getComputedStyle(row);
			const timestamp = row.querySelector('span.text-xs');
			require(!!timestamp === (settings.show_timestamps && !settings.center_system_messages), 'System timestamp ignored visibility setting');
			require(rowStyle.backgroundColor === (settings.show_system_message_background ? enabledBackground : 'rgba(0, 0, 0, 0)'), 'System background ignored its setting');
			const content = row.querySelector('[data-chat-system-message-content]').getBoundingClientRect();
			const lines = [...row.querySelectorAll('[data-chat-system-message-line]')];
			require(lines.length === (settings.center_system_messages ? 2 : 0), 'System divider lines ignored alignment');
			if (settings.center_system_messages) {
				const rects = lines.map((line) => line.getBoundingClientRect());
				require(Math.abs(rects[0].width - rects[1].width) <= 0.5, 'System divider lines are not equal widths');
				require(rowStyle.columnGap === getComputedStyle(divider).columnGap, 'System divider spacing differs from the history divider');
				for (const [i, line] of lines.entries()) {
					const rect = rects[i];
					require(line.getAttribute('aria-hidden') === 'true', 'System divider is not decorative');
					require(rect.width >= 1.5 * parseFloat(getComputedStyle(document.documentElement).fontSize) && rect.height === 1, 'System divider line dimensions are incorrect');
					require(getComputedStyle(line).backgroundColor === getComputedStyle(dividerLine).backgroundColor, 'System divider line color differs from the history divider');
					require(Math.abs(rect.top + rect.height / 2 - (content.top + content.height / 2)) <= 0.5, 'System divider lines are not vertically centered beside the content');
				}
			}
			for (const element of row.querySelectorAll('svg, span.text-xs, strong, a')) {
				require(getComputedStyle(element).color === rowStyle.color, 'System icon, timestamp, or text did not inherit the muted color');
			}
			const icon = row.querySelector('svg');
			if (!icon) continue;
			const slot = icon.parentElement.getBoundingClientRect();
			const rect = icon.getBoundingClientRect();
			const offset = Math.max(
				Math.abs(rect.left + rect.width / 2 - (slot.left + slot.width / 2)),
				Math.abs(rect.top + rect.height / 2 - (slot.top + slot.height / 2))
			);
			maxIconCenterOffset = Math.max(maxIconCenterOffset, offset);
			require(offset <= 0.5, 'System icon is not centered within its slot');
			require(Math.abs(slot.height - parseFloat(rowStyle.lineHeight)) <= 0.5 &&
				Math.abs(slot.top - row.getBoundingClientRect().top - parseFloat(rowStyle.paddingTop)) <= 0.5,
				'System icon slot is not aligned with the first text line');
		}
	};
	for (const fontSize of [16, 32]) {
		document.documentElement.style.fontSize = fontSize + 'px';
		for (const centered of [false, true]) {
			for (const timestamps of [true, false]) {
				for (const background of [false, true]) {
					for (const striped of [true, false]) {
						Object.assign(native.settings.obj.chat, {
							center_system_messages: centered,
							show_timestamps: timestamps,
							show_system_message_background: background,
							alternate_backgrounds: striped
						});
						await native.settings.sync();
						await flush();
						await waitFor(() => gap() < 1, 200);
						checkNoticeLayout();
						checkNoticeContrast();
						require(getComputedStyle(notice).textAlign === (centered ? 'center' : 'start'), 'System alignment did not update');
						require(notices().every((row) => row.scrollWidth <= row.clientWidth + 1), 'System content overflows');
						require(getComputedStyle(previous).backgroundColor === (striped ? previousBackground : 'rgba(0, 0, 0, 0)'), 'System settings changed ordinary message backgrounds');
						require(gap() < 1, 'Notice reflow lost pinned bottom');
					}
				}
			}
		}
	}
	document.documentElement.style.fontSize = '16px';
	native.settings.obj.chat.alternate_backgrounds = false;
	native.settings.obj.chat.show_timestamps = false;
	await native.settings.sync();
	await flush();
	require(getComputedStyle(notice).backgroundColor === enabledBackground, 'Disabling striping removed the system background');
	require(!notice.querySelector('span.text-xs'), 'System timestamp ignored visibility setting');
	native.settings.obj.chat.alternate_backgrounds = true;
	native.settings.obj.chat.show_timestamps = true;
	await native.settings.sync();
	await flush();
	viewport.dispatchEvent(new WheelEvent('wheel', { deltaY: -100, bubbles: true }));
	viewport.scrollTop = viewport.scrollHeight / 2;
	viewport.dispatchEvent(new Event('scroll'));
	await wait(280);
	const box = viewport.getBoundingClientRect();
	const anchor = rows().find((row) => row.getBoundingClientRect().top >= box.top);
	const top = anchor.getBoundingClientRect().top;
	native.system('notices', { segments: [{ type: 'text', text: 'Synthetic paused notice' }] });
	native.push('notices', 'paused-chat');
	await wait(280);
	require(Math.abs(anchor.getBoundingClientRect().top - top) < 1, 'Mixed arrivals moved paused anchor');
	const paused = [...document.querySelectorAll('button')].find((button) => button.textContent.includes('chat paused'));
	require(paused?.textContent.includes('2 new messages'), 'System notice not counted as unread');
	paused.click();
	await flush();
	require(gap() < 1, 'Jump did not reach mixed timeline bottom');
	native.reset();
	await flush();
	require(notices().length === 8, 'Reset lost notices');
	await native.go(null);
	await flush();
	await native.go('notices');
	await flush();
	require(document.querySelectorAll('[data-chat-system-message]').length === 8, 'Channel return lost notices');
	const originalFrame = window.requestAnimationFrame;
	let callbacks = 0;
	window.requestAnimationFrame = (callback) =>
		originalFrame((time) => { callbacks++; callback(time); });
	try {
		native.system('notices', { segments: [{ type: 'text', text: 'Synthetic final notice' }] });
		native.push('notices', 'final-chat');
		await wait(300);
		const settled = callbacks;
		await wait(180);
		require(callbacks === settled, 'Mixed arrivals left an animation frame loop running');
	} finally {
		window.requestAnimationFrame = originalFrame;
	}
	return { contrast, focusContrast, maxIconCenterOffset, count: 9, idleFrames: callbacks };
}`;

const checkUsernamesSource = `async function checkUsernames() {
	const native = window.chatNative;
	const flush = async () => {
		await new Promise((resolve) => setTimeout(resolve, 0));
		for (let i = 0; i < 2; i++) await new Promise(requestAnimationFrame);
	};
	const waitFor = async (predicate, timeout = 1000) => {
		const deadline = performance.now() + timeout;
		while (performance.now() < deadline) {
			const value = predicate();
			if (value) return value;
			await new Promise(requestAnimationFrame);
		}
		return predicate();
	};
	const settlePause = async () => {
		await new Promise((resolve) => setTimeout(resolve, 350));
		for (let i = 0; i < 3; i++) await new Promise(requestAnimationFrame);
	};
	const require = (condition, message) => { if (!condition) throw new Error(message); };
	Object.assign(native.settings.obj.chat, {
		message_limit: 1000,
		show_login_for_different_display_name: true,
		use_display_names_in_mentions: false,
		bold_mentions: false,
		center_system_messages: false,
		show_system_message_background: false
	});
	native.settings.obj.translation.layout = 'message_text';
	await native.settings.sync();
	native.users.clear();
	native.seed('usernames', 0);
	for (let i = 0; i < 100; i++)
		native.custom('usernames', {
			chatter_user_id: 'writer',
			chatter_user_login: 'writer',
			chatter_user_name: '작성자',
			color: '#4488bb',
			text: 'Hello @MaYa, message ' + i,
			fragments: [
				{ Text: { index: 0, text: 'Hello ' } },
				{ Mention: { index: 1, text: '@MaYa', login: 'maya', user_id: null, display_name: null } },
				{ Text: { index: 2, text: ', message ' + i + ' with some wrapping text.' } }
			]
		});
	await native.go('usernames');
	await flush();
	let viewport = document.querySelector('[aria-label="Chat messages"]');
	const mentions = () => [...viewport.querySelectorAll('[data-chat-username="mention"]')];
	const authors = () => [...viewport.querySelectorAll('[data-chat-username="author"]')];
	const rows = () => [...viewport.querySelectorAll('[data-chat-message-index]')];
	const gap = () => Math.abs(viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop);
	require(mentions().length === 100, 'Mention buttons missing from retained history');
	require(authors().every((button) => button.textContent === '작성자 (writer)'), 'Author name formatting failed');
	require(mentions().every((button) => button.textContent === '@MaYa'), 'Typed mention text changed');
	require(getComputedStyle(mentions()[0]).fontWeight === '400', 'Mentions should start at body weight');
	const count = rows().length;
	const beforeUnread = document.querySelector('[aria-label^="Jump to"]')?.textContent;
	native.observe({ user_id: 'maya-id', login: 'maya', display_name: '마야', color: '#9c63dc' });
	await flush();
	require(rows().length === count, 'Background metadata added a timeline row');
	require(document.querySelector('[aria-label^="Jump to"]')?.textContent === beforeUnread, 'Metadata changed unread count');
	require(mentions().every((button) => button.textContent === '@MaYa' && getComputedStyle(button).color === 'rgb(156, 99, 220)'), 'Observed colors did not enrich typed mentions');
	require(authors().every((button) => getComputedStyle(button).color === 'rgb(68, 136, 187)'), 'Metadata changed original author colors');
	Object.assign(native.settings.obj.chat, { use_display_names_in_mentions: true, bold_mentions: true });
	await native.settings.sync();
	await flush();
	require(mentions().every((button) => button.textContent === '@마야 (maya)' && getComputedStyle(button).fontWeight === '700'), 'Mention settings did not apply');
	require(gap() < 1, 'Name formatting lost bottom pinning');
	viewport.dispatchEvent(new WheelEvent('wheel', { deltaY: -500, bubbles: true }));
	viewport.scrollTop = viewport.scrollHeight / 2;
	await settlePause();
	const box = viewport.getBoundingClientRect();
	const anchor = rows().find((row) => row.getBoundingClientRect().bottom > box.top);
	const top = anchor.getBoundingClientRect().top;
	native.observe({ user_id: 'maya-id', login: 'maya', display_name: '메타데이터로변경된표시이름', color: '#9c63dc' });
	await flush();
	require(Math.abs(anchor.getBoundingClientRect().top - top) < 1, 'Metadata reflow moved the paused anchor');
	Object.assign(native.settings.obj.chat, { show_login_for_different_display_name: false, bold_mentions: false });
	await native.settings.sync();
	await flush();
	require(authors()[0].textContent === '작성자', 'Author setting did not update');
	require(mentions()[0].textContent === '@메타데이터로변경된표시이름', 'Mention setting did not follow name format');
	native.system('usernames', {
		segments: [
			{ type: 'user', user_id: 'maya-id', login: 'maya', display_name: 'maya' },
			{ type: 'text', text: ' was mentioned; plain @other remains text.' }
		]
	});
	const system = await waitFor(() => viewport.querySelector('[data-chat-system-message]'));
	require(system, 'System notice did not mount');
	require(system.querySelectorAll('button').length === 1, 'System text was scanned for mentions');
	require(system.querySelector('button').textContent === '메타데이터로변경된표시이름', 'System name did not use cached metadata');
	viewport.scrollTop = viewport.scrollHeight;
	await flush();
	native.custom('usernames', {
		chatter_user_id: 'writer',
		chatter_user_login: 'writer',
		chatter_user_name: '작성자',
		text: 'A live translated message',
		fragments: [{ Text: { index: 0, text: 'A live translated message' } }]
	});
	await waitFor(() => rows().some((row) => row.textContent.includes('A live translated message')));
	native.translate('usernames', 'Plain translated @MaYa');
	await waitFor(() => rows().findLast((row) => row.querySelector('[data-chat-translation]')));
	await new Promise((resolve) => setTimeout(resolve, 60));
	await flush();
	const translatedRow = rows().findLast((row) => row.querySelector('[data-chat-translation]'));
	require(translatedRow.querySelector('[data-chat-translation] button') === null, 'Translation rendered an interactive spacer or mention');
	const author = translatedRow.querySelector('[data-chat-username="author"]');
	const spacer = translatedRow.querySelector('[data-chat-translation] [aria-hidden="true"] .font-bold');
	const authorSpacerWidth = spacer.getBoundingClientRect().width;
	require(Math.abs(author.getBoundingClientRect().width - spacer.getBoundingClientRect().width) < 1, 'Translation author spacer has a different width');
	native.reset();
	await flush();
	require(mentions()[0].textContent === '@메타데이터로변경된표시이름', 'Reset lost cached metadata');
	await native.go(null);
	await flush();
	await native.go('usernames');
	await flush();
	viewport = document.querySelector('[aria-label="Chat messages"]');
	require(mentions().length === 100, 'Channel return lost mentions');
	const lastMention = mentions().at(-1);
	const selection = window.getSelection();
	const range = document.createRange();
	range.selectNodeContents(lastMention);
	selection.removeAllRanges();
	selection.addRange(range);
	require(selection.toString() === lastMention.textContent, 'Username text cannot be selected');
	selection.removeAllRanges();
	lastMention.focus();
	window.usernameActivations = 0;
	document.addEventListener('click', (event) => {
		if (event.target.closest?.('[data-chat-username]')) window.usernameActivations++;
	});
	require(viewport.scrollWidth <= viewport.clientWidth, 'Username rendering overflowed the chat viewport');
	return { mentions: mentions().length, authorSpacerWidth, pausedAnchorStable: true };
}`;

function assertAlternating(rows: RowSnapshot[]) {
	assert(rows.length > 1, 'Expected multiple message rows');
	assert.equal(
		new Set(rows.map((row) => row.background)).size,
		2,
		'Expected two painted backgrounds'
	);
	for (let i = 1; i < rows.length; i++) {
		assert.notEqual(
			rows[i].background,
			rows[i - 1].background,
			'Adjacent messages must have different backgrounds'
		);
	}
}

function assertStable(before: RowSnapshot[], after: RowSnapshot[]) {
	const previous = new Map(before.map((row) => [row.index, row]));
	let shared = 0;
	for (const row of after) {
		const old = previous.get(row.index);
		if (!old) continue;
		shared++;
		assert.equal(row.shade, old.shade, 'Retained messages must keep their shade');
		assert.equal(
			row.background,
			old.background,
			'Retained messages must keep their painted background'
		);
	}
	assert(shared > 0, 'Expected retained messages for stability comparison');
}

for (const width of [1200, 375] as const) {
	for (const theme of ['dark', 'light'] as const) {
		test(`chat engine feeds, system notices, and usernames (${width}px, ${theme})`, async ({
			app
		}) => {
			const { cdp, evaluate, waitFor } = app;
			const send = cdp as (method: string, params?: Record<string, unknown>) => Promise<unknown>;
			const readRows = () =>
				evaluate<
					RowSnapshot[]
				>(`Array.from(document.querySelectorAll('[data-chat-message-index]')).map(row => ({
					index: row.dataset.chatMessageIndex,
					background: getComputedStyle(row).backgroundColor,
					shade: row.classList.contains('bg-content-primary') ? 'primary' : row.classList.contains('bg-content-secondary') ? 'secondary' : null
				}))`);

			await app.viewport(width, 800);
			await app.page.goto('/app/chat/rooma?harness=chat-engine', { waitUntil: 'domcontentloaded' });
			await waitFor("document.querySelectorAll('[data-chat-message-index]').length === 3");
			await evaluate(
				`chatNative.settings.obj.appearance.theme = '${theme}'; chatNative.settings.sync()`
			);
			await waitFor(`document.documentElement.classList.contains('${theme}')`);

			const openingRows = await readRows();
			assertAlternating(openingRows);
			assert.equal(openingRows[0].shade, 'primary');
			assert.equal(
				await evaluate("document.querySelectorAll('[data-chat-history-divider]').length"),
				1
			);

			await evaluate(
				"chatNative.push('roomb','hidden-background'); chatNative.push('rooma','visible-new')"
			);
			await waitFor("document.querySelectorAll('[data-chat-message-index]').length === 4");
			assert.equal(
				await evaluate("document.body.textContent.includes('hidden-background')"),
				false
			);
			assert.equal(
				await evaluate(
					"document.querySelector('[data-chat-history-divider]').nextElementSibling.textContent.includes('visible-new')"
				),
				true
			);
			await waitFor('chatNative.acknowledgements.length > 0');
			assertStable(openingRows, await readRows());
			assertAlternating(await readRows());

			const beforeTranslation = await readRows();
			await evaluate(
				"chatNative.translate('rooma','translated-live'); chatNative.push('rooma','after-translation')"
			);
			await waitFor(
				"document.querySelectorAll('[data-chat-message-index]').length === 5 && document.body.textContent.includes('translated-live')"
			);
			for (const layout of ['language_tag', 'message_text', 'timestamp_end', 'connector']) {
				await evaluate(
					`chatNative.settings.obj.translation.layout = '${layout}'; chatNative.settings.sync()`
				);
				await waitFor("document.body.textContent.includes('translated-live')");
				assert.equal(await evaluate("document.body.textContent.includes('visible-new')"), true);
				assert.equal(
					await evaluate("!!document.querySelector('[data-translation-connector]')"),
					layout === 'connector'
				);
				assertStable(beforeTranslation, await readRows());
			}

			const liveRows = await readRows();
			assertStable(beforeTranslation, liveRows);
			assertAlternating(liveRows);

			await evaluate(
				'chatNative.settings.obj.chat.show_history_divider = false; chatNative.settings.sync()'
			);
			await waitFor("document.querySelectorAll('[data-chat-history-divider]').length === 0");
			await evaluate(
				'chatNative.settings.obj.chat.show_history_divider = true; chatNative.settings.sync(); chatNative.reset()'
			);
			await waitFor("document.querySelectorAll('[data-chat-history-divider]').length === 1");
			assert.equal(
				await evaluate(
					"document.querySelector('[data-chat-history-divider]').nextElementSibling.textContent.includes('visible-new')"
				),
				true
			);
			assertStable(liveRows, await readRows());
			assertAlternating(await readRows());

			await evaluate(
				'chatNative.settings.obj.chat.alternate_backgrounds = false; chatNative.settings.sync()'
			);
			await waitFor(
				"Array.from(document.querySelectorAll('[data-chat-message-index]')).every(row => !row.classList.contains('bg-content-primary') && !row.classList.contains('bg-content-secondary'))"
			);
			await evaluate(
				'chatNative.settings.obj.chat.alternate_backgrounds = true; chatNative.settings.sync()'
			);
			await waitFor("!!document.querySelector('[data-chat-message-index].bg-content-primary')");
			assertStable(liveRows, await readRows());
			assertAlternating(await readRows());

			await evaluate("chatNative.go('roomb')");
			await waitFor("document.body.textContent.includes('hidden-background')");
			assert.equal(await evaluate('chatNative.feeds.size'), 1);
			assert.equal(await evaluate('chatNative.metadataRequests'), 0);

			await evaluate('chatNative.go(null)');
			await waitFor("!!document.querySelector('[data-settings-page]')");
			assert.equal(await evaluate('chatNative.feeds.size'), 0);
			await evaluate(`document.querySelector('[aria-controls="settings-more-chat"]').click()`);
			await waitFor("document.body.textContent.includes('2 joined')");

			await evaluate("chatNative.push('rooma','while-settings'); chatNative.go('rooma')");
			await waitFor("document.body.textContent.includes('while-settings')");
			assert.equal(
				await evaluate("document.querySelector('[data-chat-history-divider]').nextElementSibling"),
				null
			);

			await evaluate("chatNative.delays.roomb = 150; chatNative.go('roomb')");
			await delay(10);
			await evaluate("chatNative.go('rooma')");
			await delay(200);
			assert.equal(await evaluate('chatNative.feeds.size'), 1);
			assert.equal(await evaluate("document.body.textContent.includes('while-settings')"), true);

			let beforeTrim = await readRows();
			assertStable(liveRows, beforeTrim);
			assertAlternating(beforeTrim);
			for (const limit of [5, 3, 1]) {
				await evaluate(
					`chatNative.settings.obj.chat.message_limit = ${limit}; chatNative.settings.obj.chat.message_gc_interval_ms = 10; chatNative.settings.sync()`
				);
				await waitFor(`document.querySelectorAll('[data-chat-message-index]').length === ${limit}`);
				const trimmed = await readRows();
				assertStable(beforeTrim, trimmed);
				if (limit > 1) assertAlternating(trimmed);
				await evaluate('chatNative.reset()');
				await delay(50);
				assert.deepEqual(await readRows(), trimmed);
				await evaluate("chatNative.go('roomb')");
				await waitFor("document.body.textContent.includes('hidden-background')");
				await evaluate("chatNative.go('rooma')");
				await waitFor("document.body.textContent.includes('while-settings')");
				assert.deepEqual(await readRows(), trimmed);
				beforeTrim = trimmed;
			}

			await evaluate(
				"chatNative.translate('roomb','background-translation'); chatNative.push('rooma','limit-one-new')"
			);
			await waitFor(
				"document.querySelectorAll('[data-chat-message-index]').length === 1 && document.body.textContent.includes('limit-one-new')"
			);
			assert.notEqual((await readRows())[0].shade, beforeTrim[0].shade);

			await evaluate(
				"chatNative.resolutionErrors.unknown = 'channel not found'; chatNative.go('unknown')"
			);
			await waitFor("document.body.textContent.includes('channel not found')");
			assert.equal(await evaluate('chatNative.feeds.size'), 0);
			assert.equal(
				await evaluate("document.querySelectorAll('[data-chat-input-error]').length"),
				1
			);

			await evaluate("chatNative.roomFailures.rooma = '403 Forbidden'; chatNative.go('rooma')");
			await waitFor("document.body.textContent.includes('403 Forbidden')");
			assert.equal(await evaluate('chatNative.feeds.size'), 1);
			await evaluate('chatNative.go(null)');
			await waitFor("!!document.querySelector('[data-settings-page]')");
			await evaluate("chatNative.go('rooma')");
			await waitFor("document.body.textContent.includes('403 Forbidden')");
			assert.equal(
				await evaluate("document.querySelectorAll('[data-chat-input-error]').length"),
				1
			);

			await evaluate(`(${checkSystemMessagesSource})()`);
			for (const centered of [false, true]) {
				for (const background of [false, true]) {
					await evaluate(
						`Object.assign(chatNative.settings.obj.chat, { center_system_messages: ${centered}, show_system_message_background: ${background} }); chatNative.settings.sync()`
					);
					await waitFor(
						`getComputedStyle(document.querySelector('[data-chat-system-message]')).textAlign === '${centered ? 'center' : 'start'}' && (getComputedStyle(document.querySelector('[data-chat-system-message]')).backgroundColor !== 'rgba(0, 0, 0, 0)') === ${background}`
					);
				}
			}

			await evaluate(`(${checkUsernamesSource})()`);
			for (const [key, code, keyCode] of [
				['Enter', 'Enter', 13],
				[' ', 'Space', 32]
			] as const) {
				await send('Input.dispatchKeyEvent', {
					type: 'rawKeyDown',
					key,
					code,
					windowsVirtualKeyCode: keyCode
				});
				await send('Input.dispatchKeyEvent', {
					type: 'char',
					text: keyCode === 13 ? '\r' : ' ',
					key,
					code,
					windowsVirtualKeyCode: keyCode
				});
				await send('Input.dispatchKeyEvent', {
					type: 'keyUp',
					key,
					code,
					windowsVirtualKeyCode: keyCode
				});
			}
			assert.equal(await evaluate('window.usernameActivations'), 2);
			assert.equal(await evaluate('!!document.querySelector("[role=dialog]")'), false);
			await evaluate('document.activeElement.click()');
			assert.equal(await evaluate('window.usernameActivations'), 3);

			const mentionCards = await evaluate<
				{ user_id: string; login: string; channel_login: string }[]
			>('window.chatNative.cardRequests');
			assert.equal(
				mentionCards.length,
				3,
				'mouse, Enter and Space each open the mentioned user card'
			);
			assert(
				mentionCards.every(
					(target) =>
						target.user_id === 'maya-id' &&
						target.login === 'maya' &&
						target.channel_login === 'usernames'
				)
			);

			await evaluate(
				`document.querySelector('[data-chat-username="author"]').click(); document.querySelector('[data-chat-username="system"]').click();`
			);
			const allCards = await evaluate<
				{ channel_id: string; anchor: { x: number; width: number } }[]
			>('window.chatNative.cardRequests');
			assert.equal(allCards.length, 5, 'author and system references also open user cards');
			assert(
				allCards.every(
					(target) =>
						target.channel_id === 'usernames' &&
						Number.isFinite(target.anchor.x) &&
						target.anchor.width > 0
				)
			);
			assert.equal(await evaluate('location.pathname'), '/app/chat/usernames');
		});
	}
}

for (const width of [1200, 375] as const) {
	for (const scale of [1, 1.25] as const) {
		for (const layout of ['language_tag', 'message_text', 'timestamp_end', 'connector'] as const) {
			test(`historical translation backlog (${width}px, DPR ${scale}, ${layout})`, async ({
				browser
			}) => {
				const context = await browser.newContext({
					viewport: { width, height: 800 },
					deviceScaleFactor: scale
				});
				const page = await context.newPage();
				const errors: string[] = [];
				page.on('pageerror', (error) => errors.push(error.message));
				const evaluate = <T = unknown>(expression: string) =>
					page.evaluate(expression) as Promise<T>;
				const waitFor = (expression: string) => waitForExpression(page, expression, 15000);

				try {
					await page.goto(`${capturedBaseURL()}/app/chat/rooma?harness=chat-engine`, {
						waitUntil: 'domcontentloaded'
					});
					await waitFor("document.querySelectorAll('[data-chat-message-index]').length === 3");
					await evaluate('chatNative.go(null)');
					await waitFor('chatNative.feeds.size === 0');
					await evaluate(
						`chatNative.settings.obj.chat.message_limit = 1000; chatNative.settings.obj.translation.layout = '${layout}'; chatNative.settings.sync(); chatNative.seed('backlog', 500, true); chatNative.go('backlog')`
					);
					await waitFor("document.querySelectorAll('[data-chat-message-index]').length === 504");
					await evaluate(`(${checkHistoricalTranslationsSource})()`);
					await evaluate(
						'chatNative.settings.obj.chat.message_limit = 1; chatNative.settings.sync(); chatNative.reset()'
					);
					await waitFor("document.querySelectorAll('[data-chat-message-index]').length === 1");
					assert.deepEqual(errors, [], 'No uncaught frontend exceptions');
				} finally {
					await context.close();
				}
			});
		}
	}
}
