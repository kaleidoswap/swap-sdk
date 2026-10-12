The invoice is the synthetic 100,000 sat regtest invoice already used by the
TypeScript pay-through tests (generated with lightning-invoice, timestamp
1800000000). It is not a live payment request. The encoded LNURL names
`https://pay.example/lnurl`; WASM tests provide in-memory fetch responses and
native core tests use a loopback HTTP service. These tests verify transport and
fixed-amount resolution, not live provider behavior, expiry or metadata binding.
