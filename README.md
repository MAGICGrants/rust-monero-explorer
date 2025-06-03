# Rust Monero Explorer API

This is a Monero block explorer API written in Rust, built on Cuprate. Features are currently limited to a `/transaction` and `/block` endpoints, and new features will be added as the Cuprate project grows.

## Building and running

To be able to run the program, you need a synced Cuprate node. The explorer will attempt to find
the Cuprate directory at `/home/<user>/.local/share/cuprate` by default, you can use `-d <path>` to
set a custom directory.

First, install the required packages:

```bash
# ArchLinux-based
sudo pacman -S gcc glibc

# Debian-based
sudo apt install libgcc1 libc6
```

You can build the program using:

```bash
cargo build --release
```

Then run:

```bash
./target/release/rust-monero-explorer-api
```

## Docker

You can spin up a Docker container as follows:

```bash
docker run --name rust-monero-explorer-api -p 8081:8081 -v <REPLACE_WITH_CUPRATE_DIR>:/home/cuprate/.local/share/cuprate -d ghcr.io/magicgrants/rust-monero-explorer-api:latest
```

If you wish to run `cuprated` as well, you can simply use:

```bash
docker compose up -d
```

## Command line options

```
$ ./target/release/rust-monero-explorer-api -h

Usage: rust-monero-explorer-api [OPTIONS]

Options:
  -d, --cuprate-dir <CUPRATE_DIR>
  -p, --port <PORT>                [default: 8081]
  -i, --bind-ip <BIND_IP>          [default: 127.0.0.1]
  -h, --help                       Print help
  -V, --version                    Print version
```

## Endpoints

### `GET /api/block/<block_number>`

```json
{
  "hash": "d29c51c4354a396f370335035540fc94372bd277c2f65b50cc5bd2608a92c69c",
  "timestamp": 1697813342,
  "weight": 100297,
  "cumulative_generated_coins": 18347197280903987000,
  "cumulative_difficulty_low": 312831186335450240,
  "cumulative_difficulty_high": 0,
  "cumulative_rct_outs": 82193323,
  "long_term_weight": 176470,
  "transactions": [
    {
      "hash": "e4516854a5984eaf5f8750ac7af41d1e0b2c602a2297a673001e8c0af88eba11",
      "version": 2,
      "is_coinbase": true,
      "weight": 2219,
      "extra": "0148b077c1706fe9070f867d478c74431276194bf6139ca3e852abe553b8d9bee40209016c16bd243ed88d7f"
    },
    ...
  ]
}
```

### `GET /api/transaction/<tx_hash>`

```json
{
  "hash": "fc4f31a3c568e1584b7ceb2cb1b2ffb02c1a8a968f78d5a032baf82ff90187d0",
  "version": 2,
  "unlock_time": 0,
  "is_coinbase": false,
  "confirmation_height": 3000000,
  "timestamp": 1697813342,
  "weight": 2216,
  "inputs": [
    {
      "amount": 0,
      "key_image": "645fba8a0be85364d955c2796acb582f743a1a0e64e9aa16595d6f948bac7508",
      "mixins": [
        {
          "height": 2932485,
          "public_key": "2b454431c761008af372929fd672e2e5670c3e8b7016e133ae47ceb928f8ff32",
          "tx_hash": "b610f576147e7ad6fa1afbe76ab41c1c49c123e8a37244d97964fddf9e8e6de8"
        },
        ...
      ]
    },
    ...
  ],
  "outputs": [
    {
      "amount": 0,
      "public_key": "d519e7c800f03a2f38bfbf8087628721f44bfae992ec453b73d3c817b1015c26"
    },
    {
      "amount": 0,
      "public_key": "32c2334828c763909e403be1db3e7a9a9469c3de1144e9d31f52094437774152"
    }
  ],
  "extra": "01b84f7e585851b9f1d82dc462991f7191418dd75f3cf4d3c90bd0451136f908dd0209018b24d81a9039ede7"
}
```

## Contributing

Pull requests welcome! Thanks for supporting MAGIC Grants.

## License

[MIT](https://github.com/MAGICGrants/rust-monero-explorer-api/blob/main/LICENSE)
