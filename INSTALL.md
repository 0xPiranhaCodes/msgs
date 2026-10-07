# Install a shared macOS build

You should receive these two files together:

- `msgs-<version>-macos-universal.tar.gz`
- `msgs-<version>-macos-universal.tar.gz.sha256`

The binary supports both Apple Silicon and Intel Macs.

## 1. Verify the download

In Terminal, change to the directory containing both files and run:

```sh
shasum -a 256 -c msgs-<version>-macos-universal.tar.gz.sha256
```

Continue only if the command prints `OK` and you trust the person who sent the
files.

## 2. Extract and install

```sh
tar -xzf msgs-<version>-macos-universal.tar.gz
mkdir -p ~/.local/bin
cp msgs-<version>-macos-universal/msgs ~/.local/bin/msgs
chmod 755 ~/.local/bin/msgs
```

If `~/.local/bin` is not already on your `PATH`, add this to `~/.zshrc`:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Then start a new terminal and check the installation:

```sh
msgs --help
```

## Unsigned binary and Gatekeeper

This internal build is intentionally unsigned. macOS may block it after
download. First verify the checksum and sender as described above. You can then
right-click the binary in Finder and choose **Open**, or approve it in
**System Settings → Privacy & Security**.

If macOS still retains the quarantine flag, remove it only for this verified
binary:

```sh
xattr -d com.apple.quarantine ~/.local/bin/msgs
```

Do not bypass Gatekeeper for an artifact whose checksum or sender you have not
verified.
