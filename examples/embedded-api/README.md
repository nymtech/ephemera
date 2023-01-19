# Ephemera embedded API demonstration

This example assumes that exactly 3 Ephemera nodes are configured for cluster(otherwise cluster never reaches consensus)

## What happens

1. It starts 3 Ephemera nodes
2. Example app sends a message to Ephemera node 1
3. Example app queries all 3 nodes for the message(stored on their corresponding databases)

## Running

```bash
cargo run -- --node-config-1 ~/.ephemera/node1/ephemera.toml --node-config-2 ~/.ephemera/node2/ephemera.toml  --node-config-3 ~/.ephemera/node3/ephemera.toml
```

## Expected ouput 

```text
Sending request RbMsg { id: "19380bfe-58e7-40c2-9dd8-e9d8e2945965", node_id: "client", timestamp: Some(Timestamp { seconds: 1674141847, nanos: 613089000 }), reliable_broadcast: Some(PrePrepare(PrePrepareMsg { payload: [80, 97, 121, 108, 111, 97, 100] })) }
```

```text
Node 1 received message EphemeraMessage { request_id: "19380bfe-58e7-40c2-9dd8-e9d8e2945965", message: [80, 97, 121, 108, 111, 97, 100], signatures: [Signature { id: "/ip4/127.0.0.1/tcp/3002", signature: "f56d69857cc67d51ba96938a1573ce2dba352514943ff33e82a89c504fe66bf4cc2c14540a8f01e08245f6f60cf5776325326c95ec1efedaf8b41eebfd71c20b" }, Signature { id: "/ip4/127.0.0.1/tcp/3003", signature: "cd28c62439c46ef465a5121eff22760da79def47d87ad46b55ea3294663e051b487be22f31e0de417f545241e72de437f84de45767184b76cffe4b151e3d4109" }, Signature { id: "/ip4/127.0.0.1/tcp/3001", signature: "15acfd5c03072744380f6a9e61b75f42e93f5a7d26b9eab4089c05699963885740d158fc5bed541ac1b8dd759b785747cec86517caceff9ae09e383eb0756d0d" }] }
```

```text
Node 2 received message EphemeraMessage { request_id: "19380bfe-58e7-40c2-9dd8-e9d8e2945965", message: [80, 97, 121, 108, 111, 97, 100], signatures: [Signature { id: "/ip4/127.0.0.1/tcp/3002", signature: "f56d69857cc67d51ba96938a1573ce2dba352514943ff33e82a89c504fe66bf4cc2c14540a8f01e08245f6f60cf5776325326c95ec1efedaf8b41eebfd71c20b" }, Signature { id: "/ip4/127.0.0.1/tcp/3003", signature: "cd28c62439c46ef465a5121eff22760da79def47d87ad46b55ea3294663e051b487be22f31e0de417f545241e72de437f84de45767184b76cffe4b151e3d4109" }, Signature { id: "/ip4/127.0.0.1/tcp/3001", signature: "15acfd5c03072744380f6a9e61b75f42e93f5a7d26b9eab4089c05699963885740d158fc5bed541ac1b8dd759b785747cec86517caceff9ae09e383eb0756d0d" }] }
```

```text
Node 3 received message EphemeraMessage { request_id: "19380bfe-58e7-40c2-9dd8-e9d8e2945965", message: [80, 97, 121, 108, 111, 97, 100], signatures: [Signature { id: "/ip4/127.0.0.1/tcp/3002", signature: "f56d69857cc67d51ba96938a1573ce2dba352514943ff33e82a89c504fe66bf4cc2c14540a8f01e08245f6f60cf5776325326c95ec1efedaf8b41eebfd71c20b" }, Signature { id: "/ip4/127.0.0.1/tcp/3001", signature: "15acfd5c03072744380f6a9e61b75f42e93f5a7d26b9eab4089c05699963885740d158fc5bed541ac1b8dd759b785747cec86517caceff9ae09e383eb0756d0d" }, Signature { id: "/ip4/127.0.0.1/tcp/3003", signature: "cd28c62439c46ef465a5121eff22760da79def47d87ad46b55ea3294663e051b487be22f31e0de417f545241e72de437f84de45767184b76cffe4b151e3d4109" }] }
```