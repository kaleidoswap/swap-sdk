#!/bin/bash
set -xe

cd proxy
docker compose down
docker compose up --remove-orphans -d

cd ../boltz
./start.sh
