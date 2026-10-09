//! Database schema migrations using refinery.

use crate::error::{DbError, Result};
use rusqlite::Connection;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

/// Executes all pending schema migrations against the provided database connection.
pub fn run_migrations(conn: &mut Connection) -> Result<refinery::Report> {
    embedded::migrations::runner()
        .run(conn)
        .map_err(|e| DbError::Migration(e.to_string()))
}

/// Returns the list of migrations defined in the binary.
pub fn get_migrations() -> Vec<refinery::Migration> {
    embedded::migrations::runner().get_migrations().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_embedded_and_run() {
        let mut conn = Connection::open_in_memory().expect("open in memory");
        let report = run_migrations(&mut conn).expect("run migrations");
        assert_eq!(report.applied_migrations().len(), 6);

        // Second run should apply 0 migrations (idempotent)
        let second_report = run_migrations(&mut conn).expect("second run");
        assert_eq!(second_report.applied_migrations().len(), 0);
    }
}
