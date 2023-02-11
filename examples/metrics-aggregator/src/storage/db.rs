use std::path::PathBuf;

use rusqlite::{params, Connection};

use crate::contract::MixnodeToReward;
use crate::metrics::types::MixnodeResult;

mod migrations {
    use refinery::embed_migrations;

    embed_migrations!("migrations");
}

pub(crate) struct Storage {
    connection: Connection,
}

impl Storage {
    pub(crate) fn new(db_path: PathBuf) -> Self {
        let mut connection = Connection::open(db_path).unwrap();
        Storage::run_migrations(&mut connection).unwrap();
        Self { connection }
    }

    pub(crate) fn run_migrations(connection: &mut Connection) -> anyhow::Result<()> {
        log::info!("Running database migrations");
        match migrations::migrations::runner().run(connection) {
            Ok(ok) => {
                log::info!("Database migrations completed:{:?} ", ok);
                Ok(())
            }
            Err(err) => {
                log::error!("Database migrations failed: {}", err);
                Err(anyhow::anyhow!(err))
            }
        }
    }

    pub(crate) fn submit_mixnode_statuses(
        &mut self,
        timestamp: i64,
        mixnode_results: Vec<MixnodeResult>,
    ) -> anyhow::Result<()> {
        let tx = self.connection.transaction()?;
        let sql =
            "INSERT INTO mixnode_status (mix_id, reliability, timestamp) VALUES (?1, ?2, ?3);";

        for res in mixnode_results {
            tx.execute(sql, params![&res.mix_id, &res.reliability, &timestamp])?;
        }

        tx.commit().map_err(|err| anyhow::anyhow!(err))
    }

    pub(crate) fn get_mixnode_average_reliability_in_interval(
        &self,
        id: usize,
        start: u64,
        end: u64,
    ) -> anyhow::Result<Option<f32>> {
        log::trace!(
            "Getting mixnode average reliability for mixnode {} in interval {} - {}",
            id,
            start,
            end
        );
        let mut statement = self.connection.prepare_cached("SELECT AVG(reliability) FROM mixnode_status WHERE mix_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3")?;

        let avg = statement.query_row(params![&id, &start, &end], |row| row.get(0))?;

        Ok(avg)
    }

    pub(crate) fn contract_submit_mixnode_reward(
        &mut self,
        epoch: i64,
        timestamp: i64,
        res: MixnodeToReward,
    ) -> anyhow::Result<usize> {
        let sql = "INSERT INTO contract_mixnode_reward (mix_id, epoch, reliability, timestamp) VALUES (?1, ?2, ?3, ?4);";
        self.connection
            .execute(
                sql,
                params![&res.mix_id, epoch, &res.performance, &timestamp],
            )
            .map_err(|err| anyhow::anyhow!(err))
    }
}
