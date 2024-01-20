#!/bin/bash

set -euxo

docker buildx build --platform linux/amd64 -t webapp_build -f Dockerfile.webapp_build .
AWS_PROFILE=$1 docker run --rm -it -d --name webapp_tmp webapp_build:latest $1-combustion
docker cp webapp_tmp:/webapp/target/release/webapp /tmp/webapp
docker buildx build --platform linux/amd64 /tmp -t webapp -f Dockerfile.webapp
docker stop webapp_tmp
