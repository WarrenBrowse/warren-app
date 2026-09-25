#!/usr/bin/env bash
set -eu

# pacman runs no removal scriptlet on an upgrade, so the GUI is closed here the
# way before-remove.sh closes it for deb and rpm. The packaging prepends this to
# before-install.sh to make the pacman pre_upgrade (tasks/distribution.cjs).
# SIGTERM for some reason causes the app to crash sometimes and SIGINT works as expected.
pkill -2 -x "warren-gui" || true
sleep 0.5
pkill -9 -x "warren-gui" || true
