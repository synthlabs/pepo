export function parseColonMacro(input: string, minChars = 2): string | null {
	const safeMinChars = Number.isFinite(minChars) && minChars > 0 ? Math.floor(minChars) : 2;
	const match = input.match(new RegExp(`(^|\\s):(\\w{${safeMinChars},})$`));
	return match ? match[2] : null;
}
