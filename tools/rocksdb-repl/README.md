# RocksDb CLI

Simple utility(which runs full database!) to query RocksDB. Can't find a free GUI.

## Usage

Currently only supports querying by key.

```bash
cargo run -- --db-path /path/to/db
```

```bash
> get block_id:50a75cba-390d-4d2c-bf7b-60b71e9cd3a8
Value: {"header":{"id":"50a75cba-390d-4d2c-bf7b-60b71e9cd3a8","timestamp":1676126840571,"creator":"12D3KooWSRicihcusHTengn6UT3NqwRVFSS74VQFcE5D8Wak87SU","height":0,"label":"50a75cba-390d-4d2c-bf7b-60b71e9cd3a8"},"signed_messages":[],"signature":{"signature":"77bf2925e1ecc0f29e3798df9cc5cbb7d0596ce62cb5189598886a5b7a9b3e2875c044f4e80a99d5bb50eb21c41914fe83f8881340d07a70979981b357871103","public_key":"0801122011c3056299123c6ee3b3c5647aa68791f9edb70344a77097956ea50f0bd0c3c9"}}
```