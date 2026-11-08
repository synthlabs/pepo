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

const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.resolve(root, 'target/network-ui-check');
assert(output.startsWith(path.resolve(root, 'target') + path.sep));
await mkdir(output, { recursive: true });
const browser = [
	process.env.BROWSER_BIN,
	'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
	'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
	'/usr/bin/chromium',
	'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
].find((candidate) => candidate && existsSync(candidate));
assert(browser, 'Set BROWSER_BIN to an installed Chromium browser.');
await writeFile(
	path.join(output, 'native.svelte.js'),
	`
export const params = new URLSearchParams(location.search);
export const page = $state({ url: new URL('/app/chat/maya', location.origin), params: { id: 'maya' } });
export const resolve = value => value;
export const goto = async value => { page.url = new URL(value, location.origin); page.params = value.includes('/chat/') ? { id: 'maya' } : {}; };
export const isTauriMobile = false;
export const native = window.networkFixture = { listeners: new Map(), retries: [], logs: [], snapshot: { revision: 0, issues: [] }, networkListeners: 0, snapshots: 0 };
window.__TAURI_INTERNALS__ = {};
export class SyncedState {
  obj = $state({}); ready = $state(true); initialized = Promise.resolve();
  constructor(key, fallback) {
    this.obj = key === 'auth_state' ? { phase: 'authorized', device_code: '', token: { user_id: '2', login: 'jerod', profile_image_url: '' } }
      : key === 'channel_cache' ? { channels: { maya: { broadcaster_id: '1', display_name: 'Maya', is_live: false } } } : structuredClone(fallback ?? {});
  }
  async sync() { return true; }
}
export const commands = {
  getNetworkStatus: () => { native.snapshots++; return native.snapshots === 1 ? new Promise((resolve) => { native.initial = resolve; }) : Promise.resolve(native.snapshot); },
  retryNetworkOperation: async (id) => { native.retries.push(id); },
  getChatEngineStatus: async () => ({ connected: true, joined: 1, eligible: 1, lingering: 0, pending: 0, limited: 0, failures: [] }),
  getFollowedChannels: async () => ({ status: 'ok', data: [{ id: '1', login: 'maya', display_name: 'Maya', profile_image_url: '' }] }),
  getChannelInfo: async () => ({ status: 'ok', data: { broadcaster_id: '1', broadcaster_login: 'maya', broadcaster_name: 'Maya' } }),
  logout: async () => ({ status: 'ok' })
};
export async function listen(name, callback) {
  native.listeners.set(name, callback); if (name === 'network_status') native.networkListeners++;
  return () => { native.listeners.delete(name); if (name === 'network_status') native.networkListeners--; };
}
export async function invoke(command) { if (command === 'plugin:inbound|captured_errors') return []; if (command === 'plugin:inbound|capabilities') return { has_config: false }; return null; }
export const error = async (message, options) => { native.logs.push({ message, options }); };
export const warn = async () => {};
export const info = async () => {};
export const debug = async () => {};
export const trace = async () => {};
export default { info, warn, error, debug, trace };
`
);
await writeFile(
	path.join(output, 'settings.svelte.js'),
	`
import { DEFAULT_APP_SETTINGS } from '${path.join(root, 'src/lib/settings.ts').replaceAll('\\', '/')}';
import { applySettingsUpdate } from '${path.join(root, 'src/lib/settings/autosave.ts').replaceAll('\\', '/')}';
const value = structuredClone(DEFAULT_APP_SETTINGS);
const params = new URLSearchParams(location.search);
value.layout.sidebar_open = params.get('open') !== 'false';
value.appearance.theme = params.get('theme') ?? 'dark';
value.providers.http_connect_timeout_secs = 5;
value.providers.http_request_timeout_secs = 15;
export const settingsState = $state({ value, ready: true, saving: false, result: 'idle' });
export const getNormalizedAppSettings = () => settingsState.value;
export async function updateAppSettings(update) { settingsState.value = applySettingsUpdate(settingsState.value, update); return true; }
`
);
await writeFile(path.join(output, 'noop.svelte'), '');
await writeFile(
	path.join(output, 'internal.js'),
	"export { default as InternalRoot } from './noop.svelte';"
);
await writeFile(
	path.join(output, 'Fixture.svelte'),
	`
<script>
  import { onMount, tick } from 'svelte';
  import Layout from '${path.join(root, 'src/routes/app/+layout.svelte').replaceAll('\\', '/')}';
  import NetworkStatus from '$lib/components/network-status.svelte';
  import SettingsPage from '$lib/components/settings/settings-page.svelte';
  import { applyThemePreference } from '$lib/settings';
  import { appErrors } from '$lib/errors/controller';
  import { native, page, goto, params } from './native.svelte.js';
  import { settingsState } from './settings.svelte.js';
  let verdict = $state('running');
  let details = $state('');
  let mounted = $state(true);
  let login = $state(false);
  $effect(() => applyThemePreference(settingsState.value.appearance.theme));
  const pause = (ms = 35) => new Promise((resolve) => setTimeout(resolve, ms));
  function check(condition, message) { if (!condition) throw new Error(message); }
  function button(text) { const target = [...document.querySelectorAll('button,[role="menuitem"]')].find((node) => node.textContent.trim() === text); check(target, 'Missing action: ' + text); return target; }
  async function click(text) { button(text).click(); await pause(); }
  let revision = 0;
  function emit(issues) { native.snapshot = { revision: ++revision, issues }; native.listeners.get('network_status')?.({ payload: native.snapshot }); }
  const avatar = () => document.querySelector('[data-account-menu-trigger]');
  const drawer = () => document.querySelector('[data-network-drawer]');
  const tooltip = () => [...document.querySelectorAll('[data-network-tooltip]')].find((node) => node.closest('[data-tooltip-content]')?.getAttribute('data-state') !== 'closed');
  function rgb(css) { const canvas = document.createElement('canvas'); const context = canvas.getContext('2d'); context.fillStyle = css; context.fillRect(0, 0, 1, 1); return [...context.getImageData(0, 0, 1, 1).data]; }
  function backdrop(node) { for (; node; node = node.parentElement) { const color = rgb(getComputedStyle(node).backgroundColor); if (color[3] === 255) return color; } return rgb('white'); }
  function contrast(left, right) { const luminance = color => color.slice(0, 3).map(value => value / 255).map(value => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4).reduce((total, value, i) => total + value * [0.2126, 0.7152, 0.0722][i], 0); const values = [luminance(left), luminance(right)].sort((a, b) => b - a); return (values[0] + 0.05) / (values[1] + 0.05); }
  async function closeDrawer() { await click('Close'); await pause(350); }
  onMount(() => { void (async () => {
    for (let i = 0; !native.initial && i < 40; i++) await pause();
    check(native.initial, 'Native status did not connect');
    check(native.networkListeners === 1, 'More than one native subscription');
    check(!document.querySelector('[data-network-avatar-status]'), 'Healthy avatar showed an indicator');
    const issue = { id: 'FFZ emotes:timeout', feature: 'FFZ emotes', category: 'timeout', message: 'A slow request timed out.', failures: 17, affected: 17, cached: true, last_failure_at: Date.now(), retry_at: Date.now() + 15000, retryable: true, core: false, can_retry: true };
    const seven = { ...issue, id: '7tv:timeout', feature: '7TV emotes' };
    const bttv = { ...issue, id: 'bttv:timeout', feature: 'BTTV emotes' };
    for (let i = 1; i <= 17; i++) emit([{ ...issue, failures: i, affected: i }]);
    native.initial({ revision: 0, issues: [] }); await pause();
    check(document.querySelectorAll('[data-network-avatar-status]').length === 1, 'Startup burst did not produce one avatar indicator');
    const dot = document.querySelector('[data-network-avatar-status]');
    check(contrast(rgb(getComputedStyle(dot).backgroundColor), backdrop(dot.parentElement)) >= 3, 'Avatar indicator has insufficient contrast');
    check(!document.querySelector('header [data-network-status]'), 'Old header button remained');
    check(!document.querySelector('[data-network-header-status]'), 'Chat route showed a Settings icon');
    check(appErrors.snapshot().notifications.length === 0, 'A transient failure produced a popup');
    emit([seven, bttv, { ...seven, id: '7tv:connection' }]); await pause();
    avatar().focus(); await pause(250);
    check(tooltip(), 'Avatar focus did not show the tooltip');
    check(tooltip().textContent.includes('Some features are still refreshing in the background:'), 'Tooltip heading changed');
    check([...tooltip().querySelectorAll('li')].map((node) => node.textContent).join('|') === '7TV emotes|BTTV emotes', 'Feature list was duplicated or out of order');
    avatar().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await pause(200);
    check(!tooltip(), 'Escape did not dismiss the tooltip');
    avatar().blur(); avatar().dispatchEvent(new PointerEvent('pointerenter', { pointerType: 'mouse' })); await pause(250);
    check(tooltip(), 'Avatar hover did not show the tooltip');
    avatar().click(); await pause(200);
    check(document.querySelector('[role="menu"]'), 'Account avatar stopped opening its menu');
    check(avatar().getAttribute('data-state') === 'open', 'Account menu lost its open-state styling');
    check(!tooltip() && !drawer(), 'Account menu opened status UI');
    await click('Settings'); await pause(250);
    check(page.url.pathname === '/app/settings', 'Settings navigation failed');
    check(!document.querySelector('[data-network-avatar-status]'), 'Settings retained the avatar indicator');
    const icon = document.querySelector('[data-network-header-status]');
    check(icon && document.querySelector('header h1').parentElement.contains(icon), 'Settings status icon was outside the left title group');
    check(contrast(rgb(getComputedStyle(icon.querySelector('svg')).color), backdrop(icon)) >= 3, 'Settings status icon has insufficient contrast');
    icon.focus(); await pause(250); check(tooltip(), 'Settings focus did not show tooltip');
    icon.click(); await pause(); check(!drawer(), 'Settings status icon opened the drawer');
    emit([bttv, seven, { ...seven, id: '7tv:unavailable', retryable: false }, { ...issue, id: 'chat', feature: 'Chat', core: true }]);
    button('Background activity').focus(); await pause(); icon.focus(); await pause(250);
    check(tooltip().textContent.includes('Reconnecting:') && tooltip().textContent.includes('Some features need attention:'), 'Urgent states were labeled as refreshing');
    check([...tooltip().querySelectorAll('li')].filter((node) => node.textContent === '7TV emotes').length === 1, 'Mixed severity duplicated a feature');
    check(icon.querySelector('.text-destructive'), 'Urgent status had no distinct icon');
    emit([issue]); await pause();
    await click('Background activity'); await pause(250);
    check(drawer(), 'Chat settings did not open the drawer');
    check(document.querySelectorAll('[data-network-issues] > li').length === 1, 'Failure group was duplicated');
    check(drawer().textContent.includes('Using saved data'), 'Cache fallback was not explained');
    await click('Retry now'); check(native.retries[0] === issue.id, 'Retry targeted the wrong incident');
    await click('Report problem');
    const frozen = appErrors.snapshot().report;
    check(frozen?.error?.message.includes('17'), 'Report lost the selected failure counts');
    emit([]); await pause();
    check(!document.querySelector('[data-network-header-status]'), 'Recovered icon remained visible');
    check(appErrors.snapshot().report.id === frozen.id, 'Recovery changed the selected report');
    appErrors.closeReport(); await pause(350);
    await click('Background activity'); check(drawer().textContent.includes('Everything has caught up.'), 'Healthy drawer button did not work');
    await closeDrawer();
    check(document.activeElement === button('Background activity'), 'Drawer did not restore focus');
    await click('App'); await click('Advanced');
    const input = document.querySelector('[id="setting-providers-http_request_timeout_secs"]');
    check(input?.value === '15', 'Saved provider timeout was overwritten');
    input.value = '45'; input.dispatchEvent(new Event('input', { bubbles: true })); input.dispatchEvent(new FocusEvent('blur', { bubbles: true })); await pause();
    check(settingsState.value.providers.http_request_timeout_secs === 45, 'Advanced timeout did not save');
    check(settingsState.value.network.request_timeout_secs === 30, 'Provider override changed the API timeout');
    await goto('/app/chat/maya'); emit([seven, bttv]); await pause();
    emit([bttv]); await pause(); check(document.querySelector('[data-network-avatar-status]'), 'Partial recovery hid remaining issues');
    check(native.networkListeners === 1 && native.snapshots === 1, 'Navigation duplicated native status work');
    mounted = false; await tick(); await pause(); check(native.networkListeners === 0, 'Unmount leaked the status subscription');
    login = true; await tick(); await pause();
    const loginButton = document.querySelector('[data-network-status]');
    check(loginButton, 'Standalone login status button disappeared'); loginButton.click(); await pause();
    check(drawer(), 'Standalone login button did not open the drawer'); await closeDrawer();
    check(document.activeElement === loginButton, 'Standalone drawer lost its focus return');
    login = false; await tick(); await pause(); check(native.networkListeners === 0, 'Login status leaked its subscription');
    mounted = true; await tick(); await pause();
    check(native.networkListeners === 1 && native.snapshots === 3, 'Remount did not establish exactly one subscription');
    emit([seven, bttv]); await pause();
    const view = params.get('view') ?? 'chat';
    if (view !== 'chat') { await goto('/app/settings'); await pause(); }
    if (view === 'drawer') { await click('Background activity'); await pause(600); }
    else { (view === 'chat' ? avatar() : document.querySelector('[data-network-header-status]')).focus(); await pause(250); check(tooltip(), 'Final tooltip was unavailable'); }
    check(document.documentElement.scrollWidth <= window.innerWidth, 'Status UI overflowed the desktop width');
    verdict = 'passed'; details = 'Avatar/menu, settings tooltip, shared drawer, recovery, frozen report, subscription cleanup and network settings passed.';
  })().catch((error) => { verdict = 'failed'; details = error.stack ?? String(error); }); });
</script>
{#if mounted}<Layout>{#if page.url.pathname === '/app/settings'}<SettingsPage />{:else}<p class="p-4 text-sm">Chat remains available while emotes refresh.</p>{/if}</Layout>{/if}
{#if login}<NetworkStatus />{/if}
<output data-network-check={verdict} class="sr-only">{details}</output>
`
);
await writeFile(
	path.join(output, 'entry.js'),
	`import { mount } from 'svelte'; import './native.svelte.js'; import Fixture from './Fixture.svelte'; import '${path.join(root, 'src/app.css').replaceAll('\\', '/')}'; mount(Fixture, { target: document.getElementById('app') });`
);
await writeFile(
	path.join(output, 'index.html'),
	'<!doctype html><html class="dark"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="app" style="display:flex;height:100vh;min-width:0;flex-direction:column"></div><script type="module" src="/entry.js"></script></body></html>'
);
const native = path.join(output, 'native.svelte.js');
await build({
	root: output,
	configFile: false,
	logLevel: 'warn',
	plugins: [svelte(), tailwindcss()],
	resolve: {
		alias: [
			{ find: '$lib/bindings.ts', replacement: native },
			{ find: '$lib/bindings', replacement: native },
			{ find: '$lib/tauri', replacement: native },
			{ find: '$internal', replacement: path.join(output, 'internal.js') },
			...['$app/state', '$app/paths', '$app/navigation', 'tauri-svelte-synced-store'].map(
				(find) => ({ find, replacement: native })
			),
			{ find: '$lib/stores/settings.svelte', replacement: path.join(output, 'settings.svelte.js') },
			{ find: '$utils/log', replacement: native },
			...['@tauri-apps/api/event', '@tauri-apps/api/core', '@tauri-apps/plugin-log'].map(
				(find) => ({ find, replacement: native })
			),
			{ find: '$lib', replacement: path.join(root, 'src/lib') },
			{ find: '$utils', replacement: path.join(root, 'utils/js') }
		]
	},
	build: { outDir: path.join(output, 'dist'), emptyOutDir: true }
});
const dist = path.join(output, 'dist');
const server = createServer(async (request, response) => {
	try {
		const filename = path.resolve(dist, '.' + new URL(request.url, 'http://localhost').pathname);
		assert(filename.startsWith(dist + path.sep) || filename === dist);
		const target = filename === dist ? path.join(dist, 'index.html') : filename;
		const extension = path.extname(target);
		response.setHeader(
			'content-type',
			{
				'.html': 'text/html',
				'.js': 'text/javascript',
				'.css': 'text/css',
				'.woff2': 'font/woff2'
			}[extension] ?? 'application/octet-stream'
		);
		response.end(await readFile(target));
	} catch {
		response.statusCode = 404;
		response.end();
	}
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
try {
	for (const { width, theme, open, view } of [
		...[1280, 800].flatMap((width) =>
			['dark', 'light'].flatMap((theme) =>
				[true, false].map((open) => ({ width, theme, open, view: open ? 'settings' : 'chat' }))
			)
		),
		{ width: 1280, theme: 'dark', open: true, view: 'drawer' },
		{ width: 800, theme: 'light', open: false, view: 'drawer' }
	]) {
		const screenshot = path.join(
			output,
			`network-${width}-${theme}-${open ? 'expanded' : 'collapsed'}-${view}.png`
		);
		const child = spawn(
			browser,
			[
				'--headless=new',
				'--disable-gpu',
				'--no-first-run',
				'--disable-background-networking',
				'--disable-extensions',
				'--no-proxy-server',
				`--user-data-dir=${path.join(output, `profile-${width}-${Date.now()}`)}`,
				`--window-size=${width},800`,
				'--virtual-time-budget=15000',
				'--dump-dom',
				`--screenshot=${screenshot}`,
				`http://127.0.0.1:${server.address().port}/index.html?theme=${theme}&open=${open}&view=${view}`
			],
			{ windowsHide: true }
		);
		let stdout = '';
		let stderr = '';
		child.stdout.on('data', (bytes) => {
			stdout += bytes;
		});
		child.stderr.on('data', (bytes) => {
			stderr += bytes;
		});
		const timeout = setTimeout(() => child.kill(), 45000);
		const code = await new Promise((resolve, reject) => {
			child.on('error', reject);
			child.on('exit', resolve);
		});
		clearTimeout(timeout);
		assert.equal(code, 0, stderr.slice(-2000));
		await writeFile(screenshot.replace('.png', '.html'), stdout);
		assert(
			stdout.includes('data-network-check="passed"'),
			stdout.match(/<output[\s\S]*?<\/output>/)?.[0] ?? stderr.slice(-2000)
		);
		console.log(
			`Network UI passed at ${width}px, ${theme}, ${open ? 'expanded' : 'collapsed'}, ${view}; screenshot: ${screenshot}`
		);
	}
} finally {
	server.close();
}
