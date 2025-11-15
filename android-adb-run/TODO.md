== Improve the rust device user event monitoring ==

lets improve the stream_touch_events, rather than running "timeout 1s" shell_command, lets open a adb shell dev.shell with reader and writer, then we can just keep reading to see if the getevent generated any output ?

doc: https://docs.rs/adb_client/latest/adb_client/struct.ADBUSBDevice.html
