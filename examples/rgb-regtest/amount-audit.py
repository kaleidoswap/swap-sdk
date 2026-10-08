#!/usr/bin/env python3
"""Read-only exact integer reconciliation of the recorded TS run; run audit-amounts first."""
import hashlib
import json
from pathlib import Path
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parent
RUN = ROOT / 'run'
REPORT = json.loads((ROOT / 'ts-validation-report.json').read_text())
ORDERS = json.loads((RUN / 'amount-audit-orders.json').read_text())
WALLETS = json.loads((RUN / 'amount-audit-wallets.json').read_text())
ESPLORA = 'http://127.0.0.1:23002'

def get(path):
    with urlopen(ESPLORA + path, timeout=30) as response:
        return json.load(response)

def ceildiv(n, d):
    return (n + d - 1) // d

def original_card(order):
    # Recover the historical rate from this deterministic feed, proving it against
    # the order's verified pairHash rather than guessing the old price from today's feed.
    for sequence in range(1, 1001):
        n, d = 100000000000 + sequence, 1000000
        if order['kind'] == 'submarine':
            n, d = d, n
        rate = {'numerator': n * 9950, 'denominator': d * 10000}
        limits = {'min': 1000000000 if order['kind'] == 'submarine' else 50000,
                  'max': 10000000000000 if order['kind'] == 'submarine' else 100000000,
                  'max_zero_conf': 0}
        card = {'rate': rate, 'limits': limits,
                'fees': {'base_fee': 0, 'network_fee': 1000, 'service_bps': 50}}
        encoded = json.dumps(card, separators=(',', ':')).encode()
        if hashlib.sha256(encoded).hexdigest() == order['pairHash']:
            return sequence, rate
    raise AssertionError('historical rate did not match the verified pair hash')

def chain_transaction(txid):
    tx = get('/tx/' + txid)
    assert tx['status']['confirmed']
    ins = sum(i['prevout']['value'] for i in tx['vin'])
    outs = sum(o['value'] for o in tx['vout'])
    assert ins - outs == tx['fee']
    return tx, {'txid': txid, 'inputSat': ins, 'outputSat': outs,
                'minerFeeSat': tx['fee'], 'confirmed': True,
                'unaccountedSat': ins - outs - tx['fee']}

flows = {}
labels = ['submarine', 'submarineRefund', 'reverse', 'reverseRefund']
for label, order in zip(labels, ORDERS):
    r = REPORT[label]
    assert order['lockTxid'] == r['lockTxid']
    assert order['rgbAmount'] == r['amount']
    seq, rate = original_card(order)
    n, d = rate['numerator'], rate['denominator']
    if order['kind'] == 'submarine':
        target = order['toAmount']
        lo, hi = 1000000000, 10000000000000
        def quote(amount):
            gross = amount * n // d
            return gross - 1000 - gross * 50 // 10000
        while lo < hi:
            mid = (lo + hi) // 2
            if quote(mid) >= target:
                hi = mid
            else:
                lo = mid + 1
        assert quote(lo) >= target and quote(lo-1) < target
        quoted_contract = ceildiv(lo, 100)
        assert quoted_contract == order['fromAmount'] == r['amount']
        gross = lo * n // d
        service = gross * 50 // 10000
        assert order['serviceFee'] == service and order['networkFee'] == 1000
        assert gross - service - 1000 == target
        pricing = {'historicalTick': seq, 'pairHashVerified': True,
                   'recomputedDeposit': quoted_contract, 'invoiceSat': target,
                   'grossSat': gross, 'serviceFeeSat': service, 'networkAllowanceSat': 1000,
                   'depositRoundingUpCardUnits': quoted_contract * 100 - lo,
                   'cardUnitsPerContractUnit': 100, 'quoteDifferenceUnits': 0}
    else:
        # Maker RGB venue: 1560 sat estimated lock fee + 1521 sat claim carrier,
        # converted at the locked card rate and rounded up, above the card's floor.
        gross = order['fromAmount'] * n // d
        network_raw = ceildiv(3081 * n, d)
        service_raw = gross * 50 // 10000
        net_raw = gross - network_raw - service_raw
        net = net_raw // 100
        network, service = ceildiv(network_raw, 100), ceildiv(service_raw, 100)
        assert net == order['toAmount'] == r['amount']
        assert network == order['networkFee'] and service == order['serviceFee']
        pricing = {'historicalTick': seq, 'pairHashVerified': True,
                   'invoiceSat': order['fromAmount'], 'grossContractUnitsFloor': gross // 100,
                   'serviceFeeContractUnitsCeil': service,
                   'networkAllowanceContractUnitsCeil': network, 'recomputedPayout': net,
                   'payoutRoundingDownCardUnits': net_raw % 100,
                   'cardUnitsPerContractUnit': 100, 'quoteDifferenceUnits': 0,
                   'roundedBreakdownDifferenceUnits': net + network + service - gross // 100}
    assert order['protocolFee'] == 0
    wallet = 'maker' if label in ['submarine', 'reverseRefund'] else 'taker' if label == 'submarineRefund' else 'reverse'
    op = next(o for o in WALLETS['wallets'][wallet]['actualTransitionAllocations'] if o['flow'] == label)
    assert op['amount'] == r['amount'] and op['status'] == 'Applied'
    spend_id = op['txid']
    lock, lock_money = chain_transaction(r['lockTxid'])
    spend, spend_money = chain_transaction(spend_id)
    vout = order['lockVout']
    assert lock['vout'][vout]['value'] == order['htlcSat']
    assert any(i['txid'] == r['lockTxid'] and i['vout'] == vout for i in spend['vin'])
    assert spend['vout'][1]['value'] == (546 if label == 'reverse' else order['htlcSat'])
    if label == 'submarine':
        payment_hash = next(p['hash'] for p in WALLETS['lightning']['maker']['payments'] if p['direction'] == 1 and p['status'] == 1)
        sender, receiver = 'maker', 'taker'
    elif label == 'submarineRefund':
        payment_hash = json.loads((RUN/'ts-hold.json').read_text())['sha256']
        sender, receiver = 'maker', 'taker'
    else:
        recovery = json.loads((RUN / ('ts-reverse.json' if label == 'reverse' else 'ts-reverse-refund.json')).read_text())
        payment_hash = recovery['preimage']['sha256']
        sender, receiver = 'taker', 'maker'
    pay = next(p for p in WALLETS['lightning'][sender]['payments'] if p['hash'] == payment_hash)
    received = next(p for p in WALLETS['lightning'][receiver]['payments'] if p['hash'] == payment_hash)
    assert pay['amountMsat'] == received['amountMsat'] == 100000000
    success = label in ['submarine', 'reverse']
    assert pay['status'] == (1 if success else 2)
    if success:
        assert received['status'] == 1 and pay['feePaidMsat'] == 0
    flows[label] = {'rgbQuoted': r['amount'], 'rgbActualTransition': op['amount'],
                    'unaccountedRgbUnits': 0, 'pricing': pricing,
                    'lightning': {'sender': sender, 'receiver': receiver,
                                  'senderAmountMsat': pay['amountMsat'], 'receiverAmountMsat': received['amountMsat'],
                                  'senderStatus': pay['status'], 'receiverRecordStatus': received['status'],
                                  'routingFeePaidMsat': pay['feePaidMsat'], 'settledAmountMsat': 100000000 if success else 0},
                    'bitcoinLock': lock_money, 'bitcoinSpend': spend_money}
assert WALLETS['rgbTotal'] == REPORT['issued'] == 2000000000
for name, wallet in WALLETS['wallets'].items():
    assert wallet['settled'] == wallet['spendableAllocationSum'] == REPORT['settledBalances'][name]
channel_balances = json.loads((RUN / 'amount-audit-lightning-balances.json').read_text())
assert channel_balances['unresolvedHtlcs'] == 0
assert channel_balances['claimablePlusCommitmentFeesSat'] + channel_balances['anchorOutputsSat'] == channel_balances['channelFundingSat']
result = {'network': 'regtest', 'assetId': REPORT['assetId'], 'rgbDecimals': 6,
          'flows': flows, 'lightningChannelBalances': channel_balances, 'walletBalances': REPORT['settledBalances'], 'unaccountedRgbUnits': 0,
          'notes': ['Each quoted RGB amount exactly matches the actual committed transition and final spendable wallet allocation.',
                    'Bitcoin miner fees and the quoted spread/service/network allowances are costs, not missing RGB.',
                    'Reverse fee components round up separately while payout rounds down: their displayed sum exceeds floor(gross) by one contract unit in this run.',
                    'Failed Lightning inbound records remain pending in node bookkeeping; payer records are failed, no unresolved HTLCs remain, and the entire channel value reconciles including commitment fees and two anchor outputs. Null routing fee on failed payments is unavailable, not asserted zero.']}
(ROOT / 'amount-audit-report.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'unaccountedRgbUnits': 0, 'quoteDifferences': {k:v['pricing']['quoteDifferenceUnits'] for k,v in flows.items()},
                  'reverseRoundedBreakdownDifferenceUnits': flows['reverse']['pricing']['roundedBreakdownDifferenceUnits'],
                  'totalSwapMinerFeesSat': sum(v['bitcoinLock']['minerFeeSat'] + v['bitcoinSpend']['minerFeeSat'] for v in flows.values())}, indent=2))
