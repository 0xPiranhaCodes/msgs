# msgs

`msgs` is a terminal chat client that auto-discovers and mesh-connects with
other `msgs` instances reachable on the same network, scoping who can find
each other to people at the same employer with no shared secret to type in.

## Language

**Room domain**:
The lowercased, trimmed substring after `@` in a user's git-configured
`user.email`. It is the sole input to room-key derivation — everyone whose
git email shares a room domain lands in the same room automatically.
_Avoid_: Room passphrase, roomphase, room name

**Room**:
The set of `msgs` instances that currently share a room domain and can
discover and decrypt each other's traffic. Membership is not confidential:
the room domain is public knowledge (an employer's email domain), so the
only real access control is network reachability, not secrecy of the room
domain itself.
_Avoid_: Mesh (mesh is the transport, not the group)

**Identity**:
The display name and email a peer presents, read from `git config
user.name` / `user.email` at startup. Not authenticated beyond "derived
from the device's git config" — any peer on the room can claim any name.
