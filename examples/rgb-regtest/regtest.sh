#!/usr/bin/env bash
set -euo pipefail
example_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
compose=(docker compose -f "$example_dir/compose.yaml")
cli() { "${compose[@]}" exec -T bitcoind bitcoin-cli -regtest -rpcuser=user -rpcpassword=pass "$@"; }
case "${1:-}" in
  up)
    umask 077
    mkdir -p "$example_dir/run/certs" "$example_dir/run/ldk-maker" "$example_dir/run/ldk-taker"
    for name in maker taker; do
      if [ "$name" = maker ]; then port=13636; else port=13646; fi
      if [ ! -f "$example_dir/run/certs/$name.crt" ]; then
        openssl req -x509 -newkey rsa:2048 -nodes -days 7 -keyout "$example_dir/run/certs/$name.key" -out "$example_dir/run/certs/$name.crt" -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,DNS:ldk-$name,IP:127.0.0.1" -addext "basicConstraints=critical,CA:FALSE" -addext "extendedKeyUsage=serverAuth" >/dev/null 2>&1
      fi
      cat > "$example_dir/run/ldk-$name.toml" <<CONFIG
[node]
network = "regtest"
listening_addresses = ["0.0.0.0:9735"]
grpc_service_address = "0.0.0.0:$port"
rest_service_address = "0.0.0.0:$((port + 1))"
alias = "rgb-sdk-$name"
[tls]
cert_path = "/certs/$name.crt"
key_path = "/certs/$name.key"
hosts = ["localhost", "ldk-$name"]
[storage.disk]
dir_path = "/data"
[log]
level = "Info"
[bitcoind]
rpc_address = "bitcoind:18443"
rpc_user = "user"
rpc_password = "pass"
CONFIG
    done
    "${compose[@]}" up -d --wait --wait-timeout 90
    if ! cli -rpcwallet=miner getwalletinfo >/dev/null 2>&1; then
      cli createwallet miner >/dev/null
      address="$(cli -rpcwallet=miner getnewaddress)"
      cli -rpcwallet=miner generatetoaddress 111 "$address" >/dev/null
    fi
    ;;
  cli) shift; cli "$@" ;;
  mine) cli -rpcwallet=miner -generate "${2:?block count required}" >/dev/null ;;
  sendtoaddress) cli -rpcwallet=miner sendtoaddress "${2:?address required}" "${3:?BTC amount required}" ;;
  publish-pairs)
    "${compose[@]}" exec -T postgres psql -U postgres -d maker -v ON_ERROR_STOP=1 -c "UPDATE pairs SET enabled=FALSE,sdk_visible=FALSE,sdk_default=FALSE; UPDATE pairs SET enabled=TRUE,sdk_visible=TRUE,sdk_default=TRUE WHERE (pair_id,swap_kind) IN (('USDT-RGB/BTC@LN','submarine'),('BTC@LN/USDT-RGB','reverse'));"
    ;;
  down) "${compose[@]}" down -v ;;
  *) echo 'usage: regtest.sh up|down|mine BLOCKS|sendtoaddress ADDRESS BTC|cli ARGS...' >&2; exit 2 ;;
esac
