use cuprate_blockchain::{config::ConfigBuilder, ops, tables::{OpenTables, Tables}, types::PreRctOutputId} ;
use cuprate_database::{ConcreteEnv, DatabaseRo, Env, EnvInner};
use cuprate_types::json::tx::Transaction;
use hex::FromHex;
use actix_web::{get, web::{self}, App, HttpServer, Responder};
use monero_serai;
use serde::Serialize;
use rayon::prelude::*;

#[derive(Serialize)]
struct TransactionInput {
    pub amount: u64,
    pub key_image: String,
    pub mixins: Vec<TransactionInputMixin>,
}

#[derive(Serialize)]
struct TransactionInputMixin {
    pub height: u32,
    pub public_key: String,
    pub tx_hash: String,
}

#[derive(Serialize)]
struct TransactionOutput {
    pub amount: u64,
    pub public_key: String,
}

#[derive(Serialize)]
struct TransactionResponse {
    pub hash: String,
    pub version: u8,
    pub unlock_time: u64,
    pub is_coinbase: bool,
    pub confirmation_height: usize,
    pub timestamp: u64,
    pub weight: usize,
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<TransactionOutput>,
    pub extra: String,
}

#[get("/api/transaction/{tx_hash}")]
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
    let tx_height = tables.tx_heights().get(&tx_id).unwrap();
    let tx_block = tables.block_infos().get(&tx_height).unwrap();
    let tx = ops::tx::get_tx_from_id(&tx_id, tables.tx_blobs()).unwrap();

    let response: TransactionResponse = match tx.clone().into() {
        Transaction::V1 { prefix } => {
            let mut inputs: Vec<TransactionInput> = Vec::with_capacity(prefix.vin.len());

            for input in &prefix.vin {
                let mut mixins: Vec<TransactionInputMixin> = Vec::with_capacity(
                    input.key.key_offsets.len()
                );

                for key_offset in &input.key.key_offsets {
                    let output = tables.outputs().get(
                        &PreRctOutputId {
                            amount: input.key.amount,
                            amount_index: key_offset.clone()
                        }
                    ).unwrap();

                    let mixin_tx_blob = tables.tx_blobs().get(&output.tx_idx).unwrap();
                    let mixin_tx_hash = monero_serai::transaction::Transaction::read(&mut mixin_tx_blob.0.as_slice()).unwrap().hash();

                    mixins.push(TransactionInputMixin {
                        height: output.height,
                        public_key: hex::encode(output.key),
                        tx_hash: hex::encode(mixin_tx_hash),
                    });
                }
                
                inputs.push(TransactionInput {
                    amount: input.key.amount,
                    key_image: hex::encode(*input.key.k_image),
                    mixins: mixins
                });
            }

            let mut outputs: Vec<TransactionOutput> = Vec::with_capacity(prefix.vout.len());

            for output in &prefix.vout {
                let public_key_buff = match &output.target {
                    cuprate_types::json::output::Target::Key { key } => {
                        key
                    },
                    cuprate_types::json::output::Target::TaggedKey { tagged_key } => {
                        &tagged_key.key
                    }
                };

                outputs.push(TransactionOutput {
                    amount: output.amount,
                    public_key: hex::encode(**public_key_buff)
                })
            }

            TransactionResponse {
                hash: tx_hash.clone(),
                version: prefix.version,
                unlock_time: prefix.unlock_time,
                is_coinbase: prefix.vin.len() == 0,
                confirmation_height: tx_height,
                timestamp: tx_block.timestamp,
                weight: tx.weight(),
                extra: hex::encode(prefix.extra),
                outputs,
                inputs,
            }
        },
        Transaction::V2 { prefix, rct_signatures: _, rctsig_prunable: _ } => {
            let inputs = prefix.vin
                .par_iter()
                .map(|input| {
                    let mixins: Vec<TransactionInputMixin> = input
                        .key
                        .key_offsets
                        .clone()
                        .par_iter()
                        .enumerate()
                        .map(|(key_offset_i, key_offset)| {
                            let new_tx_ro = env_inner.tx_ro().unwrap();
                            let new_tables = env_inner.open_tables(&new_tx_ro).unwrap();
                            
                            let mut key_offset_sum: u64 = input.key.key_offsets[0..key_offset_i].iter().copied().sum();
                            key_offset_sum += key_offset;
                            let rct_output = new_tables.rct_outputs().get(&key_offset_sum).unwrap();
                            // let mixin_tx_blob = new_tables.tx_blobs().get(&rct_output.tx_idx).unwrap();
                            let mixin_tx_block = new_tables.block_infos().get(&(rct_output.height as usize)).unwrap();
                            let mixin_tx_hash: [u8; 32] = if rct_output.tx_idx == mixin_tx_block.mining_tx_index {
                                let tx_blob = new_tables.tx_blobs().get(&rct_output.tx_idx).unwrap();
                                monero_serai::transaction::Transaction::read(&mut tx_blob.0.as_slice()).unwrap().hash()
                            } else {
                                let block_tx_index = (rct_output.tx_idx - mixin_tx_block.mining_tx_index - 1) as usize;
                                new_tables.block_txs_hashes().get(&(rct_output.height as usize)).unwrap()[block_tx_index]
                            };

                            TransactionInputMixin {
                                height: rct_output.height,
                                public_key: hex::encode(rct_output.key),
                                tx_hash: hex::encode(mixin_tx_hash),
                            }
                        })
                        .collect();
                
                    TransactionInput {
                        amount: input.key.amount,
                        key_image: hex::encode(*input.key.k_image),
                        mixins,
                    }
                })
                .collect();

            let mut outputs: Vec<TransactionOutput> = Vec::with_capacity(prefix.vout.len());

            for output in &prefix.vout {
                let public_key_buff = match &output.target {
                    cuprate_types::json::output::Target::Key { key } => {
                        key
                    },
                    cuprate_types::json::output::Target::TaggedKey { tagged_key } => {
                        &tagged_key.key
                    }
                };

                outputs.push(TransactionOutput {
                    amount: output.amount,
                    public_key: hex::encode(**public_key_buff)
                })
            }

            TransactionResponse {
                hash: tx_hash.clone(),
                version: prefix.version,
                unlock_time: prefix.unlock_time,
                is_coinbase: prefix.vin.len() == 0,
                confirmation_height: tx_height,
                timestamp: tx_block.timestamp,
                weight: tx.weight(),
                extra: hex::encode(prefix.extra),
                outputs,
                inputs,
            }
        }
    };

    Ok(web::Json(response))
}

#[derive(Serialize)]
struct BlockTransactionResponse {
    pub hash: String,
    pub version: u8,
    pub is_coinbase: bool,
    pub weight: usize,
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
    
        transactions.push(BlockTransactionResponse {
            hash: hex::encode(tx.hash()),
            version: tx.version(),
            is_coinbase: true,
            weight: tx.weight(),
            extra: hex::encode(tx_prefix.extra.clone()),
        });
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
    let cuprate_dir = "/run/media/artur/Misc/.cuprate";
    
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
    .bind(("127.0.0.1", 8081))?
    .run()
    .await
}