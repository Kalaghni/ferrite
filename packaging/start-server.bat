@echo off
rem Dedicated FERRITE server. Edit the options below to taste.
rem   --port N     UDP port to listen on (default 27500)
rem   --map NAME   foundry, quarry, ridgeline or depot (default foundry)
rem   --bots N     players per team; empty slots are bots (default 12)
rem   --skill X    bot skill, 0.7 easier, 1.3 harder (default 1.0)
rem   --tickets N  tickets per team per round (default 250)
"%~dp0ferrite-server.exe" --port 27500 --bots 12
pause
