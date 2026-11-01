#!/bin/bash
# usage: wldrag.sh X1 Y1 X2 Y2 shot; drag inside the nested (x11-backend) weston window.
export DISPLAY=:99
xdotool mousemove $1 $2; sleep 0.4
xdotool mousedown 1; sleep 0.4
for i in 1 2 3 4 5 6 7 8; do
  xdotool mousemove $(( $1 + ($3 - $1) * i / 8 )) $(( $2 + ($4 - $2) * i / 8 )); sleep 0.25
done
sleep 0.8
xdotool mouseup 1
sleep 3
import -window root ~/dnd/$5.png
