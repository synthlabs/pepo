import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

// Run with node scripts/check-sidebar-header.mjs; BROWSER_BIN can select Chromium.
const root = fileURLToPath(new URL('../', import.meta.url));
const preview = path.join(root, 'target/sidebar-header-preview');
const browserPath = [
	process.env.BROWSER_BIN,
	'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
	'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
	'/usr/bin/chromium',
	'/usr/bin/chromium-browser',
	'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
].find((candidate) => candidate && existsSync(candidate));
assert(browserPath, 'Set BROWSER_BIN to an installed Chromium browser');
await mkdir(preview, { recursive: true });
await writeFile(
	path.join(preview, 'mocks.js'),
	`
const channel = { id: '1', login: 'maya', display_name: 'Maya', profile_image_url: '' };
export const commands = {
  getFollowedChannels: async () => ({ status: 'ok', data: [channel] })
};
export class SyncedState {
  constructor(key, fallback) {
    this.obj = key === 'auth_state'
      ? { phase: 'authorized', token: { user_id: '2', login: 'jerod', profile_image_url: '' } }
      : fallback;
  }
}
export const page = { url: new URL('http://localhost/app/chat/maya') };
export const goto = () => {};
export const resolve = value => value;
export default { debug() {}, error() {} };
`
);
await writeFile(
	path.join(preview, 'preview.svelte'),
	`
<script>
  import * as Sidebar from '$lib/components/ui/sidebar';
  import AppSidebar from '$lib/components/app-sidebar.svelte';
  import ChannelHeader from '$lib/components/chat/channel-header.svelte';
  let { open = $bindable(true), language = 'English', tags = true, long = false } = $props();
  const header = $derived({
    channelName: 'Maya', isPartner: true, state: 'live',
    title: long ? 'A very long stream title 🦊 '.repeat(20) : 'Original stream title 🦊',
    category: 'Terraria', language,
    tags: tags ? ['Hair', 'DropsEnabled', 'AnExceptionallyLongTagThatShouldRemainFullyReadable'] : [],
    viewerCount: 4646, startedAt: new Date(Date.now() - 10065000).toISOString()
  });
</script>
<Sidebar.Provider bind:open class="h-dvh min-h-0 overflow-hidden">
  <AppSidebar collapsible="icon" />
  <main class="flex h-full min-w-0 flex-1 flex-col overflow-hidden">
    <header class="flex h-12 min-w-0 shrink-0 items-center overflow-hidden border-b" data-preview-header>
      <Sidebar.Trigger class="mx-2 shrink-0 md:hidden" data-mobile-opener />
      <ChannelHeader {header} />
    </header>
  </main>
</Sidebar.Provider>
`
);
await writeFile(
	path.join(preview, 'index.html'),
	'<!doctype html><html><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><script type="module" src="./main.js"></script></body></html>'
);
await writeFile(
	path.join(preview, 'main.js'),
	`
import { mount } from 'svelte';
import Preview from './preview.svelte';
import '../../src/app.css';
const params = new URLSearchParams(location.search);
document.documentElement.classList.toggle('dark', params.get('theme') !== 'light');
mount(Preview, { target: document.body, props: {
  open: params.get('open') !== 'false', language: params.has('no-language') ? null : 'English',
  tags: !params.has('no-tags'), long: params.has('long')
} });
`
);
const mocks = path.join(preview, 'mocks.js');
await build({
	configFile: false,
	root: preview,
	base: './',
	plugins: [svelte({ configFile: false }), tailwindcss()],
	resolve: {
		alias: [
			{ find: /^\$lib\/bindings(?:\.ts)?$/, replacement: mocks },
			...[
				'$app/state',
				'$app/navigation',
				'$app/paths',
				'$utils/log',
				'tauri-svelte-synced-store'
			].map((find) => ({ find, replacement: mocks })),
			{ find: '$lib', replacement: path.join(root, 'src/lib') },
			{ find: '$utils', replacement: path.join(root, 'utils/js') }
		]
	},
	build: { outDir: path.join(preview, 'dist'), emptyOutDir: true }
});
const dist = path.join(preview, 'dist');
const server = createServer(async (req, res) => {
	try {
		const url = new URL(req.url, 'http://localhost');
		const filename = path.resolve(
			dist,
			'.' + (url.pathname === '/' ? '/index.html' : url.pathname)
		);
		assert(filename.startsWith(dist + path.sep));
		res.setHeader(
			'Content-Type',
			{ '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css' }[
				path.extname(filename)
			] ?? 'application/octet-stream'
		);
		res.end(await readFile(filename));
	} catch {
		res.writeHead(404).end();
	}
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const browser = spawn(
	browserPath,
	[
		'--headless',
		'--disable-gpu',
		'--no-first-run',
		'--hide-scrollbars',
		'--remote-debugging-port=0',
		`--user-data-dir=${path.join(preview, 'browser-profile')}`,
		'about:blank'
	],
	{ windowsHide: true }
);
let socket;
let send;
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
try {
	const endpoint = await new Promise((resolve, reject) => {
		let logs = '';
		const timer = setTimeout(() => reject(new Error('Browser startup timed out')), 15000);
		browser.stderr.on('data', (chunk) => {
			logs += chunk;
			const match = logs.match(/DevTools listening on (ws:\/\/\S+)/);
			if (match) {
				clearTimeout(timer);
				resolve(match[1]);
			}
		});
		browser.on('error', (error) => {
			clearTimeout(timer);
			reject(error);
		});
		browser.on('exit', (code) => {
			clearTimeout(timer);
			reject(new Error(`Browser exited ${code}: ${logs}`));
		});
	});
	socket = new WebSocket(endpoint);
	await new Promise((resolve, reject) => {
		socket.onopen = resolve;
		socket.onerror = reject;
	});
	let sequence = 0;
	const pending = new Map();
	socket.onmessage = ({ data }) => {
		const message = JSON.parse(data);
		const request = pending.get(message.id);
		if (!request) return;
		clearTimeout(request.timer);
		pending.delete(message.id);
		if (message.error) request.reject(new Error(JSON.stringify(message.error)));
		else request.resolve(message.result);
	};
	send = (method, params = {}, sessionId) =>
		new Promise((resolve, reject) => {
			const id = ++sequence;
			const timer = setTimeout(() => {
				pending.delete(id);
				reject(new Error(`Timed out: ${method}`));
			}, 10000);
			pending.set(id, { resolve, reject, timer });
			socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
		});
	const evaluate = async (sessionId, expression) => {
		const result = await send(
			'Runtime.evaluate',
			{ expression, awaitPromise: true, returnByValue: true },
			sessionId
		);
		assert(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
		return result.result?.value;
	};
	const waitFor = async (sessionId, expression) => {
		for (let attempt = 0; attempt < 100; attempt++) {
			const result = await evaluate(sessionId, expression);
			if (result) return result;
			await delay(50);
		}
		const screenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
		await writeFile(path.join(preview, 'failure.png'), Buffer.from(screenshot.data, 'base64'));
		await writeFile(
			path.join(preview, 'failure.html'),
			await evaluate(sessionId, 'document.documentElement.outerHTML')
		);
		assert.fail(`Timed out waiting for ${expression}`);
	};
	const cases = [
		{ name: 'collapsed-dark', width: 1200, open: false },
		{ name: 'collapsed-light', width: 1200, open: false, theme: 'light' },
		{ name: 'expanded-dark', width: 1200 },
		{ name: 'expanded-light', width: 1200, theme: 'light' },
		{ name: 'fallback-tags', width: 1200, 'no-language': true },
		{ name: 'language-only', width: 1200, 'no-tags': true },
		{ name: 'no-metadata', width: 1200, 'no-language': true, 'no-tags': true },
		{ name: 'narrow-desktop', width: 820, long: true },
		{ name: 'mobile', width: 375, long: true },
		{ name: 'small-mobile', width: 320, long: true }
	];
	for (const test of cases) {
		const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
		const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
		await send(
			'Emulation.setDeviceMetricsOverride',
			{ width: test.width, height: 600, deviceScaleFactor: 1, mobile: test.width < 768 },
			sessionId
		);
		await send(
			'Page.navigate',
			{
				url: `http://127.0.0.1:${server.address().port}/?${new URLSearchParams(Object.entries(test).map(([key, value]) => [key, String(value)]))}`
			},
			sessionId
		);
		await waitFor(
			sessionId,
			'document.querySelector("[data-header-viewers]")?.textContent.trim() === "4,646"'
		);
		await delay(250);
		const layout = await evaluate(
			sessionId,
			`({
      overflow: document.documentElement.scrollWidth > innerWidth,
      height: document.querySelector('[data-preview-header]').getBoundingClientRect().height,
      pills: document.querySelector('[data-header-tags]')?.children.length ?? 0,
      label: document.querySelector('[data-header-tags]')?.textContent.trim(),
      trigger: !!document.querySelector('[data-header-tags-trigger]')
    })`
		);
		assert.equal(layout.overflow, false, test.name + ' horizontal overflow');
		assert.equal(layout.height, 48, test.name + ' header height');
		if (test.width === 1200) {
			assert.equal(
				layout.pills,
				test['no-language'] && test['no-tags'] ? 0 : 1,
				test.name + ' one pill'
			);
			assert.equal(
				layout.label,
				test['no-language'] ? (test['no-tags'] ? undefined : 'Tags') : 'English'
			);
			assert.equal(layout.trigger, !test['no-tags'], test.name + ' tooltip availability');
		}
		if (test.open === false) {
			const styles = [];
			for (const selector of [
				'[data-sidebar="header"] [data-sidebar="trigger"]',
				'[data-sidebar-home]',
				'[data-sidebar="header"] [data-popover-trigger]',
				'[data-sidebar="content"] [data-sidebar="menu-button"]',
				'[data-sidebar="footer"] [data-sidebar="menu-button"]'
			]) {
				assert(
					await evaluate(sessionId, `!!document.querySelector(${JSON.stringify(selector)})`),
					`Missing ${selector}`
				);
				const center = await evaluate(
					sessionId,
					`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`
				);
				await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...center }, sessionId);
				await delay(200);
				styles.push(
					await evaluate(
						sessionId,
						`(() => {
          const el = document.querySelector(${JSON.stringify(selector)}), r = el.getBoundingClientRect(), s = getComputedStyle(el);
          const rail = document.querySelector('[data-sidebar="sidebar"]').getBoundingClientRect();
          return { width: r.width, height: r.height, left: r.left, railWidth: rail.width,
            background: s.backgroundColor, color: s.color, radius: s.borderRadius };
        })()`
					)
				);
				const hoverScreenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
				await writeFile(
					path.join(preview, `${test.name}-hover-${styles.length}.png`),
					Buffer.from(hoverScreenshot.data, 'base64')
				);
			}
			for (const style of styles) {
				assert.equal(style.height, 40, test.name + ' 40px hover height');
				assert.equal(style.left, 0, test.name + ' full rail hover');
				assert.equal(style.width, style.railWidth, test.name + ' full rail hover width');
				assert.equal(style.background, styles[3].background, test.name + ' shared hover color');
				assert.notEqual(
					style.background,
					'rgba(0, 0, 0, 0)',
					test.name + ' visible hover background'
				);
				assert.equal(style.color, styles[3].color, test.name + ' shared hover foreground');
				assert.equal(style.radius, styles[3].radius, test.name + ' shared corner radius');
			}
			const centers = await evaluate(
				sessionId,
				`Array.from(document.querySelectorAll('[data-sidebar="header"] svg')).map(el => { const r = el.getBoundingClientRect(); return r.y + r.height / 2; })`
			);
			assert.deepEqual(
				centers.sort((a, b) => a - b),
				[24, 72, 120],
				test.name + ' collapse, Home, Search icon centers'
			);
			console.log(test.name + ' hover styles', JSON.stringify(styles));
		}
		if (test.width >= 768) {
			const controls = await evaluate(
				sessionId,
				`(() => {
          const rect = selector => {
            const r = document.querySelector(selector).getBoundingClientRect();
            return { x: r.x, y: r.y, width: r.width, height: r.height, right: r.right, bottom: r.bottom };
          };
          return {
            search: rect('[data-sidebar-search] input, [data-sidebar-search] [data-popover-trigger]'),
            home: rect('[data-sidebar-home]'),
            toggle: rect('[data-sidebar="header"] [data-sidebar="trigger"]'),
            header: rect('[data-sidebar="header"]'),
            sidebar: rect('[data-sidebar="panel"]')
          };
        })()`
			);
			assert.equal(
				controls.sidebar.width,
				test.open === false ? 48 : 256,
				test.name + ' sidebar width'
			);
			assert.equal(
				controls.header.height,
				test.open === false ? 144 : 48,
				test.name + ' controls height'
			);
			if (test.open === false) {
				assert(controls.toggle.bottom <= controls.home.y, test.name + ' collapse above Home');
				assert(controls.home.bottom <= controls.search.y, test.name + ' Home above Search');
			} else {
				assert(controls.search.right <= controls.home.x, test.name + ' Search before Home');
				assert(controls.home.right <= controls.toggle.x, test.name + ' Home before collapse');
				assert.equal(controls.search.y, controls.home.y, test.name + ' aligned Home');
				assert.equal(controls.home.y, controls.toggle.y, test.name + ' aligned collapse');
			}
			await send(
				'Input.dispatchMouseEvent',
				{
					type: 'mouseMoved',
					x: controls.home.x + controls.home.width / 2,
					y: controls.home.y + controls.home.height / 2
				},
				sessionId
			);
			const homeTooltip = `Array.from(document.querySelectorAll('[data-tooltip-content]')).some(el => el.textContent.trim() === 'Home' && el.dataset.state !== 'closed' && el.getBoundingClientRect().width > 0)`;
			await waitFor(sessionId, homeTooltip);
			await send(
				'Input.dispatchMouseEvent',
				{ type: 'mouseMoved', x: test.width - 5, y: 400 },
				sessionId
			);
			await evaluate(
				sessionId,
				`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
			);
			for (const type of ['keyDown', 'keyUp']) {
				await send(
					'Input.dispatchKeyEvent',
					{ type, key: 'Tab', code: 'Tab', modifiers: 8 },
					sessionId
				);
			}
			await waitFor(
				sessionId,
				`document.activeElement === document.querySelector('[data-sidebar-home]')`
			);
			await waitFor(sessionId, homeTooltip);
			assert.equal(
				await evaluate(sessionId, `getComputedStyle(document.activeElement).boxShadow !== 'none'`),
				true,
				test.name + ' Home keyboard focus ring'
			);
			await evaluate(sessionId, 'document.activeElement.blur()');

			if (test.open === false) {
				await evaluate(
					sessionId,
					`document.querySelector('[data-sidebar-search] [data-popover-trigger]').click()`
				);
				await waitFor(
					sessionId,
					`document.activeElement === document.querySelector('#search-floating')`
				);
				assert.equal(
					await evaluate(sessionId, `!!document.querySelector('[data-sidebar-home]')`),
					true,
					test.name + ' Home remains beside popover'
				);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[data-sidebar="header"]').getBoundingClientRect().height`
					),
					144
				);
				for (const type of ['keyDown', 'keyUp']) {
					await send('Input.dispatchKeyEvent', { type, key: 'Escape', code: 'Escape' }, sessionId);
				}
				await waitFor(sessionId, `!document.querySelector('#search-floating')`);
			} else {
				await evaluate(
					sessionId,
					`(() => {
          const input = document.querySelector('[data-sidebar-search] input');
          input.value = 'maya'; input.dispatchEvent(new Event('input', { bubbles: true })); input.focus();
        })()`
				);
				await waitFor(sessionId, `!document.querySelector('[data-sidebar-home]')`);
				await delay(250);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[data-sidebar-search] input').getBoundingClientRect().width`
					),
					controls.search.width + 40,
					test.name + ' focused input expands'
				);
				const focused = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
				await writeFile(
					path.join(preview, test.name + '-search-focused.png'),
					Buffer.from(focused.data, 'base64')
				);
				for (const type of ['keyDown', 'keyUp']) {
					await send('Input.dispatchKeyEvent', { type, key: 'Tab', code: 'Tab' }, sessionId);
				}
				await waitFor(sessionId, `document.activeElement?.textContent.trim() === 'Clear search'`);
				assert.equal(
					await evaluate(sessionId, `!!document.querySelector('[data-sidebar-home]')`),
					false,
					test.name + ' clear button keeps Home hidden'
				);
				await evaluate(sessionId, 'document.activeElement.click()');
				await waitFor(
					sessionId,
					`document.activeElement === document.querySelector('[data-sidebar-search] input') && document.activeElement.value === ''`
				);
				assert.equal(
					await evaluate(sessionId, `!!document.querySelector('[data-sidebar-home]')`),
					false,
					test.name + ' cleared input keeps focus layout'
				);
				await evaluate(
					sessionId,
					`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
				);
				await waitFor(sessionId, `!!document.querySelector('[data-sidebar-home]')`);
				await delay(250);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[data-sidebar-search] input').getBoundingClientRect().width`
					),
					controls.search.width,
					test.name + ' empty input shrinks on blur'
				);
				await evaluate(
					sessionId,
					`(() => {
          const input = document.querySelector('[data-sidebar-search] input');
          input.focus(); input.value = 'maya'; input.dispatchEvent(new Event('input', { bubbles: true }));
        })()`
				);
				await waitFor(sessionId, `!document.querySelector('[data-sidebar-home]')`);
				await evaluate(
					sessionId,
					`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
				);
				await waitFor(sessionId, `!!document.querySelector('[data-sidebar-home]')`);
				await delay(250);
				assert.equal(
					await evaluate(
						sessionId,
						`document.querySelector('[data-sidebar-search] input').getBoundingClientRect().width`
					),
					controls.search.width,
					test.name + ' filled input shrinks on blur'
				);
				assert.equal(
					await evaluate(sessionId, `document.querySelector('[data-sidebar-search] input').value`),
					'maya',
					test.name + ' blur preserves query'
				);
			}
		}
		if (layout.trigger) {
			const center = await evaluate(
				sessionId,
				`(() => { const r = document.querySelector('[data-header-tags-trigger]').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`
			);
			await send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...center }, sessionId);
			const tooltip = await waitFor(
				sessionId,
				`(() => {
        const el = document.querySelector('[data-header-tags-tooltip]'); if (!el?.getBoundingClientRect().width) return null;
        return { tags: Array.from(el.children).map(tag => tag.textContent), overflow: Array.from(el.children).some(tag => tag.scrollWidth > tag.clientWidth + 1) };
      })()`
			);
			assert.deepEqual(tooltip.tags, [
				'Hair',
				'DropsEnabled',
				'AnExceptionallyLongTagThatShouldRemainFullyReadable'
			]);
			assert.equal(tooltip.overflow, false, test.name + ' wrapped full tag text');
			await delay(200);
			const tagsScreenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
			await writeFile(
				path.join(preview, test.name + '-tags.png'),
				Buffer.from(tagsScreenshot.data, 'base64')
			);
			await send(
				'Input.dispatchMouseEvent',
				{ type: 'mouseMoved', x: test.width - 5, y: 400 },
				sessionId
			);
			await send(
				'Input.dispatchKeyEvent',
				{ type: 'keyDown', key: 'Escape', code: 'Escape' },
				sessionId
			);
		}
		if (test.name === 'expanded-dark') {
			const result = await evaluate(
				sessionId,
				`new Promise(resolve => {
        const input = document.querySelector('[data-sidebar="header"] input');
        const toggle = document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]');
        const rect = node => node.getBoundingClientRect();
        const aligned = rect(input).y === rect(toggle).y && rect(input).height === rect(toggle).height;
        input.value = 'maya'; input.dispatchEvent(new Event('input', { bubbles: true })); input.focus();
        toggle.click();
        setTimeout(() => {
          const collapsed = document.querySelector('[data-popover-trigger]');
          const collapsedFocus = document.activeElement === collapsed;
          toggle.click();
          setTimeout(() => resolve({ aligned, collapsedFocus,
            value: document.querySelector('[data-sidebar="header"] input').value,
            expandedFocus: document.activeElement === document.querySelector('[data-sidebar="header"] input')
          }), 300);
        }, 300);
      })`
			);
			assert.deepEqual(
				result,
				{ aligned: true, collapsedFocus: true, value: 'maya', expandedFocus: true },
				'search query and focus survive both transitions'
			);
			await evaluate(
				sessionId,
				`document.querySelector('[data-sidebar="header"] [data-sidebar="trigger"]').focus()`
			);
			await waitFor(sessionId, `!!document.querySelector('[data-sidebar-home]')`);
		}
		if (test.width < 768) {
			await evaluate(sessionId, `document.querySelector('[data-mobile-opener]').click()`);
			await waitFor(sessionId, `document.querySelector('[data-mobile="true"] input')`);
			assert.equal(
				await evaluate(sessionId, `document.querySelector('[data-mobile="true"] input').readOnly`),
				false,
				'editable mobile search'
			);
			assert.equal(
				await evaluate(sessionId, `!!document.querySelector('[data-popover-trigger]')`),
				false,
				'inline mobile search'
			);
		}
		await delay(200);
		const screenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
		await writeFile(path.join(preview, test.name + '.png'), Buffer.from(screenshot.data, 'base64'));
		console.log('PASS', test.name, JSON.stringify(layout));
		await send('Target.closeTarget', { targetId });
	}
	console.log(`Passed ${cases.length} sidebar/header browser scenarios. Screenshots: ${preview}`);
} finally {
	if (send) await send('Browser.close').catch(() => browser.kill());
	else browser.kill();
	socket?.close();
	await new Promise((resolve) => server.close(resolve));
}
