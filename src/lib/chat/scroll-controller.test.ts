import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ChatScrollController } from './scroll-controller';

class TestResizeObserver {
	static instances: TestResizeObserver[] = [];
	observe = vi.fn();
	disconnect = vi.fn();
	constructor(private callback: ResizeObserverCallback) {
		TestResizeObserver.instances.push(this);
	}
	resize() {
		this.callback([], this as unknown as ResizeObserver);
	}
}

let frames: Map<number, FrameRequestCallback>;
let nextFrame: number;
let controllers: ChatScrollController[];

beforeEach(() => {
	vi.useFakeTimers();
	frames = new Map();
	nextFrame = 0;
	controllers = [];
	TestResizeObserver.instances = [];
	vi.stubGlobal('ResizeObserver', TestResizeObserver);
	vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
		frames.set(++nextFrame, callback);
		return nextFrame;
	});
	vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id));
});

afterEach(() => {
	for (const controller of controllers) controller.dispose();
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

function flushFrame() {
	const pending = [...frames];
	for (const [id, callback] of pending) {
		if (!frames.delete(id)) continue;
		callback(performance.now());
	}
}

function rect(top: number, height: number): DOMRect {
	return {
		x: 0,
		y: top,
		top,
		bottom: top + height,
		left: 0,
		right: 115,
		width: 115,
		height,
		toJSON: () => ({})
	};
}

function fixture() {
	const viewport = document.createElement('div');
	const content = document.createElement('div');
	viewport.append(content);
	let position = 600;
	let height = 1000;
	let threshold = 32;
	let render = Promise.resolve();
	let releaseRender: (() => void) | undefined;
	const writes: number[] = [];
	const rows = new Map<string, { node: HTMLElement; top: number }>();
	Object.defineProperties(viewport, {
		scrollTop: {
			get: () => position,
			set: (value: number) => {
				position = Math.max(0, Math.min(value, height - 400));
				writes.push(position);
			}
		},
		scrollHeight: { get: () => Math.round(height) },
		clientHeight: { value: 400 },
		clientWidth: { value: 100 },
		offsetWidth: { value: 115 },
		offsetHeight: { value: 400 }
	});
	viewport.getBoundingClientRect = () => rect(0, 400);
	for (let index = 0; index < 10; index++) {
		const node = document.createElement('div');
		node.dataset.chatMessageIndex = String(index);
		const row = { node, top: index * 100 };
		node.getBoundingClientRect = () => rect(row.top - position, 100);
		rows.set(String(index), row);
		content.append(node);
	}
	const callbacks = {
		onPinnedChange: vi.fn(),
		onClearUnread: vi.fn(),
		onHistoryUpdate: vi.fn(),
		onResume: vi.fn()
	};
	const controller = new ChatScrollController({
		messageSelector: '[data-chat-message-index]',
		afterRender: () => render,
		getThresholdPx: () => threshold,
		...callbacks
	});
	controllers.push(controller);
	controller.attach(viewport, content);
	return {
		controller,
		viewport,
		content,
		callbacks,
		rows,
		writes,
		observer: TestResizeObserver.instances.at(-1)!,
		setHeight: (value: number) => {
			height = value;
		},
		setPosition: (value: number) => {
			position = value;
		},
		setThreshold: (value: number) => {
			threshold = value;
		},
		holdRender: () => {
			render = new Promise((resolve) => {
				releaseRender = resolve;
			});
		},
		render: async () => {
			releaseRender?.();
			await render;
		},
		pause: async (top = 300) => {
			controller.handleWheelIntent(new WheelEvent('wheel', { deltaY: -100 }));
			position = top;
			controller.refreshScrollState();
			await vi.advanceTimersByTimeAsync(251);
		}
	};
}

describe('ChatScrollController', () => {
	it('follows pinned content after rendering, including fractional bottoms', async () => {
		const f = fixture();
		f.holdRender();
		expect(f.controller.beforeContentChange('append')).toBe(true);
		f.setHeight(1200.4);
		f.controller.afterContentChange();
		expect(frames.size).toBe(0);
		await f.render();
		flushFrame();
		expect(f.viewport.scrollTop).toBeCloseTo(800.4, 8);
		expect(f.controller.pinned).toBe(true);
		expect(f.callbacks.onClearUnread).toHaveBeenCalledOnce();
	});

	it.each(['append', 'trim', 'translation'] as const)(
		'preserves a paused anchor during %s',
		async (kind) => {
			const f = fixture();
			await f.pause();
			f.callbacks.onClearUnread.mockClear();
			expect(f.controller.beforeContentChange(kind)).toBe(true);
			f.rows.get('3')!.top += 80;
			f.controller.afterContentChange();
			await f.render();
			flushFrame();
			expect(f.viewport.scrollTop).toBe(380);
			expect(f.controller.pinned).toBe(false);
			expect(f.callbacks.onClearUnread).not.toHaveBeenCalled();
		}
	);

	it('keeps the first snapshot across a burst and interim scroll events', async () => {
		const f = fixture();
		await f.pause();
		f.controller.beforeContentChange('append');
		f.rows.get('3')!.top += 50;
		f.controller.afterContentChange();
		f.controller.refreshScrollState();
		f.controller.beforeContentChange('translation');
		f.rows.get('3')!.top += 30;
		f.controller.afterContentChange();
		await f.render();
		expect(frames.size).toBe(1);
		flushFrame();
		expect(f.viewport.scrollTop).toBe(380);
	});

	it('restores pinned content and viewport resizing without pausing', () => {
		const f = fixture();
		expect(f.observer.observe.mock.calls.map(([node]) => node)).toEqual([f.content, f.viewport]);
		f.setHeight(1300);
		f.observer.resize();
		window.dispatchEvent(new Event('resize'));
		flushFrame();
		expect(f.viewport.scrollTop).toBe(900);
		expect(f.controller.pinned).toBe(true);
	});

	it('reapplies paused reflow anchors until the existing settle deadline', async () => {
		const f = fixture();
		await f.pause();
		f.rows.get('3')!.top += 40;
		f.observer.resize();
		flushFrame();
		expect(f.viewport.scrollTop).toBe(340);
		await vi.advanceTimersByTimeAsync(251);
		f.rows.get('3')!.top += 20;
		f.observer.resize();
		expect(frames.size).toBe(0);
		expect(f.viewport.scrollTop).toBe(340);
	});

	it('lets upward input take ownership before a pending render completes', async () => {
		const f = fixture();
		f.holdRender();
		f.controller.beforeContentChange('append');
		f.setHeight(1100);
		f.controller.afterContentChange();
		f.controller.handleWheelIntent(new WheelEvent('wheel', { deltaY: -100 }));
		f.setPosition(550);
		f.controller.refreshScrollState();
		await f.render();
		await vi.advanceTimersByTimeAsync(50);
		flushFrame();
		expect(f.viewport.scrollTop).toBe(550);
		expect(f.controller.interacting).toBe(true);
		expect(f.controller.pinned).toBe(false);
		expect(f.controller.beforeContentChange('translation')).toBe(false);
	});

	it('cancels an already scheduled pinned restore when manual scrolling begins', async () => {
		const f = fixture();
		f.observer.resize();
		f.controller.handleWheelIntent(new WheelEvent('wheel', { deltaY: -1 }));
		expect(frames.size).toBe(0);
		f.setPosition(500);
		f.controller.refreshScrollState();
		await vi.advanceTimersByTimeAsync(50);
		expect(f.viewport.scrollTop).toBe(500);
	});

	it('ignores downward input while pinned and resumes only at the exact bottom', async () => {
		const f = fixture();
		f.controller.handleWheelIntent(new WheelEvent('wheel', { deltaY: 100 }));
		expect(f.controller.interacting).toBe(false);
		await f.pause(570);
		f.controller.handleWheelIntent(new WheelEvent('wheel', { deltaY: 100 }));
		f.setPosition(599);
		f.controller.refreshScrollState();
		expect(f.controller.pinned).toBe(false);
		f.setPosition(600);
		f.controller.refreshScrollState();
		expect(f.controller.pinned).toBe(true);
		expect(f.controller.interacting).toBe(false);
		expect(f.callbacks.onResume).toHaveBeenCalledOnce();
	});

	it('keeps scrollbar ownership until release, even at the bottom', async () => {
		const f = fixture();
		f.controller.handleScrollbarPointerIntent({ clientX: 110, clientY: 100 } as PointerEvent);
		f.setPosition(300);
		f.controller.refreshScrollState();
		await vi.advanceTimersByTimeAsync(1000);
		expect(f.controller.interacting).toBe(true);
		f.setPosition(600);
		f.controller.refreshScrollState();
		expect(f.controller.pinned).toBe(false);
		window.dispatchEvent(new Event('pointerup'));
		expect(f.controller.pinned).toBe(true);
		expect(f.callbacks.onResume).toHaveBeenCalledOnce();
	});

	it('extends touch ownership from the latest movement and settles after release', async () => {
		const f = fixture();
		const touch = (clientY: number) => ({ touches: [{ clientY }] }) as unknown as TouchEvent;
		f.controller.handleTouchStartIntent(touch(100));
		f.controller.handleTouchMoveIntent(touch(120));
		await vi.advanceTimersByTimeAsync(200);
		f.controller.handleTouchMoveIntent(touch(140));
		f.controller.handleTouchEndIntent();
		await vi.advanceTimersByTimeAsync(200);
		expect(f.controller.interacting).toBe(true);
		await vi.advanceTimersByTimeAsync(51);
		expect(f.controller.interacting).toBe(false);
		expect(f.controller.pinned).toBe(false);
		expect(f.callbacks.onHistoryUpdate).toHaveBeenCalledTimes(3);
	});

	it.each(['frame', 'timer'])(
		'flushes once when the %s wins the restoration race',
		async (winner) => {
			const f = fixture();
			f.controller.beforeContentChange('append');
			f.setHeight(1200);
			f.controller.afterContentChange();
			await f.render();
			if (winner === 'frame') flushFrame();
			await vi.advanceTimersByTimeAsync(50);
			flushFrame();
			expect(f.writes).toEqual([800]);
			expect(frames.size).toBe(0);
		}
	);

	it('publishes pinned intent before resume flushes history and suppresses nested restores', async () => {
		const f = fixture();
		await f.pause();
		const order: string[] = [];
		f.callbacks.onPinnedChange.mockImplementation((pinned) => {
			if (pinned) order.push('pin');
		});
		f.callbacks.onResume.mockImplementation(() => {
			order.push('resume');
			expect(f.controller.beforeContentChange('append')).toBe(false);
			expect(f.controller.beforeContentChange('trim')).toBe(false);
			f.setHeight(1400);
		});
		f.callbacks.onClearUnread.mockImplementation(() => order.push('clear'));
		f.controller.resumeToBottom();
		expect(order).toEqual(['pin', 'resume', 'pin', 'clear']);
		expect(f.viewport.scrollTop).toBe(1000);
		await f.render();
		flushFrame();
		expect(f.viewport.scrollTop).toBe(1000);
	});

	it('uses the current threshold when replacing paused history', async () => {
		const f = fixture();
		await f.pause(570);
		f.setThreshold(10);
		const wasPinned = f.controller.beforeReset();
		expect(wasPinned).toBe(false);
		f.rows.get('5')!.top += 20;
		f.controller.afterReset(wasPinned);
		await f.render();
		flushFrame();
		expect(f.viewport.scrollTop).toBe(590);
		expect(f.controller.pinned).toBe(false);
	});

	it('resumes pinned resets and falls back to the saved position if a paused anchor disappears', async () => {
		const f = fixture();
		f.controller.afterReset(f.controller.beforeReset());
		expect(f.callbacks.onResume).toHaveBeenCalledOnce();
		await f.render();
		flushFrame();
		await f.pause();
		const wasPinned = f.controller.beforeReset();
		f.rows.get('3')!.node.remove();
		f.controller.afterReset(wasPinned);
		await f.render();
		flushFrame();
		expect(f.viewport.scrollTop).toBe(300);
		expect(f.controller.pinned).toBe(false);
	});

	it('repins on viewport wake without overriding a manual interaction', async () => {
		const f = fixture();
		f.setHeight(1200);
		document.dispatchEvent(new Event('visibilitychange'));
		flushFrame();
		expect(f.viewport.scrollTop).toBe(800);
		await f.pause();
		window.dispatchEvent(new Event('resize'));
		document.dispatchEvent(new Event('visibilitychange'));
		expect(frames.size).toBe(0);
		expect(f.viewport.scrollTop).toBe(300);
	});

	it('disposes observers, listeners, and scheduled work without later callbacks', async () => {
		const f = fixture();
		f.holdRender();
		f.controller.beforeContentChange('append');
		f.controller.afterContentChange();
		f.observer.resize();
		f.controller.handleWheelIntent(new WheelEvent('wheel', { deltaY: -1 }));
		f.controller.dispose();
		f.controller.dispose();
		for (const callback of Object.values(f.callbacks)) callback.mockClear();
		await f.render();
		await vi.advanceTimersByTimeAsync(1000);
		window.dispatchEvent(new Event('resize'));
		document.dispatchEvent(new Event('visibilitychange'));
		window.dispatchEvent(new Event('pointerup'));
		f.observer.resize();
		f.controller.resumeToBottom();
		flushFrame();
		expect(frames.size).toBe(0);
		expect(vi.getTimerCount()).toBe(0);
		expect(f.observer.disconnect).toHaveBeenCalledOnce();
		expect(f.writes).toEqual([]);
		for (const callback of Object.values(f.callbacks)) expect(callback).not.toHaveBeenCalled();
	});

	it('cancels a paused reflow frame and its expiry timer on disposal', async () => {
		const f = fixture();
		await f.pause();
		f.rows.get('3')!.top += 40;
		f.observer.resize();
		f.writes.length = 0;
		f.controller.dispose();
		flushFrame();
		await vi.advanceTimersByTimeAsync(1000);
		expect(f.writes).toEqual([]);
		expect(vi.getTimerCount()).toBe(0);
	});
});
