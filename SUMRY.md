[commit]: # '111a7a46042be7857374ecbf13dcfb3dd475f805'

Features:

- (chat): start using fragments to render message
- (core:emotes): initial emote manager shell
- (core): set version in title bar
- (chat:badges): implement badges
- (core:badges): skeleton badge manager implementation
- (chat): base fallback badges implemented
- (chat): better timestamps, and user colors now
- (chat): ability to send chat messages
- (login): build out initial login page
- (core): upgraded tailwind v3 -&gt; v4
- (chat): show basic chat messages

Fixes:

- (build): update action versions
- (core:token): fix background token watchdog
- (core:eventsub): handle error response in watchdog
- (core): censor access token when debug logging token struct
- (sidebar): cleanup sidebar paddings
- (sidebar): sizing remains consistent between open and collapsed

Misc:

- (deps): update deps
- (docs): update screenshot recent version
- (chat): move chat message index generation to rust
- (docs): update screenshot archive of recent versions
- (login): update login flow to use new synced state
- (login): split up token flow in prep for synced state
- (core): move logged in app under /app
- (deps): update shadcn components
- (build): generate sumry file
- move vergo out of repo
