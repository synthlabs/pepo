import { describe, expect, it } from 'vitest';

import type { ChannelStatus, Stream } from '$lib/bindings';
import { channelHeader, fitHeaderMetadata, streamUptime } from './channel-header';

function stream(overrides: Partial<Stream> = {}): Stream {
	return {
		game_id: 'game-id',
		game_name: 'Game',
		id: 'stream-id',
		language: 'en',
		is_mature: false,
		started_at: '2026-01-01T00:00:00Z',
		tags: [],
		thumbnail_url: '',
		title: 'Live title',
		user_id: '1',
		user_name: 'Maya',
		user_login: 'maya',
		viewer_count: 42,
		...overrides
	};
}

function status(overrides: Partial<ChannelStatus> = {}): ChannelStatus {
	return {
		broadcaster_id: '1',
		login: 'maya',
		display_name: 'Maya',
		profile_image_url: '',
		is_live: false,
		is_partner: false,
		stream: null,
		...overrides
	};
}

describe('channelHeader', () => {
	it('exposes structured live metadata without changing title capitalization', () => {
		expect(channelHeader('maya', status({ is_live: true, stream: stream() }))).toEqual({
			channelName: 'Maya',
			isPartner: false,
			state: 'live',
			title: 'Live title',
			category: 'Game',
			language: 'English',
			tags: [],
			viewerCount: 42,
			startedAt: '2026-01-01T00:00:00Z'
		});
	});

	it('formats offline channels with display name', () => {
		expect(channelHeader('maya', status({ is_partner: true }))).toEqual({
			channelName: 'Maya',
			isPartner: true,
			state: 'offline',
			title: null,
			category: null,
			language: null,
			tags: [],
			viewerCount: null,
			startedAt: null
		});
	});

	it('falls back to route login while status is missing', () => {
		expect(channelHeader('maya', null)).toMatchObject({
			channelName: 'maya',
			state: 'loading',
			isPartner: false
		});
	});

	it('falls back to route login when display name is empty', () => {
		expect(channelHeader('maya', status({ display_name: '' }))).toMatchObject({
			channelName: 'maya',
			state: 'offline'
		});
	});

	it.each([null, undefined, '', '  '])('omits channel content for route %s', (login) => {
		expect(channelHeader(login, status())).toBeNull();
	});

	it('does not claim a live channel with missing stream data is offline', () => {
		expect(channelHeader('maya', status({ is_live: true }))?.state).toBe('loading');
	});

	it('keeps zero viewers and confirmed partner status', () => {
		expect(
			channelHeader('maya', status({ is_partner: true, stream: stream({ viewer_count: 0 }) }))
		).toMatchObject({
			viewerCount: 0,
			isPartner: true
		});
	});

	it('separates the language from the remaining tags and removes language duplicates', () => {
		const header = channelHeader(
			'maya',
			status({ stream: stream({ tags: ['Hair', 'english', 'EN', 'DropsEnabled'] }) })
		);
		expect(header?.language).toBe('English');
		expect(header?.tags).toEqual(['Hair', 'DropsEnabled']);
	});

	it('handles missing and unrecognized language codes', () => {
		expect(
			channelHeader('maya', status({ stream: stream({ language: '', tags: ['Hair'] }) }))?.tags
		).toEqual(['Hair']);
		expect(
			channelHeader('maya', status({ stream: stream({ language: 'not_a_language' }) }))?.tags
		).toEqual([]);
		expect(
			channelHeader('maya', status({ stream: stream({ language: 'not_a_language' }) }))?.language
		).toBe('not_a_language');
	});
});

describe('streamUptime', () => {
	const startedAt = '2026-01-01T00:00:00Z';
	it.each([
		[0, '0:00:00'],
		[59999, '0:00:59'],
		[60000, '0:01:00'],
		[3599999, '0:59:59'],
		[3600000, '1:00:00'],
		[10065000, '2:47:45'],
		[100800000, '28:00:00'],
		[-1000, '0:00:00']
	])('formats an elapsed %s milliseconds', (elapsed, expected) => {
		expect(streamUptime(startedAt, Date.parse(startedAt) + elapsed)).toBe(expected);
	});
	it.each([null, '', 'invalid'])('omits uptime for %s', (value) => {
		expect(streamUptime(value, Date.now())).toBeNull();
	});
});

describe('fitHeaderMetadata', () => {
	it.each([
		[460, 2, true, true],
		[459, 1, true, true],
		[395, 0, true, true],
		[343, 0, false, true],
		[271, 0, false, false],
		[100, 0, false, false]
	])(
		'fits metadata into %spx without dropping required content',
		(availableWidth, tagCount, showCategory, showUptime) => {
			expect(
				fitHeaderMetadata({
					availableWidth,
					requiredWidth: 200,
					tagWidths: [40, 60],
					categoryWidth: 60,
					uptimeWidth: 60
				})
			).toEqual({ tagCount, showCategory, showUptime });
		}
	);
	it('does not reserve space for absent metadata', () => {
		expect(
			fitHeaderMetadata({
				availableWidth: 200,
				requiredWidth: 200,
				tagWidths: [],
				categoryWidth: 0,
				uptimeWidth: 0
			})
		).toEqual({ tagCount: 0, showCategory: false, showUptime: false });
	});
});
