# Rustbustion

Having fun with the [Combustion Thermometer](https://combustion.inc/)

# Raspberry Pi Combustion Interface

The code for a Raspberry Pi application that connects to the Bluetooth thermometer, pulls data, and optionally uploads to S3 is in `src/main.rs`.

## Bluetooth Interface

The bluetooth interface is the Rust program in this repo. It's responsible for talking to the thermometer and exposing the information to wherever it needs to go.

Simply `cargo run` to start the program scanning for the bluetooth accessory and print temperature updates.

To push data to an S3 bucket add the bucket name as a parameter: `cargo run <BUCKETNAME>`. It uses the AWS SDK so it will get credentials from the environment. The bucketname will be `<YOUR NAME>-combustion` as from the cdk below.

## Raspberry Pi Interface

Run the raspberry pi interface to the ST7789 TFT by simply `python3 display.py`. It assumes the Rust program is running.

## Setup

The Rust program uses the [Bluer](https://docs.rs/bluer) library which is the official Rust interface to the BlueZ Linux Bluetooth protocol stack.

I had to [upgrade BlueZ](https://scribles.net/updating-bluez-on-raspberry-pi-from-5-43-to-5-50/) including installing some dependencies.

## Enabling the Service

Edit `systemd/rustbustion.service` and find the `Enviroment` line. Edit the `AWS_PROFILE` line to point to your profile name (stored in `~/.aws/credentials`).

Copy `systemd/rustbustion.service` and `systemd/display.service` to `/etc/systemd/system/`, then

```
sudo systemctl daemon-reload
sudo systemctl start rustbustion.service
sudo systemctl start display.service
```

to start them, verify they work, then `sudo systemctl enable <both services>` to enable them to start at startup.

## Running on Mac

There is a simple implementation for MacOS that returns fake values for the purposes of testing.

# Webapp

This repo also contains a web application that will display the latest temperature data. The code is in `src/bin/webapp`.

## CDK

`cdk/` contains CDK code to stand up a self-contained stack in AWS. It stands up:

1. An S3 bucket for the Raspberry Pi program to upload to.
2. An ECR repository to push a Docker image to.
3. An EC2 instance that pulls the Docker image and runs it, exposing port 80 for the website.

`cdk bootstrap --context name=<YOUR NAME>` to bootstrap your AWS account.

`cdk synth --context name=<YOUR NAME>` to synthesize the template.

`cdk deploy --context name=<YOUR NAME>` to deploy the template. The S3 bucket is `<YOUR NAME>-combustion` which you can use for the Raspberry Pi program.

## Developing and Building

For developing you can simply `cargo run --bin webapp <bucket name>` for running locally and open up http://127.0.0.1:8080.

To build and push the docker image you can:

```
docker build --tag webapp -f Dockerfile.webapp .
aws ecr get-login-password --region <REGION> | docker login --username AWS --password-stdin <ACCOUNTID>.dkr.ecr.<REGION>.amazonaws.com
docker tag <YOUR NAME>:latest <ACCOUNTID>.dkr.ecr.<REGION>.amazonaws.com/<YOUR NAME>:latest
docker push <ACCOUNTID>.dkr.ecr.<REGION>.amazonaws.com/<YOUR NAME>:latest
```

Replacing the values in the `<>` appropriately.

## Running in the world

Log into your EC2 instance using SSM and run the `/tmp/update.sh` script. On the first EC2 instance startup you will also have to `usermod -aG docker ssm-user` and log out and back in using SSM.
