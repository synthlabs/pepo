import {
	captureScrollSnapshot,
	getPinnedBatchScrollSnapshot,
	restoreScrollAfterRender,
	scrollToBottom
} from '$lib/chat/autoscroll';
import { measureHistoryOriginSpacer } from '$lib/chat/history-origin';
import { MessageHistoryScheduler } from '$lib/chat/message-history';
import { DEFAULT_APP_SETTINGS } from '$lib/settings';

async function createFixture() {
	const settings = DEFAULT_APP_SETTINGS.chat;
	const feed = document.createElement('div');
	feed.style.cssText =
		"position:absolute;top:30px;left:10px;height:200px;width:750px;overflow-x:hidden;overflow-y:auto;overflow-anchor:none;background:#080e19;color:white;font:14px/20px 'Segoe UI',sans-serif";
	const content = document.createElement('div');
	const spacer = document.createElement('div');
	feed.append(spacer, content);
	document.body.append(feed);

	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = 28;
	const context = canvas.getContext('2d')!;
	context.fillStyle = '#9363ec';
	context.fillRect(0, 0, 28, 28);
	context.fillStyle = '#fdba24';
	context.fillRect(6, 6, 16, 16);
	const icon = canvas.toDataURL();

	const makeRow = (index: number) => {
		const row = document.createElement('div');
		row.dataset.chatMessageIndex = String(index);
		row.style.cssText =
			'box-sizing:border-box;display:block;width:100%;padding:4px 8px;font-size:14px;line-height:20px;background:' +
			(index % 2 ? '#0e1829' : '#080e19');
		const line = document.createElement('div');
		line.style.cssText = 'min-width:0;text-wrap:wrap;overflow-wrap:anywhere';
		const stamp = document.createElement('span');
		stamp.style.cssText = 'font-size:12px;line-height:16px;color:#64748b;white-space:nowrap';
		stamp.textContent = '4:15 PM';
		line.append(stamp, ' ');
		const badges = document.createElement('span');
		badges.style.cssText =
			'display:inline-flex;align-items:center;gap:4px;vertical-align:middle;white-space:nowrap';
		if (index % 3) {
			const button = document.createElement('button');
			button.style.cssText =
				'font:inherit;padding:0;margin:0;border:0;background:transparent;color:inherit';
			const image = document.createElement('img');
			image.src = icon;
			image.style.cssText = 'display:inline;vertical-align:middle;max-width:none;height:20px';
			button.append(image);
			badges.append(button);
		}
		line.append(badges, ' ');
		const username = document.createElement('span');
		username.style.cssText = 'white-space:nowrap;font-weight:700;color:#70c7ff';
		username.textContent = 'User' + index + ': ';
		line.append(username, 'fixed text to inspect vertical movement ');
		if (index % 3 === 2) {
			const emote = document.createElement('span');
			emote.style.cssText = 'display:inline-block;vertical-align:middle;line-height:1';
			const image = document.createElement('img');
			image.src = icon;
			image.style.cssText = 'display:block;max-width:none;height:28px;min-width:28px';
			emote.append(image);
			line.append(emote);
		}
		row.append(line);
		return row;
	};

	const messages: number[] = [];
	const batches: number[] = [];
	const gcPasses: number[] = [];
	const state = { ...settings, paused: false, interacting: false };
	const selector = '[data-chat-message-index]';
	let initialized = false;
	let resuming = false;
	let unread = 0;

	const snapshot = () =>
		initialized && !resuming
			? getPinnedBatchScrollSnapshot(null, feed, selector, !state.paused)
			: null;
	const restore = (saved: ReturnType<typeof snapshot>) => {
		if (saved) restoreScrollAfterRender(feed, saved, selector);
	};
	const trimRows = (count: number, compensated = true) => {
		const saved = snapshot();
		spacer.style.height =
			(compensated
				? measureHistoryOriginSpacer(
						spacer,
						Array.from(content.children) as HTMLElement[],
						new Set(messages.slice(0, count).map(String)),
						devicePixelRatio
					)
				: 0) + 'px';
		messages.splice(0, count);
		for (let i = 0; i < count; i++) content.firstElementChild?.remove();
		restore(saved);
	};

	const history = new MessageHistoryScheduler<number>({
		getState: () => ({ ...state, message_count: messages.length }),
		append: (batch) => {
			const saved = snapshot();
			messages.push(...batch);
			const fragment = document.createDocumentFragment();
			for (const index of batch) fragment.append(makeRow(index));
			content.append(fragment);
			batches.push(batch.length);
			restore(saved);
		},
		trim: (count) => {
			trimRows(count);
			gcPasses.push(count);
		}
	});

	for (let i = 0; i < settings.message_limit; i++) history.enqueue(i);
	await Promise.all([...content.querySelectorAll('img')].map((image) => image.decode()));
	await document.fonts.ready;
	feed.scrollTop = 8000;
	state.paused = true;
	history.update();
	initialized = true;
	batches.length = 0;

	const initial = captureScrollSnapshot(feed, selector)!;
	const anchor = [...content.children].find(
		(row) => (row as HTMLElement).dataset.chatMessageIndex === initial.anchor?.key
	) as HTMLElement;
	const initialTop = anchor.getBoundingClientRect().top - feed.getBoundingClientRect().top;

	const metadata = () => ({
		count: messages.length,
		first: messages[0],
		last: messages.at(-1),
		unread,
		batches: [...batches],
		gcPasses: [...gcPasses],
		retained: anchor.isConnected,
		drift: anchor.isConnected
			? anchor.getBoundingClientRect().top - feed.getBoundingClientRect().top - initialTop
			: null
	});

	let baseline: Uint8ClampedArray | undefined;
	let width = 0;
	let height = 0;

	window.fixture = {
		prepareOrigin: async () => {
			history.dispose();
			state.paused = false;
			spacer.style.height = '0px';
			messages.length = 0;
			const fragment = document.createDocumentFragment();
			for (let index = 0; index < 1500; index++) {
				messages.push(index);
				fragment.append(makeRow(index));
			}
			content.replaceChildren(fragment);
			await Promise.all([...content.querySelectorAll('img')].map((image) => image.decode()));
			scrollToBottom(feed);
			baseline = undefined;
			return content.lastElementChild!.getBoundingClientRect().top;
		},
		trimOrigin: (count: number, compensated: boolean) => {
			const tail = content.lastElementChild!;
			const before = tail.getBoundingClientRect().top;
			trimRows(count, compensated);
			return {
				top: tail.getBoundingClientRect().top,
				movement: tail.getBoundingClientRect().top - before,
				spacer: spacer.getBoundingClientRect().height
			};
		},
		stressOrigin: (passes: number) => {
			const tail = content.lastElementChild!;
			const before = tail.getBoundingClientRect().top;
			let movement = 0;
			let maxSpacer = 0;
			for (let pass = 0; pass < passes; pass++) {
				trimRows(1);
				movement = Math.max(movement, Math.abs(tail.getBoundingClientRect().top - before));
				maxSpacer = Math.max(maxSpacer, spacer.getBoundingClientRect().height);
			}
			return { movement, maxSpacer, retained: tail.isConnected };
		},
		metadata,
		enqueue: (batch: number[]) => {
			for (const message of batch) {
				if (state.paused) unread++;
				history.enqueue(message);
			}
			return metadata();
		},
		resume: () => {
			state.paused = false;
			resuming = true;
			try {
				history.resume();
			} finally {
				resuming = false;
			}
			scrollToBottom(feed);
			unread = 0;
			return metadata();
		},
		dispose: () => history.dispose()
	};

	window.compareScreenshot = async (png: string) => {
		const bytes = Uint8Array.from(atob(png), (character) => character.charCodeAt(0));
		const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
		const offscreen = new OffscreenCanvas(bitmap.width, bitmap.height);
		const ctx = offscreen.getContext('2d')!;
		ctx.drawImage(bitmap, 0, 0);
		const pixels = ctx.getImageData(0, 0, bitmap.width, bitmap.height).data;
		bitmap.close();
		if (!baseline) {
			baseline = pixels;
			width = offscreen.width;
			height = offscreen.height;
			return { changedPixels: 0 };
		}
		if (offscreen.width !== width || offscreen.height !== height)
			throw new Error('Screenshot dimensions changed');
		let changedPixels = 0;
		let interiorChangedPixels = 0;
		let nonImageChangedPixels = 0;
		const box = feed.getBoundingClientRect();
		const images = [...content.querySelectorAll('img')]
			.map((image) => image.getBoundingClientRect())
			.filter((rect) => rect.bottom > box.top + 35 && rect.top < box.top + 155);
		for (let i = 0; i < pixels.length; i += 4) {
			if (
				pixels[i] !== baseline[i] ||
				pixels[i + 1] !== baseline[i + 1] ||
				pixels[i + 2] !== baseline[i + 2]
			) {
				changedPixels++;
				const y = Math.floor(i / 4 / width);
				if (y > 0 && y < height - 1) interiorChangedPixels++;
				const xCss = box.left + 8 + ((i / 4) % width) / devicePixelRatio;
				const yCss = box.top + 35 + y / devicePixelRatio;
				const edge = 1 / devicePixelRatio;
				if (
					!images.some(
						(rect) =>
							xCss >= rect.left - edge &&
							xCss <= rect.right + edge &&
							yCss >= rect.top - edge &&
							yCss <= rect.bottom + edge
					)
				)
					nonImageChangedPixels++;
			}
		}
		return { changedPixels, interiorChangedPixels, nonImageChangedPixels };
	};

	const box = feed.getBoundingClientRect();
	return {
		count: messages.length,
		dpr: devicePixelRatio,
		clip: {
			x: box.left + 8,
			y: box.top + 35,
			width: Math.min(300, innerWidth - box.left - 24),
			height: 120,
			scale: 1
		}
	};
}

void createFixture().then((result) => {
	window.fixtureReady = result;
});
