# Pepo todo

- [ ] Extract autoscroll math from `src/routes/app/chat/[id]/+page.svelte` into a pure module + tests.
- [ ] Tests for `Emote` conversions in `src-tauri/src/emote/mod.rs` (`from_emote_fragment`, `From<&UserEmote>`, `From<&GlobalEmote>`)
- [ ] HTTP-mocked tests for emote providers (`src-tauri/src/emote/providers/{twitch,bttv,ffz,seventv}.rs`) and OAuth (`src-tauri/src/token.rs`)
- [ ] WebSocket-mocked tests for `src-tauri/src/eventsub.rs`.
- [ ] Tauri command integration tests via `tauri::test::mock_app()`.
