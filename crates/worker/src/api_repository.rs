//! Domain API persistence. Identifiers come only from validated route/field allowlists;
//! every caller value is bound as a parameter. D1 transport remains in Store.
use crate::mutation::{self, Scope};
use crate::repository::{Error, Repository, Result, Store};
use serde_json::{Value, json};
use tickets_core::{domain::Dependency, history::Actor};

// Correlated subqueries are part of one D1 request; ordered inner SELECTs keep
// arrays deterministic without multiplying ticket rows through joins.
pub(crate) const TICKET_SELECT: &str = "tickets.*,
 (SELECT key_prefix FROM projects WHERE id=tickets.project_id) || '-' || number AS key,
 (SELECT json_group_array(json(label)) FROM
   (SELECT json_object('id',l.id,'name',l.name,'color',l.color) AS label
    FROM labels l JOIN ticket_labels tl ON l.id=tl.label_id
    WHERE tl.ticket_id=tickets.id ORDER BY l.id)) AS labels,
 (SELECT json_group_array(blocked_id) FROM
   (SELECT blocked_id FROM ticket_dependencies WHERE blocker_id=tickets.id ORDER BY blocked_id)) AS blocks,
 (SELECT json_group_array(blocker_id) FROM
   (SELECT blocker_id FROM ticket_dependencies WHERE blocked_id=tickets.id ORDER BY blocker_id)) AS blocked_by,
 (SELECT json_group_array(json(pr)) FROM
   (SELECT json_object('repo',pr.repo,'number',pr.number,'url',pr.url,'title',pr.title,'state',pr.state,'closes',json(CASE pr.closes WHEN 1 THEN 'true' ELSE 'false' END),'head_sha',pr.head_sha,'updated_at',pr.updated_at,'tickets_check',checks.tickets_check,'ci_status',checks.ci_status,'tickets_check_run_id',checks.check_run_id,'checks_updated_at',checks.updated_at) AS pr
    FROM ticket_pull_requests pr LEFT JOIN github_pull_request_checks checks
      ON checks.repo=pr.repo AND checks.number=pr.number AND checks.head_sha=pr.head_sha
    WHERE pr.ticket_id=tickets.id ORDER BY pr.repo,pr.number)) AS pull_requests";

fn selection(entity: &str) -> &str {
    if entity == "tickets" {
        TICKET_SELECT
    } else if entity == "projects" {
        "projects.*, (SELECT count(*) FROM tickets WHERE project_id=projects.id AND status NOT IN ('done','canceled')) AS open_ticket_count, (SELECT count(*) FROM tickets WHERE project_id=projects.id AND status='done') AS done_ticket_count, (SELECT max(updated_at) FROM tickets WHERE project_id=projects.id) AS last_activity_at"
    } else {
        "*"
    }
}

pub(crate) struct ApiRepository<'a, S>(pub &'a S, pub &'a Actor);
impl<S: Store> ApiRepository<'_, S> {
    pub async fn ticket_projects(&self, id: i64) -> Result<Vec<i64>> {
        let related: Vec<Value> = self.0.query("SELECT DISTINCT project_id FROM tickets WHERE id IN (SELECT blocked_id FROM ticket_dependencies WHERE blocker_id=? UNION SELECT blocker_id FROM ticket_dependencies WHERE blocked_id=?)", vec![id.into(), id.into()]).await?;
        Ok(related
            .iter()
            .filter_map(|v| v["project_id"].as_i64())
            .collect())
    }
    pub async fn external_projects(&self, external: &str) -> Result<Vec<i64>> {
        let related: Vec<Value> = self.0.query("SELECT DISTINCT project_id FROM tickets WHERE external_id=? OR id IN (SELECT blocked_id FROM ticket_dependencies WHERE blocker_id=(SELECT id FROM tickets WHERE external_id=?) UNION SELECT blocker_id FROM ticket_dependencies WHERE blocked_id=(SELECT id FROM tickets WHERE external_id=?))", vec![external.into(), external.into(), external.into()]).await?;
        Ok(related
            .iter()
            .filter_map(|v| v["project_id"].as_i64())
            .collect())
    }

    async fn mutate_one(&self, scope: Scope, sql: &str, params: Vec<Value>) -> Result<Value> {
        mutation::execute(self.0, self.1, scope, sql, params)
            .await?
            .into_iter()
            .next()
            .ok_or(Error::NotFound)
    }
    pub async fn history(&self, ticket: i64, limit: i64, cursor: i64) -> Result<Value> {
        let mut rows: Vec<Value> = self.0.query("SELECT h.* FROM ticket_history h JOIN ticket_subjects s ON s.id=h.subject_id WHERE s.ticket_id=? AND (?=0 OR h.id<?) ORDER BY h.id DESC LIMIT ?",vec![ticket.into(),cursor.into(),cursor.into(),(limit+1).into()]).await?;
        let more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let next = if more {
            rows.last().map(|v| v["id"].to_string())
        } else {
            None
        };
        for row in &mut rows {
            for field in ["old_value", "new_value"] {
                if let Some(value) = row[field].as_str() {
                    row[field] =
                        serde_json::from_str(value).map_err(|e| Error::Storage(e.to_string()))?;
                }
            }
        }
        Ok(json!({"data":rows,"next_cursor":next}))
    }
    async fn rows(&self, sql: &str, p: Vec<Value>) -> Result<Vec<Value>> {
        self.0.query(sql, p).await
    }
    async fn one(&self, sql: &str, p: Vec<Value>) -> Result<Value> {
        self.rows(sql, p)
            .await?
            .into_iter()
            .next()
            .ok_or(Error::NotFound)
    }
    pub async fn get(&self, entity: &str, id: i64) -> Result<Value> {
        self.one(
            &format!("SELECT {} FROM {entity} WHERE id=?", selection(entity)),
            vec![id.into()],
        )
        .await
    }
    pub async fn ticket_key(&self, prefix: &str, number: i64) -> Result<Value> {
        self.one(&format!("SELECT {TICKET_SELECT} FROM tickets WHERE project_id=(SELECT id FROM projects WHERE key_prefix=?) AND number=?"),vec![prefix.into(),number.into()]).await
    }
    pub async fn enrich(&self, entity: &str, mut v: Value) -> Result<Value> {
        if v.get("id").is_none() {
            return Ok(v);
        }
        if entity == "tickets" {
            if v.get("key").is_none() {
                let id = v["id"]
                    .as_i64()
                    .ok_or_else(|| Error::Storage("Invalid ticket ID".into()))?;
                v = self.get("tickets", id).await?;
            }
            for field in ["labels", "blocks", "blocked_by", "pull_requests"] {
                let text = v[field]
                    .as_str()
                    .ok_or_else(|| Error::Storage(format!("Invalid ticket {field}")))?;
                let array: Value =
                    serde_json::from_str(text).map_err(|e| Error::Storage(e.to_string()))?;
                if !array.is_array() {
                    return Err(Error::Storage(format!("Invalid ticket {field}")));
                }
                v[field] = array;
            }
        }
        if entity == "tickets" || entity == "comments" {
            v["body_html"] =
                tickets_core::markdown::render(v["body"].as_str().unwrap_or("")).into();
        }
        if entity == "projects" {
            v["archived"] = json!(v["archived"] == 1 || v["archived"] == true);
        }
        Ok(v)
    }
    pub async fn create(&self, entity: &str, input: &Value, upsert: bool) -> Result<Value> {
        tickets_core::api::validate_create(entity, input).map_err(Error::Validation)?;
        let obj = input.as_object().unwrap();
        let mut fields: Vec<_> = obj.keys().cloned().collect();
        let mut values: Vec<_> = obj.values().cloned().collect();
        let mut expressions = vec!["?".to_owned(); fields.len()];
        if entity == "tickets" {
            self.get("projects", obj["project_id"].as_i64().unwrap())
                .await?;
            fields.push("number".into());
            expressions.push("(SELECT next_ticket_number FROM projects WHERE id=?)".into());
            values.push(obj["project_id"].clone());
        }
        let conflict = if upsert {
            let set = obj
                .keys()
                .filter(|k| k.as_str() != "external_id" && k.as_str() != "project_id")
                .map(|k| format!("{k}=excluded.{k}"))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                " ON CONFLICT(external_id) DO UPDATE SET {set}, number=CASE WHEN tickets.project_id<>excluded.project_id THEN excluded.number ELSE tickets.number END, project_id=excluded.project_id, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')"
            )
        } else {
            String::new()
        };
        let sql = format!(
            "INSERT INTO {entity}({}) VALUES({}){conflict} RETURNING *",
            fields.join(","),
            expressions.join(",")
        );
        if entity == "tickets" {
            let scope = if upsert {
                Scope::external(input["external_id"].as_str().unwrap())
            } else {
                Scope::create()
            };
            self.mutate_one(scope, &sql, values).await
        } else {
            self.one(&sql, values).await
        }
    }

    pub async fn patch(&self, entity: &str, existing: &Value, input: &Value) -> Result<Value> {
        tickets_core::api::validate(entity, input).map_err(Error::Validation)?;
        let obj = input.as_object().unwrap();
        if let Some(p) = obj.get("project_id") {
            self.get("projects", p.as_i64().unwrap()).await?;
        }
        let mut set = vec![];
        let mut values = vec![];
        for (k, v) in obj {
            set.push(format!("{k}=?"));
            values.push(v.clone());
        }
        if entity == "tickets" {
            // Compare inside the statement, not with a stale read: concurrent moves
            // to the same project must preserve the already allocated number.
            if let Some(project) = obj.get("project_id") {
                set.push("number=CASE WHEN project_id<>? THEN (SELECT next_ticket_number FROM projects WHERE id=?) ELSE number END".into());
                values.extend([project.clone(), project.clone()]);
            }
        }
        if entity == "tickets" || entity == "comments" {
            set.push("updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')".into());
        }
        values.push(existing["id"].clone());
        let sql = format!(
            "UPDATE {entity} SET {} WHERE id=? RETURNING *",
            set.join(",")
        );
        if entity == "tickets" {
            self.mutate_one(
                Scope::ticket(existing["id"].as_i64().unwrap()),
                &sql,
                values,
            )
            .await
        } else {
            self.one(&sql, values).await
        }
    }
    pub async fn delete(&self, entity: &str, id: i64) -> Result<()> {
        let sql = format!("DELETE FROM {entity} WHERE id=? RETURNING id");
        if entity == "tickets" || entity == "labels" {
            let scope = if entity == "tickets" {
                Scope::ticket(id)
            } else {
                Scope::label(id)
            };
            self.mutate_one(scope, &sql, vec![id.into()]).await?;
        } else {
            self.one(&sql, vec![id.into()]).await?;
        }
        Ok(())
    }
    pub async fn create_comment(&self, ticket: i64, input: &Value) -> Result<Value> {
        tickets_core::api::validate_create("comments", input).map_err(Error::Validation)?;
        self.one(
            "INSERT INTO comments(ticket_id,author,body) VALUES(?,'owner',?) RETURNING *",
            vec![ticket.into(), input["body"].clone()],
        )
        .await
    }
    pub async fn link(
        &self,
        kind: &str,
        ticket: i64,
        other: i64,
        add: bool,
    ) -> Result<Option<i64>> {
        let r = Repository::with_actor(self.0, self.1.clone());
        if kind == "labels" {
            self.get("labels", other).await?;
            if add {
                r.add_label(ticket, other).await?;
            } else {
                self.mutate_one(Scope::ticket(ticket), "DELETE FROM ticket_labels WHERE ticket_id=? AND label_id=? RETURNING ticket_id",vec![ticket.into(),other.into()]).await?;
            }
        } else {
            let project = self.get("tickets", other).await?["project_id"].as_i64();
            let edge = Dependency {
                blocker_id: ticket,
                blocked_id: other,
            };
            if add {
                r.add_dependency(edge).await?;
            } else {
                self.mutate_one(Scope::relation(ticket,other), "DELETE FROM ticket_dependencies WHERE blocker_id=? AND blocked_id=? RETURNING blocker_id",vec![ticket.into(),other.into()]).await?;
            }
            return Ok(project);
        }
        Ok(None)
    }
    pub async fn list(
        &self,
        entity: &str,
        filters: &[(String, String)],
        ticket: Option<i64>,
        limit: i64,
        cursor: i64,
    ) -> Result<Value> {
        let mut clauses = vec!["id>?".to_owned()];
        let mut args = vec![cursor.into()];
        if let Some(ticket) = ticket {
            clauses.push("ticket_id=?".into());
            args.push(ticket.into());
        }
        for (key, v) in filters {
            let clause = match key.as_str() {
                "project" => "project_id=?",
                "status" => "status=?",
                "assignee" => "assignee=?",
                "label" => {
                    "EXISTS(SELECT 1 FROM ticket_labels l WHERE l.ticket_id=tickets.id AND l.label_id=?)"
                }
                "blocked" if v == "true" => {
                    "EXISTS(SELECT 1 FROM ticket_dependencies d JOIN tickets b ON b.id=d.blocker_id WHERE d.blocked_id=tickets.id AND b.status NOT IN ('done','canceled'))"
                }
                "blocked" => {
                    "NOT EXISTS(SELECT 1 FROM ticket_dependencies d JOIN tickets b ON b.id=d.blocker_id WHERE d.blocked_id=tickets.id AND b.status NOT IN ('done','canceled'))"
                }
                "q" => "id IN (SELECT rowid FROM tickets_search WHERE tickets_search MATCH ?)",
                _ => unreachable!("validated filter"),
            };
            match key.as_str() {
                "blocked" => {}
                "q" => args.push(format!("\"{}\"", v.replace('"', "\"\"")).into()),
                "project" | "label" => args.push(v.parse::<i64>().unwrap().into()),
                _ => args.push(v.clone().into()),
            }
            clauses.push(clause.into());
        }
        args.push((limit + 1).into());
        let mut data = self
            .rows(
                &format!(
                    "SELECT {} FROM {entity} WHERE {} ORDER BY id LIMIT ?",
                    selection(entity),
                    clauses.join(" AND ")
                ),
                args,
            )
            .await?;
        let more = data.len() > limit as usize;
        data.truncate(limit as usize);
        let next = if more {
            data.last().map(|v| v["id"].to_string())
        } else {
            None
        };
        let mut out = vec![];
        for v in data {
            out.push(self.enrich(entity, v).await?);
        }
        Ok(json!({"data":out,"next_cursor":next}))
    }
}
