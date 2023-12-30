# Rustbustion

Having fun with the [Combustion Thermometer](https://combustion.inc/)

## Bluetooth Interface

The bluetooth interface is the Rust program in this repo. It's responsible for talking to the thermometer and exposing the information to wherever it needs to go.

Simply `cargo run` to start the program scanning for the bluetooth accessory.

## Raspberry Pi Interface

Run the raspberry pi interface to the ST7789 TFT by simply `python3 display.py`. It assumes the Rust program is running.

## Setup

The Rust program uses the [Bluer](https://docs.rs/bluer) library which is the official Rust interface to the BlueZ Linux Bluetooth protocol stack.

I had to [upgrade BlueZ](https://scribles.net/updating-bluez-on-raspberry-pi-from-5-43-to-5-50/) including installing some dependencies.

## Running on Mac

There is a simple implementation for MacOS that returns fake values for the purposes of testing.
