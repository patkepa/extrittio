#![cfg(feature = "phase0-turso")]

use std::fs::{self, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokio::sync::Mutex;
use turso::transaction::TransactionBehavior;
use turso::{Builder, Connection, Error, Value, params};

const HELPER_DATABASE_PATH: &str = "EXTRITTIO_PHASE0_DATABASE_PATH";
const HELPER_LOCK_PATH: &str = "EXTRITTIO_PHASE0_LOCK_PATH";
const HELPER_READY_PATH: &str = "EXTRITTIO_PHASE0_READY_PATH";

fn assert_send_sync<T: Send + Sync>() {}

fn database_path(temp_dir: &TempDir) -> PathBuf {
    temp_dir.path().join("phase0.db")
}

fn path_string(path: &Path) -> String {
    path.to_str()
        .expect("phase-0 test paths must be valid UTF-8")
        .to_owned()
}

async fn scalar_i64(connection: &Connection, sql: &str) -> i64 {
    let mut rows = connection.query(sql, ()).await.unwrap();
    let row = rows.next().await.unwrap().expect("expected one row");
    let value = row.get(0).unwrap();
    assert!(rows.next().await.unwrap().is_none());
    value
}

async fn scalar_text(connection: &Connection, sql: &str) -> String {
    let mut rows = connection.query(sql, ()).await.unwrap();
    let row = rows.next().await.unwrap().expect("expected one row");
    let value = row.get(0).unwrap();
    assert!(rows.next().await.unwrap().is_none());
    value
}

async fn consume_pragma(connection: &Connection, sql: &str) -> Vec<Value> {
    let mut rows = connection.query(sql, ()).await.unwrap();
    let mut values = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        values.push(row.get_value(0).unwrap());
    }
    values
}

fn spawn_helper(test_name: &str, environment: &[(&str, &Path)]) -> Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--ignored", "--exact", test_name, "--nocapture"]);
    for (name, value) in environment {
        command.env(name, value);
    }
    command.spawn().unwrap()
}

fn wait_for_ready_file(child: &mut Child, ready_path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if ready_path.exists() {
            return;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("phase-0 helper exited before becoming ready: {status}");
        }
        assert!(
            Instant::now() < deadline,
            "phase-0 helper did not become ready within 10 seconds"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[tokio::test]
async fn local_engine_supports_required_sql_and_durability_subset() {
    assert_send_sync::<turso::Database>();
    assert_send_sync::<Connection>();

    let temp_dir = tempfile::tempdir().unwrap();
    let path = database_path(&temp_dir);
    let database = Builder::new_local(&path_string(&path))
        .build()
        .await
        .unwrap();
    let mut connection = database.connect().unwrap();
    connection.busy_timeout(Duration::from_secs(5)).unwrap();

    consume_pragma(&connection, "PRAGMA foreign_keys = ON").await;
    consume_pragma(&connection, "PRAGMA synchronous = FULL").await;

    assert_eq!(scalar_text(&connection, "PRAGMA journal_mode").await, "wal");
    assert_eq!(scalar_i64(&connection, "PRAGMA foreign_keys").await, 1);
    assert_eq!(scalar_i64(&connection, "PRAGMA synchronous").await, 2);

    connection
        .execute_batch(
            "CREATE TABLE parents (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );
            CREATE TABLE samples (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL,
                correlation_id TEXT NOT NULL UNIQUE,
                reading REAL NOT NULL CHECK (reading >= 0),
                recorded_at_us INTEGER NOT NULL,
                payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
                payload_blob BLOB NOT NULL,
                FOREIGN KEY (parent_id) REFERENCES parents(id)
            );",
        )
        .await
        .unwrap();

    connection
        .execute(
            "INSERT INTO parents (id, name) VALUES (?1, ?2)",
            params![1_i64, "parent"],
        )
        .await
        .unwrap();

    let timestamp_us = 253_402_300_799_999_999_i64;
    let blob = vec![0_u8, 1, 2, 127, 128, 255];
    let json = r#"{"nested":{"enabled":true},"value":1.25}"#;
    let mut returned = connection
        .query(
            "INSERT INTO samples (
                id, parent_id, correlation_id, reading, recorded_at_us,
                payload_json, payload_blob
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(correlation_id) DO UPDATE SET reading = excluded.reading
             RETURNING id, recorded_at_us, payload_blob",
            params![
                1_i64,
                1_i64,
                "correlation-1",
                12.5_f64,
                timestamp_us,
                json,
                blob.clone()
            ],
        )
        .await
        .unwrap();
    let returned_row = returned.next().await.unwrap().unwrap();
    assert_eq!(returned_row.get::<i64>(0).unwrap(), 1);
    assert_eq!(returned_row.get::<i64>(1).unwrap(), timestamp_us);
    assert_eq!(returned_row.get::<Vec<u8>>(2).unwrap(), blob);
    assert!(returned.next().await.unwrap().is_none());

    let mut json_rows = connection
        .query(
            "SELECT json_extract(payload_json, '$.nested.enabled'),
                    row_number() OVER (ORDER BY recorded_at_us DESC)
             FROM samples",
            (),
        )
        .await
        .unwrap();
    let json_row = json_rows.next().await.unwrap().unwrap();
    assert_eq!(json_row.get::<i64>(0).unwrap(), 1);
    assert_eq!(json_row.get::<i64>(1).unwrap(), 1);
    assert!(json_rows.next().await.unwrap().is_none());

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await
        .unwrap();
    transaction
        .execute(
            "INSERT INTO parents (id, name) VALUES (2, 'rolled-back')",
            (),
        )
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(
        scalar_i64(&connection, "SELECT count(*) FROM parents WHERE id = 2").await,
        0
    );

    let unique_error = connection
        .execute("INSERT INTO parents (id, name) VALUES (2, 'parent')", ())
        .await
        .unwrap_err();
    assert!(matches!(unique_error, Error::Constraint(_)));

    let foreign_key_error = connection
        .execute(
            "INSERT INTO samples (
                id, parent_id, correlation_id, reading, recorded_at_us,
                payload_json, payload_blob
             ) VALUES (2, 999, 'foreign-key', 1, 0, '{}', x'00')",
            (),
        )
        .await
        .unwrap_err();
    assert!(matches!(foreign_key_error, Error::Constraint(_)));

    let check_error = connection
        .execute(
            "INSERT INTO samples (
                id, parent_id, correlation_id, reading, recorded_at_us,
                payload_json, payload_blob
             ) VALUES (3, 1, 'check', -1, 0, '{}', x'00')",
            (),
        )
        .await
        .unwrap_err();
    assert!(matches!(check_error, Error::Constraint(_)));

    let checkpoint = consume_pragma(&connection, "PRAGMA wal_checkpoint(TRUNCATE)").await;
    assert!(!checkpoint.is_empty());
    assert_eq!(
        scalar_text(&connection, "PRAGMA integrity_check").await,
        "ok"
    );

    drop(connection);
    drop(database);

    let reopened = Builder::new_local(&path_string(&path))
        .build()
        .await
        .unwrap();
    let reopened_connection = reopened.connect().unwrap();
    assert_eq!(
        scalar_i64(&reopened_connection, "SELECT count(*) FROM samples").await,
        1
    );
    assert_eq!(
        scalar_text(&reopened_connection, "PRAGMA integrity_check").await,
        "ok"
    );
}

#[test]
fn process_lock_rejects_a_second_process() {
    let temp_dir = tempfile::tempdir().unwrap();
    let lock_path = temp_dir.path().join("extrittio.lock");
    let ready_path = temp_dir.path().join("lock-ready");
    let mut child = spawn_helper(
        "phase0_process_lock_holder",
        &[
            (HELPER_LOCK_PATH, &lock_path),
            (HELPER_READY_PATH, &ready_path),
        ],
    );
    wait_for_ready_file(&mut child, &ready_path);

    let contender = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();
    assert!(matches!(
        contender.try_lock(),
        Err(TryLockError::WouldBlock)
    ));

    child.kill().unwrap();
    child.wait().unwrap();
    contender
        .try_lock()
        .expect("the OS must release the lock when the owner process exits");
}

#[test]
#[ignore = "phase-0 subprocess helper"]
fn phase0_process_lock_holder() {
    let lock_path = PathBuf::from(std::env::var_os(HELPER_LOCK_PATH).unwrap());
    let ready_path = PathBuf::from(std::env::var_os(HELPER_READY_PATH).unwrap());
    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .unwrap();
    lock_file.try_lock().unwrap();
    fs::write(ready_path, b"ready").unwrap();
    std::thread::sleep(Duration::from_secs(30));
}

#[tokio::test]
async fn forced_process_termination_reopens_with_integrity() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = database_path(&temp_dir);
    let ready_path = temp_dir.path().join("writer-ready");
    let mut child = spawn_helper(
        "phase0_crash_writer_helper",
        &[
            (HELPER_DATABASE_PATH, &path),
            (HELPER_READY_PATH, &ready_path),
        ],
    );
    wait_for_ready_file(&mut child, &ready_path);
    std::thread::sleep(Duration::from_millis(100));
    child.kill().unwrap();
    child.wait().unwrap();

    let reopened = Builder::new_local(&path_string(&path))
        .build()
        .await
        .unwrap();
    let connection = reopened.connect().unwrap();
    assert_eq!(
        scalar_text(&connection, "PRAGMA integrity_check").await,
        "ok"
    );
    assert!(scalar_i64(&connection, "SELECT count(*) FROM crash_rows").await >= 1);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "phase-0 subprocess helper"]
async fn phase0_crash_writer_helper() {
    let path = PathBuf::from(std::env::var_os(HELPER_DATABASE_PATH).unwrap());
    let ready_path = PathBuf::from(std::env::var_os(HELPER_READY_PATH).unwrap());
    let database = Builder::new_local(&path_string(&path))
        .build()
        .await
        .unwrap();
    let mut connection = database.connect().unwrap();
    consume_pragma(&connection, "PRAGMA synchronous = FULL").await;
    connection
        .execute(
            "CREATE TABLE crash_rows (id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            (),
        )
        .await
        .unwrap();
    connection
        .execute(
            "INSERT INTO crash_rows (id, value) VALUES (1, 'durable')",
            (),
        )
        .await
        .unwrap();
    fs::write(ready_path, b"ready").unwrap();

    let mut id = 2_i64;
    loop {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await
            .unwrap();
        for _ in 0..32 {
            transaction
                .execute(
                    "INSERT INTO crash_rows (id, value) VALUES (?1, ?2)",
                    params![id, format!("row-{id}")],
                )
                .await
                .unwrap();
            id += 1;
        }
        transaction.commit().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "manual phase-0 throughput probe; run with --release --ignored --exact phase0_load_probe"]
async fn phase0_load_probe() {
    const BATCHES: i64 = 50;
    const BATCH_SIZE: i64 = 100;

    let temp_dir = tempfile::tempdir().unwrap();
    let path = database_path(&temp_dir);
    let database = Builder::new_local(&path_string(&path))
        .build()
        .await
        .unwrap();
    let writer = database.connect().unwrap();
    writer.busy_timeout(Duration::from_secs(5)).unwrap();
    consume_pragma(&writer, "PRAGMA synchronous = FULL").await;
    writer
        .execute_batch(
            "CREATE TABLE telemetry (
                id INTEGER PRIMARY KEY,
                device_id TEXT NOT NULL,
                received_at_us INTEGER NOT NULL,
                temperature REAL NOT NULL
            );
            CREATE INDEX telemetry_device_time
                ON telemetry(device_id, received_at_us DESC);
            CREATE TABLE device_latest_state (
                device_id TEXT PRIMARY KEY,
                telemetry_id INTEGER NOT NULL,
                temperature REAL NOT NULL
            );
            CREATE TABLE commands (
                id INTEGER PRIMARY KEY,
                payload TEXT NOT NULL
            );",
        )
        .await
        .unwrap();

    let reader = database.connect().unwrap();
    reader.busy_timeout(Duration::from_secs(5)).unwrap();
    let writer = Arc::new(Mutex::new(writer));
    let done = Arc::new(AtomicBool::new(false));
    let started = Instant::now();

    let telemetry_writer = {
        let writer = Arc::clone(&writer);
        let done = Arc::clone(&done);
        tokio::spawn(async move {
            for batch in 0..BATCHES {
                let mut connection = writer.lock().await;
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .await
                    .unwrap();
                for offset in 0..BATCH_SIZE {
                    let id = batch * BATCH_SIZE + offset + 1;
                    let temperature = 20.0 + (id % 100) as f64 / 10.0;
                    transaction
                        .execute(
                            "INSERT INTO telemetry (
                                id, device_id, received_at_us, temperature
                             ) VALUES (?1, 'device-1', ?2, ?3)",
                            params![id, 1_800_000_000_000_000_i64 + id, temperature],
                        )
                        .await
                        .unwrap();
                    transaction
                        .execute(
                            "INSERT INTO device_latest_state (
                                device_id, telemetry_id, temperature
                             ) VALUES ('device-1', ?1, ?2)
                             ON CONFLICT(device_id) DO UPDATE SET
                                telemetry_id = excluded.telemetry_id,
                                temperature = excluded.temperature",
                            params![id, temperature],
                        )
                        .await
                        .unwrap();
                }
                transaction.commit().await.unwrap();
                drop(connection);
                tokio::task::yield_now().await;
            }
            done.store(true, Ordering::Release);
        })
    };

    let dashboard_reader = {
        let done = Arc::clone(&done);
        tokio::spawn(async move {
            let mut reads = 0_u64;
            while !done.load(Ordering::Acquire) {
                let mut rows = reader
                    .query(
                        "SELECT count(*), avg(temperature),
                                received_at_us / 3600000000
                         FROM telemetry
                         GROUP BY received_at_us / 3600000000",
                        (),
                    )
                    .await
                    .unwrap();
                while rows.next().await.unwrap().is_some() {}
                reads += 1;
                tokio::task::yield_now().await;
            }
            reads
        })
    };

    let priority_command = {
        let writer = Arc::clone(&writer);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let command_started = Instant::now();
            let connection = writer.lock().await;
            connection
                .execute(
                    "INSERT INTO commands (id, payload) VALUES (1, 'priority')",
                    (),
                )
                .await
                .unwrap();
            command_started.elapsed()
        })
    };

    telemetry_writer.await.unwrap();
    let dashboard_reads = dashboard_reader.await.unwrap();
    let command_latency = priority_command.await.unwrap();
    let elapsed = started.elapsed();

    let verification = writer.lock().await;
    assert_eq!(
        scalar_i64(&verification, "SELECT count(*) FROM telemetry").await,
        BATCHES * BATCH_SIZE
    );
    assert_eq!(
        scalar_i64(
            &verification,
            "SELECT telemetry_id FROM device_latest_state WHERE device_id = 'device-1'",
        )
        .await,
        BATCHES * BATCH_SIZE
    );
    assert_eq!(
        scalar_i64(&verification, "SELECT count(*) FROM commands").await,
        1
    );

    let rows_per_second = (BATCHES * BATCH_SIZE) as f64 / elapsed.as_secs_f64();
    println!(
        "phase0-load rows={} elapsed_ms={} rows_per_second={:.1} dashboard_reads={} command_latency_ms={:.3}",
        BATCHES * BATCH_SIZE,
        elapsed.as_millis(),
        rows_per_second,
        dashboard_reads,
        command_latency.as_secs_f64() * 1000.0
    );
}
