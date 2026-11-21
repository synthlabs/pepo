export function formatUsername(
	displayName: string | null | undefined,
	login: string,
	showLogin: boolean
): string {
	const name = displayName?.trim() || login;
	return showLogin && login && name.toLowerCase() !== login.toLowerCase()
		? `${name} (${login})`
		: name;
}

export function usernameColor(color: string | null | undefined): string | undefined {
	return color && /^#[\da-f]{6}$/i.test(color) ? color : undefined;
}

export interface UsernameActivation {
	userId: string | null;
	login: string;
	displayName: string;
	channelId: string;
	channelLogin: string;
	anchor: HTMLButtonElement;
}
