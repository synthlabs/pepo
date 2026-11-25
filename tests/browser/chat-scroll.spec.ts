import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { capturedBaseURL, test, waitForExpression } from '../support/browser/fixtures';

interface ScrollMetadata {
	count: number;
	first: number;
	last: number;
	unread: number;
	batches: number[];
	gcPasses: number[];
	retained: boolean;
	drift: number | null;
}

for (const scale of [1, 1.1, 1.25, 1.5, 1.75, 2] as const) {
	test(`chat scroll origin stability and paused batching (DPR ${scale})`, async ({ browser }) => {
		const context = await browser.newContext({
			viewport: { width: 800, height: 600 },
			deviceScaleFactor: scale
		});
		const page = await context.newPage();
		const session = await context.newCDPSession(page);
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		const evaluate = <T = unknown>(expression: string): Promise<T> =>
			page.evaluate(expression) as Promise<T>;

		try {
			await page.goto(`${capturedBaseURL()}/?harness=chat-scroll`, {
				waitUntil: 'domcontentloaded'
			});
			const initial = await waitForExpression<{
				count: number;
				dpr: number;
				clip: { x: number; y: number; width: number; height: number; scale: number };
			}>(page, 'window.fixtureReady ?? null', 15000);
			assert(Math.abs(initial.dpr - scale) <= 0.001, `Unexpected DPR: ${initial.dpr}`);

			const capture = async () => {
				const shot = await session.send('Page.captureScreenshot', {
					format: 'png',
					fromSurface: true,
					captureBeyondViewport: false,
					clip: initial.clip
				});
				return evaluate<{
					changedPixels: number;
					interiorChangedPixels?: number;
					nonImageChangedPixels?: number;
				}>('window.compareScreenshot(' + JSON.stringify(shot.data) + ')');
			};
			const waitForMeta = async (condition: string) =>
				waitForExpression<ScrollMetadata>(
					page,
					`(() => { const value = window.fixture.metadata(); return (${condition}) ? value : null; })()`,
					10000
				);

			await capture();
			let next = initial.count;
			for (let batch = 0; batch < 8; batch++) {
				const arrivals = Array.from({ length: 25 }, () => next++);
				const queued = await evaluate<ScrollMetadata>(
					'window.fixture.enqueue(' + JSON.stringify(arrivals) + ')'
				);
				assert(
					queued.count === initial.count + batch * 25,
					`DPR ${scale}: paused arrivals appended before the batch deadline`
				);
				const visible = await waitForMeta(`value.count === ${next}`);
				assert(
					visible.retained && visible.drift !== null && Math.abs(visible.drift) < 0.001,
					`DPR ${scale}: paused anchor moved`
				);
				assert(
					visible.first === 0 && visible.gcPasses.length === 0,
					`DPR ${scale}: paused history was collected`
				);
				assert(visible.unread === (batch + 1) * 25, `DPR ${scale}: unread arrivals were lost`);
				const pixels = await capture();
				assert(
					pixels.changedPixels === 0,
					`DPR ${scale}: retained message pixels changed: ${pixels.changedPixels}`
				);
			}

			const queued = [next++, next++];
			await evaluate('window.fixture.enqueue(' + JSON.stringify(queued) + ')');
			const resumed = await evaluate<ScrollMetadata>('window.fixture.resume()');
			assert(
				resumed.count === initial.count &&
					resumed.first === next - initial.count &&
					resumed.last === next - 1,
				`DPR ${scale}: resume lost or reordered history`
			);
			assert(
				resumed.gcPasses.length === 1 && resumed.gcPasses[0] === 202 && resumed.unread === 0,
				`DPR ${scale}: resume did not perform one catch-up cleanup`
			);
			await delay(300);
			assert(
				(await evaluate<ScrollMetadata>('window.fixture.metadata()')).count === initial.count,
				`DPR ${scale}: pending timer published messages twice`
			);

			const live = await evaluate<ScrollMetadata>('window.fixture.enqueue([' + next++ + '])');
			assert(
				live.count === initial.count + 1 && live.last === next - 1,
				`DPR ${scale}: live following did not append immediately`
			);
			const collected = await waitForMeta('value.gcPasses.length === 2');
			assert(
				collected.count === initial.count && collected.gcPasses[1] === 1,
				`DPR ${scale}: live GC was not batched`
			);

			await evaluate('window.fixture.prepareOrigin()');
			let uncorrectedMovement = 0;
			for (let pass = 0; pass < 24; pass++) {
				const value = await evaluate<{ movement: number }>(
					'window.fixture.trimOrigin(' + [1, 3, 8, 5][pass % 4] + ',false)'
				);
				uncorrectedMovement = Math.max(uncorrectedMovement, Math.abs(value.movement));
			}
			if (scale !== 2) {
				assert(uncorrectedMovement > 0.01, `DPR ${scale}: control did not reproduce displacement`);
			}

			await evaluate('window.fixture.prepareOrigin()');
			await capture();
			for (let pass = 0; pass < 24; pass++) {
				const value = await evaluate<{ movement: number; spacer: number }>(
					'window.fixture.trimOrigin(' + [1, 3, 8, 5][pass % 4] + ',true)'
				);
				assert(Math.abs(value.movement) < 0.001, `DPR ${scale}: corrected cleanup moved final row`);
				assert(value.spacer <= 10, `DPR ${scale}: spacer exceeded ten CSS pixels`);
				const pixels = await capture();
				assert(
					pixels.changedPixels === 0 || (scale === 1.75 && pixels.nonImageChangedPixels === 0),
					`DPR ${scale}: cleanup changed retained pixels: ${JSON.stringify(pixels)}`
				);
			}

			const stressBatches = scale === 1 ? 10 : 1;
			for (let batch = 0; batch < stressBatches; batch++) {
				const stress = await evaluate<{ retained: boolean; movement: number; maxSpacer: number }>(
					'window.fixture.stressOrigin(100)'
				);
				assert(
					stress.retained && stress.movement < 0.001,
					`DPR ${scale}: long cleanup run drifted`
				);
				assert(stress.maxSpacer <= 10, `DPR ${scale}: long cleanup run exceeded spacer bound`);
			}

			await evaluate('window.fixture.dispose()');
			assert.deepEqual(errors, [], 'No uncaught frontend exceptions');
		} finally {
			await session.detach().catch(() => {});
			await context.close();
		}
	});
}
