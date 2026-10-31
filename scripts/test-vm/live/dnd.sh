#!/bin/bash
# usage: dnd.sh <file> <shot-name>; drags <file> from dragsrc onto the app window.
. /tmp/session.env
python3 "$(dirname "$0")/dragsrc.py" "$1" >/tmp/dragsrc.log 2>&1 &
SRC=$!
sleep 2
xdotool mousemove 1215 65; sleep 0.3
xdotool mousedown 1; sleep 0.3
for p in "1200 70" "1180 90" "1100 150" "900 250" "700 350" "600 380" "550 400"; do
  xdotool mousemove $p; sleep 0.25
done
sleep 0.8
xdotool mouseup 1
sleep 4
mkdir -p ~/dnd && import -window root ~/dnd/$2.png
kill $SRC 2>/dev/null
cat /tmp/dragsrc.log
