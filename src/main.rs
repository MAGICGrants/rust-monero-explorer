use cuprate_blockchain::{config::ConfigBuilder, ops, tables::{OpenTables, Tables}, types::PreRctOutputId} ;
use cuprate_database::{ConcreteEnv, DatabaseRo, Env, EnvInner, RuntimeError};
use cuprate_types::json::tx::Transaction;
use hex::{FromHex, FromHexError};
use actix_web::{error, get, http::StatusCode, web::{self}, App, HttpResponse, HttpServer, Responder};
use monero_serai;
use serde::Serialize;
use rayon::prelude::*;
use clap::Parser;
use std::{net::IpAddr, path::PathBuf, process::exit};
use derive_more::derive::{Display, Error};
use regex::Regex;

#[derive(Serialize)]
struct ApiErrorBody {
    error: String,
}

#[derive(Debug, Display, Error)]
enum AppError {
    #[display("TX_HASH_NOT_FOUND")]
    TransactionNotFoundError,
    #[display("INVALID_TX_HASH")]
    InvalidTxHashError,
    #[display("INVALID_BLOCK_HEIGHT")]
    InvalidBlockHeightError,
    #[display("BLOCK_NOT_FOUND")]
    BlockNotFoundError,
    #[display("INTERNAL_SERVER_ERROR")]
    InternalServerError,
}

impl error::ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let json_response = ApiErrorBody {
            error: self.to_string(),
        };

        HttpResponse::build(self.status_code())
            .json(json_response)
    }
    fn status_code(&self) -> StatusCode {
        match *self {
            AppError::TransactionNotFoundError => StatusCode::BAD_REQUEST,
            AppError::InvalidTxHashError => StatusCode::BAD_REQUEST,
            AppError::InvalidBlockHeightError => StatusCode::BAD_REQUEST,
            AppError::BlockNotFoundError => StatusCode::BAD_REQUEST,
            AppError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<FromHexError> for AppError {
    fn from(_: FromHexError) -> Self {
        AppError::InternalServerError
    }
}

impl From<RuntimeError> for AppError {
    fn from(_: RuntimeError) -> Self {
        AppError::InternalServerError
    }
}

impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        AppError::InternalServerError
    }
}


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
) -> Result<impl Responder, AppError> {
    let tx_hash_regex = Regex::new(r"^[0-9a-fA-F]{64}$")
        .map_err(|_| AppError::TransactionNotFoundError)?;

    if !tx_hash_regex.is_match(&tx_hash) {
        return Err(AppError::InvalidTxHashError)
    }

    let tx_hash_buff = <[u8; 32]>::from_hex(tx_hash.clone().to_string())?;

    let env_inner = env.env_inner();
    let tx_ro = env_inner.tx_ro()?;
    let tables = env_inner.open_tables(&tx_ro)?;

    let tx_id = tables.tx_ids().get(&tx_hash_buff).map_err(|_| AppError::TransactionNotFoundError)?;
    let tx_height = tables.tx_heights().get(&tx_id)?;
    let tx_block = tables.block_infos().get(&tx_height)?;
    let tx = ops::tx::get_tx_from_id(&tx_id, tables.tx_blobs())?;

    let response: TransactionResponse = match tx.clone().into() {
        Transaction::V1 { prefix } => {
            let inputs: Result<Vec<TransactionInput>, AppError>= prefix.vin.par_iter().map(|input| {
                let mixins: Result<Vec<TransactionInputMixin>, AppError> = input
                    .key
                    .key_offsets
                    .clone()
                    .par_iter()
                    .enumerate()
                    .map(|(key_offset_i, key_offset)| {
                        let new_tx_ro = env_inner.tx_ro()?;
                        let new_tables = env_inner.open_tables(&new_tx_ro)?;

                        let mut key_offset_sum: u64 = input.key.key_offsets[0..key_offset_i].iter().copied().sum();
                        key_offset_sum += key_offset;

                        let output = new_tables.outputs().get(
                            &PreRctOutputId {
                                amount: input.key.amount,
                                amount_index: key_offset_sum
                            }
                        )?;

                        let mixin_tx_block = new_tables.block_infos().get(&(output.height as usize))?;
                        let mixin_tx_hash: [u8; 32] = if output.tx_idx == mixin_tx_block.mining_tx_index {
                            let tx_blob = new_tables.tx_blobs().get(&output.tx_idx)?;
                            monero_serai::transaction::Transaction::read(&mut tx_blob.0.as_slice())?.hash()
                        } else {
                            let block_tx_index = (output.tx_idx - mixin_tx_block.mining_tx_index - 1) as usize;
                            new_tables.block_txs_hashes().get(&(output.height as usize))?[block_tx_index]
                        };

                        Ok(TransactionInputMixin {
                            height: output.height,
                            public_key: hex::encode(output.key),
                            tx_hash: hex::encode(mixin_tx_hash),
                        })
                    })
                    .collect();
                
                Ok(TransactionInput {
                    amount: input.key.amount,
                    key_image: hex::encode(*input.key.k_image),
                    mixins: mixins?
                })
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
                inputs: inputs?,
            }
        },
        Transaction::V2 { prefix, rct_signatures: _, rctsig_prunable: _ } => {
            let inputs: Result<Vec<TransactionInput>, AppError> = prefix.vin
                .par_iter()
                .map(|input| {
                    let mixins: Result<Vec<TransactionInputMixin>, AppError> = input
                        .key
                        .key_offsets
                        .clone()
                        .par_iter()
                        .enumerate()
                        .map(|(key_offset_i, key_offset)| {
                            let new_tx_ro = env_inner.tx_ro()?;
                            let new_tables = env_inner.open_tables(&new_tx_ro)?;
                            
                            let mut key_offset_sum: u64 = input.key.key_offsets[0..key_offset_i].iter().copied().sum();
                            key_offset_sum += key_offset;
                            let rct_output = new_tables.rct_outputs().get(&key_offset_sum)?;
                            let mixin_tx_block = new_tables.block_infos().get(&(rct_output.height as usize))?;
                            let mixin_tx_hash: [u8; 32] = if rct_output.tx_idx == mixin_tx_block.mining_tx_index {
                                let tx_blob = new_tables.tx_blobs().get(&rct_output.tx_idx)?;
                                monero_serai::transaction::Transaction::read(&mut tx_blob.0.as_slice())?.hash()
                            } else {
                                let block_tx_index = (rct_output.tx_idx - mixin_tx_block.mining_tx_index - 1) as usize;
                                new_tables.block_txs_hashes().get(&(rct_output.height as usize))?[block_tx_index]
                            };

                            Ok(TransactionInputMixin {
                                height: rct_output.height,
                                public_key: hex::encode(rct_output.key),
                                tx_hash: hex::encode(mixin_tx_hash),
                            })
                        })
                        .collect();
                
                    Ok(TransactionInput {
                        amount: input.key.amount,
                        key_image: hex::encode(*input.key.k_image),
                        mixins: mixins?,
                    })
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
                inputs: inputs?,
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
) -> Result<impl Responder, AppError> {
    let env_inner = env.env_inner();
    let tx_ro = env_inner.tx_ro()?;
    let tables = env_inner.open_tables(&tx_ro)?;

    let height = height.parse::<usize>().ok().ok_or(AppError::InvalidBlockHeightError)?;
    let block_info = ops::block::get_block_info(&height, tables.block_infos())
        .map_err(|_| AppError::BlockNotFoundError)?;
    let block_tx_hashes = tables.block_txs_hashes().get(&height)?; 
    
    let mut transactions: Vec<BlockTransactionResponse> = Vec::with_capacity(block_tx_hashes.len());
    
    for tx_hash in block_tx_hashes.iter() {
        let tx = ops::tx::get_tx(&tx_hash, tables.tx_ids(), tables.tx_blobs())?;
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

#[derive(Parser, Debug)]
#[command(author, version, about = "A Monero block explorer API built on Cuprate.", long_about = None)]
struct Args {
    #[arg(short = 'd', long, value_name = "CUPRATE_DIR")]
    cuprate_dir: Option<PathBuf>,

    #[arg(short, long, value_name = "PORT", default_value_t = 8081)]
    port: u16,

    #[arg(short = 'i', long, default_value_t = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))]
    bind_ip: IpAddr,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let default_user_dir = PathBuf::from(
        format!("/home/{}/.local/share/cuprate", whoami::username())
    );

    let final_cuprate_dir = args.cuprate_dir.unwrap_or(default_user_dir);
    let mut data_file_path = final_cuprate_dir.clone();
    data_file_path.push("blockchain");
    data_file_path.push("data.mdb");
    
    if !final_cuprate_dir.exists() || !data_file_path.exists() {
        println!("Could not find Cuprate directory at {:?}", final_cuprate_dir);
        exit(1);
    }

    println!("Using cuprate dir at {:?}", final_cuprate_dir);
  
    let config = ConfigBuilder::new()
        .data_directory(final_cuprate_dir)
        .build();

    let env = cuprate_blockchain::open(config).unwrap();
    let env_state = web::Data::new(env);

    println!("Listening on {}:{}", args.bind_ip, args.port);

    HttpServer::new(move || {
        App::new()
            .app_data(env_state.clone())
            .service(get_tx)
            .service(get_block)
    })
    .bind((args.bind_ip, args.port))?
    .run()
    .await
}