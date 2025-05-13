use std::sync::Mutex;
use cuprate_blockchain::{config::ConfigBuilder, ops, tables::{OpenTables, Tables}} ;
use cuprate_database::{ConcreteEnv, DatabaseRo, Env, EnvInner};
use cuprate_types::json::tx::Transaction;
use hex::FromHex;
use actix_web::{get, web, App, HttpServer, Responder};
use serde::Serialize;

struct EnvState { env: Mutex<ConcreteEnv> }

#[get("/api/tx/{tx_hash}")]
async fn get_tx(
    env_state: web::Data<EnvState>,
    tx_hash: web::Path<String>
) -> std::io::Result<impl Responder> {
    let tx_hash_buff = <[u8; 32]>::from_hex(tx_hash.to_string())
        .unwrap();

    let env = env_state.env.lock().unwrap();
    let env_inner = env.env_inner();
    let tx_ro = env_inner.tx_ro().unwrap();
    let tables = env_inner.open_tables(&tx_ro).unwrap();

    let tx = ops::tx::get_tx(
            &tx_hash_buff,
            tables.tx_ids(),
            tables.tx_blobs()
        )
        .unwrap();

    let response: Transaction = tx.into();

    Ok(web::Json(response))
}

#[derive(Serialize)]
struct BlockResponse  {
    pub timestamp: u64,
    pub cumulative_generated_coins: u64,
    pub weight: usize,
    pub cumulative_difficulty_low: u64,
    pub cumulative_difficulty_high: u64,
    pub hash: String,
    pub cumulative_rct_outs: u64,
    pub long_term_weight: usize,
    pub tx_hashes: Vec<String>,
}

#[get("/api/block/{height}")]
async fn get_block(
    env_state: web::Data<EnvState>,
    height: web::Path<String>
) -> std::io::Result<impl Responder> {
    let env = env_state.env.lock().unwrap();
    let env_inner = env.env_inner();
    let tx_ro = env_inner.tx_ro().unwrap();
    let tables = env_inner.open_tables(&tx_ro).unwrap();

    let height = height.parse::<usize>().ok().unwrap();
    let block_info = ops::block::get_block_info(&height, tables.block_infos()).unwrap();
    let block_tx_hashes = tables.block_txs_hashes().get(&height).unwrap();
    let block_tx_num = block_tx_hashes.len();    
    let mut tx_hashes: Vec<String> = Vec::with_capacity(block_tx_num);

    for tx_id_bytes in block_tx_hashes.iter() {
        tx_hashes.push(hex::encode(&tx_id_bytes));
    }

    let response = BlockResponse {
        timestamp: block_info.timestamp,
        cumulative_generated_coins: block_info.cumulative_generated_coins,
        weight: block_info.weight,
        cumulative_difficulty_low: block_info.cumulative_difficulty_low,
        cumulative_difficulty_high: block_info.cumulative_difficulty_high,
        hash: hex::encode(&block_info.block_hash),
        cumulative_rct_outs: block_info.cumulative_rct_outs,
        long_term_weight: block_info.long_term_weight,
        tx_hashes,
    };

    Ok(web::Json(response))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let user = whoami::username();
    let cuprate_dir = format!("/home/{}/.local/share/cuprate", user);
    
    let config = ConfigBuilder::new()
        .data_directory(cuprate_dir.into())
        .build();

    let env = cuprate_blockchain::open(config).unwrap();
    let env_state = web::Data::new(EnvState { env: env.into() });

    HttpServer::new(move || {
        App::new()
            .app_data(env_state.clone())
            .service(get_tx)
            .service(get_block)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}