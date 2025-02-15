#!/bin/sh
osascript -e 'Tell application "System Events" to display dialog "Enter your password:" default answer "" with hidden answer buttons {"OK"} default button "OK"' -e 'text returned of result'
