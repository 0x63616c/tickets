//! Ranked, bounded search in one Store call (two D1 calls including auth).
use crate::repository::{Result, Store};
use serde_json::{Value, json};
use tickets_core::search::{SearchQuery, pattern};

pub async fn search<S: Store>(
    store: &S,
    q: &SearchQuery,
    recent: &[i64],
    limit: i64,
) -> Result<Value> {
    let mut args: Vec<Value> = vec![
        q.text.clone().into(),
        serde_json::to_string(recent).unwrap().into(),
    ];
    let mut clauses = vec!["1=1".to_string()];
    if let Some(status) = q.status {
        args.push(serde_json::to_value(status).unwrap());
        clauses.push(format!("t.status=?{}", args.len()));
    }
    if let Some(project) = &q.project {
        args.push(project.clone().into());
        clauses.push(format!("p.key_prefix=?{}", args.len()));
    }
    let mut scores = vec![];
    for term in &q.terms {
        args.push(pattern(term, false).into());
        let literal = args.len();
        args.push(pattern(term, true).into());
        let fuzzy = args.len();
        let title_literal = format!("t.title LIKE ?{literal} ESCAPE '\\'");
        let title_fuzzy = format!("t.title LIKE ?{fuzzy} ESCAPE '\\'");
        let body_literal = format!("t.body LIKE ?{literal} ESCAPE '\\'");
        let body_fuzzy = format!("t.body LIKE ?{fuzzy} ESCAPE '\\'");
        clauses.push(format!(
            "(lower(p.key_prefix || '-' || t.number)=?1 OR {title_fuzzy} OR {body_fuzzy})"
        ));
        scores.push(format!("CASE WHEN {title_literal} THEN 100 WHEN {title_fuzzy} THEN 55 WHEN {body_literal} THEN 35 WHEN {body_fuzzy} THEN 10 ELSE 0 END"));
    }
    if q.text.is_empty() && !q.filtered() {
        clauses.push("(t.id IN (SELECT value FROM json_each(?2)) OR (t.status NOT IN ('done','canceled') AND (t.status='in_progress' OR nullif(trim(t.assignee),'') IS NOT NULL)))".into());
    }
    let relevance = if scores.is_empty() {
        "0".into()
    } else {
        scores.join(" + ")
    };
    args.push(limit.into());
    let sql = format!("SELECT t.id,t.project_id,p.key_prefix || '-' || t.number AS key,t.title,t.status,t.priority,t.assignee,t.updated_at,t.external_url,
        (lower(p.key_prefix || '-' || t.number)=?1) AS exact,
        ({relevance} + CASE WHEN lower(t.title)=?1 THEN 100 ELSE 0 END
        + CASE WHEN t.status NOT IN ('done','canceled') THEN 8 ELSE 0 END
        + max(0, 6 - (julianday('now')-julianday(t.updated_at))/7)
        + coalesce((SELECT 12-cast(key AS REAL)*0.5 FROM json_each(?2) WHERE value=t.id LIMIT 1),0)) AS score
        FROM tickets t JOIN projects p ON p.id=t.project_id WHERE {}
        ORDER BY exact DESC,CASE WHEN ?1='' AND t.id IN (SELECT value FROM json_each(?2)) THEN 1 ELSE 0 END DESC,score DESC,t.updated_at DESC,t.id DESC LIMIT ?{}", clauses.join(" AND "),args.len());
    let mut data: Vec<Value> = store.query(&sql, args).await?;
    for row in &mut data {
        if let Some(object) = row.as_object_mut() {
            object.remove("score");
            object.remove("exact");
        }
    }
    Ok(json!({"data":data}))
}
