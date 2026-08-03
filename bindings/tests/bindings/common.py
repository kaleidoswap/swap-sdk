import kaleidorg_swap_sdk
import asyncio

network = kaleidorg_swap_sdk.Network.REGTEST
boltz_api = kaleidorg_swap_sdk.BoltzApiClientV2.default(network)
btc_chain = kaleidorg_swap_sdk.btc_chain_from_network(network)
lbtc_chain = kaleidorg_swap_sdk.lbtc_chain_from_network(network)

electrum_btc = kaleidorg_swap_sdk.ClientConnection.ELECTRUM(
    kaleidorg_swap_sdk.ElectrumBuilder(url="localhost:19001", tls=False)
)
electrum_lbtc = kaleidorg_swap_sdk.ClientConnection.ELECTRUM(
    kaleidorg_swap_sdk.ElectrumBuilder(url="localhost:19002", tls=False)
)
chain_client = kaleidorg_swap_sdk.ChainClient(
    kaleidorg_swap_sdk.ClientConfig(
        network=network, bitcoin=electrum_btc, liquid=electrum_lbtc
    )
)


async def cli_docker(cmd: str):
    process = await asyncio.create_subprocess_shell(
        f'docker exec -i boltz-scripts bash -c "source /etc/profile.d/utils.sh && {cmd}"',
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    await process.wait()
    return (await process.stdout.read()).decode().strip("\n")


async def new_invoice(sats: int):
    return await cli_docker(f"lncli-sim 1 addinvoice {sats} | jq -r '.payment_request'")


async def pay_invoice(invoice: str):
    await cli_docker(f"lncli-sim 1 payinvoice --force {invoice}")


async def chain_cli(chain: kaleidorg_swap_sdk.Chain, cmd: str):
    return await cli_docker(
        f"{'bitcoin' if chain.is_bitcoin() else 'elements'}-cli-sim-client {cmd}"
    )


async def getnewaddress(chain: kaleidorg_swap_sdk.Chain):
    return await chain_cli(chain, "getnewaddress")


async def sendtoaddress(chain: kaleidorg_swap_sdk.Chain, address: str, amount: int):
    btc_amount = f"{amount / 100_000_000:.8f}"
    return await chain_cli(chain, f"sendtoaddress {address} {btc_amount}")


async def mine_block():
    await cli_docker("bitcoin-cli-sim-client -generate 1")
    await cli_docker("elements-cli-sim-client -generate 1")


async def delay():
    await asyncio.sleep(5)


async def next_status(updates: kaleidorg_swap_sdk.BoltzWsUpdates, status: str):
    while True:
        try:
            update = await asyncio.wait_for(updates.next(), timeout=5)
        except asyncio.TimeoutError:
            raise TimeoutError(f"Timeout waiting for status '{status}'")
        print("Waiting for status", update.status)
        if update.status == status:
            return update
