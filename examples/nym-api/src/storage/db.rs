use std::path::PathBuf;

use chrono::Local;
use refinery::Runner;
use rusqlite::{params, Connection, OptionalExtension};

use crate::contract::MixnodeToReward;
use crate::epoch::EpochInfo;
use crate::metrics::types::MixnodeResult;

#[derive(Clone)]
pub struct MetricsStorageType;

#[derive(Clone)]
pub struct ContractStorageType;

pub enum StorageType {
    Metrics,
    Contract,
}

pub struct Storage<T> {
    connection: Connection,
    phantom: std::marker::PhantomData<T>,
}

impl<T> Storage<T> {
    pub fn init(db_path: String, migrations: Runner) -> Self {
        let db_file = PathBuf::from(db_path.clone());
        if db_file.exists() {
            log::info!("Removing previous database file: {}", db_file.display());
            std::fs::remove_file(&db_file).unwrap();
        }
        log::info!("Using database file: {}", db_file.display());

        let mut connection = Connection::open(db_path).unwrap();

        Storage::<T>::run_migrations(&mut connection, migrations).unwrap();

        Self {
            connection,
            phantom: Default::default(),
        }
    }

    pub fn run_migrations(connection: &mut Connection, migrations: Runner) -> anyhow::Result<()> {
        log::info!("Running database migrations");
        match migrations.run(connection) {
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
}

impl Storage<MetricsStorageType> {
    pub fn submit_mixnode_statuses(
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

    pub fn get_mixnode_average_reliability(
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

    pub fn save_rewarding_results(&mut self, epoch: u64, mixnodes: usize) -> anyhow::Result<()> {
        let timestamp = Local::now().timestamp();

        let tx = self.connection.transaction()?;
        let sql = "INSERT INTO rewarding_report (epoch_id, eligible_mixnodes, timestamp) VALUES (?1, ?2, ?3);";

        tx.execute(sql, params![&epoch, &mixnodes, timestamp])?;

        tx.commit().map_err(|err| anyhow::anyhow!(err))
    }
}

impl Storage<ContractStorageType> {
    pub fn contract_submit_mixnode_rewards(
        &mut self,
        epoch: u64,
        timestamp: i64,
        nym_api_id: &str,
        res: Vec<MixnodeToReward>,
    ) -> anyhow::Result<()> {
        let sql = "INSERT INTO contract_mixnode_reward (mix_id, epoch, nym_api_id, reliability, timestamp) VALUES (?1, ?2, ?3, ?4, ?5);";

        let tx = self.connection.transaction()?;

        for mix in res {
            tx.execute(
                sql,
                params![&mix.mix_id, epoch, nym_api_id, &mix.performance, &timestamp],
            )?;
        }

        tx.commit().map_err(|err| anyhow::anyhow!(err))
    }

    pub(crate) fn save_epoch(&mut self, info: &EpochInfo) -> anyhow::Result<()> {
        let sql = "INSERT INTO epoch_info (epoch_id, start_time, duration) VALUES (?1, ?2, ?3);";
        self.connection.execute(
            sql,
            params![&info.epoch_id, &info.start_time, &info.duration],
        )?;
        Ok(())
    }

    pub(crate) fn update_epoch(&mut self, info: &EpochInfo) -> anyhow::Result<()> {
        let sql = "UPDATE epoch_info SET epoch_id = ?1, start_time = ?2, duration = ?3;";
        self.connection.execute(
            sql,
            params![&info.epoch_id, &info.start_time, &info.duration],
        )?;
        Ok(())
    }

    pub(crate) fn get_epoch(&self) -> anyhow::Result<Option<EpochInfo>> {
        let sql = "SELECT epoch_id, start_time, duration FROM epoch_info";

        let mut statement = self.connection.prepare_cached(sql)?;

        if let Some(info) = statement
            .query_row(params![], |row| {
                Ok(EpochInfo::new(row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()?
        {
            return Ok(Some(info));
        }
        Ok(None)
    }
}
