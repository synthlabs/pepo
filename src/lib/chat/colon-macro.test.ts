import { describe, expect, it } from 'vitest';
import { parseColonMacro } from './colon-macro';

describe('parseColonMacro', () => {
	it('returns null for empty input', () => {
		expect(parseColonMacro('')).toBeNull();
	});

	it('returns null when there is no colon', () => {
		expect(parseColonMacro('hello world')).toBeNull();
	});

	it('returns null for one-character queries (regex requires \\w{2,})', () => {
		expect(parseColonMacro(':a')).toBeNull();
	});

	it('matches a colon-prefixed query at the start of the string', () => {
		expect(parseColonMacro(':foo')).toBe('foo');
	});

	it('matches a colon-prefixed query after whitespace', () => {
		expect(parseColonMacro('hello :foo')).toBe('foo');
	});

	it('returns null when the colon is glued to a preceding word (e.g. URL ports, time)', () => {
		expect(parseColonMacro('http://example.com:foo')).toBeNull();
	});

	it('returns null when the colon-query is not at the end', () => {
		expect(parseColonMacro(':foo bar')).toBeNull();
	});

	it('only returns the most recent colon-query', () => {
		expect(parseColonMacro(':first word :second')).toBe('second');
	});

	it('returns null for trailing non-word characters', () => {
		expect(parseColonMacro(':foo!')).toBeNull();
	});

	it('locks current ASCII-only \\w behavior — non-ASCII trailing chars do not match', () => {
		expect(parseColonMacro(':café')).toBeNull();
	});
});
