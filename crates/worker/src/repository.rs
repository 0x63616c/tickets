//! Shared parameterized queries. Only the Store adapter knows about D1.
use serde::de::DeserializeOwned;
use serde_json::Value;
use tickets_core::{dependency::DependencyGraph, domain::*};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Storage(String),
    Validation(String),
    Dependency(tickets_core::dependency::DependencyError),
    NotFound,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// Queries may return rows (including INSERT/UPDATE RETURNING) or an empty list.
/// No Send bound: the Worker runtime is single threaded.
#[allow(async_fn_in_trait)]
pub trait Store {
    async fn query<T: DeserializeOwned>(&self, sql: &str, params: Vec<Value>) -> Result<Vec<T>>;
}
pub struct Repository<S> {
    store: S,
}
impl<S: Store> Repository<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
    async fn one<T: DeserializeOwned>(&self, sql: &str, params: Vec<Value>) -> Result<T> {
        self.store
            .query(sql, params)
            .await?
            .into_iter()
            .next()
            .ok_or(Error::NotFound)
    }
    async fn execute(&self, sql: &str, params: Vec<Value>) -> Result<()> {
        self.store.query::<Value>(sql, params).await?;
        Ok(())
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
        validate_name(&p.name)
            .and_then(|()| validate_prefix(&p.key_prefix))
            .and_then(|()| validate_color(&p.color))
            .map_err(Error::Validation)?;
        self.one(
            "UPDATE projects SET name=?,key_prefix=?,color=?,archived=?,github_owner=?,
             github_project_number=?,github_repo=? WHERE id=? RETURNING *",
            vec![
                p.name.clone().into(),
                p.key_prefix.clone().into(),
                p.color.clone().into(),
                i64::from(p.archived).into(),
                p.github_owner.clone().into(),
                p.github_project_number.into(),
                p.github_repo.clone().into(),
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
        self.one(
            "INSERT INTO tickets(project_id,number,title,body)
             SELECT id,next_ticket_number,?,? FROM projects WHERE id=? RETURNING *",
            vec![title.into(), body.into(), project.into()],
        )
        .await
    }
    /// Project/number and creation timestamp are immutable; moving projects needs explicit renumbering.
    pub async fn save_ticket(&self, t: &Ticket) -> Result<Ticket> {
        validate_name(&t.title).map_err(Error::Validation)?;
        if !t.sort_order.is_finite() {
            return Err(Error::Validation("sort order must be finite".into()));
        }
        self.one(
            "UPDATE tickets SET title=?,body=?,status=?,priority=?,assignee=?,sort_order=?,
             external_url=?,external_id=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
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
                t.id.into(),
            ],
        )
        .await
    }
    pub async fn delete_ticket(&self, id: i64) -> Result<()> {
        self.execute("DELETE FROM tickets WHERE id=?", vec![id.into()])
            .await
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
        self.execute(
            "INSERT INTO ticket_labels(ticket_id,label_id) VALUES(?,?)",
            vec![ticket.into(), label.into()],
        )
        .await
    }
    pub async fn remove_label(&self, ticket: i64, label: i64) -> Result<()> {
        self.execute(
            "DELETE FROM ticket_labels WHERE ticket_id=? AND label_id=?",
            vec![ticket.into(), label.into()],
        )
        .await
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
        self.execute(
            "INSERT INTO ticket_dependencies(blocker_id,blocked_id) VALUES(?,?)",
            vec![edge.blocker_id.into(), edge.blocked_id.into()],
        )
        .await
    }
    pub async fn remove_dependency(&self, edge: Dependency) -> Result<()> {
        self.execute(
            "DELETE FROM ticket_dependencies WHERE blocker_id=? AND blocked_id=?",
            vec![edge.blocker_id.into(), edge.blocked_id.into()],
        )
        .await
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
        self.one(
            "INSERT INTO comments(ticket_id,author,body) VALUES(?,?,?) RETURNING *",
            vec![ticket.into(), author.into(), body.into()],
        )
        .await
    }
}

#[cfg(target_arch = "wasm32")]
impl Store for worker::D1Database {
    async fn query<T: DeserializeOwned>(&self, sql: &str, params: Vec<Value>) -> Result<Vec<T>> {
        use worker::wasm_bindgen::JsValue;
        let params: Vec<JsValue> = params
            .into_iter()
            .map(|p| match p {
                Value::Null => JsValue::NULL,
                Value::String(s) => JsValue::from_str(&s),
                Value::Number(n) => JsValue::from_f64(n.as_f64().unwrap()),
                Value::Bool(b) => JsValue::from_f64(if b { 1.0 } else { 0.0 }),
                _ => unreachable!("only scalar SQL arguments"),
            })
            .collect();
        let result = self
            .prepare(sql)
            .bind(&params)
            .map_err(|e| Error::Storage(e.to_string()))?
            .all()
            .await
            .map_err(|e| Error::Storage(e.to_string()))?;
        if !result.success() {
            return Err(Error::Storage(
                result.error().unwrap_or_else(|| "D1 query failed".into()),
            ));
        }
        result.results().map_err(|e| Error::Storage(e.to_string()))
    }
}
