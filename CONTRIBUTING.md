# Contributing to msgs

Thank you for helping improve `msgs`.

## Before opening a pull request

1. Discuss substantial behavior or protocol changes in an issue first.
2. Keep changes focused and add tests for new behavior or bug fixes.
3. Run:

```sh
make check
cargo build --release
```

4. Update user-facing documentation when commands or behavior change.

## Project invariants

Contributions must preserve these guarantees unless a public design discussion
explicitly changes them:

- no central relay or elected host;
- no message history sent to late joiners;
- no messages, identities, peers, or application logs persisted;
- bounded in-memory state and protocol frames;
- transparent terminal backgrounds;
- authenticated encryption for discovery and peer traffic.

Do not include secrets, company VPN details, real chat content, or personal data
in tests, issues, or pull requests.

## Security reports

Do not report vulnerabilities in public issues. Follow `SECURITY.md`.

## Pull requests

Use the pull-request template, explain the user impact, and keep commits
reviewable. By contributing, you agree that your contribution is licensed
under the MIT License.
