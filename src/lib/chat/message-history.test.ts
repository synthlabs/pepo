import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ChannelMessage, ChannelMessageTranslationUpdate } from '$lib/bindings';
import { DEFAULT_APP_SETTINGS } from '$lib/settings';
import { MessageHistoryScheduler } from './message-history';
import {
	applyTranslationUpdate,
	attachPendingTranslation,
	type PendingTranslations
} from './translation';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function createHistory(paused = false) {
	const messages = [1, 2, 3];
	const state = { ...DEFAULT_APP_SETTINGS.chat, message_limit: 3, paused, interacting: false };
	const append = vi.fn((batch: number[]) => messages.push(...batch));
	const trim = vi.fn((count: number) => messages.splice(0, count));
	const scheduler = new MessageHistoryScheduler<number>({
		getState: () => ({ ...state, message_count: messages.length }),
		append,
		trim
	});
	return { messages, state, append, trim, scheduler };
}

describe('message history scheduling', () => {
	it('appends live messages immediately and collects all excess in one delayed pass', () => {
		const { messages, append, trim, scheduler } = createHistory();
		scheduler.enqueue(4);
		scheduler.enqueue(5);
		expect(append.mock.calls).toEqual([[[4]], [[5]]]);
		expect(messages).toEqual([1, 2, 3, 4, 5]);
		vi.advanceTimersByTime(249);
		expect(trim).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(trim).toHaveBeenCalledExactlyOnceWith(2);
		expect(messages).toEqual([3, 4, 5]);
		expect(vi.getTimerCount()).toBe(0);
	});

	it('uses a fixed 250 ms batch deadline while paused and keeps the older history', () => {
		const { messages, append, trim, scheduler } = createHistory(true);
		scheduler.enqueue(4);
		vi.advanceTimersByTime(200);
		scheduler.enqueue(5);
		vi.advanceTimersByTime(49);
		expect(messages).toEqual([1, 2, 3]);
		vi.advanceTimersByTime(1);
		expect(append).toHaveBeenCalledExactlyOnceWith([4, 5]);
		expect(messages).toEqual([1, 2, 3, 4, 5]);
		vi.advanceTimersByTime(1000);
		expect(trim).not.toHaveBeenCalled();
		expect(vi.getTimerCount()).toBe(0);
	});

	it('continues publishing four batches per second under continuous paused traffic', () => {
		const { messages, append, trim, scheduler } = createHistory(true);
		for (let i = 4; i < 24; i++) {
			scheduler.enqueue(i);
			vi.advanceTimersByTime(50);
		}
		expect(append).toHaveBeenCalledTimes(4);
		expect(messages).toEqual(Array.from({ length: 23 }, (_, i) => i + 1));
		expect(trim).not.toHaveBeenCalled();
	});

	it('cancels a queued GC pass when chat pauses', () => {
		const { messages, state, trim, scheduler } = createHistory();
		scheduler.enqueue(4);
		vi.advanceTimersByTime(200);
		state.paused = true;
		scheduler.update();
		vi.advanceTimersByTime(1000);
		expect(messages).toEqual([1, 2, 3, 4]);
		expect(trim).not.toHaveBeenCalled();
	});

	it('flushes pending arrivals before one resume cleanup without a later duplicate flush', () => {
		const { messages, state, append, trim, scheduler } = createHistory(true);
		scheduler.enqueue(4);
		scheduler.enqueue(5);
		vi.advanceTimersByTime(50);
		state.paused = false;
		scheduler.resume();
		expect(append).toHaveBeenCalledExactlyOnceWith([4, 5]);
		expect(trim).toHaveBeenCalledExactlyOnceWith(2);
		expect(messages).toEqual([3, 4, 5]);
		vi.advanceTimersByTime(1000);
		expect(append).toHaveBeenCalledTimes(1);
		expect(trim).toHaveBeenCalledTimes(1);
		expect(vi.getTimerCount()).toBe(0);
	});

	it('batches GC while paused when retention is disabled', () => {
		const { messages, state, trim, scheduler } = createHistory(true);
		state.pause_message_gc_while_paused = false;
		scheduler.enqueue(4);
		scheduler.enqueue(5);
		vi.advanceTimersByTime(499);
		expect(messages).toEqual([1, 2, 3, 4, 5]);
		expect(trim).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(trim).toHaveBeenCalledExactlyOnceWith(2);
		expect(messages).toEqual([3, 4, 5]);
	});

	it('defers GC until manual viewport ownership ends', () => {
		const { messages, state, trim, scheduler } = createHistory();
		scheduler.enqueue(4);
		state.interacting = true;
		scheduler.update();
		vi.advanceTimersByTime(500);
		expect(trim).not.toHaveBeenCalled();
		state.interacting = false;
		scheduler.update();
		vi.advanceTimersByTime(250);
		expect(trim).toHaveBeenCalledExactlyOnceWith(1);
		expect(messages).toEqual([2, 3, 4]);
	});

	it('supports optional live batching and flushes in order when it is disabled', () => {
		const { messages, state, append, scheduler } = createHistory();
		state.batch_live_messages = true;
		scheduler.enqueue(4);
		scheduler.enqueue(5);
		vi.advanceTimersByTime(249);
		expect(append).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(append).toHaveBeenCalledExactlyOnceWith([4, 5]);
		scheduler.enqueue(6);
		state.batch_live_messages = false;
		scheduler.enqueue(7);
		expect(append.mock.calls[1]).toEqual([[6, 7]]);
		expect(messages).toEqual([1, 2, 3, 4, 5, 6, 7]);
	});

	it('applies retention and limit changes through the scheduled GC policy', () => {
		const { messages, state, trim, scheduler } = createHistory(true);
		scheduler.enqueue(4);
		vi.advanceTimersByTime(250);
		state.message_limit = 2;
		scheduler.update();
		vi.advanceTimersByTime(500);
		expect(trim).not.toHaveBeenCalled();
		state.pause_message_gc_while_paused = false;
		scheduler.update();
		vi.advanceTimersByTime(249);
		expect(trim).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(messages).toEqual([3, 4]);
		expect(trim).toHaveBeenCalledExactlyOnceWith(2);
	});

	it('honors independently configured append and GC intervals', () => {
		const { messages, state, trim, scheduler } = createHistory(true);
		state.message_batch_interval_ms = 100;
		state.message_gc_interval_ms = 500;
		state.pause_message_gc_while_paused = false;
		scheduler.enqueue(4);
		vi.advanceTimersByTime(99);
		expect(messages).toEqual([1, 2, 3]);
		vi.advanceTimersByTime(1);
		expect(messages).toEqual([1, 2, 3, 4]);
		vi.advanceTimersByTime(499);
		expect(trim).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(messages).toEqual([2, 3, 4]);
	});

	it('drops pending work and cancels both timers when the channel is disposed', () => {
		const { messages, state, append, trim, scheduler } = createHistory();
		scheduler.enqueue(4);
		state.batch_live_messages = true;
		scheduler.enqueue(5);
		expect(vi.getTimerCount()).toBe(2);
		scheduler.dispose();
		scheduler.enqueue(6);
		scheduler.resume();
		vi.advanceTimersByTime(1000);
		expect(append).toHaveBeenCalledTimes(1);
		expect(trim).not.toHaveBeenCalled();
		expect(messages).toEqual([1, 2, 3, 4]);
		expect(vi.getTimerCount()).toBe(0);
	});

	it('attaches a translation received while its message is waiting in the append queue', () => {
		const messages: ChannelMessage[] = [];
		const pending: PendingTranslations = new Map();
		const scheduler = new MessageHistoryScheduler<ChannelMessage>({
			getState: () => ({
				...DEFAULT_APP_SETTINGS.chat,
				paused: true,
				interacting: false,
				message_count: messages.length
			}),
			append: (batch) =>
				messages.push(...batch.map((message) => attachPendingTranslation(message, pending))),
			trim: (count) => messages.splice(0, count)
		});
		scheduler.enqueue({
			ts: '2026-10-24T00:00:00Z',
			broadcaster_user_id: 'channel',
			broadcaster_user_name: 'Channel',
			broadcaster_user_login: 'channel',
			chatter_user_id: 'user',
			chatter_user_name: 'User',
			message_id: 'queued',
			text: 'original',
			fragments: [{ Text: { index: 0, text: 'original' } }],
			message_type: 'text',
			badges: [],
			color: '#ffffff',
			translation: null,
			index: 1
		});
		const update: ChannelMessageTranslationUpdate = {
			message_id: 'queued',
			translation: { source_language: 'ru', target_language: 'en', translated_text: 'translated' }
		};
		expect(applyTranslationUpdate(messages, update, pending).changed).toBe(false);
		vi.advanceTimersByTime(250);
		expect(messages).toHaveLength(1);
		expect(messages[0].translation?.translated_text).toBe('translated');
		expect(pending.size).toBe(0);
	});
});
