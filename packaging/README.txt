FERRITE - 12v12 front-line conquest
https://github.com/Kalaghni/ferrite

NEW IN v0.10
  Four game modes: Conquest, Team Deathmatch (fast), Capture the Flag
  (slow) and Stronghold (hold the centre). Pick one on HOST & PLAY, or
  MIXED for a new one each round; the lobby can vote for another.
  Kill streaks: at 3, 5 and 7 kills your target drops armour, overdrive
  or overcharge. Other kills may leave a health pack or ammunition.
  Walk over a pickup to take it - so can anyone else.
  Credits: every round pays, a win far more than a loss. Spend them in
  the ARMORY on seven new weapon finishes and six new soldier looks.
  Everything from earlier versions stays free.
  A first launch opens with a short introduction and 400 credits.
  The classic maps each have their own time of day and weather, far more
  growing on them, wind in the grass, falling leaves, mist and birds.
  Touch controls: an on-screen stick and buttons. Try them with --touch
  or SETTINGS > CONTROLS > MOUSE > TOUCH CONTROLS.
  Everyone in a match needs v0.10.0, including the dedicated server.

NEW IN v0.9.2
  Every map is spread out by half again, with buildings, sheds, tanks,
  lookouts and ruins in the new ground between the objectives. Players
  spawn spread across open ground, away from other players.
  Pick your team in the lobby (M). Your sight for each weapon is
  remembered. Number keys change your loadout only while the loadout
  panel is open. F11 toggles fullscreen. Lava shows the damage overlay.
  Everyone in a match needs v0.9.2, including the dedicated server.

NEW IN v0.9.1
  New song in the music rotation: "Barbed Wire" by breadKernel. It plays
  on the launcher, and in matches when music during matches is on.
  v0.9.0 and v0.9.1 players can share a match.

NEW IN v0.9
  Slag Cathedral: a bell foundry on the scale of a cathedral, with a vaulted
  casting nave, raised galleries, covered service lanes and a dedicated map
  score. Enable music during matches to hear it while fighting.
  Two lava channels cross its casting floor. Standing in one burns you;
  use the grated bridges or take a running jump.
  Relay: the four antenna masts are now towers you can climb. Walk the
  stair to a railed deck 6 or 9 metres up. Rails stop you falling, not
  bullets; the two plate parapets are the cover.
  Everyone in a match needs v0.9.0, including the dedicated server.

NEW IN v0.8
  Two new maps: Spillway (drained waterworks) and Relay (highland signal
  station). The original four maps keep their layouts.
  All six soldier looks have faceted armor, team-marked field packs and
  improved movement and weapon animations based on the launcher artwork.
  Existing guns and sights have cleaner models, fitted mounts and open
  aiming apertures.
  Six finishes fit every gun: Oxide, Arctic, Woodland, Midnight, Hazard and
  Sandstorm. In the loadout, click the large primary/sidearm models or use
  7 / 8 to cycle finishes. Choices are saved per weapon and appear on the
  equipped gun immediately. A different pending gun receives its selected
  finish when deployed. The weapon roster and gameplay statistics are unchanged.

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
  Music by breadKernel: "Liquid Plasma", "Submariner", "Barbed Wire" and
  "Foundrite Lobby".
  Copyright breadKernel, all rights reserved. The music and artwork are
  not covered by the game's MIT licence and may not be reused elsewhere.
  Typeface: Quantico by Matthew Desmond, SIL Open Font License
  (see Quantico-OFL.txt).
