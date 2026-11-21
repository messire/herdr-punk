# herdr-punk changelog

Changes of this fork on top of upstream [herdrdev/herdr](https://github.com/herdrdev/herdr).
Upstream changes are in `CHANGELOG.md`.

## Unreleased — branch `feat/server-only`
- `deck-server`: server-only binary without the TUI client, for Netrunner Deck.

## Preview punk-2026-11-03 — branch `feat/accounts-sidebar-section`
- `pane.read` reports the terminal cursor and the alternate-screen flag.

## Preview punk-2026-10-24
- Coalesced `pane.output_changed` events with a real `pane.read` revision.
- `pane.output_changed` available as an `events.subscribe` subscription.
- CI: output-event and key-name tests in the Windows build; tolerant prerelease publishing.

## Preview punk-2026-10-23
- Client-side interference animation for a tab bar entry (`ui.tab_bar_glitch`).

## Preview punk-2026-10-19
- Daemon theme with a pane ANSI palette.
- `ui.tab_bar_right_styles` for right-side tab bar entries.
- `terminal.powershell_startup_command` for new Windows panes.
- Folded agents section stays under the spaces content.
