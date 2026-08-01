import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, test } from 'vitest';

const appLayout = readFileSync(path.resolve('src/routes/app/+layout.svelte'), 'utf8');
const chatPage = readFileSync(path.resolve('src/routes/app/chat/[id]/+page.svelte'), 'utf8');

describe('chat layout containment', () => {
	test('provides a definite, non-growing app-shell height chain', () => {
		expect(appLayout).toContain('class="h-dvh min-h-0 overflow-hidden"');
		expect(appLayout).toContain(
			'class="flex h-full min-h-0 w-full max-w-full min-w-0 flex-col flex-nowrap overflow-hidden"'
		);
		expect(appLayout).toContain("cn('flex min-h-0 w-full grow overflow-hidden'");
	});

	test('keeps the feed shrinkable and the composer surfaces fixed', () => {
		expect(chatPage).toContain(
			'class="flex h-full min-h-0 w-full flex-col flex-nowrap overflow-hidden"'
		);
		expect(chatPage).toContain('class="relative min-h-0 grow"');
		expect(chatPage).toContain('class="shrink-0 cursor-not-allowed bg-red-950 text-center"');
		expect(chatPage).toContain('class="relative shrink-0 border-t"');
		expect(chatPage).toContain(
			'class="bg-background placeholder:text-muted-foreground h-full min-w-0 flex-1 p-3 text-sm outline-hidden focus:border-none focus:ring-0 disabled:cursor-not-allowed disabled:opacity-50"'
		);
	});
});
