#!/usr/bin/env node
import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from 'svelte/compiler';
import { extendTailwindMerge } from 'tailwind-merge';

const DEFAULT_ROOTS = ['src'];
const CLASS_HELPERS = new Set(['cn']);

const mergeTailwindClasses = extendTailwindMerge({
	extend: {
		conflictingClassGroups: {
			break: ['arbitrary..overflow-wrap'],
			'arbitrary..overflow-wrap': ['break']
		}
	}
});

function normalizeClassList(classList) {
	return classList.trim().split(/\s+/).filter(Boolean).join(' ');
}

function lineStartsFor(source) {
	const starts = [0];

	for (let index = 0; index < source.length; index += 1) {
		if (source[index] === '\n') {
			starts.push(index + 1);
		}
	}

	return starts;
}

function positionAt(lineStarts, offset) {
	let low = 0;
	let high = lineStarts.length - 1;

	while (low <= high) {
		const middle = Math.floor((low + high) / 2);
		if (lineStarts[middle] <= offset) {
			low = middle + 1;
		} else {
			high = middle - 1;
		}
	}

	const lineIndex = Math.max(0, high);

	return {
		line: lineIndex + 1,
		column: offset - lineStarts[lineIndex] + 1
	};
}

function isObject(value) {
	return value !== null && typeof value === 'object';
}

function isStringLiteral(node) {
	return isObject(node) && node.type === 'Literal' && typeof node.value === 'string';
}

function isClassHelperCall(node) {
	return (
		isObject(node) &&
		node.type === 'CallExpression' &&
		node.callee?.type === 'Identifier' &&
		CLASS_HELPERS.has(node.callee.name)
	);
}

function walk(node, visit) {
	if (Array.isArray(node)) {
		for (const child of node) {
			walk(child, visit);
		}
		return;
	}

	if (!isObject(node)) {
		return;
	}

	visit(node);

	for (const [key, child] of Object.entries(node)) {
		if (
			key === 'parent' ||
			key === 'loc' ||
			key === 'name_loc' ||
			key === 'start' ||
			key === 'end' ||
			key === 'type'
		) {
			continue;
		}

		walk(child, visit);
	}
}

function stringLiteralsIn(node) {
	const literals = [];

	walk(node, (child) => {
		if (isStringLiteral(child)) {
			literals.push(child);
		}
	});

	return literals;
}

function classCandidatesFromAttribute(attribute) {
	if (Array.isArray(attribute.value)) {
		const staticText = attribute.value
			.filter((part) => part.type === 'Text')
			.map((part) => part.data)
			.join(' ');

		if (staticText.trim().length === 0) {
			return [];
		}

		const firstTextPart = attribute.value.find((part) => part.type === 'Text');

		return [
			{
				value: staticText,
				offset: firstTextPart?.start ?? attribute.start
			}
		];
	}

	const expression = attribute.value?.type === 'ExpressionTag' ? attribute.value.expression : undefined;

	if (isStringLiteral(expression)) {
		return [
			{
				value: expression.value,
				offset: expression.start
			}
		];
	}

	if (!isClassHelperCall(expression)) {
		return [];
	}

	return expression.arguments.flatMap((argument) =>
		stringLiteralsIn(argument).map((literal) => ({
			value: literal.value,
			offset: literal.start
		}))
	);
}

function checkClassCandidate(candidate, lineStarts, filePath) {
	const original = normalizeClassList(candidate.value);

	if (original.length === 0) {
		return undefined;
	}

	const merged = normalizeClassList(mergeTailwindClasses(original));

	if (merged === original) {
		return undefined;
	}

	return {
		filePath,
		...positionAt(lineStarts, candidate.offset),
		original,
		merged
	};
}

export function findTailwindClassConflicts(source, filePath = '<inline>') {
	const ast = parse(source, { filename: filePath, modern: true });
	const lineStarts = lineStartsFor(source);
	const diagnostics = [];

	walk(ast.fragment, (node) => {
		if (node.type !== 'Attribute' || node.name !== 'class') {
			return;
		}

		for (const candidate of classCandidatesFromAttribute(node)) {
			const diagnostic = checkClassCandidate(candidate, lineStarts, filePath);
			if (diagnostic) {
				diagnostics.push(diagnostic);
			}
		}
	});

	return diagnostics;
}

function collectSvelteFiles(entry) {
	const absolute = path.resolve(entry);
	const stats = statSync(absolute);

	if (stats.isFile()) {
		return absolute.endsWith('.svelte') ? [absolute] : [];
	}

	if (!stats.isDirectory()) {
		return [];
	}

	const files = [];

	for (const dirent of readdirSync(absolute, { withFileTypes: true })) {
		if (dirent.name === 'node_modules' || dirent.name === '.svelte-kit') {
			continue;
		}

		const child = path.join(absolute, dirent.name);
		files.push(...collectSvelteFiles(child));
	}

	return files;
}

function displayPath(filePath) {
	return path.relative(process.cwd(), filePath).replaceAll(path.sep, '/');
}

function formatDiagnostic(diagnostic) {
	return [
		`${displayPath(diagnostic.filePath)}:${diagnostic.line}:${diagnostic.column} Tailwind class conflict`,
		`  original: ${diagnostic.original}`,
		`  merged:   ${diagnostic.merged}`
	].join('\n');
}

export function runCli(entries = DEFAULT_ROOTS) {
	const roots = entries.length > 0 ? entries : DEFAULT_ROOTS;
	const diagnostics = [];

	for (const entry of roots) {
		for (const filePath of collectSvelteFiles(entry)) {
			const source = readFileSync(filePath, 'utf8');
			diagnostics.push(...findTailwindClassConflicts(source, filePath));
		}
	}

	if (diagnostics.length === 0) {
		console.log('No Tailwind class conflicts found.');
		return 0;
	}

	console.log(diagnostics.map(formatDiagnostic).join('\n\n'));
	console.log(`\n${diagnostics.length} Tailwind class conflict(s) found.`);
	return 1;
}

const thisFilePath = fileURLToPath(import.meta.url);

if (process.argv[1] && path.resolve(process.argv[1]) === thisFilePath) {
	process.exitCode = runCli(process.argv.slice(2));
}
