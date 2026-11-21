# herdr-punk ideas

Possible fork features, not a plan. Context: the fork serves Netrunner Deck, see its `ROADMAP-own-runtime.md`.

## API for Deck
- Server-side filter of `pane.output_changed` by `pane_id`.
- Connection list: active clients and subscriptions with their state.
- Per-pane process stats (CPU, memory) to spot leaking agents.
- Agent status from Claude Code hooks, not only from the screen.

## Maintenance
- One integration branch on top of current upstream, rebased regularly.
- Offer `pane.output_changed` and the cursor in `pane.read` upstream to drop the fork cost.
- `deck-server` in the punk preview builds.
- Apache-2.0 hygiene: NOTICE and marks on modified files.
