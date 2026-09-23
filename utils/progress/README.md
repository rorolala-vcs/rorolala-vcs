# Rorolala Utils Progress

What a long run says about itself while it runs: the signals it sends, and the lines those
signals are drawn as.

A crate of two halves and no opinion about the terminal:

- **saying** — a [`Progress`] is a handle a work hands down to what it starts, and a [`Task`]
  is one thing being done. A run that wants no progress says nothing with
  [`Progress::silent`], so the same code runs either way.
- **drawing** — [`total_line`], [`task_line`] and [`bar`] turn what was said into the lines a
  terminal shows, and nothing here writes to one: what reads the signals decides that, and
  whether it draws them, writes them out as records, or ignores them.

```text
⠋ [============        ]
⠋ Task ↑ [===>     ] a thing being done
⠋ Task ↓ [     <===] another thing being done
```
