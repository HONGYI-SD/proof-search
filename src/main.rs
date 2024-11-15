use warp::{Filter, Rejection, Reply};
use serde::Serialize;
use tokio_postgres::{NoTls, Client};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

#[derive(Serialize)]
struct BridgeTxProof {
    index: i64,
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
    let (client, connection) = tokio_postgres::connect(
        "host=13.215.160.229 port=7530 user=solana password=fji289afhfia&#&wiofhe9419ut9@* dbname=validator",
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

    warp::serve(routes).run(([127, 0, 0, 1], 6688)).await;
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
        "SELECT id, signature, proof FROM bridge_transaction WHERE signature = $1",
        &[&sig],
    ).await {
        Ok(row) => {
            let tx = BridgeTxProof {
                index: row.get(0),
                signature: row.get(1),
                proof: row.get(2),
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
