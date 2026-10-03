# Fusion analysis board

Double-click `Start-Analysis-Board.cmd` in the repository root. It builds the native engine and opens the board in your default browser. Keep its terminal open; Ctrl+C stops the local server. The first build can take several minutes. Rust and the Windows C++ build tools must be installed (the same tools used for the existing Rust build).

From PowerShell, you can also run:
```powershell
cd "C:\Users\dcruz\Documents\Github Projects\fusion"
.\Start-Analysis-Board.cmd
```

1. Click **Load example** to create a four-line stack with a well for an I piece, or draw your own settled blocks.
2. Click/drag empty cells to fill; click/drag filled cells or right-drag to erase. Keyboard users can toggle a focused cell with Space/Enter.
3. Choose current piece and hold. Enter the next queue in order, excluding the current piece. Spaces and commas are accepted.
4. Optionally enter combo, back-to-back and pending garbage.
5. Click **Analyze position**. Green cells show Fusion's recommendation. The text indicates hold use, clears and the search score.
6. Click **Apply recommendation** to lock it, clear lines and advance the queue/hold and chain counters. Click **Undo** to restore the previous position.
7. When the queue runs out, supply new pieces before analyzing again.

This is native Fusion search served by a small loopback-only server, not a mocked recommendation. No cloud account, Python, neural weights or WebAssembly toolchain are needed. Positions show 20 visible rows; the engine retains 40 rows. Full rows are rejected because they should already have cleared. Search uses depth 8, beam 300 and a 1.5-second budget, with no speculative bag extension; quiescence may extend the line. The score is an internal search evaluation, not a probability of winning. Unknown opponent timing and future garbage arrivals are not simulated. Perfect-clear bonuses are disabled, matching the existing coaching bridge.

For development, use `cargo run --release --bin analysis_board`. Pass `-- --no-browser` to print the address without automatically opening a browser. The server chooses an available port; each launch prints its URL.

This first feature does not import replays or screenshots.

