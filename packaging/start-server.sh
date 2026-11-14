#!/bin/sh
# Dedicated FERRITE server. Edit the options below to taste.
#   --port N     UDP port to listen on (default 27500)
#   --map NAME   foundry, quarry, ridgeline or depot (default foundry)
#   --bots N     players per team; empty slots are bots (default 12)
#   --difficulty NAME   recruit, regular (default), veteran or elite
#   --tickets N  tickets per team per round (default 250)
cd "$(dirname "$0")" && exec ./ferrite-server --port 27500 --bots 12
