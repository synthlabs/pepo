/* eslint-disable @typescript-eslint/no-explicit-any */
declare global {
	interface Window {
		__TAURI_INTERNALS__?: Record<string, any>;
		cardNative?: any;
		chatNative?: any;
		networkFixture?: any;
		settingsNative?: any;
		translationNative?: any;
		fixture?: any;
		fixtureReady?: any;
		routeFixture?: any;
		compareScreenshot?: (png: string) => Promise<{
			changedPixels: number;
			interiorChangedPixels?: number;
			nonImageChangedPixels?: number;
		}>;
	}
}

export {};
