# ravel-ui

A front door to every [ravel](https://github.com/repolex-ai/ravel) soul on the machine.

ravel turns an agent's conversation transcripts into a graph, and builds a **memory index** over it:
single memories at the bottom, and summaries above them, each level covering a window twice as
long as the level below. `ravel-ui` draws that tree for any soul, and opens any memory to the
conversation turns it was made from.

It is the ravel counterpart of [git-lex-ui](https://github.com/repolex-ai/git-lex-ui): one
daemon (`raveld`) sees every soul, and one page shows any of them.

## Run it

`raveld` must be running (it listens on `127.0.0.1:7881`). Then:

```sh
cd web && npm install && npm run build && cd ..
cargo run --release -- --port 8889
```

The server binds `127.0.0.1:8889` and opens your browser.

| Flag | Default | Description |
|---|---|---|
| `--port <n>` | `8889` | Port for the front door. Fails loudly if taken. |
| `--daemon-port <n>` | `7881` | Where `raveld` listens. |
| `--no-open` | `false` | Do not open a browser tab on startup. |
| `--web <dir>` | `./web/dist` | Frontend build directory, read on every request. |

For frontend work, run `npm run dev` in `web/` as well. Vite serves the page on
`http://localhost:5174` and forwards `/api` to the server on 8889.

## What it draws

- **Left:** every soul raveld holds, grouped into souls with a memory index, souls not indexed
  yet, and the demo props. Below that, a search over the selected soul's memories and summaries.
- **Middle:** the memory tree as a timeline. Time runs left to right. The bottom row is single
  memories, one tick each, at the moment they happened. Each row above summarizes windows twice as
  long as the row below. Scroll to zoom, drag to pan, double-click to fit. The soul's startup view
  (what `ravel memory` prints) is coloured on top, ochre for its oldest lines and green for now. Selecting a node darkens
  the windows that contain it and the nodes it covers.
- **Right:** with nothing selected, what the startup view costs, by level. With a node selected, its text, the summaries it sits inside, and what it was made from:
  the windows a summary covers, or the source turns a memory was read from.

The address bar carries the soul and the selection (`#e3d71e/m886192468fecb91`), so a view of
one memory can be sent to someone.

## Read-only

`raveld` can also sync, import, run the memory index (which spends money on model calls) and
shut down. `ravel-ui` reaches none of those. Its server forwards only the read endpoints it names,
and refuses any soul or memory id that would need escaping.
