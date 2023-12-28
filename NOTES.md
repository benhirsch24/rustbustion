# Notes

## Starting out

* Had to update the bluetooth stack on Raspberry Pi (https://scribles.net/updating-bluez-on-raspberry-pi-from-5-43-to-5-50/)
* Was able to discover the combustion thing but once I went to connect to it to query it further now I can no longer discover it
* Looks like the Rust bluetooth library I'm using (bluez) talks to the Linux bluetooth stack over DBus
* Had an issue where the device was showing up "C2:71:..." but the manufacturer data was showing as none which is where I identify the thermometer. By starting bluetoothd (`sudo bluetoothd`) and then using `bluetoothctl` I could figure this out. `Bluetoothctl scan on` showed the thermometer with the manufacturer data (how I found the address) but the Rust code did not show the manufacturer. I figured out by reading the source code that the first thing `bluer` does is get the list of known devices, and that function call seems to return my thermometer. `bluetoothctl devices` also shows it as a known device. I `bluetoothctl remove <device>` to remove it from the known devices, at which point it shows back up in my scanner and with the ManufacturerData. Wonder if this means that DBus only returns the MD when you're discovering devices, not after they've been discovered.
* Figuring out the bit packing was annoying as fuck! Had to write a C program to really understand it. (Go into details). Eventually found a good Rust program.
