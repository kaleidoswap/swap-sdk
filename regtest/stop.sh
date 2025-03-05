#!/bin/bash
set -xe

cd boltz
./stop.sh

cd ../proxy
docker compose down --volumes
