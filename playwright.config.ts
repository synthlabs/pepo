import { fileURLToPath } from 'node:url';
import { defineConfig } from '@playwright/test';

export default defineConfig({
	testDir: './tests/browser',
	fullyParallel: false,
	workers: 1,
	retries: 0,
	timeout: 180000,
	outputDir: 'target/test-results/browser',
	reporter: [['list'], ['json', { outputFile: 'target/test-results/playwright.json' }]],
	use: {
		viewport: { width: 1200, height: 800 },
		deviceScaleFactor: 1,
		reducedMotion: 'no-preference',
		screenshot: 'only-on-failure',
		trace: 'retain-on-failure',
		launchOptions: {
			args: ['--disable-gpu', '--disable-dev-shm-usage', '--disable-background-networking']
		}
	},
	projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
	webServer: {
		command: 'node tests/support/browser/dev-server.mjs --host 127.0.0.1 --port 0',
		wait: { stdout: /Pepo browser port (?<pepo_browser_port>\d+)/ },
		reuseExistingServer: false,
		timeout: 90000,
		cwd: fileURLToPath(new URL('.', import.meta.url)),
		env: { ENABLE_INTERNAL: '0' }
	}
});
