# Development

## Prerequisites

- Rust 1.70 or later
- tmux (for testing)

## Building from source

```bash
git clone https://github.com/yysd5/tsps.git
cd tsps
cargo build --release
```

The binary will be available at `target/release/tsps`.

## Testing

To test the command locally without installing:

```bash
# Build in debug mode for faster compilation
cargo build

# Test help and version
./target/debug/tsps --help
./target/debug/tsps --version

# Test with tmux (must be run inside a tmux session)
tmux new-session -d -s test-session
tmux attach -t test-session

# Inside tmux session, test the command:
./target/debug/tsps 3 .
./target/debug/tsps 4 /tmp

./target/debug/tsps -l ./examples/dev.yaml
./target/debug/tsps -l ./examples/simple.yaml
```

## Testing & Formatting & Linting

```bash
cargo test    # Testing
cargo fmt     # Formatting
cargo clippy  # Linting
```
