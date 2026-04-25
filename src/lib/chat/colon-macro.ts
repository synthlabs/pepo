const COLON_MACRO_RE = /(^|\s):(\w{2,})$/;

export function parseColonMacro(input: string): string | null {
	const match = input.match(COLON_MACRO_RE);
	return match ? match[2] : null;
}
