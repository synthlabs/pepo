import { vi } from 'vitest';
import type {
	InstallationDto,
	InventoryDto,
	ModelDto,
	PairDto,
	ServiceError,
	ServiceSnapshot
} from '$utils/translate';

export const RU_EN = { source: 'rus', target: 'eng' };
export const EN_RU = { source: 'eng', target: 'rus' };
export function model(pair = RU_EN): ModelDto {
	return {
		id: `${pair.source}-${pair.target}-v1`,
		pair,
		version: '1',
		architecture: 'test',
		catalogSha256: 'a'.repeat(64),
		downloadBytes: '1024',
		installedBytes: '2048'
	};
}
export function local(
	pair = RU_EN,
	state: InventoryDto['state'] = 'present_unverified'
): InventoryDto {
	return {
		directory: `models/${pair.source}-${pair.target}`,
		pair,
		state,
		installed: state === 'present_unverified' ? model(pair) : null,
		matchesCurrentCatalog: true,
		detail: null
	};
}
export function installation(): InstallationDto {
	return {
		id: 'install-1',
		model: model(),
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
			fileDownloadBytes: '256',
			fileDownloadTotal: '1024',
			completedDownloadBytes: '0',
			downloadTotal: '1024',
			fileInstalledBytes: '0',
			fileInstalledTotal: '2048',
			completedInstalledBytes: '0',
			installedTotal: '2048',
			reusedBytes: '0',
			networkBytes: '256',
			detail: null
		}
	};
}
export function fixture() {
	let state: ServiceSnapshot = {
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
				models: [model(), model(EN_RU)],
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
	let entries: InventoryDto[] = [];
	let installationSequence = 0;
	const watchers = new Set<(state: ServiceSnapshot) => void>();
	const emit = (patch: Partial<ServiceSnapshot>) => {
		state = {
			...state,
			...structuredClone(patch),
			revision: (BigInt(state.revision) + 1n).toString()
		};
		for (const watcher of watchers) watcher(structuredClone(state));
	};
	const client = {
		watchState: vi.fn(async (callback: (snapshot: ServiceSnapshot) => void) => {
			watchers.add(callback);
			callback(structuredClone(state));
			return () => {
				watchers.delete(callback);
			};
		}),
		status: vi.fn(async () => structuredClone(state)),
		inventory: vi.fn(async () => structuredClone(entries)),
		initialize: vi.fn(async () => {
			emit({ phase: 'ready', error: null });
			return structuredClone(state);
		}),
		refreshCatalog: vi.fn(async () => structuredClone(state.catalog)),
		planInstall: vi.fn(async (pair: PairDto) => ({
			pair,
			catalogSha256: 'a'.repeat(64),
			modelId: model(pair).id,
			model: model(pair)
		})),
		startInstall: vi.fn(async (plan: { model: ModelDto }) => {
			const operation = {
				...installation(),
				id: `install-${++installationSequence}`,
				model: plan.model
			};
			emit({ installation: operation });
			return operation;
		}),
		installation: vi.fn(async () => structuredClone(state.installation!)),
		cancelInstall: vi.fn(async () => {}),
		verifyModel: vi.fn(async (pair: PairDto) => model(pair)),
		removeModel: vi.fn(async () => true),
		discardDownloads: vi.fn(async () => false),
		release: vi.fn(async () => {
			emit({ phase: 'paused' });
			return structuredClone(state);
		}),
		resume: vi.fn(async () => {
			emit({ phase: 'ready' });
			return structuredClone(state);
		}),
		dispose: vi.fn(async () => {})
	};
	return {
		client,
		emit,
		watchers,
		state: () => structuredClone(state),
		rows: (rows: InventoryDto[]) => {
			entries = rows;
		},
		finish: (error: ServiceError | null = null, warnings: string[] = []) => {
			const operation = state.installation!;
			if (!error)
				entries = [
					...entries.filter(
						(entry) =>
							entry.pair?.source !== operation.model.pair.source ||
							entry.pair?.target !== operation.model.pair.target
					),
					local(operation.model.pair)
				];
			emit({
				modelsRevision: String(BigInt(state.modelsRevision) + 1n),
				installation: {
					...operation,
					finished: true,
					result: error
						? null
						: { status: 'installed', model: operation.model, cleanupWarnings: warnings },
					error
				}
			});
		}
	};
}

export async function settle() {
	for (let pass = 0; pass < 30; pass++) await Promise.resolve();
}
