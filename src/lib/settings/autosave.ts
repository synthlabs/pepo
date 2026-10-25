import type { AppSettings } from '$lib/bindings';
import { normalizeAppSettings } from '$lib/settings';

export type EditableSettingsSection = 'appearance' | 'layout' | 'chat' | 'emotes';
export type SettingsPatch = {
	[Section in EditableSettingsSection]?: Partial<AppSettings[Section]>;
};
export type SettingsUpdate = SettingsPatch | ((settings: AppSettings) => SettingsPatch);
export type SettingsSaveResult = 'idle' | 'saved' | 'error';
export interface SettingsSnapshot {
	value: AppSettings;
	ready: boolean;
	saving: boolean;
	result: SettingsSaveResult;
}

export function applySettingsPatch(settings: AppSettings, patch: SettingsPatch): AppSettings {
	return {
		...settings,
		appearance: { ...settings.appearance, ...patch.appearance },
		layout: { ...settings.layout, ...patch.layout },
		chat: { ...settings.chat, ...patch.chat },
		emotes: { ...settings.emotes, ...patch.emotes }
	};
}

export function applySettingsUpdate(settings: AppSettings, update: SettingsUpdate): AppSettings {
	return applySettingsPatch(settings, typeof update === 'function' ? update(settings) : update);
}

interface PendingSave {
	update: SettingsUpdate;
	resolve: (saved: boolean) => void;
}

export class SettingsAutosave {
	private confirmed: AppSettings | undefined;
	private pending: PendingSave[] = [];
	private processing = false;
	private result: SettingsSaveResult = 'idle';
	// Echoes have no revision; remember our snapshots even after their requests settle.
	private sentSnapshots = new Set<string>();

	constructor(
		private sync: (settings: AppSettings) => Promise<boolean>,
		private publish: (snapshot: SettingsSnapshot) => void
	) {}

	receive(settings: AppSettings): void {
		const value = normalizeAppSettings(settings);
		if (this.sentSnapshots.has(JSON.stringify(value))) return;
		this.confirmed = structuredClone(value);
		this.emit();
	}

	update(update: SettingsUpdate): Promise<boolean> {
		if (!this.confirmed) return Promise.resolve(false);
		const completion = new Promise<boolean>((resolve) => {
			this.pending.push({
				update: typeof update === 'function' ? update : structuredClone(update),
				resolve
			});
		});
		this.emit();
		void this.drain();
		return completion;
	}

	private emit(): void {
		if (!this.confirmed) return;
		const value = this.pending.reduce(
			(settings, save) => applySettingsUpdate(settings, save.update),
			this.confirmed
		);
		this.publish({
			value: structuredClone(value),
			ready: true,
			saving: this.pending.length > 0,
			result: this.result
		});
	}

	private async drain(): Promise<void> {
		if (this.processing || !this.confirmed) return;
		this.processing = true;
		while (this.pending.length > 0) {
			const save = this.pending[0];
			const outgoing = applySettingsUpdate(this.confirmed, save.update);
			this.sentSnapshots.add(JSON.stringify(outgoing));
			let saved: boolean;
			try {
				saved = (await this.sync(structuredClone(outgoing))) === true;
			} catch {
				saved = false;
			}
			if (saved) this.confirmed = applySettingsUpdate(this.confirmed, save.update);
			this.result = saved ? 'saved' : 'error';
			this.pending.shift();
			this.emit();
			save.resolve(saved);
		}
		this.processing = false;
	}
}
