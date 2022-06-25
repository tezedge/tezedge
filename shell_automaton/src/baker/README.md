# TezEdge Embedded Baker

This is a daemon for creating, signing and injecting endorsements and blocks
for the Tezos blockchain, in accordance to the Tenderbake consensus algorithm.

Unlike [External Baker](../../../apps/baker/README.md), this one runs inside
the node, so for baking you don't need a separate software or tool. With
additional configuration you can activate baker (or even multiple bakers)
inside the node.

## Running

### Setup TezEdge Node

This is baker embedded inside **TezEdge** node, so you need to
[set it up first](../../../README.md#how-to-build).

### Prepare an account

Skip this section if you already have a Tezos account ready for baking.

Pick a new <delegate_alias> and generate a new Tezos account using the Octez client.

```
tezos-client -E "http://localhost:18732" gen keys <delegate_alias>
```

You need to fund this account with at least 6000 ꜩ. Register the account as a delegate and wait the amount of time equal to between 5 and 6 cycles, depending on the position in the cycle (approximately 15 days).

```
tezos-client -E "http://localhost:18732" register key <delegate_alias> as delegate
```

By default, tezos-client stores the secret key for the account in the `$HOME/.tezos-client` directory.

See the [baking documentation](../../../baking/mainnet/README.md#initialize-keys-for-bakerendorser) for more details.

See the [Key management](https://tezos.gitlab.io/user/key-management.html) guide for more information.

### Use ledger

Alternatively, you can use an external ledger.

Install tezos baker app in ledger by using ledger-live gui tool. You also need to open baker app and enable baking.

Run signer, it will be running in background:
```
nohup tezos-signer \
    -E "http://localhost:18732" \
    launch http signer &
```

After that, run this in the terminal
```
tezos-signer \
    -E "http://localhost:18732" \
    list connected ledgers
```

It will print something like this:
```
To use keys at BIP32 path m/44'/1729'/0'/0' (default Tezos key path), use one
of:
  tezos-client import secret key ledger0 "ledger://reckless-duck-mysterious-wallaby/bip25519/0h/0h"
  tezos-client import secret key ledger0 "ledger://reckless-duck-mysterious-wallaby/ed25519/0h/0h"
  tezos-client import secret key ledger0 "ledger://reckless-duck-mysterious-wallaby/secp256k1/0h/0h"
  tezos-client import secret key ledger0 "ledger://reckless-duck-mysterious-wallaby/P-256/0h/0h"
```

User must choose ed25519 link and run:
```
tezos-signer \
    -E "http://localhost:18732" \
    import secret key <delegate_alias> "ledger://reckless-duck-mysterious-wallaby/ed25519/0h/0h"
```

This will print `added: tz1...`, it is your public key. Run the following command to import it. The command has `secret key` words, but it is working with the link that contains public key hash, the real secret key is still inside the ledger, and isn't exposed.
```
tezos-client
    -E "http://localhost:18732" \
    import secret key <delegate_alias> http://localhost:6732/tz1...
```

And finally, the Tezos Baking application on the Ledger should be configured for baking:
```
tezos-client \
    -E "http://localhost:18732" \
    setup ledger to bake for <delegate_alias>
```

If you did not done it before, you need to fund this account with at least 6000 ꜩ. Register the account as delegate and wait the amount of time equal to between 5 and 6 cycles, depending on the position in the cycle (approximately 15 days).

```
tezos-client -E "http://localhost:18732" register key <delegate_alias> as delegate
```

### Run the TezEdge node + Embedded baker

Assuming that the secret key (or locator for remote signing) is in
`$HOME/.tezos-client`, the additional cli arguments for `light-node` is:

```bash
--baker-data-dir "$HOME/.tezos-client"
--baker-name <delegate_alias>
--liquidity-baking-escape-vote off
```

See `light-node --help` for more details about those arguments.

### Common problems

1. After creating an account and registering it as a delegate, make sure
   you wait at least 5 cycles (approximately 15 days) before you start to bake.
