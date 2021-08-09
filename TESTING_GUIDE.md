# Testing guide

<!-- TOC GFM -->

* [Unit tests](#unit-tests)
* [Test the PoA consensus](#test-the-poa-consensus)
    * [Build and run the dev chain](#build-and-run-the-dev-chain)
        * [Using Docker](#using-docker)
        * [Compile from the source locally](#compile-from-the-source-locally)
    * [Store the data](#store-the-data)
        * [canyon-cli](#canyon-cli)
        * [Polkadot.js](#polkadotjs)
    * [Check the state](#check-the-state)
    * [Sync the chain](#sync-the-chain)

<!-- /TOC -->

This document contains a guide for testing the deliverables in the scope of [Phrase 2 of canyon network grant](https://github.com/w3f/Grants-Program/pull/488).

## Unit tests

The tests are either put into a seperate file `tests.rs` or included as an internal private module `tests` in the source file. So we could just go to the directory of each crate and run `cargo test`.

- [cc-rpc](./client/rpc/)
- [cc-datastore](./client/datastore)
- [cc-consensus-poa](./client/consensus/poa)
- [pallet-poa](./pallets/poa)

## Test the PoA consensus

### Build and run the dev chain

#### Using Docker

The docker image is hosted on https://hub.docker.com/r/canyonlabs/canyon .

```bash
$ docker pull canyonlabs/canyon:w3f-grant-2
```

```bash
# Prepare a local directory for the chain data
$ sudo rm -rf data
$ mkdir data

# Make sure we won't have the permission error.
$ chown 1000.1000 $(pwd)/data -R

# Run the docker in the background, remember to mount the local directory we just created.
$ docker run -d -it --name canyon -p 9933:9933 -p 9944:9944 -p 30333:30333 -v $(pwd)/data:/canyon canyonlabs/canyon:w3f-grant-2 canyon --dev -d /canyon --log=info,runtime=debug,poa=trace,rpc::permastore=debug

# Show the logs
$ docker logs -f canyon
```

#### Compile from the source locally

```bash
$ git clone https://github.com/canyon-network/canyon --branch w3f-grant-2
$ cargo build --release
```

Start the dev chain with a fresh db and enable the related log:

```bash
$ rm -rf d && ./target/release/canyon --dev -d d --log=info,runtime=debug,poa=trace,rpc::permastore=debug
```

### Store the data

#### canyon-cli

With futher developments on the consensus, we have abandoned the way of modifying the `UncheckedExtrinsic` structure for sending the store extrinsic together with the transaction data, for we could just simply use a new RPC for that purpose and it also makes the future JS integration painless.

Since it's a deliverable of the grant, the CLI interface for storing data has been implemented too in [canyon-cli](https://canyon-network/canyon-cli):

```bash
$ git clone https://github.com/canyon-network/canyon-cli --branch w3f-grant-2
$ cargo build --release
```

Check the help of `permastore` command:

```bash
$ ./target/release/canyon-cli permastore --help
canyon-cli-permastore 0.1.0
Permastore

USAGE:
    canyon-cli permastore <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    help               Prints this message or the help of the given subcommand(s)
    store              Submit the `store` extrinsic only
    store-with-data    Submit the `store` extrinsic and the transaction data
    submit             Submit the transction data only
```

Now we only support storing the small files(<= 10MiB), so we could just use `store-with-data`:

```bash
# Pass the data directory using `--data`
$ ./target/release/canyon-cli permastore store-with-data --data "web3 foundation"
data size in bytes: 15
        chunk root: 0xf9b321d3edaae871bbda8918480e18ca4b5c5c7fe1b8a77c84d78bf42939486e
  Submitted result: 0x31226eda234c2584841fdf24744d688415e6fa3bc0092a701e59c2fba89a9fe4

# Pass the data file using `--path`
$ ./target/release/canyon-cli permastore store-with-data --path LICENSE
data size in bytes: 1068
        chunk root: 0xdd3236327d0c4cceab6aaf5b72e9c54feb4362ec8c37d98330da59c99bce5a51
  Submitted result: 0xa7da9af10def640d37b3888f53bca4d9e29156a3b21fc4298b407953a2735d87
```

#### Polkadot.js

### Check the state

Console output.

Validator storage capacity

### Sync the chain

Failed to author blocks
