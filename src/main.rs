use warp::{Filter, Rejection, Reply};
use serde::{Deserialize, Serialize};
use serde_yaml;
use tokio_postgres::{NoTls, Client};
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

#[derive(Deserialize)]
struct Config {
    database: DatabaseConfig,
}

#[derive(Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    user: String,
    password: String,
    dbname: String,
}

#[derive(Serialize)]
struct BridgeTxProof {
    index: i64,
    root_program_slot: i64,
    signature: String,
    proof: String,
}

#[derive(Serialize)]
struct RpcResponse<T> {
    jsonrpc: String,
    result: Option<T>,
    error: Option<RpcError>,
    id: u32,
}

#[derive(Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl<T> RpcResponse<T> {
    fn success(result: T, id: u32) -> Self {
        RpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    fn error(code: i32, message: String, id: u32) -> Self {
        RpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError { code, message }),
            id,
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <config_file>", args[0]);
        std::process::exit(1);
    }
    let config_file = &args[1];

    // 加载配置文件
    let config: Config = load_config(config_file).expect("Failed to load configuration");

    // 创建 PostgreSQL 客户端
    let (client, connection) = tokio_postgres::connect(
        &format!(
            "host={} port={} user={} password={} dbname={}",
            config.database.host,
            config.database.port,
            config.database.user,
            config.database.password,
            config.database.dbname
        ),
        NoTls,
    )
    .await
    .expect("Failed to connect to PostgreSQL");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    let db_client = Arc::new(Mutex::new(client));

    let proof_route = warp::path("proof")
        .and(warp::get())
        .and(warp::query::<HashMap<String, String>>())
        .and(with_db(db_client.clone()))
        .and_then(handle_get_proof);

    let not_found_route = warp::any().map(|| {
        let response = RpcResponse::<()>::error(
            -32601,
            "Method not found. Only 'proof' route is supported.".to_string(),
            1,
        );
        warp::reply::json(&response)
    });

    let routes = proof_route.or(not_found_route);

    warp::serve(routes).run(([0, 0, 0, 0], 6688)).await;
}

fn with_db(
    db_client: Arc<Mutex<Client>>,
) -> impl Filter<Extract = (Arc<Mutex<Client>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || db_client.clone())
}

async fn handle_get_proof(
    query: HashMap<String, String>,
    db_client: Arc<Mutex<Client>>,
) -> Result<impl Reply, Rejection> {
    let sig = match query.get("signature") {
        Some(name) => name.clone(),
        None => {
            let response = RpcResponse::<()>::error(-32602, "Missing 'signature' parameter".to_string(), 1);
            return Ok(warp::reply::json(&response));
        }
    };

    let client = db_client.lock().await;
    match client.query_one(
        "SELECT id, root_program_slot, signature, proof FROM bridge_transaction WHERE signature = $1",
        &[&sig],
    ).await {
        Ok(row) => {
            let tx = BridgeTxProof {
                index: row.get(0),
                root_program_slot: row.get(1),
                signature: row.get(2),
                proof: row.get(3),
            };

            let response = RpcResponse::success(tx, 1);
            Ok(warp::reply::json(&response))
        }
        Err(_) => {
            let response = RpcResponse::<()>::error(-32000, "tx not found".to_string(), 1);
            Ok(warp::reply::json(&response))
        }
    }
}

fn load_config(filename: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(filename)?;
    let config: Config = serde_yaml::from_str(&content)?;
    Ok(config)
}