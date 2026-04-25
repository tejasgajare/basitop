## What this changes

A short summary of the change and why it's needed.

## Testing

- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` is clean
- [ ] Ran the binary locally on Apple Silicon (mention which chip)
- [ ] If the change touches `powermetrics.rs`, ran `PM_LIVE=1 sudo -E cargo test pm_live -- --nocapture --test-threads=1`

## Screenshots / recording

For visual changes, please include a before/after screenshot or terminal recording.

## Checklist

- [ ] No new clippy warnings
- [ ] No `unsafe` blocks added (or new ones are documented)
- [ ] Updated README / CONTRIBUTING if user-facing behavior changed
