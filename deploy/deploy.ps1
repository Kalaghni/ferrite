# Deploy the dedicated server to josh-docker (192.168.2.122) as container
# `ferrite`, built on the remote from the working tree (committed or not).
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$tgz = Join-Path $env:TEMP 'ferrite-src.tgz'
tar -czf $tgz --exclude=./target --exclude=./.git --exclude=./docs --exclude=./resources --exclude=./verification --exclude=./dist .
scp -q $tgz josh-docker:/root/ferrite-src.tgz
Remove-Item $tgz
# docker writes build progress to stderr; don't let PowerShell treat that as fatal
$ErrorActionPreference = 'Continue'
ssh josh-docker "rm -rf /root/ferrite && mkdir -p /root/ferrite && cd /root/ferrite && tar -xzf /root/ferrite-src.tgz && rm /root/ferrite-src.tgz && cd deploy && docker compose up -d --build --quiet-build 2>&1 && sleep 3 && docker logs --tail 8 ferrite 2>&1"
if ($LASTEXITCODE -ne 0) { throw "deploy failed ($LASTEXITCODE)" }
