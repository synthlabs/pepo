import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import type { CDPSession } from '@playwright/test';
import { capturedBaseURL, test, until } from '../support/browser/fixtures';

// Per-action DOM trace snapshots across 500 rows push consecutive resizes past PAUSED_REFLOW_SETTLE_MS (250ms).
test.use({ trace: 'off' });

type EvaluateFn = <T = unknown>(expression: string) => Promise<T>;

interface RouteMetadata {
	count: number;
	first: string | undefined;
	last: string | undefined;
	spacer: number;
	listHeight: number;
	viewportWidth: number;
	distance: number;
	paused: boolean;
	error: string | null;
}

async function checkNoticeScroll(session: CDPSession, evaluate: EvaluateFn) {
	const state = () => evaluate<RouteMetadata>('window.routeFixture.metadata()');
	const show = async (message: string) => {
		await evaluate('window.routeFixture.showError(' + JSON.stringify(message) + ')');
		await delay(350);
		return state();
	};
	const expire = async () => {
		const deadline = Date.now() + 8000;
		while ((await state()).error !== null && Date.now() < deadline) await delay(50);
		assert.equal((await state()).error, null, 'Notice did not expire');
		await delay(100);
		return state();
	};
	const short = 'Message cannot be empty';
	const long =
		'Your previous message is still sending. Please wait before submitting another. '.repeat(3);

	for (const motion of ['no-preference', 'reduce'] as const) {
		await session.send('Emulation.setEmulatedMedia', {
			features: [{ name: 'prefers-reduced-motion', value: motion }]
		});
		for (const message of [short, long]) {
			const shown = await show(message);
			assert(
				Math.abs(shown.distance) < 1 && !shown.paused,
				`${motion}: notice lost live following: ${JSON.stringify(shown)}`
			);
		}
		const hidden = await expire();
		assert(
			Math.abs(hidden.distance) < 1 && !hidden.paused,
			`${motion}: expiry lost live following`
		);
		await evaluate('window.routeFixture.pause()');
		const anchor = await evaluate<{ key: string; top: number }>('window.routeFixture.anchor()');
		const checkAnchor = async () => {
			assert((await state()).paused, `${motion}: notice resumed paused chat`);
			const top = await evaluate<number>(
				'window.routeFixture.anchorTop(' + JSON.stringify(anchor.key) + ')'
			);
			assert(Math.abs(top - anchor.top) < 0.001, `${motion}: notice moved paused anchor`);
		};
		for (const message of [short, long]) {
			await show(message);
			await checkAnchor();
			assert(
				await evaluate<boolean>(`(() => {
					const button = document.querySelector('button[aria-label^="Jump to"]');
					const error = document.querySelector('[data-chat-input-error]').getBoundingClientRect();
					const rect = button.getBoundingClientRect();
					return rect.bottom <= error.top && document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2)?.closest('button') === button;
				})()`),
				'Paused control is unobscured and clickable'
			);
		}
		await expire();
		await checkAnchor();
		await show(short);
		await evaluate('window.routeFixture.resume()');
		await delay(200);
		const resumed = await state();
		assert(
			!resumed.paused && Math.abs(resumed.distance) < 1,
			'Jump works with an active error: ' + JSON.stringify(resumed)
		);
		await expire();
	}
	await session.send('Emulation.setEmulatedMedia', { features: [] });
}

async function checkControllerScroll(evaluate: EvaluateFn) {
	const width = await evaluate<number>('document.body.getBoundingClientRect().width');
	for (const target of [width - 208, width]) {
		const state = await evaluate<RouteMetadata>('window.routeFixture.resize(' + target + ', 1)');
		assert(Math.abs(state.viewportWidth - target) < 20, 'Fixture did not resize the chat viewport');
		assert(!state.paused && Math.abs(state.distance) < 1, 'Animated resize lost pinned arrivals');
	}
	const pinned = await evaluate<RouteMetadata>('window.routeFixture.metadata()');
	const translated = await evaluate<RouteMetadata>(
		'window.routeFixture.translate(' + JSON.stringify(pinned.last) + ')'
	);
	assert(translated.listHeight > pinned.listHeight, 'Fixture did not render the translation');
	assert(!translated.paused && Math.abs(translated.distance) < 1, 'Translation lost pinned bottom');
	await evaluate('window.routeFixture.pause()');
	const anchor = await evaluate<{ key: string; top: number }>('window.routeFixture.anchor()');
	const checkAnchor = async () => {
		const snap = await evaluate<{ paused: boolean; top: number }>(
			`({ paused: window.routeFixture.metadata().paused, top: window.routeFixture.anchorTop(${JSON.stringify(anchor.key)}) })`
		);
		assert(snap.paused, 'Reflow resumed paused chat');
		assert(
			Math.abs(snap.top - anchor.top) < 1,
			`Resize or translation moved the paused anchor (${snap.top} vs ${anchor.top})`
		);
	};
	const paused = await evaluate<RouteMetadata>('window.routeFixture.metadata()');
	const translatedAbove = await evaluate<RouteMetadata>(
		'window.routeFixture.translate(' + JSON.stringify(paused.first) + ')'
	);
	assert(
		translatedAbove.listHeight > paused.listHeight,
		'Fixture did not grow content above the anchor'
	);
	await checkAnchor();
	let previousHeight = translatedAbove.listHeight;
	for (const target of [width - 208, width]) {
		const state = await evaluate<RouteMetadata>('window.routeFixture.resize(' + target + ')');
		assert(
			state.listHeight !== previousHeight,
			'Fixture did not reflow messages during resizing: ' +
				JSON.stringify({ target, previousHeight, state })
		);
		previousHeight = state.listHeight;
		await checkAnchor();
	}
	await evaluate('window.routeFixture.resume()');
	const interrupted = await evaluate<RouteMetadata>('window.routeFixture.interruptRestore()');
	assert(interrupted.paused && interrupted.distance > 50, 'Queued restore overrode manual input');
	await delay(300);
	assert(
		(await evaluate<RouteMetadata>('window.routeFixture.metadata()')).paused,
		'Manual settle resumed chat'
	);
	await evaluate('window.routeFixture.resume()');
	await delay(200);
	const resumed = await evaluate<RouteMetadata>('window.routeFixture.metadata()');
	assert(
		!resumed.paused && Math.abs(resumed.distance) < 1,
		'Resume after interruption lost bottom: ' + JSON.stringify(resumed)
	);
	await evaluate('document.body.style.transition = ""');
}

async function checkPinnedResizeGeometry(evaluate: EvaluateFn) {
	const trials = await evaluate<
		(RouteMetadata & {
			reads: { observer: number; outsideObserver: number };
			widths: number;
			heights: number;
			maxDistance: number;
		})[]
	>('window.routeFixture.measurePinnedResize()');
	for (const trial of trials) {
		const details = JSON.stringify(trial);
		assert.equal(trial.count, 500, 'Geometry trial requires 500 retained messages: ' + details);
		assert(trial.widths >= 3 && trial.heights >= 2, 'Trial did not animate and reflow: ' + details);
		assert(trial.reads.observer >= 3, 'Pinning did not read post-layout geometry: ' + details);
		assert(trial.reads.outsideObserver <= 1, 'Resize queued repeated geometry reads: ' + details);
		assert(trial.maxDistance < 1, 'A sampled resize lost the pinned bottom: ' + details);
		assert(!trial.paused && Math.abs(trial.distance) < 1, 'Resize lost live following: ' + details);
	}
}

for (const scale of [1, 1.1, 1.25, 1.5, 1.75, 2] as const) {
	test(`chat route scroll controller and geometry (DPR ${scale})`, async ({ browser }) => {
		const context = await browser.newContext({
			viewport: { width: 800, height: 600 },
			deviceScaleFactor: scale
		});
		const page = await context.newPage();
		const session = await context.newCDPSession(page);
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		const evaluate: EvaluateFn = <T = unknown>(expression: string) =>
			page.evaluate(expression) as Promise<T>;

		try {
			await page.goto(`${capturedBaseURL()}/?harness=chat-scroll-route`, {
				waitUntil: 'domcontentloaded'
			});
			const initial = await until(
				() =>
					evaluate<{
						clip: { x: number; y: number; width: number; height: number; scale: number };
						dpr: number;
					} | null>('window.fixtureReady ?? null'),
				`DPR ${scale}: route did not initialize`,
				20000
			);
			const capture = async () => {
				const shot = await session.send('Page.captureScreenshot', {
					format: 'png',
					fromSurface: true,
					captureBeyondViewport: false,
					clip: initial.clip
				});
				return evaluate<{ changedPixels: number; nonImageChangedPixels: number }>(
					'window.routeFixture.compare(' + JSON.stringify(shot.data) + ')'
				);
			};

			await capture();
			for (let pass = 0; pass < 24; pass++) {
				const state = await evaluate<RouteMetadata & { movement: number }>(
					'window.routeFixture.cleanup(' + [1, 3, 8, 5][pass % 4] + ')'
				);
				assert(
					state.movement < 0.001,
					'DPR ' + scale + ': cleanup frame moved ' + JSON.stringify(state)
				);
				assert(state.spacer <= 10, 'Spacer exceeded ten CSS pixels');
				const pixels = await capture();
				assert(
					pixels.changedPixels === 0 || (scale === 1.75 && pixels.nonImageChangedPixels === 0),
					'DPR ' + scale + ': retained pixels changed ' + JSON.stringify(pixels)
				);
			}

			if (scale === 1) {
				await session.send('Emulation.setDeviceMetricsOverride', {
					width: 800,
					height: 600,
					deviceScaleFactor: 1.25,
					mobile: false
				});
				await delay(100);
				assert(
					(await evaluate<{ movement: number }>('window.routeFixture.cleanup(3)')).movement < 0.001,
					'Scale change cleanup moved'
				);
				await session.send('Emulation.setDeviceMetricsOverride', {
					width: 800,
					height: 600,
					deviceScaleFactor: 1.234567,
					mobile: false
				});
				await delay(100);
				assert.equal(
					(await evaluate<{ spacer: number }>('window.routeFixture.cleanup(3)')).spacer,
					0,
					'Unsupported scale did not use zero spacer'
				);
				await session.send('Emulation.clearDeviceMetricsOverride');
				await delay(100);
			}

			await evaluate(
				'window.routeFixture.setChat({ message_limit: 500, message_gc_interval_ms: 250 })'
			);
			const live = await evaluate<RouteMetadata>('window.routeFixture.emit(130)');
			assert(live.count > 500, 'Live arrivals did not publish before cleanup');
			await delay(350);
			const collected = await evaluate<RouteMetadata>('window.routeFixture.metadata()');
			assert.equal(collected.count, 500);
			assert(Math.abs(collected.distance) < 1, 'Live following lost bottom');
			assert(
				(await evaluate<RouteMetadata>('window.routeFixture.pause()')).paused,
				'Wheel did not pause route'
			);

			const anchor = await evaluate<{ key: string; top: number }>('window.routeFixture.anchor()');
			assert.equal(
				(await evaluate<RouteMetadata>('window.routeFixture.emit(25)')).count,
				500,
				'Paused arrivals published immediately'
			);
			await delay(350);
			assert.equal(
				(await evaluate<RouteMetadata>('window.routeFixture.metadata()')).count,
				525,
				'Paused history was collected'
			);
			assert(
				Math.abs(
					(await evaluate<number>(
						'window.routeFixture.anchorTop(' + JSON.stringify(anchor.key) + ')'
					)) - anchor.top
				) < 0.001,
				'Paused anchor moved'
			);

			const resumed = await evaluate<RouteMetadata>('window.routeFixture.resume()');
			assert.equal(resumed.count, 500);
			assert(Math.abs(resumed.distance) < 1, 'Resume lost bottom');

			await checkControllerScroll(evaluate);
			await checkPinnedResizeGeometry(evaluate);
			if (scale === 1) await checkNoticeScroll(session, evaluate);

			await evaluate('window.routeFixture.pause()');
			const pausedAnchor = await evaluate<{ key: string; top: number }>(
				'window.routeFixture.anchor()'
			);
			await evaluate('window.routeFixture.setChat({ pause_message_gc_while_paused: false })');
			await evaluate('window.routeFixture.emit(25)');
			await delay(650);
			assert.equal(
				(await evaluate<RouteMetadata>('window.routeFixture.metadata()')).count,
				500,
				'Opt-in paused cleanup did not run'
			);
			assert(
				Math.abs(
					(await evaluate<number>(
						'window.routeFixture.anchorTop(' + JSON.stringify(pausedAnchor.key) + ')'
					)) - pausedAnchor.top
				) < 0.001,
				'Opt-in paused cleanup moved anchor'
			);

			const replaced = await evaluate<RouteMetadata & { expectedLast: string }>(
				'window.routeFixture.replaceOnResume()'
			);
			assert.equal(replaced.count, 1);
			assert.equal(replaced.last, replaced.expectedLast);
			assert.equal(replaced.spacer, 0);
			assert.deepEqual(errors, [], 'No uncaught frontend exceptions');
		} finally {
			await session.detach().catch(() => {});
			await context.close();
		}
	});
}
