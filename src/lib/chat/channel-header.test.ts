import { describe, expect, it } from 'vitest';

import type { ChannelStatus, Stream } from '$lib/bindings';
import { channelHeader } from './channel-header';

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
		stream: null,
		...overrides
	};
}

describe('channelHeader', () => {
	it('formats live channels with display name and title', () => {
		expect(channelHeader('maya', status({ is_live: true, stream: stream() }))).toEqual({
			text: 'Maya: Live title',
			viewerCount: 42
		});
	});

	it('formats offline channels with display name', () => {
		expect(channelHeader('maya', status())).toEqual({
			text: 'Maya is offline.',
			viewerCount: null
		});
	});

	it('falls back to route login while status is missing', () => {
		expect(channelHeader('maya', null)).toEqual({
			text: 'maya',
			viewerCount: null
		});
	});

	it('falls back to route login when display name is empty', () => {
		expect(channelHeader('maya', status({ display_name: '' }))).toEqual({
			text: 'maya is offline.',
			viewerCount: null
		});
	});
});
