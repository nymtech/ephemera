## Ephemera - _lightweight_ reliable broadcast protocol

### General Info

* Accepts signed messages from clients which go to mempool
   *  Messages are signed by the client
* Puts the messages from mempool into a block and runs reliable broadcast protocol to disseminate the block
   * Blocks are signed by the node


#### How to run

[README](../scripts/README.md)

#### Example Target Application

[README](../examples/http-ws/README.md)

### Websocket

Finalized blocks are sent to websocket subscribers

### Database

Finalized blocks are also stored in database

Currently, it runs two versions of databases:
 * RocksDB
 * SQLite

It seems better to use RocksDB in the end...