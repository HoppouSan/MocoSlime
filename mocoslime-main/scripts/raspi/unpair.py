#!/usr/bin/python3
import os
import json

cmd = "for device in $(bluetoothctl devices  | grep -o \"[[:xdigit:]:]\\{8,17\\}\"); do echo \"removing bluetooth device: $device | $(bluetoothctl remove $device)\"; done"
os.system(cmd)

try:
  orig_config = json.load(open('mocoslime/mocoslime.json')) # load mocoslime.json to get ip, port, tps
  orig_config['addresses'] = []
except:
  print("mocoslime.json not found. Ignoring.")

try:
  with open("mocoslime/mocoslime.json", "w") as f:
    json.dump(orig_config, f, indent=4)
except:
  print("")
