use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256, keccak256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    sol,
    rpc::types::TransactionRequest,
};
use anyhow::{Context, Result};
use clap::Parser;
use rand::Rng;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::Instant;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    PfftHash,
    r#"[
        {"name":"freeMint","type":"function","inputs":[{"name":"powNonce","type":"uint256"}],"outputs":[],"stateMutability":"nonpayable"},
        {"name":"getInfo","type":"function","inputs":[],"outputs":[
            {"name":"currentMinted","type":"uint256"},
            {"name":"remainingSupply","type":"uint256"},
            {"name":"currentDecayRate","type":"uint256"},
            {"name":"nextMintAmount","type":"uint256"}
        ],"stateMutability":"view"},
        {"name":"currentPowHexZeros","type":"function","inputs":[],"outputs":[{"name":"","type":"uint256"}],"stateMutability":"view"},
        {"name":"POW_DIFFICULTY_BITS","type":"function","inputs":[],"outputs":[{"name":"","type":"uint256"}],"stateMutability":"view"},
        {"name":"currentPowStage","type":"function","inputs":[],"outputs":[{"name":"","type":"uint256"}],"stateMutability":"view"},
        {"name":"MAX_SUPPLY","type":"function","inputs":[],"outputs":[{"name":"","type":"uint256"}],"stateMutability":"view"},
        {"name":"BASE_MINT_AMOUNT","type":"function","inputs":[],"outputs":[{"name":"","type":"uint256"}],"stateMutability":"view"}
    ]"#
);

#[derive(Parser, Debug)]
#[command(name = "pffthash-mint", about = "CLI minter untuk pffthash.com")]
struct Args {
    /// Private key wallet (hex, tanpa 0x)
    #[arg(short, long, env = "PRIVATE_KEY")]
    private_key: String,

    /// RPC URL Ethereum
    #[arg(short, long, env = "RPC_URL", default_value = "https://eth.llamarpc.com")]
    rpc: String,

    /// Jumlah mint yang ingin dilakukan
    #[arg(short, long, default_value = "1")]
    count: u32,

    /// Jumlah thread untuk PoW solving
    #[arg(short, long, default_value = "4")]
    threads: usize,

    /// Hanya tampilkan info contract, tidak mint
    #[arg(long)]
    info: bool,
}

const CONTRACT: &str = "0xEFAd2Eab7172dDEbE5Ce7a41f5Ddf8fCcE4Ca0CB";

/// Solve PoW: cari nonce dimana keccak256(abi.encode(sender, nonce))
/// punya leading zero bits >= difficulty_bits
fn solve_pow(sender: Address, difficulty_bits: u64, threads: usize) -> U256 {
    println!("  🔧 Solving PoW dengan {} thread (difficulty: {} bits)...", threads, difficulty_bits);

    let found = Arc::new(AtomicBool::new(false));
    let result = Arc::new(AtomicU64::new(0));
    let result_high = Arc::new(AtomicU64::new(0));
    let result_mid = Arc::new(AtomicU64::new(0));
    let result_low = Arc::new(AtomicU64::new(0));
    let attempts = Arc::new(AtomicU64::new(0));

    let start = Instant::now();

    std::thread::scope(|s| {
        for t in 0..threads {
            let found = Arc::clone(&found);
            let result_high = Arc::clone(&result_high);
            let result_mid = Arc::clone(&result_mid);
            let result_low = Arc::clone(&result_low);
            let attempts = Arc::clone(&attempts);

            s.spawn(move || {
                let mut rng = rand::thread_rng();
                let sender_bytes = sender.as_slice();

                loop {
                    if found.load(Ordering::Relaxed) {
                        break;
                    }

                    // Generate random nonce (256-bit)
                    let n0: u64 = rng.gen();
                    let n1: u64 = rng.gen();
                    let n2: u64 = rng.gen();
                    let n3: u64 = rng.gen();

                    // abi.encode(address, uint256) = 32 bytes address (padded) + 32 bytes nonce
                    let mut data = [0u8; 64];
                    // address padded to 32 bytes (left-padded with zeros)
                    data[12..32].copy_from_slice(sender_bytes);
                    // uint256 nonce (big-endian)
                    data[32..40].copy_from_slice(&n0.to_be_bytes());
                    data[40..48].copy_from_slice(&n1.to_be_bytes());
                    data[48..56].copy_from_slice(&n2.to_be_bytes());
                    data[56..64].copy_from_slice(&n3.to_be_bytes());

                    let hash = keccak256(&data);
                    let hash_bytes = hash.as_slice();

                    // Count leading zero bits
                    let mut zeros = 0u64;
                    for byte in hash_bytes {
                        if *byte == 0 {
                            zeros += 8;
                        } else {
                            zeros += byte.leading_zeros() as u64;
                            break;
                        }
                    }

                    let local_attempts = attempts.fetch_add(1, Ordering::Relaxed);
                    if local_attempts % 500_000 == 0 && t == 0 {
                        let elapsed = start.elapsed().as_secs_f64();
                        let rate = local_attempts as f64 / elapsed / 1_000_000.0;
                        print!("\r  ⚡ {:.1}M hash/s | {} attempts...", rate, local_attempts);
                        use std::io::Write;
                        std::io::stdout().flush().ok();
                    }

                    if zeros >= difficulty_bits {
                        if !found.swap(true, Ordering::SeqCst) {
                            result_high.store(n0, Ordering::SeqCst);
                            result_mid.store(n1, Ordering::SeqCst);
                            result_low.store(n2, Ordering::SeqCst);
                            result.store(n3, Ordering::SeqCst);
                        }
                        break;
                    }
                }
            });
        }
    });

    println!();
    let elapsed = start.elapsed();
    let total = attempts.load(Ordering::Relaxed);
    println!("  ✅ Solved dalam {:.2}s ({} attempts)", elapsed.as_secs_f64(), total);

    // Reconstruct U256 nonce
    let h = result_high.load(Ordering::SeqCst);
    let m = result_mid.load(Ordering::SeqCst);
    let l = result_low.load(Ordering::SeqCst);
    let lo = result.load(Ordering::SeqCst);

    let mut bytes = [0u8; 32];
    bytes[0..8].copy_from_slice(&h.to_be_bytes());
    bytes[8..16].copy_from_slice(&m.to_be_bytes());
    bytes[16..24].copy_from_slice(&l.to_be_bytes());
    bytes[24..32].copy_from_slice(&lo.to_be_bytes());

    U256::from_be_bytes(bytes)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔══════════════════════════════════════╗");
    println!("║     pffthash.com CLI Minter v0.1     ║");
    println!("╚══════════════════════════════════════╝");
    println!();

    // Setup provider + wallet
    let pk = args.private_key.trim_start_matches("0x");
    let signer: PrivateKeySigner = pk.parse().context("Private key tidak valid")?;
    let wallet_addr = signer.address();
    let wallet = EthereumWallet::from(signer);

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_builtin(&args.rpc)
        .await
        .context("Gagal connect ke RPC")?;

    let contract_addr: Address = CONTRACT.parse()?;
    let contract = PfftHash::new(contract_addr, &provider);

    println!("🔗 RPC     : {}", args.rpc);
    println!("👛 Wallet  : {}", wallet_addr);
    println!("📄 Contract: {}", CONTRACT);
    println!();

    // Fetch contract info
    println!("📊 Fetching contract info...");
    let info = contract.getInfo().call().await
        .context("Gagal fetch getInfo()")?;
    let pow_bits = contract.POW_DIFFICULTY_BITS().call().await
        .context("Gagal fetch POW_DIFFICULTY_BITS()")?._0;
    let pow_stage = contract.currentPowStage().call().await
        .context("Gagal fetch currentPowStage()")?._0;
    let max_supply = contract.MAX_SUPPLY().call().await
        .context("Gagal fetch MAX_SUPPLY()")?._0;

    println!("  Minted       : {} / {}", info.currentMinted, max_supply);
    println!("  Remaining    : {}", info.remainingSupply);
    println!("  Next Mint Amt: {}", info.nextMintAmount);
    println!("  PoW Stage    : {}", pow_stage);
    println!("  PoW Difficulty: {} bits", pow_bits);
    println!();

    if info.remainingSupply == U256::ZERO {
        println!("❌ Supply sudah habis!");
        return Ok(());
    }

    if args.info {
        return Ok(());
    }

    // Mint loop
    for i in 1..=args.count {
        println!("🚀 Mint #{}/{}", i, args.count);

        // Solve PoW
        let difficulty = pow_bits.to::<u64>();
        let nonce = solve_pow(wallet_addr, difficulty, args.threads);
        println!("  🎲 Nonce: {}", nonce);

        // Send transaction
        println!("  📤 Sending transaction...");
        let tx = contract.freeMint(nonce).send().await
            .context("Gagal kirim transaksi")?;

        let tx_hash = tx.tx_hash();
        println!("  🔗 TX Hash: 0x{:x}", tx_hash);
        println!("  ⏳ Waiting confirmation...");

        let receipt = tx.get_receipt().await
            .context("Gagal dapat receipt")?;

        if receipt.status() {
            println!("  ✅ MINT SUKSES! Block: {}", receipt.block_number.unwrap_or(0));
        } else {
            println!("  ❌ Transaksi GAGAL! Check Etherscan.");
        }
        println!();
    }

    println!("🎉 Selesai!");
    Ok(())
}
