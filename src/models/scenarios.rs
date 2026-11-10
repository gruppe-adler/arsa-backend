use crate::endpoints::server::Branch;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait,
    entity::prelude::*,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[sea_orm::model]
#[derive(
    Debug, Default, Clone, PartialEq, Eq, DeriveEntityModel, ToSchema, Deserialize, Serialize,
)]
#[sea_orm(table_name = "scenario")]
#[serde(rename_all = "camelCase")]
#[schema(as = Scenario)]
pub struct Model {
    // Vanilla scenario paths are identical on stable and experimental,
    // so a path is only unique per branch.
    #[sea_orm(primary_key, auto_increment = false)]
    pub path: String,

    pub name: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub branch: Branch,
}

impl ActiveModelBehavior for ActiveModel {}

/// Databases created before the composite key have `path` as the only primary key.
/// SQLite can't change a primary key in place, so rebuild the table, keeping its rows.
/// Must run before the schema sync.
pub async fn migrate_composite_primary_key(db: &DatabaseConnection) -> Result<(), DbErr> {
    let primary_key: Vec<String> = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT name FROM pragma_table_info('scenario') WHERE pk > 0",
        ))
        .await?
        .iter()
        .map(|row| row.try_get_by_index::<String>(0))
        .collect::<Result<_, _>>()?;

    // No table yet (fresh database) or already migrated.
    if primary_key != ["path"] {
        return Ok(());
    }

    let txn = db.begin().await?;
    txn.execute_unprepared(r#"ALTER TABLE "scenario" RENAME TO "scenario_old""#)
        .await?;
    txn.execute_unprepared(
        r#"CREATE TABLE "scenario" ("path" varchar NOT NULL, "name" varchar NOT NULL, "branch" varchar NOT NULL, PRIMARY KEY ("path", "branch"))"#,
    )
    .await?;
    txn.execute_unprepared(
        r#"INSERT INTO "scenario" ("path", "name", "branch") SELECT "path", "name", "branch" FROM "scenario_old""#,
    )
    .await?;
    txn.execute_unprepared(r#"DROP TABLE "scenario_old""#).await?;
    txn.commit().await
}
