import test from 'node:test';
import assert from 'node:assert/strict';
import { findTailwindClassConflicts } from './lint-tailwind-classes.mjs';

function lint(source) {
	return findTailwindClassConflicts(source, 'Component.svelte');
}

test('detects overflow-wrap arbitrary property conflicts with break utilities', () => {
	const diagnostics = lint('<div class="[overflow-wrap:anywhere] break-words"></div>');

	assert.equal(diagnostics.length, 1);
	assert.equal(diagnostics[0].merged, 'break-words');
});

test('detects reversed overflow-wrap conflict order', () => {
	const diagnostics = lint('<div class="break-words [overflow-wrap:anywhere]"></div>');

	assert.equal(diagnostics.length, 1);
	assert.equal(diagnostics[0].merged, '[overflow-wrap:anywhere]');
});

test('detects conflicts with matching variants', () => {
	const diagnostics = lint('<div class="md:[overflow-wrap:anywhere] md:break-words"></div>');

	assert.equal(diagnostics.length, 1);
	assert.equal(diagnostics[0].merged, 'md:break-words');
});

test('allows conflicts separated by different variants', () => {
	const diagnostics = lint('<div class="sm:[overflow-wrap:anywhere] md:break-words"></div>');

	assert.equal(diagnostics.length, 0);
});

test('allows clean class lists', () => {
	const diagnostics = lint('<div class="min-w-0 text-wrap break-words"></div>');

	assert.equal(diagnostics.length, 0);
});

test('checks static strings inside cn expressions', () => {
	const diagnostics = lint('<div class={cn("px-2 px-4", dynamic)}></div>');

	assert.equal(diagnostics.length, 1);
	assert.equal(diagnostics[0].merged, 'px-4');
});

test('ignores dynamic expressions', () => {
	const diagnostics = lint('<div class={dynamicClasses}></div>');

	assert.equal(diagnostics.length, 0);
});
