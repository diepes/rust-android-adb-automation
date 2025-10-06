# Code instructions related to the git repo

## Goal

* Creations of automation library in Rust, that used the installed adb android developer bridge to controll a android phone.

## adb

* after usb setup connection to phone from commandline with

      adb connect oneplus6:5555

* verify connection with 

      adb devices


## Coding guidance

* Keep the code modular and clean using rust structs where required

* Prefer that will simplify changes

* Try to use TTD where we create a test before we implement the item.

* Keep changes to small managable tasks that requires limited effort to achieve, trying to avoid big changes.

