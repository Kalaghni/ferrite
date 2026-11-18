FERRITE - 12v12 front-line conquest
https://github.com/Kalaghni/ferrite

WHAT'S NEW
  What each release added:
  https://github.com/Kalaghni/ferrite/blob/main/CHANGELOG.md

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
  ferrite-client.txt next to the game, along with your player id: servers
  keep your record under that id, so it follows you if you change callsign.

  PLAYER ICON: pick an emblem and a colour on the home screen, or drop a PNG
  on the launcher window to use your own picture (it is shrunk to 16 x 16).

UPDATES
  The launcher looks for a newer release when it opens and shows UPDATE &
  RESTART when there is one. SETTINGS > VIDEO > CHECK FOR UPDATES turns that
  off. "ferrite --update" updates from a terminal. Everyone in a match must
  be on the same release.
  A download is installed only if it matches the release's SHA256SUMS. The
  files it replaces are kept in the ferrite-previous folder until the next
  update. If a new version fails to start, the next start puts the previous
  one back by itself. To go back by hand: SETTINGS > VIDEO > ROLL BACK, or
  "ferrite --rollback" from a terminal.

RUN A DEDICATED SERVER
  Run start-server.bat (Windows) or ./start-server.sh (Linux, macOS).
  It listens on UDP port 27500 and logs a status line every 10 seconds.
  Options: --port N  --map NAME  --rotation all|off|a,b,c  --lobby SECS
           --bots N  --difficulty recruit|regular|veteran|elite
           --tickets N  --seed N
  Games on your network show up on the home screen by themselves. --name NAME
  sets what a dedicated server is called there; --no-discovery hides it.
  (Discovery uses UDP 27499; the game itself uses UDP 27500.)
  Every round starts in a lobby (map vote, loadouts, ready-up). --lobby 0
  turns it off.
  The server moves to the next map after every round. --rotation quarry,depot
  plays only those; --rotation off stays on one map.
  Maps: foundry valley, quarry, ridgeline, depot, spillway, relay, slag cathedral
  (--maps lists them). A name can be shortened: --map ridge. Players get
  the map from the server.
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
  Q/F/wheel swap to sidearm       T change sight  L loadout
  M change team  Tab scoreboard   F3 net stats    -/= mouse sensitivity
  E use: ride a seedline or work a shade crank (Canopy)
  V or middle mouse: melee (a kill from behind)
  Enter or Y: chat to everyone    U: chat to your team    (Tab switches)
  In the lobby, R is the ready key.
  Esc release mouse, then Q to leave the match

CUSTOM CONTROLS AND CONTROLLERS
  SETTINGS > CONTROLS: MOVE, FIGHT, UI and KIT pages rebind keyboard/mouse
  actions. Click a row to replace its inputs; click + to add an alternative.
  Mouse 4 and Mouse 5 side buttons are supported. To use V and Mouse 5 for
  melee, replace the row with V, then click + and press Mouse 5.
  Escape cancels, Delete clears, and RESET KEYBINDS restores defaults.
  Assignments are saved per installation.
  PAD settings enable controllers and adjust sensitivity, deadzone and Y
  inversion. LS moves, RS looks, LT aims, RT fires. A jumps, B crouches,
  X reloads, Y swaps, LB throws a grenade, RB/RS melees, LS toggles sprint.
  D-pad: sight/loadout/team/stats. View/Back: scoreboard. Start: pause.
  Menus use stick/D-pad cursor movement, A select and B back.

GRAPHICS
  ferrite --shadows low      smaller shadow map, for older graphics cards
  ferrite --shadows off      no sun shadows at all

SKIP THE HOME SCREEN
  ferrite --connect host[:port]     join directly
  ferrite --host                    host directly
  ferrite --host --map quarry       host directly on a chosen map

CREDITS
  Music by breadKernel: "Liquid Plasma", "Submariner", "Barbed Wire",
  "Foundrite Lobby" and "Slag Cathedral" (made with strudel.cc).
  Copyright breadKernel, all rights reserved. The music and artwork are
  not covered by the game's MIT licence and may not be reused elsewhere.
  Typeface: Quantico by Matthew Desmond, SIL Open Font License
  (see Quantico-OFL.txt).
