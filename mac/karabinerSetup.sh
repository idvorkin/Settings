#!/bin/sh

cli=/Applications/Karabiner.app/Contents/Library/bin/karabiner

$cli set private.ctrl2capTerminal 1
/bin/echo -n .
$cli set repeat.wait 43
/bin/echo -n .
$cli set private.ctrl2command 1
/bin/echo -n .
$cli set repeat.initial_wait 100
/bin/echo -n .
$cli set private.termial_goop 1
/bin/echo -n .
$cli set private.vimXCode 1
/bin/echo -n .
$cli set private.fixing-alt-tab 1
/bin/echo -n .
$cli set private.FlipScrollMsUsbTrackball 1
/bin/echo -n .

/bin/echo
