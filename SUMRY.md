[commit]: # 'b328aa029b94493ef0409583a4891001c9e3429d'

Features:

- (packaging): add platform-aware build and install targets
- (updater): defer package-managed updates
- (chat): add connector translation layout
- (channel): include channel name and offline indicator in chat header
- (build): add simple makefile so workflows are simpler
- (internal): support different translation layouts
- (settings): wire in initial settings wiring and central settings object
- (badges): add badge caching so speed up startup
- (auth): token manager now supports managing multiple tokens
- (ui): update account menu and add multi-account ui components
- (ui): add image to emote tooltip
- (ui): add tooltip for badges

Fixes:

- (ui): align collapsed sidebar hovers and simplify header tags
- (ui): sidebar search input properly transitions
- (ui): cleanup search icon interaction
- (ci): update Windows test revision
- (ci): refresh Windows packaging tests
- (ci): update packaging test revision
- (ci): repair action setup and installer verification
- (logging): default internal builds to debug
- (internal): restore portable starter
- (linux): properly style window title bar and fix interaction bug
- (build): restore shared packaging from utils main
- (chat): input box should properly render on linux now
- (linux): disable nvidia explicit sync on wayland
- (chat): autoscroll better handles slow scrolling
- (chat): text formatting
- (core): fix linting and formatting
- (tooltips): adjust sizing for tooltips
- (chat): update autoscroll logic so it doesn&#39;t fight you
- (emotes): tooltip now properly renders long emote names and wide emotes
- (chat): sidebar toggling and window resizing no longer trigger chat scroll pause
- (ui): tooltips now properly render above content
- (chat): properly show username in input placeholder
- (chat): get rid of rendering stutter when scrolling is paused
- (ui): fix update notification styling

Misc:

- chore: fix deps
- (ui): refresh channel header and sidebar controls
- (deps): update deps
- (deps): update utils to 2fbbceb
- (deps): track utils main
- chore: sync internal starter interface
- (build): upgrade pnpm version
- (rust): refresh lockfile
- (release): publish arch packages to aur
- (chat): update chat paused button
- (chat): rework autoscrolling so it is more consistent
- (updater): version bump 0.2.26
- (deps): update deps
