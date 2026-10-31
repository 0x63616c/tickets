use rusqlite::{Connection, params_from_iter, types::Value as SqlValue};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tickets_worker::repository::{Error, Result, Store};

pub struct Sqlite(pub Connection);
impl Sqlite {
    pub fn new() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_COMPOUND_SELECT, 5)
            .unwrap();
        conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_FUNCTION_ARG, 32)
            .unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
            .collect();
        files.sort();
        for file in files {
            conn.execute_batch(&std::fs::read_to_string(&file).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        }
        Self(conn)
    }
}
impl Store for Sqlite {
    async fn batch(
        &self,
        statements: Vec<tickets_worker::repository::Statement>,
    ) -> Result<Vec<Vec<Value>>> {
        let tx = self
            .0
            .unchecked_transaction()
            .map_err(|e| Error::Storage(e.to_string()))?;
        let mut results = vec![];
        for statement in statements {
            results.push(self.query(&statement.sql, statement.params).await?);
        }
        tx.commit().map_err(|e| Error::Storage(e.to_string()))?;
        Ok(results)
    }

    async fn query<T: DeserializeOwned>(&self, sql: &str, params: Vec<Value>) -> Result<Vec<T>> {
        let run = || -> std::result::Result<Vec<T>, Box<dyn std::error::Error>> {
            let params: Vec<SqlValue> = params
                .into_iter()
                .map(|p| match p {
                    Value::Null => SqlValue::Null,
                    Value::Bool(b) => SqlValue::Integer(i64::from(b)),
                    Value::String(s) => SqlValue::Text(s),
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            SqlValue::Integer(i)
                        } else {
                            SqlValue::Real(n.as_f64().unwrap())
                        }
                    }
                    Value::Array(bytes) => {
                        SqlValue::Blob(bytes.iter().map(|b| b.as_u64().unwrap() as u8).collect())
                    }
                    _ => panic!("unsupported parameter"),
                })
                .collect();
            let mut stmt = self.0.prepare(sql)?;
            let names: Vec<String> = stmt.column_names().map(|s| s.to_string()).collect();
            let mut rows = stmt.query(params_from_iter(params))?;
            let mut output = vec![];
            while let Some(row) = rows.next()? {
                let mut object = serde_json::Map::new();
                for (i, name) in names.iter().enumerate() {
                    let v: SqlValue = row.get(i)?;
                    object.insert(
                        name.clone(),
                        match v {
                            SqlValue::Null => Value::Null,
                            SqlValue::Integer(i) => i.into(),
                            SqlValue::Real(r) => r.into(),
                            SqlValue::Text(s) => s.into(),
                            SqlValue::Blob(bytes) => serde_json::json!(bytes),
                        },
                    );
                }
                output.push(serde_json::from_value(Value::Object(object))?);
            }
            Ok(output)
        };
        run().map_err(|e| Error::Storage(e.to_string()))
    }
}
