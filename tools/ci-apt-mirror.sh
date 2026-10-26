#!/usr/bin/env bash
# Point a GitHub Ubuntu runner's apt at Ubuntu's own archive instead of Azure's mirror.
#
# Azure's mirror stalled for most of an hour on 2026-10-26, more than once. Rewriting the
# sources alone does nothing on the newer images: their sources say
# `mirror+file:/etc/apt/apt-mirrors.txt`, so apt takes its servers from that list, and Azure's
# is first in it. Both places are rewritten here, whichever the image uses.
set -euo pipefail
shopt -s nullglob

for f in /etc/apt/apt-mirrors.txt /etc/apt/sources.list.d/*.sources /etc/apt/sources.list; do
	[[ -f "$f" ]] || continue
	sudo sed -E -i 's#https?://azure\.archive\.ubuntu\.com/ubuntu#http://archive.ubuntu.com/ubuntu#g' "$f"
done

# A stalled file is retried instead of failing the job's download outright.
echo 'Acquire::Retries "3";' | sudo tee /etc/apt/apt.conf.d/80-retries >/dev/null

if [[ -f /etc/apt/apt-mirrors.txt ]]; then
	echo "apt mirrors:"
	cat /etc/apt/apt-mirrors.txt
fi
