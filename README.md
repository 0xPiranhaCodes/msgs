# msgs

`msgs` is an ephemeral, serverless chat with a keyboard-first terminal
interface. Every client connects directly to the others; there is no dedicated
host and no durable history. Everyone whose git email shares a domain (e.g.
`@yourcompany.com`) and is reachable on the same network auto-joins the same
room — no passphrase to type or share.

The TUI deliberately leaves every background cell unset, preserving your
terminal's theme, transparency, or background image.

## Project status

`msgs` is an early `0.1.x` project intended for trusted teams on the same
network. The supported platform is macOS on Apple Silicon and Intel. Protocol
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

`msgs` derives discovery and Noise transport keys from your git email's
domain and joins automatically — nothing to type, nothing to share.

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

## Network discovery

Clients announce on UDP multicast `239.255.77.77:45872` and accept encrypted
peer links on TCP port `45873`. Firewall and VPN policy must permit those
flows; many VPNs (hub-and-spoke WireGuard, some corporate VPNs) don't forward
multicast between peers at all, so you may see no one even though you're on
the same network.

If multicast doesn't reach everyone, seed the mesh with any running
participant:

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
encrypted transport. Both are derived from your **room domain** — the part of
your git `user.email` after `@` (see `CONTEXT.md`), lowercased and trimmed.
There is no passphrase to choose or share: everyone with a matching email
domain who is reachable on the network joins the same room automatically.

This means room membership is **not a secret** — an employer's email domain
is public knowledge, not a shared secret you keep safe. The only real access
control is network reachability: the room domain just partitions a shared
network into per-company rooms so you don't see every other company's
traffic. Don't rely on this for anything that needs protection from someone
who already knows your company's domain and can reach your network (e.g. a
guest on the same office Wi-Fi). Git names and emails are display labels, not
verified corporate identities; anyone on the room can claim another label.

`msgs` keeps at most 1,000 current-session entries in memory. It writes no
messages, identities, peers, configuration, or application logs to disk, and
newcomers receive no earlier messages. Your terminal emulator, operating
system, or device-management software may independently retain screen
contents.

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
