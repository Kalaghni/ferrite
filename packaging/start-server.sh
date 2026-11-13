#!/bin/sh
# Dedicated FERRITE server. Edit the options below to taste.
#   --port N     UDP port to listen on (default 27500)
#   --bots N     players per team; empty slots are bots (default 12)
#   --skill X    bot skill, 0.7 easier, 1.3 harder (default 1.0)
#   --tickets N  tickets per team per round (default 250)
cd "$(dirname "$0")" && exec ./ferrite-server --port 27500 --bots 12
