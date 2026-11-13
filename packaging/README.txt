FERRITE - 12v12 front-line conquest
https://github.com/Kalaghni/ferrite

PLAY
  Run ferrite (ferrite.exe on Windows). The home screen lets you:
    - join a server: type its address (port defaults to 27500) and press JOIN,
      or pick one from the saved list - servers you join are remembered
    - host a match yourself: pick a MAP, then HOST AND PLAY starts a server
      inside the game, fills empty slots with bots, and lets others on your
      network join you
  Pick your team (or AUTO) next to your callsign; press M in a match to
  change sides.
  Your callsign, team, mouse sensitivity and saved servers are kept in
  ferrite-client.txt next to the game.

RUN A DEDICATED SERVER
  Run start-server.bat (Windows) or ./start-server.sh (Linux, macOS).
  It listens on UDP port 27500 and logs a status line every 10 seconds.
  Options: --port N  --map NAME  --bots N  --skill X  --tickets N  --seed N
  Maps: foundry valley, quarry, ridgeline, depot (--maps lists them). A name
  can be shortened: --map ridge. Players get the map from the server.
  Players join by typing this machine's address into the home screen.

  For others to reach it:
    - allow the program through the firewall when asked (UDP 27500)
    - to host over the internet, forward UDP 27500 on your router to this machine

PLATFORM NOTES
  Windows, Linux and macOS players can all join the same server as long as
  everyone uses the same release.
  macOS: the build is not signed. If macOS refuses to open it, run this once
    in the unpacked folder:   xattr -dr com.apple.quarantine .
  Linux: needs X11, OpenGL and ALSA, which a normal desktop install has.

CONTROLS
  WASD move      Shift sprint     Space jump      Ctrl/C crouch
  Crouch while sprinting to slide. Hold Space while pushing into a ledge to
  climb it. Long bursts open your spread; fire short ones at range.
  LMB fire       RMB aim          R reload        G grenade
  Q/F/wheel swap to sidearm       T change sight  1/2/3 pick kit
  M change team  Tab scoreboard   F3 net stats    -/= mouse sensitivity
  Esc release mouse, then Q to leave the match

GRAPHICS
  ferrite --shadows low      smaller shadow map, for older graphics cards
  ferrite --shadows off      no sun shadows at all

SKIP THE HOME SCREEN
  ferrite --connect host[:port]     join directly
  ferrite --host                    host directly
  ferrite --host --map quarry       host directly on a chosen map
