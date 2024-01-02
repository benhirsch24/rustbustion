# Notes

## WebApp

* Decided to try out Axum as the web framework
* Had some fun with figuring out how to navigate S3 "directories"
* Eventually got it working. Can see where I'll need to refactor things around and share the update structs. Eventually.
* Next up is HTML templating
* Got that working and also refactored a bit which has made things nicer. I'm definitely getting more comfortable with Rust modules which I'm glad about.
* EC2 instance was relatively painless to get up and running. Still not running the docker image yet, but that shouldn't take too much more work. Then I have to think about "deployments", so maybe that means using an ASG and just killing the EC2 instance and restarting it, setting up a simple script to manually run, or doing something like Fargate (but have to evaluate cost).
* Ya know what, I'm giving up on getting the EC2 instance to start my docker container at startup. I'll just make a script that auto-updates.
* Damn, NAT Gateways are expensive. $2 for 1 day (0.045 per hour and I was running two NAT Gateways in each AZ by default)

## S3 Pusher

* Pretty easy after getting used to the Rust AWS client.
* When writing async code, always write synchronous code. By that I mean rarely should you have a method which starts a background task, always write as much synchronous code as possible and then use it from a task.

## X-platform dev & web app

* Took a while to figure out how to do the dependency section in Cargo.toml. ChatGPT was sorta helpful, but ended up still having to go through the docs. Module conditional compilation is also a little weird but I managed to figure it out. Think I can do it better though.
* Set up a Dockerfile to test that conditional compilation worked which was pretty easy. Ran into some issue setting up bluez probably because Docker doesn't have a translation layer between the VM and the Mac bluetooth layer by default and I don't feel like setting that up. Once I pull this code down on my Raspberry Pi we'll see if it works
* Using ChatGPT for CDK code was super easy, I love having something I can write plain-english into and give me working code (for simple things)
* Womp womp, account verification.

### Thoughts on Webapp design

Jotting thoughts down before going on a walk.

* To make it simple each "session" will be a single file that gets appended with updated temps every N seconds (eg 10 seconds).
* S3 has no append, so we'll have to PutObject and overwrite and keep the data in memory the whole time.
* Maybe instead we create a folder per session and then limit the length of the files. Each file can be ~100 temps. That way we don't have to worry about super long cooks running out of mem on the Pi.
* Start out with using the RFC3339 datetime at program start time as the S3 key for now.
* EC2 instance can just be a T2 micro. Thinking that to start it can be a stupid simple static page (maybe bootstrap or something) and we render it sever-side where on request it reads the latest S3 file, has big text for the last one, then under that lists the previous 5-10 temps in small text.
* Maybe add an in-mem cache so it doesn't need to hit S3 on each request. This is my money I'm spending not my employer's!
* Then eventually I could play with graphing libraries and dynamically refreshing. But for something where I expect to load the page once every 15 minutes while I'm out and about and something is cooking I don't think dynamic reloading is necessary, just server-side render it and refresh the page.
* I like the idea of a generated link, eventually I could consider adding that as a Pi feature. Will have to think about it, "name this cook" feature. Could be a good option to play with the Whisper model on Raspberry Pi to name the S3 bucket key and then hand out the link to friends.

## Starting out

* Had to update the bluetooth stack on Raspberry Pi (https://scribles.net/updating-bluez-on-raspberry-pi-from-5-43-to-5-50/)
* Was able to discover the combustion thing but once I went to connect to it to query it further now I can no longer discover it
* Looks like the Rust bluetooth library I'm using (bluez) talks to the Linux bluetooth stack over DBus
* Had an issue where the device was showing up "C2:71:..." but the manufacturer data was showing as none which is where I identify the thermometer. By starting bluetoothd (`sudo bluetoothd`) and then using `bluetoothctl` I could figure this out. `Bluetoothctl scan on` showed the thermometer with the manufacturer data (how I found the address) but the Rust code did not show the manufacturer. I figured out by reading the source code that the first thing `bluer` does is get the list of known devices, and that function call seems to return my thermometer. `bluetoothctl devices` also shows it as a known device. I `bluetoothctl remove <device>` to remove it from the known devices, at which point it shows back up in my scanner and with the ManufacturerData. Wonder if this means that DBus only returns the MD when you're discovering devices, not after they've been discovered.
* Figuring out the bit packing was annoying as fuck! Had to write a C program to really understand it. (Go into details). Eventually found a good Rust program.
