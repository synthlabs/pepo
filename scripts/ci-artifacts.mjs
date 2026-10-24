import assert from 'node:assert/strict';
import { appendFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export function collectArtifacts(directory, platform) {
	const files = readdirSync(directory, { recursive: true, withFileTypes: true })
		.filter((entry) => entry.isFile())
		.map((entry) => path.join(entry.parentPath, entry.name))
		.filter((file) => /\.(deb|rpm|AppImage|exe|dmg|tar\.gz|sig)$/.test(file));
	const required = {
		Linux: ['.deb', '.rpm', '.AppImage'],
		Windows: ['.exe'],
		macOS: ['.dmg', '.app.tar.gz']
	}[platform];
	assert.ok(required, `Unsupported artifact platform: ${platform}`);
	for (const suffix of required) {
		assert.ok(
			files.some((file) => file.endsWith(suffix)),
			`Missing ${suffix} package`
		);
	}
	const updaterSuffix = { Linux: '.AppImage', Windows: '.exe', macOS: '.app.tar.gz' }[platform];
	assert.ok(
		files.some((file) => file.endsWith(updaterSuffix) && files.includes(`${file}.sig`)),
		'Missing paired updater artifact and signature'
	);
	return files;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
	const files = collectArtifacts(process.argv[2], process.env.RUNNER_OS);
	appendFileSync(
		process.env.GITHUB_OUTPUT,
		`paths<<ARTIFACT_PATHS\n${files.join('\n')}\nARTIFACT_PATHS\n`
	);
}
