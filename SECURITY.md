# Security Policy

## Supported versions

Security fixes are applied to the latest released `0.1.x` version and the
current default branch.

## Reporting a vulnerability

Do not open a public issue or discussion for a suspected vulnerability.

Use GitHub's **Report a vulnerability** private-reporting feature on the
repository's Security page. Include:

- affected version and platform;
- reproduction steps or a proof of concept;
- expected impact;
- suggested mitigation, if known.

Avoid including real network/VPN addresses, identities, or message contents.
Maintainers will acknowledge a complete report within seven days and
coordinate disclosure after a fix is available.

## Scope

High-priority reports include authentication bypass, plaintext disclosure,
key leakage, remote code execution, unsafe file creation, and unbounded
remote resource consumption.

Git display names and email addresses are intentionally self-asserted labels,
not verified corporate identities. Room membership is derived from a public
email domain, not a secret; anyone who knows an employer's email domain and
can reach the network joining that room is a documented design limitation
(see `CONTEXT.md` and the README's Privacy and security section), not an
authentication bypass.
