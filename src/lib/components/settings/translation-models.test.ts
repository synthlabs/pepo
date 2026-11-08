import { flushSync, mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import TranslationModels from './translation-models.svelte';
import TranslationConfirmation from './translation-confirmation.svelte';
import { TranslationSession } from '$lib/translation/session.svelte';
import { EN_RU, fixture, installation, local, RU_EN, settle } from '$lib/translation/test-fixtures';

const capture = vi.hoisted(() => vi.fn());
vi.mock('$lib/errors/controller', () => ({ appErrors: { capture } }));
const sessions: TranslationSession[] = [];
const components: ReturnType<typeof mount>[] = [];
beforeEach(() => {
	vi.spyOn(console, 'warn').mockImplementation((...args) => {
		throw new Error(args.map(String).join(' '));
	});
});
async function flushUi() {
	await settle();
	flushSync();
	await tick();
	await settle();
	await new Promise((resolve) => setTimeout(resolve, 5));
	flushSync();
}
function element<T extends HTMLElement = HTMLElement>(selector: string): T {
	const found = document.querySelector<T>(selector);
	expect(found, selector).not.toBeNull();
	return found!;
}
async function click(selector: string) {
	element(selector).click();
	await flushUi();
}
async function setup(installed = false) {
	const f = fixture();
	if (installed) f.rows([local()]);
	const session = new TranslationSession(f.client);
	sessions.push(session);
	await session.connect();
	await flushUi();
	const target = document.createElement('div');
	document.body.append(target);
	components.push(mount(TranslationModels, { target, props: { session, target: 'eng' } }));
	components.push(mount(TranslationConfirmation, { target, props: { session } }));
	await flushUi();
	return { ...f, session, target };
}
afterEach(async () => {
	for (const component of components.splice(0)) await unmount(component);
	for (const session of sessions.splice(0)) await session.dispose();
	await new Promise((resolve) => setTimeout(resolve, 50));
	document.body.replaceChildren();
	vi.restoreAllMocks();
	vi.clearAllMocks();
});

describe('Pepo translation model presentation', () => {
	it('shows both directions, installed status and accessible shared actions without verifying', async () => {
		const f = await setup(true);
		expect(document.body.textContent).toContain('Russian → English');
		expect(document.body.textContent).toContain('English → Russian');
		expect(element('[data-model-row="rus:eng"]').textContent).toContain('not verified');
		expect(f.client.verifyModel).not.toHaveBeenCalled();
		await click('[data-model-row="rus:eng"] [data-model-action="verify"]');
		expect(f.client.verifyModel).toHaveBeenCalledWith(RU_EN);
	});
	it('shows download sizes, keeps the pinned plan, and declines cancelled consent', async () => {
		const f = await setup();
		await click('[data-model-row="rus:eng"] [data-model-action="install"]');
		expect(element('[role="alertdialog"]').textContent).toContain('Download Russian → English?');
		expect(
			f.session.confirmation &&
				'plan' in f.session.confirmation &&
				f.session.confirmation.plan.catalogSha256
		).toBe('a'.repeat(64));
		expect(element('[role="alertdialog"]').textContent).not.toContain('a'.repeat(64));
		expect(element('[data-reviewed-model]').textContent).toContain('1.0 KiB');
		const buttons = [
			...document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')
		];
		buttons.find((button) => button.textContent === 'Cancel')!.click();
		await flushUi();
		expect(f.client.startInstall).not.toHaveBeenCalled();
		expect(f.session.confirmation).toBeNull();
	});
	it('accepts download consent and preserves compact progress across panel navigation', async () => {
		const f = await setup();
		await click('[data-model-row="rus:eng"] [data-model-action="install"]');
		[...document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')]
			.find((button) => button.textContent === 'Download')!
			.click();
		await flushUi();
		expect(f.client.startInstall).toHaveBeenCalledTimes(1);
		expect(element<HTMLDetailsElement>('[data-model-progress-details]').open).toBe(false);
		expect(element<HTMLProgressElement>('progress').value).toBe(25);
		await unmount(components.shift()!);
		expect(f.watchers.size).toBe(1);
		expect(f.client.cancelInstall).not.toHaveBeenCalled();
		components.push(
			mount(TranslationModels, { target: f.target, props: { session: f.session, target: 'rus' } })
		);
		await flushUi();
		expect(element('[data-model-installation]').textContent).toContain('Russian → English');
		expect(document.querySelectorAll('[data-model-row]')).toHaveLength(2);
	});
	it('waits for terminal cancellation and reports installation failure inline and globally', async () => {
		const f = await setup();
		f.emit({ installation: installation() });
		await flushUi();
		await click('[data-model-action="cancel"]');
		expect(f.client.cancelInstall).toHaveBeenCalledTimes(1);
		expect(element('[data-model-action="cancel"]').textContent).toBe('Cancelling…');
		f.finish({ code: 'cancelled', message: 'cancelled' });
		await flushUi();
		expect(element('[data-model-installation]').textContent).toContain('Installation cancelled');
		expect(capture).not.toHaveBeenCalled();
		f.emit({ installation: { ...installation(), id: 'install-2' } });
		f.finish({ code: 'http', message: 'Download failed' });
		await flushUi();
		expect(element('[role="alert"]').textContent).toContain('Download failed');
		expect(capture).toHaveBeenCalledTimes(1);
	});
	it('does not claim completion from a phase and retains cleanup warnings from a terminal result', async () => {
		const f = await setup();
		const operation = installation();
		operation.progress.phase = 'completed';
		f.emit({ installation: operation });
		await flushUi();
		expect(element('[data-model-installation]').textContent).toContain(
			'Waiting for the installation result'
		);
		f.finish(null, ['Temporary file remains']);
		await flushUi();
		expect(element('[data-model-installation]').textContent).toContain('Installation complete');
		expect(element('[data-model-installation]').textContent).toContain('Temporary file remains');
	});
	it('confirms removal and download cleanup and keeps unmanaged data read-only', async () => {
		const f = await setup(true);
		f.emit({
			workload: { ...f.state().workload!, residentModels: [{ pair: RU_EN, modelId: 'loaded' }] }
		});
		await flushUi();
		await click('[data-model-row="rus:eng"] [data-model-action="remove"]');
		expect(element('[role="alertdialog"]').textContent).toContain(
			'Queued translations will be cancelled'
		);
		f.session.respond(true);
		await flushUi();
		expect(f.client.release).toHaveBeenCalledTimes(1);
		expect(f.client.removeModel).toHaveBeenCalledWith(RU_EN);
		expect(f.client.resume).toHaveBeenCalledTimes(1);
		await click('[data-model-row="eng:rus"] [data-model-more]');
		await click('[data-model-action="clear_downloads"]');
		expect(element('[role="alertdialog"]').textContent?.replace(/\s+/g, ' ')).toContain(
			'installed model will be kept'
		);
		f.session.respond(true);
		await flushUi();
		expect(f.client.discardDownloads).toHaveBeenCalledWith(EN_RU);
		f.rows([{ ...local(RU_EN, 'unmanaged'), pair: null }]);
		await f.session.controller.refreshModels();
		await flushUi();
		expect(element('[data-model-row="models/rus-eng"]').querySelectorAll('button')).toHaveLength(0);
	});
});
