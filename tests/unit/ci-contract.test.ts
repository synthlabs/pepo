import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { parse } from 'yaml';

interface PackageJson {
	scripts: Record<string, string>;
}

interface WorkflowStep {
	name?: string;
	run?: string;
	if?: string;
}

interface ValidationWorkflow {
	jobs: {
		verify: {
			steps: WorkflowStep[];
		};
	};
}

const pkg = JSON.parse(readFileSync(path.resolve('package.json'), 'utf8')) as PackageJson;
const validation = parse(
	readFileSync(path.resolve('.github/workflows/validation.yml'), 'utf8')
) as ValidationWorkflow;
const makefile = readFileSync(path.resolve('Makefile'), 'utf8');

function suitesOf(script: string): string[] {
	const value = pkg.scripts[script] ?? '';
	return [...value.matchAll(/pnpm (?:run )?(test:[\w:]+)/g)].map((match) => match[1]);
}

describe('test suite CI contract', () => {
	it('covers every suite in pnpm test with a named step in validation.yml', () => {
		const suites = suitesOf('test');
		expect(suites.length).toBeGreaterThan(0);

		const steps = validation.jobs.verify.steps;
		const suiteSteps = steps
			.map((step) => {
				const match = step.run?.match(/^pnpm (?:run )?(test:[\w:]+)$/);
				return match ? { name: step.name, suite: match[1] } : null;
			})
			.filter((step): step is { name: string | undefined; suite: string } => step !== null);

		const missingSuites = suites.filter(
			(suite) => !suiteSteps.some((step) => step.suite === suite)
		);
		const unnamedSteps = suiteSteps.filter((step) => !step.name?.trim());

		expect(missingSuites).toEqual([]);
		expect(unnamedSteps).toEqual([]);
	});

	it('mirrors every non-test verification token from pnpm verify in validation.yml', () => {
		const allRuns = validation.jobs.verify.steps.map((step) => step.run ?? '').join('\n');
		const expectedTokens = [
			'pnpm check',
			'pnpm lint',
			'pnpm build',
			'cargo fmt --all -- --check',
			'cargo clippy --workspace --all-targets --locked -- -D warnings',
			'git diff --check',
			'git -C utils diff --check'
		];
		for (const token of expectedTokens) {
			expect(pkg.scripts.verify).toContain(token);
			expect(allRuns).toContain(token);
		}
	});

	it('exposes test, verify, and generic test-% pattern forwarding in Makefile', () => {
		expect(makefile).toMatch(/^test:\s*$/m);
		expect(makefile).toMatch(/^verify:\s*$/m);
		expect(makefile).toMatch(/^test-%:\s*$/m);
		expect(makefile).toContain('run test:$(subst -,:,$*)');
	});
});
