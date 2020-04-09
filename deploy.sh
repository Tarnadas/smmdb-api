#!/bin/bash
set -e
GIT_HASH=$(git rev-parse HEAD)
echo "using git hash $GIT_HASH"
echo "$DOCKER_PASSWORD" | docker login -u "$DOCKER_USERNAME" --password-stdin

docker pull $(grep -ioP '(?<=^from)\s+\S+' Dockerfile) &
docker pull $(grep -ioP '(?<=^from)\s+\S+' DockerfileBuild) &
wait -n

docker pull tarnadas/smmdb-api-build
docker build --cache-from=tarnadas/smmdb-api-build -t tarnadas/smmdb-api-build:latest -f ./DockerfileBuild .
docker push tarnadas/smmdb-api-build:latest
docker tag tarnadas/smmdb-api-build tarnadas/smmdb-api-build:$GIT_HASH
docker push tarnadas/smmdb-api-build:$GIT_HASH

docker pull tarnadas/smmdb-api
docker build --cache-from=tarnadas/smmdb-api -t tarnadas/smmdb-api .
docker push tarnadas/smmdb-api:latest
docker tag tarnadas/smmdb-api tarnadas/smmdb-api:$GIT_HASH
docker push tarnadas/smmdb-api:$GIT_HASH
