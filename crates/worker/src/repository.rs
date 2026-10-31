//! Shared parameterized queries. Only the Store adapter knows about D1.
use crate::mutation::{self, Scope};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tickets_core::{dependency::DependencyGraph, domain::*, history::Actor};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Storage(String),
    Unavailable,
    Validation(String),
    Dependency(tickets_core::dependency::DependencyError),
    NotFound,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if matches!(self, Self::Unavailable) {
            return f.write_str(crate::d1_errors::MARKER);
        }
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// One parameterized statement within an atomic mutation batch.
pub struct Statement {
    pub sql: String,
    pub params: Vec<Value>,
}
impl Statement {
    pub fn new(sql: impl Into<String>, params: Vec<Value>) -> Self {
        Self {
            sql: sql.into(),
            params,
        }
    }
}

/// Queries may return rows (including INSERT/UPDATE RETURNING) or an empty list.
/// No Send bound: the Worker runtime is single threaded.
#[allow(async_fn_in_trait)]
pub trait Store {
    async fn query<T: DeserializeOwned>(&self, sql: &str, params: Vec<Value>) -> Result<Vec<T>>;
    /// Must be atomic. Never implement by issuing independent queries.
    async fn batch(&self, _statements: Vec<Statement>) -> Result<Vec<Vec<Value>>> {
        Err(Error::Storage("Atomic batch is not supported".into()))
    }
}
pub struct Repository<S> {
    store: S,
    actor: Actor,
}
impl<S: Store> Repository<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            actor: Actor::system(),
        }
    }
    pub fn with_actor(store: S, actor: Actor) -> Self {
        Self { store, actor }
    }
    async fn mutate<T: DeserializeOwned>(
        &self,
        scope: Scope,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Vec<T>> {
        mutation::execute(&self.store, &self.actor, scope, sql, params).await
    }
    async fn mutate_one<T: DeserializeOwned>(
        &self,
        scope: Scope,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<T> {
        self.mutate(scope, sql, params)
            .await?
            .into_iter()
            .next()
            .ok_or(Error::NotFound)
    }
    async fn one<T: DeserializeOwned>(&self, sql: &str, params: Vec<Value>) -> Result<T> {
        self.store
            .query(sql, params)
            .await?
            .into_iter()
            .next()
            .ok_or(Error::NotFound)
    }
    pub async fn projects(&self) -> Result<Vec<Project>> {
        self.store
            .query("SELECT * FROM projects ORDER BY id", vec![])
            .await
    }
    pub async fn project(&self, id: i64) -> Result<Project> {
        self.one("SELECT * FROM projects WHERE id=?", vec![id.into()])
            .await
    }
    pub async fn create_project(&self, name: &str, prefix: &str, color: &str) -> Result<Project> {
        validate_name(name)
            .and_then(|()| validate_prefix(prefix))
            .and_then(|()| validate_color(color))
            .map_err(Error::Validation)?;
        self.one(
            "INSERT INTO projects(name,key_prefix,color) VALUES(?,?,?) RETURNING *",
            vec![name.into(), prefix.into(), color.into()],
        )
        .await
    }
    pub async fn save_project(&self, p: &Project) -> Result<Project> {
        if let Some(icon) = &p.icon {
            validate_project_icon(icon).map_err(Error::Validation)?;
        }
        validate_name(&p.name)
            .and_then(|()| validate_prefix(&p.key_prefix))
            .and_then(|()| validate_color(&p.color))
            .map_err(Error::Validation)?;
        self.one(
            "UPDATE projects SET name=?,key_prefix=?,color=?,archived=?,github_owner=?,
             github_project_number=?,github_repo=?,github_project_kind=?,icon=? WHERE id=? RETURNING *",
            vec![
                p.name.clone().into(),
                p.key_prefix.clone().into(),
                p.color.clone().into(),
                i64::from(p.archived).into(),
                p.github_owner.clone().into(),
                p.github_project_number.into(),
                p.github_repo.clone().into(),
                p.github_project_kind.clone().into(),
                p.icon.clone().into(),
                p.id.into(),
            ],
        )
        .await
    }
    pub async fn tickets(&self, project: i64) -> Result<Vec<Ticket>> {
        self.store
            .query(
                "SELECT * FROM tickets WHERE project_id=? ORDER BY sort_order,id",
                vec![project.into()],
            )
            .await
    }
    pub async fn ticket(&self, id: i64) -> Result<Ticket> {
        self.one("SELECT * FROM tickets WHERE id=?", vec![id.into()])
            .await
    }
    pub async fn create_ticket(&self, project: i64, title: &str, body: &str) -> Result<Ticket> {
        validate_name(title).map_err(Error::Validation)?;
        self.mutate_one(
            Scope::create(),
            "INSERT INTO tickets(project_id,number,title,body)
             SELECT id,next_ticket_number,?,? FROM projects WHERE id=? RETURNING *",
            vec![title.into(), body.into(), project.into()],
        )
        .await
    }
    /// Project/number and creation timestamp are immutable; moving projects needs explicit renumbering.
    pub async fn save_ticket(&self, t: &Ticket) -> Result<Ticket> {
        validate_name(&t.title).map_err(Error::Validation)?;
        if let Some(kind) = &t.r#type {
            tickets_core::ticket_type::validate_type(kind).map_err(Error::Validation)?;
        }
        let scope = t
            .scope
            .as_deref()
            .map(tickets_core::ticket_type::normalize_scope)
            .transpose()
            .map_err(Error::Validation)?
            .flatten();
        if !t.sort_order.is_finite() {
            return Err(Error::Validation("sort order must be finite".into()));
        }
        self.mutate_one(
            Scope::ticket(t.id),
            "UPDATE tickets SET title=?,body=?,status=?,priority=?,assignee=?,sort_order=?,
             external_url=?,external_id=?,type=?,scope=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id=? RETURNING *",
            vec![
                t.title.clone().into(),
                t.body.clone().into(),
                serde_json::to_value(t.status).unwrap(),
                u8::from(t.priority).into(),
                t.assignee.clone().into(),
                t.sort_order.into(),
                t.external_url.clone().into(),
                t.external_id.clone().into(),
                t.r#type.clone().into(),
                scope.into(),
                t.id.into(),
            ],
        )
        .await
    }
    pub async fn delete_ticket(&self, id: i64) -> Result<()> {
        self.mutate::<Value>(
            Scope::ticket(id),
            "DELETE FROM tickets WHERE id=?",
            vec![id.into()],
        )
        .await
        .map(|_| ())
    }
    pub async fn labels(&self) -> Result<Vec<Label>> {
        self.store
            .query("SELECT * FROM labels ORDER BY name", vec![])
            .await
    }
    pub async fn create_label(&self, name: &str, color: &str) -> Result<Label> {
        validate_name(name)
            .and_then(|()| validate_color(color))
            .map_err(Error::Validation)?;
        self.one(
            "INSERT INTO labels(name,color) VALUES(?,?) RETURNING *",
            vec![name.into(), color.into()],
        )
        .await
    }
    pub async fn ticket_labels(&self, ticket: i64) -> Result<Vec<Label>> {
        self.store
            .query(
                "SELECT l.* FROM labels l JOIN ticket_labels tl ON tl.label_id=l.id
             WHERE tl.ticket_id=? ORDER BY l.name",
                vec![ticket.into()],
            )
            .await
    }
    pub async fn add_label(&self, ticket: i64, label: i64) -> Result<()> {
        self.mutate::<Value>(
            Scope::ticket(ticket),
            "INSERT INTO ticket_labels(ticket_id,label_id) VALUES(?,?)",
            vec![ticket.into(), label.into()],
        )
        .await
        .map(|_| ())
    }
    pub async fn remove_label(&self, ticket: i64, label: i64) -> Result<()> {
        self.mutate::<Value>(
            Scope::ticket(ticket),
            "DELETE FROM ticket_labels WHERE ticket_id=? AND label_id=?",
            vec![ticket.into(), label.into()],
        )
        .await
        .map(|_| ())
    }
    pub async fn dependencies(&self) -> Result<Vec<Dependency>> {
        self.store
            .query(
                "SELECT * FROM ticket_dependencies ORDER BY blocker_id,blocked_id",
                vec![],
            )
            .await
    }
    pub async fn add_dependency(&self, edge: Dependency) -> Result<()> {
        DependencyGraph::new(self.dependencies().await?)
            .map_err(Error::Dependency)?
            .check(edge)
            .map_err(Error::Dependency)?;
        self.mutate::<Value>(
            Scope::relation(edge.blocker_id, edge.blocked_id),
            "INSERT INTO ticket_dependencies(blocker_id,blocked_id) VALUES(?,?)",
            vec![edge.blocker_id.into(), edge.blocked_id.into()],
        )
        .await
        .map(|_| ())
    }
    pub async fn remove_dependency(&self, edge: Dependency) -> Result<()> {
        self.mutate::<Value>(
            Scope::relation(edge.blocker_id, edge.blocked_id),
            "DELETE FROM ticket_dependencies WHERE blocker_id=? AND blocked_id=?",
            vec![edge.blocker_id.into(), edge.blocked_id.into()],
        )
        .await
        .map(|_| ())
    }
    pub async fn comments(&self, ticket: i64) -> Result<Vec<Comment>> {
        self.store
            .query(
                "SELECT * FROM comments WHERE ticket_id=? ORDER BY created_at,id",
                vec![ticket.into()],
            )
            .await
    }
    pub async fn create_comment(&self, ticket: i64, author: &str, body: &str) -> Result<Comment> {
        validate_name(author)
            .and_then(|()| validate_name(body))
            .map_err(Error::Validation)?;
        self.mutate_one(
            Scope::ticket(ticket),
            "INSERT INTO comments(ticket_id,author,body,author_kind) VALUES(?,?,?,CASE WHEN ?='GitHub' THEN 'github' ELSE 'system' END) RETURNING *",
            vec![ticket.into(), author.into(), body.into(), author.into()],
        )
        .await
    }
}

#[cfg(target_arch = "wasm32")]
impl Store for worker::D1Database {
    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<Vec<Value>>> {
        use worker::wasm_bindgen::JsValue;
        let statements = statements
            .into_iter()
            .map(|s| {
                let params: Vec<JsValue> = s
                    .params
                    .into_iter()
                    .map(|v| match v {
                        Value::Null => JsValue::NULL,
                        Value::String(s) => JsValue::from_str(&s),
                        Value::Number(n) => JsValue::from_f64(n.as_f64().unwrap()),
                        Value::Bool(b) => JsValue::from_f64(if b { 1.0 } else { 0.0 }),
                        Value::Array(bytes) => js_sys::Uint8Array::from(
                            bytes
                                .iter()
                                .map(|b| b.as_u64().expect("byte") as u8)
                                .collect::<Vec<_>>()
                                .as_slice(),
                        )
                        .buffer()
                        .into(),
                        _ => unreachable!("only scalar or binary SQL arguments"),
                    })
                    .collect();
                self.prepare(&s.sql)
                    .bind(&params)
                    .map_err(crate::d1_errors::binding)
            })
            .collect::<Result<Vec<_>>>()?;
        worker::D1Database::batch(self, statements)
            .await
            .map_err(crate::d1_errors::binding)?
            .into_iter()
            .map(|r| {
                if !r.success() {
                    return Err(crate::d1_errors::storage(r.error().unwrap_or_default()));
                }
                r.results().map_err(crate::d1_errors::binding)
            })
            .collect()
    }

    async fn query<T: DeserializeOwned>(&self, sql: &str, params: Vec<Value>) -> Result<Vec<T>> {
        use worker::wasm_bindgen::JsValue;
        let params: Vec<JsValue> = params
            .into_iter()
            .map(|p| match p {
                Value::Null => JsValue::NULL,
                Value::String(s) => JsValue::from_str(&s),
                Value::Number(n) => JsValue::from_f64(n.as_f64().unwrap()),
                Value::Bool(b) => JsValue::from_f64(if b { 1.0 } else { 0.0 }),
                Value::Array(bytes) => js_sys::Uint8Array::from(
                    bytes
                        .iter()
                        .map(|b| b.as_u64().expect("byte") as u8)
                        .collect::<Vec<_>>()
                        .as_slice(),
                )
                .buffer()
                .into(),
                _ => unreachable!("only scalar or binary SQL arguments"),
            })
            .collect();
        let result = self
            .prepare(sql)
            .bind(&params)
            .map_err(crate::d1_errors::binding)?
            .all()
            .await
            .map_err(crate::d1_errors::binding)?;
        if !result.success() {
            return Err(crate::d1_errors::storage(
                result.error().unwrap_or_else(|| "D1 query failed".into()),
            ));
        }
        result.results().map_err(crate::d1_errors::binding)
    }
}
impl<S: Store> Store for &S {
    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<Vec<Value>>> {
        (**self).batch(statements).await
    }

    async fn query<T: DeserializeOwned>(&self, sql: &str, params: Vec<Value>) -> Result<Vec<T>> {
        (**self).query(sql, params).await
    }
}
