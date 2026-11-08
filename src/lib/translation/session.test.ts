import { afterEach, describe, expect, it, vi } from 'vitest';
import { TranslationSession } from './session.svelte';
import { EN_RU, fixture, local, RU_EN, settle } from './test-fixtures';
import { targetModelStatus } from './status';

const capture = vi.hoisted(() => vi.fn());
vi.mock('$lib/errors/controller', () => ({ appErrors: { capture } }));
afterEach(() => vi.clearAllMocks());

describe('translation session', () => {
	it('asks consent for the selected target and waits for a terminal successful installation', async () => {
		const f = fixture();
		const session = new TranslationSession(f.client);
		let finished = false;
		const preparing = session.prepareTarget('rus', 'language').then((result) => {
			finished = true;
			return result;
		});
		await settle();
		expect(session.confirmation?.pair).toEqual(EN_RU);
		expect(session.setupIntent).toBe('language');
		expect(f.client.startInstall).not.toHaveBeenCalled();
		session.respond(true);
		await settle();
		expect(finished).toBe(false);
		const operation = f.state().installation!;
		operation.progress.phase = 'completed';
		f.emit({ installation: operation });
		await settle();
		expect(finished).toBe(false);
		f.finish();
		expect(await preparing).toBe(true);
		expect(session.setupTarget).toBeNull();
		await session.dispose();
	});
	it('declines consent without starting a download and enables installed models without a prompt', async () => {
		const f = fixture();
		const session = new TranslationSession(f.client);
		const preparing = session.prepareTarget('eng', 'enable');
		await settle();
		session.respond(false);
		expect(await preparing).toBe(false);
		expect(f.client.startInstall).not.toHaveBeenCalled();
		expect(session.setupError).toBe('');
		f.rows([local()]);
		expect(await session.prepareTarget('eng', 'enable')).toBe(true);
		expect(session.confirmation).toBeNull();
		expect(f.client.planInstall).toHaveBeenCalledTimes(1);
		expect(f.client.verifyModel).not.toHaveBeenCalled();
		await session.dispose();
	});
	it.each(['http', 'cancelled'] as const)(
		'does not apply preferences after %s installation failure',
		async (code) => {
			const f = fixture();
			const session = new TranslationSession(f.client);
			const preparing = session.prepareTarget('eng', 'enable');
			await settle();
			session.respond(true);
			await settle();
			f.finish({ code, message: 'Download ended' });
			expect(await preparing).toBe(false);
			expect(Boolean(session.setupError)).toBe(code !== 'cancelled');
			await session.dispose();
		}
	);
	it('abandons preference changes on reset or teardown without cancelling an accepted download', async () => {
		const f = fixture();
		const session = new TranslationSession(f.client);
		const preparing = session.prepareTarget('eng', 'enable');
		await settle();
		session.respond(true);
		await settle();
		session.abandonSetup();
		f.finish();
		expect(await preparing).toBe(false);
		expect(f.client.cancelInstall).not.toHaveBeenCalled();
		const second = session.prepareTarget('rus', 'language');
		await settle();
		session.respond(true);
		await settle();
		await session.dispose();
		expect(await second).toBe(false);
	});
	it('requires explicit advanced repair for damaged models and reports unsupported targets', async () => {
		const f = fixture();
		const session = new TranslationSession(f.client);
		f.rows([local(RU_EN, 'unmanaged')]);
		expect(await session.prepareTarget('eng', 'enable')).toBe(false);
		expect(session.setupError).toContain('needs attention');
		expect(f.client.planInstall).not.toHaveBeenCalled();
		f.rows([]);
		expect(await session.prepareTarget('fra', 'enable')).toBe(false);
		expect(session.setupError).toContain('No translation model');
		await session.dispose();
	});
	it('polls a terminal result if the completion event is missed', async () => {
		vi.useFakeTimers();
		const f = fixture();
		const session = new TranslationSession(f.client);
		try {
			const preparing = session.prepareTarget('eng', 'enable');
			await settle();
			session.respond(true);
			await settle();
			const latest = f.state();
			latest.revision = String(BigInt(latest.revision) + 1n);
			latest.installation!.finished = true;
			latest.installation!.result = {
				status: 'installed',
				model: latest.installation!.model,
				cleanupWarnings: []
			};
			f.client.status.mockResolvedValue(latest);
			await vi.advanceTimersByTimeAsync(1000);
			expect(await preparing).toBe(true);
		} finally {
			await session.dispose();
			vi.useRealTimers();
		}
	});
	it('waits for ready and enabled settings, connects once, and retains the session after disabling', async () => {
		const f = fixture();
		const session = new TranslationSession(f.client);
		session.start(false, true);
		session.start(true, false);
		expect(f.client.watchState).not.toHaveBeenCalled();
		session.start(true, true);
		await settle();
		session.start(true, false);
		session.start(true, true);
		expect(f.client.watchState).toHaveBeenCalledTimes(1);
		expect(f.watchers.size).toBe(1);
		await session.dispose();
		expect(f.watchers.size).toBe(0);
		expect(f.client.dispose).toHaveBeenCalledTimes(1);
	});
	it('allows connecting while disabled and retries IPC failure explicitly', async () => {
		const f = fixture();
		f.client.watchState.mockRejectedValueOnce(new Error('offline'));
		const session = new TranslationSession(f.client);
		await session.connect();
		expect(session.snapshot.connected).toBe(false);
		expect(capture).toHaveBeenCalledTimes(1);
		await session.connect();
		expect(session.snapshot.connected).toBe(true);
		await session.dispose();
	});
	it('declines pending consent on teardown and keeps the client alive through an accepted removal', async () => {
		const f = fixture();
		const session = new TranslationSession(f.client);
		await session.connect();
		await settle();
		const pending = session.controller.install(RU_EN);
		await settle();
		expect(session.confirmation?.action).toBe('install');
		await session.dispose();
		await pending;
		expect(f.client.startInstall).not.toHaveBeenCalled();

		const g = fixture();
		g.rows([local()]);
		let finish!: (removed: boolean) => void;
		g.client.removeModel.mockImplementation(
			() =>
				new Promise<boolean>((resolve) => {
					finish = resolve;
				})
		);
		const running = new TranslationSession(g.client);
		await running.connect();
		await settle();
		const removing = running.controller.remove(RU_EN);
		await settle();
		running.respond(true);
		await settle();
		const closing = running.dispose();
		expect(g.client.dispose).not.toHaveBeenCalled();
		finish(true);
		await removing;
		await closing;
		expect(g.client.dispose).toHaveBeenCalledTimes(1);
	});
	it('reports terminal installation failure once through the global error host', async () => {
		const f = fixture();
		const session = new TranslationSession(f.client);
		await session.connect();
		await settle();
		const installing = session.controller.install(RU_EN);
		await settle();
		session.respond(true);
		await installing;
		f.finish({ code: 'http', message: 'Download failed' });
		f.emit({});
		await settle();
		expect(session.snapshot.errors[0].error.message).toBe('Download failed');
		expect(capture).toHaveBeenCalledTimes(1);
		await session.dispose();
	});
	it('shows target-specific installation status without claiming verification', async () => {
		const f = fixture();
		f.rows([local()]);
		const session = new TranslationSession(f.client);
		expect(targetModelStatus(session.snapshot, 'eng')).toContain('Connect');
		await session.connect();
		await settle();
		expect(targetModelStatus(session.snapshot, 'eng')).toContain('has not been verified');
		expect(targetModelStatus(session.snapshot, 'rus')).toContain('No model');
		expect(f.client.verifyModel).not.toHaveBeenCalled();
		await session.dispose();
	});
});
