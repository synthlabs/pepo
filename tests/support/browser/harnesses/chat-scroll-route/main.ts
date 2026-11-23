/* eslint-disable @typescript-eslint/no-explicit-any */
import { mount, tick } from 'svelte';
import Route from '../../../../../src/routes/app/chat/[id]/+page.svelte';
import { native } from './native.svelte';
import '../../../../../src/app.css';

function trackResizeGeometry() {
	const NativeResizeObserver = window.ResizeObserver;
	const height = Object.getOwnPropertyDescriptor(Element.prototype, 'scrollHeight')!;
	let observerDepth = 0;
	let reads: { observer: number; outsideObserver: number } | null = null;
	window.ResizeObserver = class extends NativeResizeObserver {
		constructor(callback: ResizeObserverCallback) {
			super((entries, observer) => {
				observerDepth += 1;
				try {
					callback(entries, observer);
				} finally {
					observerDepth -= 1;
				}
			});
		}
	};
	Object.defineProperty(Element.prototype, 'scrollHeight', {
		...height,
		get(this: Element) {
			if (reads && this.getAttribute('aria-label') === 'Chat messages') {
				reads[observerDepth > 0 ? 'observer' : 'outsideObserver'] += 1;
			}
			return Reflect.apply(height.get!, this, []);
		}
	});
	return {
		NativeResizeObserver,
		readHeight: (element: Element) => Reflect.apply(height.get!, element, []) as number,
		start: () => {
			reads = { observer: 0, outsideObserver: 0 };
		},
		stop: () => {
			const result = reads;
			reads = null;
			return result;
		}
	};
}

async function createRouteFixture(geometry: ReturnType<typeof trackResizeGeometry>) {
	while (!native.feed) await new Promise((resolve) => setTimeout(resolve, 5));
	const feed = document.querySelector<HTMLElement>('[aria-label="Chat messages"]')!;
	const rows = () => [...feed.querySelectorAll<HTMLElement>('[data-chat-message-index]')];
	const spacer = () =>
		feed.querySelector<HTMLElement>('[data-chat-history-spacer]')!.getBoundingClientRect().height;
	const frames = async () => {
		await tick();
		for (let i = 0; i < 3; i++) await new Promise(requestAnimationFrame);
	};
	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = 28;
	const context = canvas.getContext('2d')!;
	context.fillStyle = '#9363ec';
	context.fillRect(0, 0, 28, 28);
	context.fillStyle = '#fdba24';
	context.fillRect(6, 6, 16, 16);
	const icon = canvas.toDataURL();
	let next = 0;
	const makeMessage = () => {
		const channel_sequence = next++;
		const index = channel_sequence + 1;
		const badge = {
			set_id: 'fixture',
			id: '1',
			title: 'Badge',
			description: 'Fixture badge',
			image_url_1x: icon,
			image_url_2x: icon,
			image_url_4x: icon
		};
		return {
			index,
			channel_sequence,
			ts: '2026-10-25T16:15:00Z',
			broadcaster_user_id: '1',
			broadcaster_user_name: 'Fixture',
			broadcaster_user_login: 'fixture',
			chatter_user_id: String(index),
			chatter_user_name: 'User' + index,
			message_id: String(index),
			text: 'fixed text to inspect vertical movement',
			kind: 'chat',
			message_type: 'text',
			color: '#70c7ff',
			translation: null,
			badges: index % 3 ? [{ set_id: 'fixture', id: '1', info: '', badge }] : [],
			fragments: [
				{ Text: { text: 'fixed text to inspect vertical movement ' } },
				...(index % 3 === 2
					? [
							{
								Emote: {
									emote: {
										id: '1',
										name: 'Icon',
										tier: '',
										provider: 'twitch',
										url: icon,
										format: [],
										scale: [],
										theme_mode: [],
										emote_type: '',
										emote_set_id: ''
									}
								}
							}
						]
					: [])
			]
		};
	};
	const emit = (count: number) => {
		for (let i = 0; i < count; i++) {
			const message = makeMessage();
			native.feed.onmessage({ cursor: message.index, event: { type: 'message', message } });
		}
	};
	const waitCount = async (count: number) => {
		for (let i = 0; i < 2000; i++) {
			await tick();
			if (rows().length === count) return;
			await new Promise((resolve) => setTimeout(resolve, 5));
		}
		throw new Error('Timed out waiting for ' + count + ' rows; got ' + rows().length);
	};
	const metadata = () => ({
		count: rows().length,
		first: rows()[0]?.dataset.chatMessageIndex,
		last: rows().at(-1)?.dataset.chatMessageIndex,
		spacer: spacer(),
		listHeight: feed.firstElementChild!.getBoundingClientRect().height,
		viewportWidth: feed.clientWidth,
		distance: feed.scrollHeight - feed.clientHeight - feed.scrollTop,
		paused: !!document.querySelector('button[aria-label^="Jump to"]'),
		error: document.querySelector('[data-chat-input-error]')?.textContent?.trim() ?? null
	});
	emit(500);
	await waitCount(500);
	await Promise.all([...feed.querySelectorAll('img')].map((image) => image.decode()));
	await document.fonts.ready;
	await frames();
	native.settings.chat.message_gc_interval_ms = 1;
	const tail = rows().at(-1)!;
	let baseline: Uint8ClampedArray | undefined;
	const box = feed.getBoundingClientRect();
	const clip = { x: box.left + 8, y: box.top + 35, width: 300, height: 120, scale: 1 };

	window.routeFixture = {
		metadata,
		measurePinnedResize: async () => {
			await waitCount(500);
			await frames();
			const originalWidth = document.body.style.width;
			const originalTransition = document.body.style.transition;
			const width = document.body.getBoundingClientRect().width;
			const trials = [];
			let samples: { distance: number; width: number; height: number }[] | null = null;
			const observer = new geometry.NativeResizeObserver(() => {
				if (!samples) return;
				samples.push({
					distance: geometry.readHeight(feed) - feed.clientHeight - feed.scrollTop,
					width: feed.clientWidth,
					height: feed.firstElementChild!.getBoundingClientRect().height
				});
			});
			observer.observe(feed);
			observer.observe(feed.firstElementChild!);
			try {
				await new Promise((resolve) => setTimeout(resolve, 80));
				document.body.style.transition = 'width 200ms linear';
				for (const target of [width - 208, width]) {
					samples = [];
					geometry.start();
					document.body.style.width = target + 'px';
					await new Promise((resolve) => setTimeout(resolve, 280));
					const captured = samples;
					samples = null;
					trials.push({
						reads: geometry.stop(),
						samples: captured.length,
						widths: new Set(captured.map((sample) => sample.width)).size,
						heights: new Set(captured.map((sample) => sample.height)).size,
						maxDistance: Math.max(...captured.map((sample) => Math.abs(sample.distance))),
						...metadata()
					});
				}
				return trials;
			} finally {
				samples = null;
				geometry.stop();
				observer.disconnect();
				document.body.style.transition = originalTransition;
				document.body.style.width = originalWidth;
			}
		},
		resize: async (width: number, arrivals = 0) => {
			document.body.style.width = document.body.getBoundingClientRect().width + 'px';
			document.body.style.transition = 'width 200ms linear';
			void document.body.offsetWidth;
			document.body.style.width = width + 'px';
			await new Promise((resolve) => setTimeout(resolve, 70));
			emit(arrivals);
			await new Promise((resolve) => setTimeout(resolve, 210));
			await frames();
			return metadata();
		},
		translate: async (key: string) => {
			native.feed.onmessage({
				cursor: next,
				event: {
					type: 'translation',
					update: {
						message_id: key,
						translation: {
							source_language: 'rus',
							target_language: 'eng',
							translated_text:
								'A translated message with enough text to wrap across several lines. '.repeat(12)
						}
					}
				}
			});
			await tick();
			await new Promise((resolve) => setTimeout(resolve, 80));
			await frames();
			return metadata();
		},
		interruptRestore: async () => {
			emit(1);
			feed.dispatchEvent(new WheelEvent('wheel', { deltaY: -150, bubbles: true }));
			feed.scrollTop -= 150;
			await new Promise((resolve) => setTimeout(resolve, 80));
			await frames();
			return metadata();
		},
		showError: async (error: string) => {
			native.feed.onmessage({ cursor: next, event: { type: 'unavailable', error } });
			await frames();
			return metadata();
		},
		emit: async (count: number) => {
			emit(count);
			await tick();
			return metadata();
		},
		cleanup: async (count: number) => {
			const snap = () => ({
				top: tail.getBoundingClientRect().top,
				scrollTop: feed.scrollTop,
				scrollHeight: feed.scrollHeight,
				clientHeight: feed.clientHeight,
				viewportHeight: feed.getBoundingClientRect().height,
				listHeight: feed.firstElementChild!.getBoundingClientRect().height,
				spacer: spacer()
			});
			const beforeGeometry = snap();
			const samples: ReturnType<typeof snap>[] = [];
			const before = tail.getBoundingClientRect().top;
			let movement = 0;
			let active = true;
			const sample = () => {
				if (samples.length < 10) samples.push(snap());
				movement = Math.max(movement, Math.abs(tail.getBoundingClientRect().top - before));
				if (active) requestAnimationFrame(sample);
			};
			requestAnimationFrame(sample);
			native.settings.chat.message_limit -= count;
			await waitCount(native.settings.chat.message_limit);
			await frames();
			active = false;
			movement = Math.max(movement, Math.abs(tail.getBoundingClientRect().top - before));
			return {
				...metadata(),
				movement,
				...(movement > 0.001 ? { beforeGeometry, samples, afterGeometry: snap() } : {})
			};
		},
		setChat: async (update: Record<string, any>) => {
			Object.assign(native.settings.chat, update);
			await tick();
		},
		pause: async () => {
			feed.dispatchEvent(new WheelEvent('wheel', { deltaY: -100, bubbles: true }));
			feed.scrollTop -= 100;
			await new Promise((resolve) => setTimeout(resolve, 300));
			await frames();
			return metadata();
		},
		anchor: () => {
			const row = rows().find(
				(item) => item.getBoundingClientRect().bottom > feed.getBoundingClientRect().top
			)!;
			return { key: row.dataset.chatMessageIndex, top: row.getBoundingClientRect().top };
		},
		anchorTop: (key: string) =>
			rows()
				.find((item) => item.dataset.chatMessageIndex === key)
				?.getBoundingClientRect().top,
		resume: async () => {
			document.querySelector<HTMLElement>('button[aria-label^="Jump to"]')!.click();
			await frames();
			return metadata();
		},
		replaceOnResume: async () => {
			emit(2);
			native.settings.chat.message_limit = 1;
			document.querySelector<HTMLElement>('button[aria-label^="Jump to"]')!.click();
			await waitCount(1);
			await frames();
			return { ...metadata(), expectedLast: String(next) };
		},
		compare: async (png: string) => {
			const bytes = Uint8Array.from(atob(png), (character) => character.charCodeAt(0));
			const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
			const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
			const ctx = surface.getContext('2d')!;
			ctx.drawImage(bitmap, 0, 0);
			const pixels = ctx.getImageData(0, 0, bitmap.width, bitmap.height).data;
			bitmap.close();
			if (!baseline) {
				baseline = pixels;
				return { changedPixels: 0, nonImageChangedPixels: 0 };
			}
			const images = [...feed.querySelectorAll('img')]
				.map((image) => image.getBoundingClientRect())
				.filter((rect) => rect.bottom >= clip.y && rect.top <= clip.y + clip.height);
			let changedPixels = 0;
			let nonImageChangedPixels = 0;
			for (let i = 0; i < pixels.length; i += 4) {
				if (
					pixels[i] === baseline[i] &&
					pixels[i + 1] === baseline[i + 1] &&
					pixels[i + 2] === baseline[i + 2]
				)
					continue;
				changedPixels++;
				const x = clip.x + ((i / 4) % surface.width) / devicePixelRatio;
				const y = clip.y + Math.floor(i / 4 / surface.width) / devicePixelRatio;
				const edge = 1 / devicePixelRatio;
				if (
					!images.some(
						(rect) =>
							x >= rect.left - edge &&
							x <= rect.right + edge &&
							y >= rect.top - edge &&
							y <= rect.bottom + edge
					)
				)
					nonImageChangedPixels++;
			}
			return { changedPixels, nonImageChangedPixels };
		}
	};

	return { clip, dpr: devicePixelRatio };
}

document.body.style.height = '100vh';
const geometry = trackResizeGeometry();
mount(Route, { target: document.body });
void createRouteFixture(geometry).then((result) => {
	window.fixtureReady = result;
});
