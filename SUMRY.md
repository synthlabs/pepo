[commit]: # '82584136c107a01c5c48ba304b714ec788f899a2'

Features:

- (usercards): adjust layout and fix focus bug
- (chat): initial usercard support
- (chat): support username detection and core primitives
- (core): support base moderation event notices
- (settings): add back button
- (chat): support system messages in the chat timeline
- (build): add ci make command

Fixes:

- (chat): adjust system notice settings
- (settings): advanced translation settings page opens correctly
- (chat): prevent message jittering when translating history
- (chat): improve performance of scrolling controller
- (chat): improve performance by reducing timestamp formatting

Misc:

- refactor: keep sidebar collapse state in localStorage
- (chat): extract scroll controller
- (updater): version bump 0.2.32
