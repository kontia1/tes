# pffthash-mint CLI

CLI minter untuk pffthash.com — otomatis solve PoW dan mint token.

## Requirements

- Rust + Cargo: https://rustup.rs
- ETH di wallet untuk gas fee

## Install

```bash
git clone / copy folder ini ke VPS
cd pffthash-mint
cargo build --release
```

Binary akan ada di `target/release/pffthash-mint`

## Setup

```bash
cp .env.example .env
# Edit .env, isi PRIVATE_KEY dengan private key wallet kamu
nano .env
```

## Usage

### Lihat info contract
```bash
./target/release/pffthash-mint --info
```

### Mint 1x
```bash
./target/release/pffthash-mint --private-key YOUR_PK
```

### Mint 5x dengan 8 thread PoW
```bash
./target/release/pffthash-mint --private-key YOUR_PK --count 5 --threads 8
```

### Dengan custom RPC
```bash
./target/release/pffthash-mint \
  --private-key YOUR_PK \
  --rpc https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY \
  --count 3 \
  --threads 8
```

### Pakai .env
```bash
export $(cat .env | xargs)
./target/release/pffthash-mint --count 1
```

## Options

| Flag | Default | Keterangan |
|------|---------|-----------|
| `--private-key` / `-p` | - | Private key wallet |
| `--rpc` / `-r` | llamarpc | Ethereum RPC URL |
| `--count` / `-c` | 1 | Jumlah mint |
| `--threads` / `-t` | 4 | Thread PoW solver |
| `--info` | false | Hanya tampilkan info |

## Tips VPS

Makin banyak CPU core → makin cepat solve PoW:
```bash
# Cek jumlah core
nproc

# Gunakan semua core
./target/release/pffthash-mint --threads $(nproc) --count 10
```
