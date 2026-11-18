"""RGB wallet adapter example, not a live swap CLI.

The caller first creates a swap with rgb_contract_id pinned. Before claiming,
rgb-lib must accept the maker's lock consignment and enforce min_confirmations.
Supply the accepted colored lock transaction in params.lockup_tx for both routes.
Choose the submarine BTC collateral cap locally; compare reverse fees with the
remaining timeout before paying the invoice.
"""

from typing import Protocol

import kaleidorg_swap_sdk as sdk


class RgbWalletAdapter(Protocol):
    async def fund_bitcoin_psbt(self, template: sdk.RgbPsbtTemplate) -> str:
        """Add spendable BTC fee inputs/change. Sign nothing yet."""
        ...

    async def color_psbt(
        self, template: sdk.RgbPsbtTemplate
    ) -> tuple[sdk.ColoredRgbPsbt, str]:
        """Return colored PSBT, actual rgb-lib allocations and operation ID.

        Call psbt_op_prepare_with_expiry with template.swap_outpoint, asset_id,
        and output_map {template.payment_output_index: template.amount}.
        """
        ...


async def finalize_rgb_spend(
    script: sdk.SwapScript,
    params: sdk.RgbPsbtParams,
    wallet: RgbWalletAdapter,
    keys: sdk.KeyPair,
    preimage: sdk.Preimage | None = None,
) -> tuple[sdk.FinalizedRgbSpend, str]:
    """Prepare, optionally fund, color and sign a claim or refund.

    Pass a preimage for a reverse claim; omit it for a submarine refund.
    Use RgbSpendFunding.CALLER_INPUTS() for refunds needing wallet BTC fee inputs.
    """
    spend = (
        await script.prepare_rgb_claim(params)
        if preimage is not None
        else await script.prepare_rgb_refund(params)
    )
    if spend.template().requires_funding:
        funded_psbt = await wallet.fund_bitcoin_psbt(spend.template())
        spend = spend.fund(funded_psbt)
    colored_psbt, operation_id = await wallet.color_psbt(spend.template())
    finalized = (
        spend.finalize_claim(colored_psbt, keys, preimage)
        if preimage is not None
        else spend.finalize_refund(colored_psbt, keys)
    )
    return finalized, operation_id


# The caller signs/finalizes its fee inputs on finalized.psbt while preserving
# the HTLC witness. finalized.transaction exists only when every input is final.
# Persist the operation/spend, psbt_op_mark_broadcast, broadcast the final tx,
# psbt_op_apply, and psbt_op_provide_receive_consignment for the receive. A refund
# must wait for the timeout; a claim must confirm before it.
