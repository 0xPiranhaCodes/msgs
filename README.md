# msgs

`msgs` is an ephemeral, serverless VPN chat with a keyboard-first terminal
interface. Every client connects directly to the others; there is no dedicated
host and no durable history.

The TUI deliberately leaves every background cell unset, preserving your
terminal's theme, transparency, or background image.

## Project status

`msgs` is an early `0.1.x` project intended for trusted teams on the same VPN.
The supported platform is macOS on Apple Silicon and Intel. Protocol
compatibility may change before version `1.0`.

## Requirements

- macOS
- Rust 1.87 or newer when installing from source
- A global Git identity:

```sh
git config --global user.name "Your Name"
git config --global user.email "you@company.example"
```

## Install

```sh
cargo install --path .
msgs
```

Everyone enters the same strong room passphrase. It is read without terminal
echo, used to derive discovery and Noise transport keys, and never saved.

## Build and share a macOS binary

The Makefile can produce one unsigned universal binary for Apple Silicon and
Intel Macs:

```sh
make help
make setup
make check
make dist
make verify-dist
```

Send the two generated files together:

```text
dist/msgs-<version>-macos-universal.tar.gz
dist/msgs-<version>-macos-universal.tar.gz.sha256
```

The archive includes `INSTALL.md` with checksum, installation, and Gatekeeper
instructions for the recipient. The build is intentionally unsigned; only
approve or remove quarantine after verifying the checksum and trusting the
sender.

For a local current-architecture build or installation:

```sh
make build
make install
```

`make install` defaults to `~/.local/bin`; override it with
`make install PREFIX=/another/prefix`.

## Keyboard controls

The app opens in compose mode.

| Key | Action |
| --- | --- |
| `Enter` | Send |
| `Esc` | Enter navigation mode |
| `i` | Return to compose mode |
| `j` / `k`, arrows | Scroll in navigation mode |
| `PgUp` / `PgDn` | Scroll by page |
| `Tab` | Switch pane |
| `p` | Toggle peer pane |
| `?` | Show key help |
| `q` | Quit from navigation mode |
| `Ctrl+C` | Quit from any mode |

The composer also accepts `/who` and `/quit`.

## VPN discovery

Clients announce on UDP multicast `239.255.77.77:45872` and accept encrypted
peer links on TCP port `45873`. Firewall and VPN policy must permit those
flows.

If the VPN blocks multicast, seed the mesh with any running participant:

```sh
msgs --peer 10.20.30.40
```

Repeat `--peer` for multiple seeds. A seed securely introduces its connected
peers so every client forms direct links. Use an explicit port when needed:

```sh
msgs --peer 10.20.30.40:5000 --port 5001
```

## Privacy and security

Discovery uses XChaCha20-Poly1305. Peer links use a Noise PSK handshake and
encrypted transport. Both are rooted in the room passphrase.

Choose a strong passphrase. Captured discovery traffic can be used to test
passphrase guesses offline. Git names and emails are display labels, not
verified corporate identities; anyone with the passphrase can claim another
label.

`msgs` keeps at most 1,000 current-session entries in memory. It writes no
messages, identities, peers, passphrases, configuration, or application logs
to disk, and newcomers receive no earlier messages. Your terminal emulator,
operating system, or device-management software may independently retain
screen contents.

## Development

```sh
make check
```

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before
opening an issue or pull request, and follow the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Security

Do not report vulnerabilities in public issues. Follow
[SECURITY.md](SECURITY.md) and use GitHub private vulnerability reporting.

## License

`msgs` is available under the [MIT License](LICENSE).
