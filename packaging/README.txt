FERRITE - 12v12 front-line conquest
https://github.com/Kalaghni/ferrite

PLAY
  Run ferrite (ferrite.exe on Windows). The home screen lets you:
    - join a server: type its address (port defaults to 27500) and press JOIN,
      or pick one from the saved list - servers you join are remembered
    - host a match yourself: HOST AND PLAY starts a server inside the game,
      fills empty slots with bots, and lets others on your network join you
  Your callsign and saved servers are kept in ferrite-client.txt next to the game.

RUN A DEDICATED SERVER
  Run start-server.bat (Windows) or ./start-server.sh (Linux, macOS).
  It listens on UDP port 27500 and logs a status line every 10 seconds.
  Options: --port N  --bots N  --skill X  --tickets N  --seed N
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
  LMB fire       RMB aim          R reload        G grenade
  Q/F/wheel swap to sidearm       T change sight  1/2/3 pick kit
  Tab scoreboard F3 net stats     -/= mouse sensitivity
  Esc release mouse, then Q to leave the match

SKIP THE HOME SCREEN
  ferrite --connect host[:port]     join directly
  ferrite --host                    host directly
