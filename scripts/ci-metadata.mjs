import { appendFileSync, readFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { execFileSync } from 'node:child_process';

const { version } = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const shortSha = execFileSync('git', ['rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).trim();
const pkgrel = process.env.GITHUB_RUN_NUMBER;
if (!/^[1-9]\d*$/.test(pkgrel ?? '')) throw new Error('GITHUB_RUN_NUMBER must be positive');
const releaseTag = `app-${version}-${shortSha}`;
const body = `https://github.com/synthlabs/pepo/releases/tag/${releaseTag}\n\n${readFileSync('SUMRY.md', 'utf8').split('\n').slice(2).join('\n')}`;
const delimiter = randomUUID();
appendFileSync(
	process.env.GITHUB_OUTPUT,
	[
		`version=${version}`,
		`short-sha=${shortSha}`,
		`release-tag=${releaseTag}`,
		`pkgrel=${pkgrel}`,
		`filename=pepo-${version}-${pkgrel}-x86_64.pkg.tar.zst`,
		`release-body<<${delimiter}`,
		body,
		delimiter,
		''
	].join('\n')
);
