#!/bin/bash
# Changelog (num = number at end of noautoupd):
# 1. raspi folder was moved, launcher.sh can't update itself

FILE=/home/mocoslime/noautoupd1
if test -f "$FILE"; then
    echo "$FILE exists. Don't run autoupdate.sh"
else
    cp /home/mocoslime/mocoslime/scripts/raspi/* /home/mocoslime
    touch /home/mocoslime/noautoupd1
fi
