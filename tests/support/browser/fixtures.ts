import assert from 'node:assert/strict';
import { mkdir, readdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { test as base, type CDPSession, type Page, type TestInfo } from '@playwright/test';

export async function until<T>(
	check: () => T | Promise<T>,
	message: string,
	timeout = 20000
): Promise<NonNullable<T>> {
	const deadline = Date.now() + timeout;
	let cause: unknown;
	while (Date.now() < deadline) {
		try {
			const value = await check();
			if (value) return value;
		} catch (error) {
			cause = error;
		}
		await delay(40);
	}
	throw new Error(message, { cause });
}

export async function createBrowserApp(page: Page, testInfo: TestInfo, session: CDPSession) {
	const artifacts = testInfo.outputPath('measurements');
	await mkdir(artifacts, { recursive: true });
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));

	const evaluate = <T = unknown>(expression: string): Promise<T> =>
		page.evaluate(expression) as Promise<T>;
	const waitFor = async <T = unknown>(expression: string, timeout = 15000): Promise<T> =>
		until(() => evaluate<T>(expression), `Timed out waiting for: ${expression}`, timeout);
	const cdp = session.send.bind(session);
	const viewport = async (width: number, height: number) => page.setViewportSize({ width, height });
	const save = async (name: string, value: unknown) => {
		await writeFile(join(artifacts, `${name}.json`), JSON.stringify(value, null, 2));
	};

	return {
		page,
		cdp,
		evaluate,
		waitFor,
		viewport,
		save,
		errors,
		artifacts
	};
}

export type BrowserApp = Awaited<ReturnType<typeof createBrowserApp>>;

export function capturedBaseURL(): string {
	const port = process.env.PEPO_BROWSER_PORT;
	assert.match(port ?? '', /^\d+$/, 'Pepo browser server port was not captured');
	return `http://127.0.0.1:${port}`;
}

export const test = base.extend<{ app: BrowserApp }>({
	// eslint-disable-next-line no-empty-pattern
	baseURL: async ({}, use) => {
		await use(capturedBaseURL());
	},
	app: async ({ page }, use, testInfo) => {
		const session = await page.context().newCDPSession(page);
		await session.send('Emulation.setFocusEmulationEnabled', { enabled: true });
		const app = await createBrowserApp(page, testInfo, session);
		try {
			await use(app);
			assert.deepEqual(app.errors, [], 'No uncaught frontend exceptions');
		} finally {
			if (testInfo.status !== testInfo.expectedStatus && !page.isClosed()) {
				try {
					await app.save('failure-state', {
						url: page.url(),
						errors: app.errors,
						html: await page.content()
					});
				} catch {
					// ignore capture failure on closed page
				}
			}
			for (const file of await readdir(app.artifacts)) {
				if (file.endsWith('.json')) {
					await testInfo.attach(file, {
						path: join(app.artifacts, file),
						contentType: 'application/json'
					});
				}
			}
			await session.detach().catch(() => {});
		}
	}
});
