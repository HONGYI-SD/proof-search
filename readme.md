This repo is a http server, supprt seach bridge tx proof by signature.
# server run 
```
cargo run -- dbconfig.yaml
```

# client test
request
```sh
curl "http://localhost:6688/proof?signature=1111"
```
response
```json
{
    "jsonrpc": "2.0",
    "result": {
        "index": 4,
        "signature": "1111",
        "proof": "proof"
    },
    "error": null,
    "id": 1
}
```