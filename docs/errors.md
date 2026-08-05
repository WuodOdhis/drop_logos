# Airdrop program error codes

The airdrop zkVM program returns structured, deterministic error codes for every
invalid-input and authorization failure. Codes are defined in
`methods/guest/src/handlers.rs` (`error_code` module) and returned as
`SpelError::Custom { code, message }`.

SPEL surfaces a handler `Err` as `Program error [<code>]: <message>` in the
guest output; the host layer sees it as a failed transaction whose proving
error includes the message. Codes are stable and must not be renumbered.

## Code table

| Code | Name                             | Condition                                                              | Recovery action                            |
|------|----------------------------------|------------------------------------------------------------------------|---------------------------------------------|
| 1    | `DISTRIBUTOR_NOT_AUTHORIZED`     | `distributor` account did not sign                                     | Sign with the distributor key; retry        |
| 2    | `DISTRIBUTION_ALREADY_INITIALIZED` | Distribution account already holds committed data                    | Use a fresh `distribution_id`; inspect PDA  |
| 3    | `INVALID_TOTAL_ALLOCATION`       | `total_allocation == 0`                                                | Set a positive allocation                   |
| 4    | `INVALID_NUM_ELIGIBLE`           | `num_eligible == 0`                                                    | Pass the real eligibility-set size          |
| 5    | `INVALID_ROOT`                   | Eligibility root is all-zero                                          | Commit a real Merkle root                   |
| 6    | `INVALID_CLOCK_ACCOUNT`          | Clock account is not the CLOCK_50 system account                       | Use the canonical clock account             |
| 7    | `NOT_DISTRIBUTOR`                | Caller is not the distributor that committed the distribution          | Call from the distributor account           |
| 8    | `DISTRIBUTION_ALREADY_FROZEN`    | `freeze_distribution` on an already-frozen distribution                | Idempotent: no-op, or use the next id       |

## Note on claim/double-claim errors

Claims are native LEZ private token transfers, not airdrop-guest instructions.
A double claim is rejected by the LEZ token/transfer runtime (spent nullifier),
surfacing as a failed transfer rather than a code in this table. See
`docs/design.md` for the claim flow.
