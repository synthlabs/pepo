import { Client } from '$lib/chat/twitch';
import { createWritableStore } from './writeable';

export const chatClient = createWritableStore('chat', new Client());
