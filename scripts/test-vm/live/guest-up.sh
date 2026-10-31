#!/bin/bash
# Xvfb + headless weston + session dbus for GUI tests in the VM.
export XDG_RUNTIME_DIR=/run/user/$(id -u)
pkill -x Xvfb; pkill -x weston; pkill -x WebKitWebDriver
nohup Xvfb :99 -screen 0 1280x800x24 >/tmp/xvfb.log 2>&1 &
nohup env WAYLAND_DISPLAY=wayland-0 wayland-headless >/tmp/weston.log 2>&1 &
sleep 2
eval "$(dbus-launch --sh-syntax)"
echo "export DBUS_SESSION_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS" > /tmp/session.env
echo "export DISPLAY=:99 GDK_BACKEND=x11 XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR WAYLAND_DISPLAY=wayland-0" >> /tmp/session.env
echo up
