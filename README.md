# Canyon Network

[![Continuous integration](https://github.com/canyon-network/canyon/actions/workflows/ci.yml/badge.svg)](https://github.com/canyon-network/canyon/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPL%20v3-blue.svg)](http://www.gnu.org/licenses/gpl-3.0)

Canyon network aims to be a permanent storage layer for Web3.0 built on Substrate framework, focusing on lightweight storage consensus as well as the high usability of data retrieval.

## Testing guide for w3f grant

This part will walk you through the use of canyon-cli for uploading some random data to the local canyon dev chain.

### Build

#### `canyon`

```bash
$ git clone https://github.com/canyon-network/canyon
$ cd canyon
$ git checkout w3f-grant-1
# The built release binary can be found in target/release/canyon.
$ cargo build --release
```

#### `canyon-cli`

```bash
$ git clone https://github.com/canyon-network/canyon-cli
$ cd canyon-cli
$ git checkout w3f-grant-1
$ cargo build --release
```

### Start the local dev chain

Ensure you have built the binary `canyon` and `canyon-cli` successfully.

Start the dev chain and you'll see the log line starting with `system digest`, in which `latest weave_size` indicates the size of stored data onto the network so far.

```bash
$ rm -rf tmp && ./target/release/canyon --dev -d tmp
...
2021-06-23 10:48:27 system digest: 0, block_size: Digest { logs: [PreRuntime(BABE, [2, 0, 0, 0, 0, 169, 53, 70, 32, 0, 0, 0, 0]), PreRuntime(poa_, [0, 0, 0, 0, 0, 0, 0, 0])] }, latest weave_size: 0
```

With the dev chain running, you can now open a new terminal to update the file using canyon-cli:

```bash
# Call `permastore::store` for storing data `helloworld`, signed by Alice.
$ ./target/release/canyon-cli permastore store helloworld
```

You can find the network data size changed:

```bash
# In canyon console
...
2021-06-23 10:57:42 system digest: 0, block*size: Digest { logs: [PreRuntime(BABE, [2, 0, 0, 0, 0, 98, 54, 70, 32, 0, 0, 0, 0]), PreRuntime(poa*, [10, 0, 0, 0, 0, 0, 0, 0])] }, latest weave_size: 10
```

Store some another random bytes:

```bash
$ ./target/release/canyon-cli permastore store web3
```

```bash
# In canyon console
...
2021-06-23 11:00:09 system digest: 0, block_size: Digest { logs: [PreRuntime(BABE, [2, 0, 0, 0, 0, 147, 54, 70, 32, 0, 0, 0, 0]), PreRuntime(poa_, [14, 0, 0, 0, 0, 0, 0, 0])] }, latest weave_size: 14
```

Basically, the CLI tool canyon-cli works and can interact with canyon chain for uploading data. Currently the data is simply stored on-chain, you can also check that via polkadot.js.org:

1. Inject the following additional types:

```json
{
  "Address": "MultiAddress",
  "LookupSource": "MultiAddress",
  "ExtrinsicIndex": "u32"
}
```

2. Assuming the `permastore::store` transaction is packed in block 233#1, which can be easily observed from the explorer page>`recent events` section. Turn to page Developer>Chain state for reading the data we just stored:

![Screenshot from 2021-06-23 11-15-13](https://user-images.githubusercontent.com/8850248/123030045-b4a3cc00-d414-11eb-8247-2d77b7597a39.png)

## License

[GPL v3](./LICENSE)
