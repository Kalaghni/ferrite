# Security

## Reporting a vulnerability

Please report security problems privately, not in a public issue.

Use GitHub's private vulnerability reporting: on the repository page go to
**Security > Report a vulnerability** (or open
`https://github.com/Kalaghni/ferrite/security/advisories/new`). That creates
a draft security advisory only the maintainers can see. There is no security
email address.

Include what you can of:

- the version (`ferrite --version`, `ferrite-server --version`) and platform;
- what an attacker needs (on the same network, on the path, able to send UDP
  to the server, a malicious server the victim joins, and so on);
- steps or a proof of concept, and what happens.

This is a small project with no fixed response time. A fix ships in a new
release, and the advisory is published once that release is out.

## Supported versions

Only the latest release gets security fixes. Everyone in a match needs the
same release anyway (the network protocol version must match), and the
launcher updates itself, so there are no maintained older branches.

## Threat model

This section describes FERRITE as of v0.12 (network protocol 19). It says
what is defended and, as plainly, what is not.

### What the server defends against

**Spoofed source addresses.** A client's first `Hello` gets only a small
`Challenge` carrying a cookie: a keyed hash of the client's address, a
per-process random key and a time bucket. The server allocates a player slot
only when the `Hello` comes back with a valid cookie, which a sender can only
do if it receives packets at that address. So spoofed addresses cannot take
slots, and the server cannot be used to reflect snapshots at a victim. The
`Challenge` is smaller than the `Hello` that caused it, so there is no
amplification either.

**Off-path impersonation.** The `Welcome` carries a random 64-bit session
token, and every later packet from that client must include it. Someone who
cannot see the traffic cannot send commands, chat or a disconnect on another
player's behalf.

**Version confusion.** Every client datagram starts with the protocol
version. An old client gets a clear "protocol mismatch" reply instead of
being misparsed.

**Resource exhaustion.** The server bounds the work any sender can cause:

- a cap on datagrams processed per tick, and per source address;
- a cap on connections per IP address and in total (64);
- rate limits on chat, icon requests, team switches and repeated `Welcome`s;
- size-limited bincode decoding (64 KiB); malformed input is dropped, never
  a panic;
- names, chat text and icons are validated and sanitised;
- the kill-feed log is bounded (the newest 256 entries).

**Cheating through the protocol.** Game logic is server-authoritative.
Clients send only inputs (buttons and view angles); the server re-simulates
movement with the same code and never accepts a position from the network.
Commands are bounds- and sequence-checked, and a per-player budget limits
how much simulated time a client can claim. Fire rate, ammunition and hits
are decided on the server, with lag compensation capped at 0.5 s of rewind.

### What it does not defend against

**Anyone on the network path.** Traffic is neither encrypted nor
authenticated end to end. Someone who can see the packets (same Wi-Fi, a
compromised router, an ISP) can read chat and game state, read the session
token, and so alter or inject packets for that player. The cookie and token
stop off-path attackers only.

**Identity.** Player ids and icons are not accounts. A player id is taken on
trust: whoever runs a server you join learns your id and could reuse it on
another server. Names are not reserved.

**Malicious servers.** A server you join decides everything you see in the
match. The client decodes its packets with the same size limits and treats
malformed ones as lost, but a hostile server is not otherwise contained.

**Client-side cheats.** The server stops a client from moving faster, firing
faster or hitting what it could not hit. It cannot stop a modified client
from aiming for the player or showing hidden information it was sent.

**Denial of service by volume.** The caps above stop one sender from making
the server do unbounded work, but cannot stop a flood large enough to fill
the network link.

### Hosting on the public Internet

FERRITE was built for LAN and VPN play. If you host on the Internet:

- Run the dedicated server (`ferrite-server`), not a match hosted from the
  game client, so a game crash or exploit is not on your desktop session.
- Run it on a machine or container that exposes only UDP 27500 (game
  traffic). UDP 27499 is LAN discovery (multicast and broadcast); do not
  expose it to the Internet. `--no-discovery` turns it off.
- Run it as an unprivileged user. The image in `deploy/Dockerfile` runs as
  `nobody` and writes only to `/data`. Note that `deploy/docker-compose.yml`
  uses host networking so discovery works on a LAN; on an Internet-facing
  host, firewall 27499 or publish only 27500/udp instead.
- Keep it updated: run the same release as your players, and update when a
  new one is out.

### Updates

The launcher's updater, as of v0.12:

- downloads only from this project's GitHub releases, over HTTPS (using the
  system's `curl`);
- checks the SHA-256 of the downloaded package against the release's
  `SHA256SUMS` file before installing it, and refuses it on a mismatch;
- keeps the previous version and rolls back automatically if the new one
  fails to start.

`SHA256SUMS` is published on the same release page as the packages, so it
protects against corrupted downloads and tampered mirrors or caches, not
against someone who controls the GitHub account or the release workflow. The
builds are not code-signed yet (Windows SmartScreen and macOS Gatekeeper warn
about them for the same reason). Builds run from a cargo `target` folder never
update themselves.

### Dependencies

CI runs `cargo deny check` (advisories, bans, licences, sources) on every
push and pull request; the configuration is `deny.toml`. A RustSec
vulnerability fails the build. Two advisories are ignored on purpose, and are
known risks:

- **RUSTSEC-2025-0141 - bincode 1.x is unmaintained.** bincode is FERRITE's
  wire format. The advisory reports no vulnerability, only that the crate
  gets no more fixes. Decoding is size-limited to 64 KiB and bad input is
  dropped. If a bincode bug is found it will not be fixed upstream; moving to
  another format is a protocol change and is tracked separately.
- **RUSTSEC-2025-0035 - macroquad soundness issues.** macroquad's global
  mutable state lets safe code that misuses its API cause a use-after-free.
  There is no fixed version. FERRITE calls macroquad from the main thread
  only, and network input does not reach it, so we do not know of a way to
  trigger it remotely. Replacing the framework would be a renderer rewrite.

Two crates, miniquad and quad-snd, are vendored with small patches
(`vendor/README.md`). RustSec advisories are matched against crates.io
packages, so a future advisory for either needs checking by hand against the
vendored copy.
