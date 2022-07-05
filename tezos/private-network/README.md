# Private Tezos Network with TezEdge Nodes

This network runs Ithaca protocol and consists of 5 TezEdge nodes and bakers.
It also can be used to test Ledger-based baking account. 

## Extracting Keys from Ledger Nano

_Do not use your own mainnet account here as this will leak your private key out of the Ledger. Reset it with new seed._

TBD

## Configuration

The [.env](.env) file contains default values for some parameters.

The only parameters that need to be changed are `LEDGER_SECRET_KEY` and `LEDGER_PRIVATE_KEY`.

## Running

``` sh
$ docker compose up --detach
```

This command will start all nodes and bakers, and activate the network with parameters specified in [parameters-template.json](parameters-template.json). Also it will create a number of accounts, feed them with some funds, and then delegate them to the ledger baking account.

To see the progress of activation and initialization, run this:

``` sh
$ docker compose logs --follow activator
```

## Using Ledger with the Network

First, stop the baker that uses Ledger's private key.

``` sh
$ docker compose stop ledger-baker
```

TBD
