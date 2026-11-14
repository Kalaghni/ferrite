@echo off
rem Dedicated FERRITE server. Edit the options below to taste.
rem   --port N     UDP port to listen on (default 27500)
rem   --map NAME   foundry, quarry, ridgeline or depot (default foundry)
rem   --rotation X all (default), off, or a list such as quarry,depot
rem   --bots N     players per team; empty slots are bots (default 12)
rem   --difficulty NAME   recruit, regular (default), veteran or elite
rem   --tickets N  tickets per team per round (default 250)
"%~dp0ferrite-server.exe" --port 27500 --bots 12
pause
