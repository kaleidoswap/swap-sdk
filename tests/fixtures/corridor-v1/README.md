`card.json` models the fields emitted by maker-rs's `maker-nostr/src/card.rs`.
It is a transport/schema fixture: the discovery key and signature are
placeholders, not a cryptographically valid signed advertisement. The amount
above JavaScript's safe integer range verifies that limits remain strings.
Unknown fields are tolerated; required fields must match their wire types.
