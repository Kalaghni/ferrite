# Changelog

What each FERRITE release added, newest first. The full notes for each release are in [docs/release-notes](docs/release-notes/).

## v0.15

- **FORGE, the map maker.** MAP MAKER on the launcher opens a start screen of your maps, each with a plan. Start blank, on a built-in map's ground, or from a copy of a built-in map, then build in 3D with every edit mirrored for both teams. F5 playtests against bots. Maps are `.fmap` files in the `maps` folder beside the game, and unsaved work is kept and offered back.
- **The library.** A drawer of models in folders (L): drag one into the world or click to stamp it. Your own live in the `prefabs` folder.
- **Terrain.** The TERRAIN tool raises, lowers, flattens, smooths, ramps and roughens the ground, and what stands on it moves with it.
- **The FORGE MCP server.** `ferrite-forge-mcp` lets an assistant such as Claude build with you, live in the editor: edit and sculpt, draw plans, take screenshots, check the map and run a fast bot match on it.
- **Spawns.** Each team deploys across zones along its own stretch of the front line, out of the enemy's sight and away from where teammates just came in, so there is no single doorway to watch.
- **v0.15.1.** Canopy remixes show their fig again (its trunks, roots and walkways were invisible walls), posts show on custom maps, and trees can be selected, moved, resized and removed in FORGE.
- **v0.15.2.** Friendlier map maker controls: the right mouse button lowers (the opposite of the brush), middle mouse looks, a clickable panel with mode chips and size and strength buttons, Ctrl/Alt+wheel for the brush, drag-to-move and a hover outline in SELECT.
- **v0.15.3.** Public servers: JOIN SERVER lists NA1 and NA2 (and any others on the list at playferrite.com) with their map, players and your ping, and dedicated servers answer that status query on their game port.

No network protocol change (still 21). The new spawns come with the server. Custom maps can be played against bots on your own machine, and other players cannot join one yet. See the [v0.15.0](docs/release-notes/v0.15.0.md), [v0.15.1](docs/release-notes/v0.15.1.md), [v0.15.2](docs/release-notes/v0.15.2.md) and [v0.15.3](docs/release-notes/v0.15.3.md) release notes.

## v0.14

- **Canopy, a new map.** A botanical reserve in the roots and lower boughs of a colossal strangler fig, with C in the hollow under its crown. A ring of walkable boughs 18 m up joins its four trunks, reached by stairs behind B and D and by ladders on the flanks; every objective can still be reached on the ground. See the [Canopy guide](docs/maps/canopy.md).
- **Seedlines.** Six powered harvesting lines run from the collector courts up to the boughs and in a ring round the tree. Stand on a dock and press E (the new Use key) to ride the line you face: forward reels you along, back returns, crouch brakes, jump lets go. You keep your sidearm and can shoot from the line.
- **Shade curtains and the crown gust.** Hold E at a crank on a trunk to let a bank of shade cloth down across the hollow, or roll it up. The cloth hides you from players and bots but stops nothing - bullets and grenades go straight through. Every two minutes a gust tears through the crown: the cranks lock and every curtain furls for half a minute.
- **The Hollow Crown.** A leaf roof 164 m across with a well of open sky over C, root-wrapped trunks, and rainforest greens in place of autumn.
- **A new map picker.** HOST & PLAY lists the maps down the right of the screen with a top-down plan of each, marked for the chosen mode. The lobby vote is a grid of map cards with plans, vote counts and meters, the modes in a column beside them, and a tip on each card; the arrow keys move your vote.
- **Flags on Canopy.** A flag dropped up the tree falls to the nearest open ground, so it is never stranded where only a climber can reach it.
- **v0.14.1.** HD weapons with rounded parts and fine detail, every setting (key bindings included) reachable from the pause menu during a match, cover props rebuilt with real shapes (Jersey barriers, corrugated containers, sandbag walls, braced crates, storage tanks), and painted enamel nameboards with emblems and lamps in place of the old dot signs.

The network protocol changed (21), so everyone in a match needs v0.14.0, including the dedicated server. See the [v0.14.0](docs/release-notes/v0.14.0.md) and [v0.14.1](docs/release-notes/v0.14.1.md) release notes.

## v0.13

- **Slag Cathedral's lava runner.** When the volcano erupts, lava pours down a stone runner on the centre line, across the south yard and through the nave, cutting the south side in two for half a minute before it crusts over. Cross on the gratings, over the gallery, or go round. See the [Slag Cathedral guide](docs/maps/slag-cathedral.md#the-lava-runner).
- **Spillway's gate deck and flood.** C stands on the old floodgate's deck, raised over the channel, with bridges and stairs. After each siren test the gates open and the channel floods to 1.9 m for about 45 seconds, and the water is deadly. See the [Spillway guide](docs/maps/spillway.md).
- **Relay's great mast.** BACKBONE now stands on the north flank, the same distance from both spawns, and its ladders can be climbed to a caged platform at 20 m and a top deck at 36.75 m. See the [Relay guide](docs/maps/relay.md#backbone-the-great-mast).
- **Tunnel portals.** The rock round the mine and cave openings is one continuous face, with no floating plate over the sign and no gaps at the corners; v0.13.1 also closes the corners of the cave's mouths seen from inside.
- **v0.13.2.** The volcano rumbles and blows with a boom you hear across the map, its lava pours down the cone as a molten river, killcams always reach the kill, the armory's soldier and gun turn when you drag them, and the lobby header has room on larger screens.
- **v0.13.3.** A new FERRITE logo: the wordmark in the launcher, lobby and pause screen, and a new app icon (v0.13.4: the //F mark on the dark tile).

The network protocol changed (20), so everyone in a match needs v0.13.0, including the dedicated server. See the [v0.13.0](docs/release-notes/v0.13.0.md), [v0.13.1](docs/release-notes/v0.13.1.md), [v0.13.2](docs/release-notes/v0.13.2.md), [v0.13.3](docs/release-notes/v0.13.3.md) and [v0.13.4](docs/release-notes/v0.13.4.md) release notes.

## v0.12

- **Underground.** Quarry's mine and Ridgeline's cave now run under the ground. The mine opens from the first terrace into a timbered drift with rails and lamps, passes under the headframe and climbs an incline back to daylight; the cave bends through the ridge under several metres of rock. See the [expanded map guide](docs/maps/landmarks.md#underground-tunnels).
- **The Fallen Giant, crashed.** The aircraft is broken in three, with torn skin, exposed frames, burnt-out engines and a furrow of debris. Buckled frames inside the hold break the long sightline through C, pallets give C cover, and every part at walking height is solid. See the [Fallen Giant guide](docs/maps/fallen-giant.md).
- **Monuments.** A broken railway viaduct with a locomotive hanging over the gap above Foundry Valley, a walking excavator whose boom reaches over the Quarry, and a hydroelectric dam that tests its sirens beyond Spillway. All are scenery beyond reach.
- **Weapon finishes.** Every finish has its own pattern, material and detailing - digital camo, tiger stripes, hazard chevrons, Damascus steel, carbon weave, glowing magma and honeycomb, an engraved gold scroll.
- **Cinematic camera.** Fly a free camera with adjustable field of view and real depth of field, hide the HUD, and save screenshots at up to four times screen size (see [Cinematic camera](README.md#cinematic-camera)). The launcher and lobby backgrounds are now shots staged in the game with it.
- **Safer online play.** A new session handshake stops spoofed addresses taking slots or impersonating players, and the server limits how much work any one address can cause. Map events (the volcano, machinery, wind) now run on the server's clock, so every player - including late joiners - sees them at the same moment.
- **Safer updates.** The launcher checks each download against the release's published SHA-256 sums, keeps the previous version and puts it back by itself if the new one fails to start; SETTINGS > VIDEO > ROLL BACK goes back by hand.

The network protocol changed (19), so everyone in a match needs v0.12.0, including the dedicated server. See the [v0.12.0 release notes](docs/release-notes/v0.12.0.md), [SECURITY.md](SECURITY.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## v0.11

- **Landmarks.** The four oldest battlefields gain authored destinations: boiler workshops and a service culvert in Foundry Valley, mine adits under a working headframe in the Quarry, ranger lodges, rock passages and wind turbines on Ridgeline, and container passages and warehouses under a 52-metre gantry in the Depot. Relay gains a giant cellular mast; a distant volcano beyond Slag Cathedral can erupt.
- **The Fallen Giant.** A crashed cargo aircraft lies across Ridgeline's saddle with C on the floor of its walkable hold, open at both broken ends and through three tears in each side. See the [Fallen Giant guide](docs/maps/fallen-giant.md).
- **Explosive changes.** Marked wall panels, walkways and machine controls break, collapse or shut down when a grenade goes off beside them. The server confirms each, updates collision and bot navigation, and a new round restores them. See the [expanded map guide](docs/maps/landmarks.md).
- **Soldiers.** Rebuilt to match the launcher art: faceted pale armour over a dark suit, a domed helmet with a glowing visor, big shoulder plates and a team-coloured pack lid.
- **Twice the detail.** About twice the polygons on every map: a finer ground mesh, finer round parts, rounder trees and bending grass.
- **Fixes.** Bots no longer see through tree canopies. In Capture the Flag the DEFEND marker no longer covers BRING IT HERE while you carry a flag.

The network protocol changed (18), so everyone in a match needs v0.11.0, including the dedicated server. See the [v0.11.0 release notes](docs/release-notes/v0.11.0.md).

## v0.10

- **Game modes.** Conquest (long), Team Deathmatch (fast: forward spawns, three-second redeploys, first to the kill target), Capture the Flag (slow: flags on B and D, three captures) and Stronghold (medium: hold C, a point a second). The host picks one or MIXED on HOST & PLAY; the lobby can vote for another before each round. Bots play all four.
- **Kill streaks and pickups.** Three kills without dying and your target drops armour; five, overdrive; seven, overcharge. Other kills may leave a health pack or a resupply. Anything on the ground can be taken by anyone.
- **Credits and the armory.** A round pays 120 to 240 credits for a win and 45 to 165 for a loss. ARMORY on the home screen sells seven new weapon finishes and six new soldier looks, previewed on a soldier standing on the map. Everything from before v0.10 stays free.
- **Getting started.** A first launch walks through callsign and icon, the modes, the controls and credits, then offers a practice match and 400 credits to start with.
- **Foliage and atmosphere.** The five classic maps each have their own hour and weather now: a gold autumn afternoon in Foundry Valley, white dusty noon in the Quarry, cold morning mist on Ridgeline, sunset over the Depot, grey overcast at the Spillway. Bushes, tall grass, flowers, ferns, reeds, ivy, moss, deadwood and saplings grow on the field, the forest thickens beyond the fence, and grass and leaves move in the wind. Leaves fall, motes drift in the light, mist lies in low ground, dust blows and birds circle. Relay gains heather and highland grass. None of it collides, and cover-sized plants are mirrored so both sides get the same.
- **Touch controls.** An on-screen stick, drag-to-look and buttons, multi-touch. `--touch` (or SETTINGS > CONTROLS > MOUSE) tries them on a desktop with the mouse as one finger. No phone build yet.

The network protocol changed (16), so everyone in a match needs v0.10.0, including the dedicated server. See the [v0.10.0 release notes](docs/release-notes/v0.10.0.md).

## v0.9.2

- **Bigger battlefields.** Every map is spread out by half again, to 354 by 204 metres. Neighbouring objectives are 59 to 98 metres apart instead of 39 to 66, the bases 318 metres apart instead of 212, and each map's hills, pit, ridge, channel or plateau grew with it. Buildings keep their size and their place beside the objective they belong to.
- **More to fight through.** The new ground on every map holds buildings with doors and windows, open sheds, tanks, stair-climbed lookouts and walled ruins, themed to the map: farms and workshops in Foundry Valley, site offices and machine sheds in Quarry, cabins and hunters' lookouts on Ridgeline, warehouses in Depot, pump houses and settling tanks on Spillway, equipment huts on Relay, and ingot stores and gas holders round Slag Cathedral.
- **Spread-out spawns.** Players deploy anywhere on open ground up to 20 metres from the centre of their base, or 6 to 16 metres from a point their team holds, choosing the spot farthest from other players.
- **Smaller things.** Standing in lava shows the damage overlay. Teams are picked in the lobby, not the launcher. Your sight for each weapon is remembered between matches. Number keys change your loadout only while the loadout panel is open. F11 toggles fullscreen. Surfaces on Slag Cathedral no longer flicker as you look around.

Collision changed on every map, so v0.9.2 cannot share a match with v0.9.1 or earlier, including the dedicated server (protocol 15). See the [v0.9.2 release notes](docs/release-notes/v0.9.2.md).

## v0.9.1

Adds "Barbed Wire" by breadKernel to the music rotation (launcher, and matches when music during matches is on). Nothing else changed: v0.9.0 and v0.9.1 players can share a match. See the [v0.9.1 release notes](docs/release-notes/v0.9.1.md).

## v0.9

- **Slag Cathedral.** A bell foundry on the scale of a cathedral: a vaulted crucible nave, furnace towers, raised side galleries and covered service flanks, with its own score. Two lava channels cross the casting floor and burn anyone who stands in them; bridges and a running jump get you over. Bell yards, lavafalls, a steam organ and leaded glass tell the story of a works still running with nobody on shift. See the [map guide](docs/maps/slag-cathedral.md).
- **Relay, rebuilt.** The four antenna masts are now towers you can climb: walk the stair to a railed deck 6 or 9 metres up and hold the view over an objective. A listening dish stands in each half, and the station has its own evening light, far ranges and Morse beacons. See the [map guide](docs/maps/relay.md).
- **Custom controls.** SETTINGS > CONTROLS has MOVE, FIGHT, UI and KIT pages for rebinding keyboard and mouse actions. Click an action to replace its bindings, or click + to add another key or mouse button. Mouse 4 and Mouse 5 are supported; conflicting assignments on other actions are rejected. Escape cancels, Delete / Backspace clears, and RESET KEYBINDS restores defaults. Bindings are saved between launches.
- **Controllers.** SETTINGS > CONTROLS > PAD enables controllers and adjusts stick sensitivity, deadzone and Y inversion. Left stick moves, right stick looks, LT aims, RT fires; A jumps, B crouches, X reloads, Y swaps, LB throws a grenade, RB / right stick melees, and left stick toggles sprint. D-pad changes sight / loadout / team / stats; View / Back shows the scoreboard and Start pauses. In menus, move the cursor with a stick or D-pad, A selects and B goes back.
- **Live finishes.** Weapon finishes repaint equipped guns immediately and replicate to other players. Finishes selected for a different pending gun apply when that gun is issued.

The network protocol and two maps changed, so everyone in a match needs v0.9.0, including the dedicated server. See the [v0.9.0 release notes](docs/release-notes/v0.9.0.md). On Debian / Ubuntu, source builds also need `libudev-dev` and `pkg-config` for controller support.

## v0.8.1

Fixes the repeated update prompt after installing v0.8.0: those packages were labelled 0.8.0 while their executables still reported 0.7.0. The compiled version is now 0.8.1, restarts target the installed executable, and release checks reject mismatched tags or binaries. Both executables support `--version` / `-V` for verification. See the [v0.8.1 release notes](docs/release-notes/v0.8.1.md).

## v0.8

- **Two new maps.** Spillway brings drained waterworks, pump halls and a floodgate; Relay adds a highland communications station, antenna towers and rocky pine flanks. Six maps are now in the rotation. The original four retain their terrain, cover and objectives.
- **New soldier shapes and animation.** All six looks have layered, faceted armor and team-marked field packs based on the launcher artwork, with articulated movement, planted feet and hands that follow the weapon's action.
- **Cleaner guns and sights.** The existing guns have corrected barrels, receivers, magazines and reload actions, shared between first person, soldiers and loadout previews. Iron sights and optics have connected mounts, protected posts and clear, aligned aiming apertures.
- **Six weapon finishes.** Oxide, Arctic, Woodland, Midnight, Hazard and Sandstorm add bands or digital camouflage to every gun. Click the large loadout models or press 7 / 8 to cycle finishes; the choice is saved per weapon and appears immediately on the equipped gun.
- **Rendering and finish fixes.** Larger model batches render in full, local corpses keep their weapon finish, and finish changes persist after direct host/join launches. Map voting fits all six choices.

The weapon roster and gameplay statistics are unchanged. See the [full v0.8 release notes](docs/release-notes/v0.8.0.md) for fixes and verification details.

## v0.3 to v0.7

v0.3 was a from-scratch rewrite (nothing from the v0.2 prototype is reused; it remains in git history). v0.4 built on it:

- **Movement.** Slide (crouch at a sprint), climb ledges up to head height (hold jump into them), and a touch more weight when starting and stopping.
- **Gunplay.** Recoil climbs and wanders while you hold the trigger and settles when you let go; sustained fire opens the cone, so bursts beat spraying at range. Assists are scored.
- **Bots.** They run from grenades, and the cautious ones fall back when badly hurt instead of fighting to the death.
- **Graphics.** Real sun shadows for the whole valley and everyone in it, light from muzzle flashes and explosions, valley haze, surface grain, grass and pebbles, bullet holes, scorch marks, shell casings, dust.
- **Animation.** Soldiers stride in the direction they move, lean into a run, kneel, tuck for a jump, slide, lower the weapon to sprint or reload, flinch when hit and fall where they die. First person has real arms, a magazine change, a working pump and bolt, and reacts to landing, strafing, sliding and climbing.
- **Sound.** It has some now - v0.3 shipped silent because the audio backend was never switched on. Shots, steps and explosions are placed left or right, enemy footsteps are audible, and near misses crack past your ear.

v0.5 adds a lobby before every round (map vote, loadouts, ready-up countdown), thirteen more guns (sixteen in all) with a loadout panel for class, primary and sidearm, map rotation, and bots that work as a squad.

v0.6 adds a new launcher and lobby look (key art, palette and typeface from the FERRITE UI kit), launcher and lobby music, a pause overlay, 3D models of every gun in the loadout picker, a bot difficulty selector, and a camera that follows whoever eliminated you while you wait to redeploy.

v0.7 adds text chat, a melee strike, a killcam, player icons, callsigns backed by a player id (your record follows you when you rename), a kill feed that puts people ahead of bots, the plumbing for weapon finishes, a Windows installer, and updates from inside the launcher.
