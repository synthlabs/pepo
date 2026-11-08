const models = ['rus:eng', 'eng:rus'].map((key) => {
	const [source, target] = key.split(':');
	return {
		id: `${source}-${target}-v1`,
		pair: { source, target },
		version: '1',
		architecture: 'fixture',
		catalogSha256: 'a'.repeat(64),
		downloadBytes: '41943040',
		installedBytes: '83886080'
	};
});
let entries = [];
let installationSequence = 0;
let state = {
	revision: '1',
	modelsRevision: '0',
	phase: 'ready',
	error: null,
	catalog: {
		refreshing: false,
		freshness: 'fresh',
		error: null,
		catalog: {
			sha256: 'a'.repeat(64),
			models,
			licenseUrl: 'https://example.invalid/license',
			licenseSha256: 'b'.repeat(64),
			recordsUrl: 'https://example.invalid/records',
			recordsSha256: 'c'.repeat(64),
			retrievedAt: '2026-11-07T00:00:00Z'
		}
	},
	installation: null,
	recovery: { rolledBack: [], cleanedUp: [], warnings: [], installerBusy: false },
	workload: { queuedRequests: 0, runningRequests: 0, cachedResults: 0, residentModels: [] }
};
const listeners = new Set();
const calls = [];
function emit(patch) {
	state = { ...state, ...patch, revision: String(BigInt(state.revision) + 1n) };
	for (const callback of listeners) callback({ payload: structuredClone(state) });
}
window.translationNative = {
	calls,
	emit,
	state: () => structuredClone(state),
	finish(error = null) {
		const operation = state.installation;
		if (!error)
			entries = [
				...entries.filter((entry) => entry.pair.source !== operation.model.pair.source || entry.pair.target !== operation.model.pair.target),
				{
					directory: `models/${operation.model.pair.source}-${operation.model.pair.target}`,
					pair: operation.model.pair,
					state: 'present_unverified',
					installed: operation.model,
					matchesCurrentCatalog: true,
					detail: null
				}
			];
		emit({
			modelsRevision: String(BigInt(state.modelsRevision) + 1n),
			installation: {
				...operation,
				finished: true,
				result: error ? null : { status: 'installed', model: operation.model, cleanupWarnings: [] },
				error
			}
		});
	}
};
export async function listen(event, callback) {
	if (event === 'translate://state-changed') listeners.add(callback);
	return () => listeners.delete(callback);
}
export async function invoke(command, args = {}) {
	const action = command.replace('plugin:translate|', '');
	calls.push(action);
	switch (action) {
		case 'status':
			return structuredClone(state);
		case 'inventory':
			return structuredClone(entries);
		case 'refresh_catalog':
			return structuredClone(state.catalog);
		case 'retry_initialize':
			emit({ phase: 'ready' });
			return structuredClone(state);
		case 'plan_install': {
			const model = models.find(
				(model) => model.pair.source === args.pair.source && model.pair.target === args.pair.target
			);
			return structuredClone({
				pair: model.pair,
				modelId: model.id,
				catalogSha256: model.catalogSha256,
				model
			});
		}
		case 'start_install': {
			const model = models.find((model) => model.id === args.plan.modelId);
			const operation = {
				id: `install-${++installationSequence}`,
				model,
				finished: false,
				result: null,
				error: null,
				progress: {
					phase: 'downloading',
					filename: 'model.bin',
					fileIndex: 1,
					fileCount: 3,
					attempt: 1,
					maxAttempts: 3,
					fileDownloadBytes: '10485760',
					fileDownloadTotal: model.downloadBytes,
					completedDownloadBytes: '0',
					downloadTotal: model.downloadBytes,
					fileInstalledBytes: '0',
					fileInstalledTotal: model.installedBytes,
					completedInstalledBytes: '0',
					installedTotal: model.installedBytes,
					reusedBytes: '0',
					networkBytes: '10485760',
					detail: null
				}
			};
			emit({ installation: operation });
			return structuredClone(operation);
		}
		case 'installation':
			return structuredClone(state.installation);
		case 'cancel_install':
			window.translationNative.finish({ code: 'cancelled', message: 'Cancelled' });
			return;
		case 'verify_model':
			return structuredClone(models[0]);
		case 'remove_model':
			entries = entries.filter(
				(entry) => entry.pair.source !== args.pair.source || entry.pair.target !== args.pair.target
			);
			emit({ modelsRevision: String(BigInt(state.modelsRevision) + 1n) });
			return true;
		case 'discard_downloads':
			return false;
		case 'release':
			emit({ phase: 'paused' });
			return structuredClone(state);
		case 'resume':
			emit({ phase: 'ready' });
			return structuredClone(state);
		default:
			throw new Error(`Unexpected fixture IPC: ${command}`);
	}
}
