use cuprate_blockchain::{config::ConfigBuilder, ops, tables::{OpenTables, Tables}} ;
use cuprate_database::{ConcreteEnv, DatabaseRo, Env, EnvInner};
use hex::FromHex;
use actix_web::{get, web::{self}, App, HttpServer, Responder};
use serde::Serialize;

#[derive(Serialize)]
struct TransactionOutput {
    pub amount: u64,
    pub public_key: String,
}

#[derive(Serialize)]
struct TransactionResponse {
    pub hash: String,
    pub version: u8,
    pub is_coinbase: bool,
    pub confirmation_height: usize,
    pub timestamp: u64,
    pub weight: usize,
    pub inputs: Vec<String>,
    pub outputs: Vec<TransactionOutput>,
    pub extra: String,
}

#[get("/api/tx/{tx_hash}")]
async fn get_tx(
    env: web::Data<ConcreteEnv>,
    tx_hash: web::Path<String>
) -> std::io::Result<impl Responder> {
    let tx_hash_buff = <[u8; 32]>::from_hex(tx_hash.clone().to_string())
        .unwrap();

    let env_inner = env.env_inner();
    let tx_ro = env_inner.tx_ro().unwrap();
    let tables = env_inner.open_tables(&tx_ro).unwrap();
    
    let tx_id = tables.tx_ids().get(&tx_hash_buff).unwrap();
    let tx = ops::tx::get_tx_from_id(&tx_id, tables.tx_blobs()).unwrap();
    let tx_height = tables.tx_heights().get(&tx_id).unwrap();
    let tx_block = tables.block_infos().get(&tx_height).unwrap();
    let tx_prefix = tx.prefix();

    let mut outputs: Vec<TransactionOutput> = Vec::with_capacity(tx_prefix.outputs.len());

    for o in &tx_prefix.outputs {
        outputs.push(TransactionOutput {
            amount: o.amount.unwrap(),
            public_key: hex::encode(o.key.to_bytes())
        })
    }

    let response = TransactionResponse {
        hash: tx_hash.clone(),
        version: tx.version(),
        is_coinbase: tx_prefix.inputs.len() == 0,
        confirmation_height: tx_height,
        timestamp: tx_block.timestamp,
        weight: tx.weight(),
        inputs: Vec::with_capacity(0),
        outputs,
        extra: hex::encode(tx_prefix.extra.clone()),
    };

    Ok(web::Json(response))
}

#[derive(Serialize)]
struct BlockTransactionResponse {
    pub hash: String,
    pub version: u8,
    pub is_coinbase: bool,
    pub weight: usize,
    pub inputs: Vec<String>,
    pub outputs: Vec<TransactionOutput>,
    pub extra: String,
}

#[derive(Serialize)]
struct BlockResponse  {
    pub hash: String,
    pub timestamp: u64,
    pub weight: usize,
    pub cumulative_generated_coins: u64,
    pub cumulative_difficulty_low: u64,
    pub cumulative_difficulty_high: u64,
    pub cumulative_rct_outs: u64,
    pub long_term_weight: usize,
    pub transactions: Vec<BlockTransactionResponse>,
}

#[get("/api/block/{height}")]
async fn get_block(
    env: web::Data<ConcreteEnv>,
    height: web::Path<String>
) -> std::io::Result<impl Responder> {
    let env_inner = env.env_inner();
    let tx_ro = env_inner.tx_ro().unwrap();
    let tables = env_inner.open_tables(&tx_ro).unwrap();

    let height = height.parse::<usize>().ok().unwrap();
    let block_info = ops::block::get_block_info(&height, tables.block_infos()).unwrap();
    let block_tx_hashes = tables.block_txs_hashes().get(&height).unwrap();  
    let mut transactions: Vec<BlockTransactionResponse> = Vec::with_capacity(block_tx_hashes.len());
    
    for tx_hash in block_tx_hashes.iter() {
        let tx = ops::tx::get_tx(&tx_hash, tables.tx_ids(), tables.tx_blobs()).unwrap();
        let tx_prefix = tx.prefix();
        let mut outputs: Vec<TransactionOutput> = Vec::with_capacity(tx_prefix.outputs.len());
    
        for o in &tx_prefix.outputs {
            outputs.push(TransactionOutput {
                amount: o.amount.unwrap(),
                public_key: hex::encode(o.key.to_bytes())
            })
        }
    
        transactions.push(BlockTransactionResponse {
            hash: hex::encode(tx.hash()),
            version: tx.version(),
            is_coinbase: true,
            weight: tx.weight(),
            inputs: Vec::with_capacity(0),
            outputs: outputs,
            extra: hex::encode(tx_prefix.extra.clone()),
        });

        break;
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
        transactions,
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
    let env_state = web::Data::new(env);

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