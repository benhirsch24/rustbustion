FROM debian:latest

RUN apt-get update
RUN apt-get install gcc xz-utils wget curl -y
RUN apt-get install libdbus-1-dev libglib2.0-dev libudev-dev libical-dev libreadline-dev -y
WORKDIR "/tmp"
RUN wget https://mirrors.edge.kernel.org/pub/linux/bluetooth/bluez-5.69.tar.xz
RUN tar xf bluez-5.69.tar.xz
WORKDIR "/tmp/bluez-5.69"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- -y
RUN echo 'source $HOME/.cargo/env' >> $HOME/.bashrc

VOLUME /rustbustion
WORKDIR "/"

# udev issue

CMD ["echo", "hello world"]
