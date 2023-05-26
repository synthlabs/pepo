import { ChatClient } from '@twurple/chat';

import { createWritableStore } from './writeable';

export const chatClient = createWritableStore('chat', new ChatClient());
