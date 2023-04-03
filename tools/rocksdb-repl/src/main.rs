use clap::Parser;
use easy_repl::{command, CommandStatus, Repl};

#[derive(Parser, Debug, Clone)]
#[command()]
pub(crate) struct Args {
    #[clap(short, long)]
    pub(crate) db_path: String,
}

fn main() {
    let db_path = Args::parse().db_path;

    let db = rocksdb::DB::open_default(db_path).unwrap();

    println!("Available prefixes for get command:");
    println!("  - block_hash:");
    println!("  - block_height:");
    println!("  - block_certificates:");
    println!("  - last_block");

    let mut repl = Repl::builder()
        .add(
            "get",
            command! {
                "Get a value",
                (key: String) => |key| {
                       if let Ok(Some(value)) = db.get(key) {
                            let value = String::from_utf8(value.to_vec()).unwrap();
                            println!("Value: {value}",);
                        } else {
                            println!("Key not found");
                        }
                    Ok(CommandStatus::Done)
                }
            },
        )
        .build()
        .expect("Failed to create repl");

    repl.run().expect("Critical REPL error");
}
