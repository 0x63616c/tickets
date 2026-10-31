//! Replay the documented browser request schedule against isolated local D1.
//! All measurements come from wrangler meta.rows_read, never SQLite VM estimates.
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn literal(source: &str, marker: &str) -> String {
    source
        .split_once(marker)
        .unwrap()
        .1
        .split_once('"')
        .unwrap()
        .1
        .split_once('"')
        .unwrap()
        .0
        .into()
}
fn execute(
    root: &Path,
    dir: &Path,
    sql: &str,
    name: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let file = dir.join(format!("{name}.sql"));
    fs::write(&file, sql)?;
    if matches!(name, "before" | "after" | "plans") {
        let port = fs::read_to_string(dir.join("port"))?;
        let client = reqwest::blocking::Client::new();
        let statements = sql
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        let mut results = Vec::<Value>::new();
        for chunk in statements.chunks(20) {
            let response = client
                .post(format!("http://127.0.0.1:{port}/"))
                .json(&chunk)
                .send()?;
            if !response.status().is_success() {
                return Err(response.text()?.into());
            }
            results.extend(response.json::<Vec<Value>>()?);
        }
        return Ok(json!(results));
    }
    let output = Command::new("wrangler")
        .current_dir(root)
        .args(["d1", "execute", "DB", "--local", "--json", "--config"])
        .arg(dir.join("wrangler.toml"))
        .arg("--persist-to")
        .arg(dir.join("state"))
        .arg("--file")
        .arg(file)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
fn read(sql: &str, queries: &mut String, cache: bool) {
    queries.push_str("SELECT user_id,coalesce(nullif(trim(users.display_name),''),users.username) AS actor_label FROM sessions JOIN users ON users.id=sessions.user_id WHERE token_hash='fixture' AND expires_at>strftime('%Y-%m-%dT%H:%M:%fZ','now');\n");
    if cache {
        queries.push_str("SELECT version FROM workspace_version WHERE id=1;\n");
    }
    queries.push_str(sql);
    queries.push_str(";\n");
}
fn collection(ticket: &str, projects: &str, queries: &mut String, cache: bool) {
    read(projects, queries, cache);
    for cursor in [0, 100, 200] {
        read(
            &format!("{ticket} WHERE tickets.id>{cursor} ORDER BY tickets.id LIMIT 101"),
            queries,
            cache,
        );
    }
    read(
        "SELECT * FROM labels WHERE id>0 ORDER BY id LIMIT 101",
        queries,
        cache,
    );
}
pub fn check(root: &Path) -> super::Result {
    let dir = root
        .join("target")
        .join(format!("d1-perf-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join("wrangler.toml"),
        "name='tickets-perf'\ncompatibility_date='2026-10-27'\n[[d1_databases]]\nbinding='DB'\ndatabase_name='tickets-perf'\ndatabase_id='00000000-0000-0000-0000-000000000000'\n",
    )?;
    let old = Command::new("git")
        .current_dir(root)
        .args(["show", "b42be78:crates/worker/src/api_repository.rs"])
        .output()?;
    if !old.status.success() {
        return Err("baseline b42be78 unavailable; fetch repository history".into());
    }
    let old = String::from_utf8(old.stdout)?;
    let old_ticket = format!(
        "SELECT {} FROM tickets",
        literal(&old, "const TICKET_SELECT:")
    );
    let old_projects = format!(
        "SELECT {} FROM projects WHERE id>0 ORDER BY id LIMIT 101",
        literal(&old, "else if entity == \"projects\" {")
    );
    let new_source = fs::read_to_string(root.join("crates/worker/src/api_repository.rs"))?;
    let new_ticket = format!(
        "SELECT {} FROM tickets JOIN projects ticket_project ON ticket_project.id=tickets.project_id",
        literal(&new_source, "const TICKET_SELECT:").replace(
            "(SELECT key_prefix FROM projects WHERE id=tickets.project_id)",
            "ticket_project.key_prefix"
        )
    );
    let selection = literal(&new_source, "else if entity == \"projects\" {");
    let source = literal(
        new_source.split_once("fn source(entity:").unwrap().1,
        "if entity == \"projects\" {",
    );
    let new_projects = format!(
        "SELECT {selection} FROM {source} WHERE projects.id>0 ORDER BY projects.id LIMIT 101"
    );
    let bind_analytics = |sql: &str| {
        sql.replace("?1", "0")
            .replace("?2", "''")
            .replace("?3", "7")
            .replace("?4", "''")
            .replace("?5", "0")
    };
    // The baseline must use its own SQL: optimizing today's analytics must not
    // retroactively reduce the historical denominator in the daily projection.
    let old_analytics = Command::new("git")
        .current_dir(root)
        .args(["show", "b42be78:crates/worker/src/analytics.sql"])
        .output()?;
    if !old_analytics.status.success() {
        return Err("baseline analytics unavailable; fetch repository history".into());
    }
    let old_analytics = bind_analytics(&String::from_utf8(old_analytics.stdout)?);
    let analytics = bind_analytics(&fs::read_to_string(
        root.join("crates/worker/src/analytics.sql"),
    )?);
    let mut migrations: Vec<_> = fs::read_dir(root.join("migrations"))?
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        .collect();
    migrations.sort();
    let efficiency = migrations
        .iter()
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("_read_efficiency.sql")
        })
        .ok_or("missing read efficiency migration")?;
    let activity_index = migrations
        .iter()
        .find(|p| p.file_name().unwrap() == "0020_project_activity.sql")
        .ok_or("missing project activity migration")?;
    let mut setup = String::from("PRAGMA foreign_keys=ON;\n");
    for path in &migrations {
        if path != efficiency && path != activity_index {
            setup.push_str(&fs::read_to_string(path)?);
            setup.push('\n');
        }
    }
    setup.push_str("INSERT INTO users(id,username) VALUES(1,'fixture'); INSERT INTO sessions(token_hash,user_id,expires_at) VALUES('fixture',1,'2099-01-01T00:00:00.000Z'); INSERT INTO projects(name,key_prefix,color) VALUES('One','ONE','#5e6ad2'),('Two','TWO','#5e6ad2'),('Three','THREE','#5e6ad2'),('Four','FOUR','#5e6ad2'),('Five','FIVE','#5e6ad2'); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<240) INSERT INTO tickets(project_id,number,title,status) SELECT (x-1)%5+1,(x-1)/5+1,'Fixture '||x,CASE x%6 WHEN 0 THEN 'done' WHEN 1 THEN 'todo' WHEN 2 THEN 'in_progress' WHEN 3 THEN 'in_review' WHEN 4 THEN 'backlog' ELSE 'canceled' END FROM n; INSERT INTO labels(name,color) VALUES('bug','#5e6ad2'),('feature','#5e6ad2'); INSERT INTO ticket_labels SELECT id,1 FROM tickets WHERE id%3=0; INSERT INTO ticket_dependencies SELECT id,id+1 FROM tickets WHERE id%7=0 AND id<240; INSERT INTO comments(ticket_id,author,body,author_kind,author_id) SELECT id,'Fixture','Example','user','1' FROM tickets WHERE id%4=0;");
    execute(root, &dir, &setup, "seed")?;
    // wrangler's CLI intentionally discards rows_read. Its local D1 binding
    // preserves it, so a disposable loopback-only Worker runs measurement queries.
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    fs::write(dir.join("port"), port.to_string())?;
    fs::write(
        dir.join("bench.mjs"),
        "export default { async fetch(request, env) { if(request.method==='GET') return new Response('ready'); try { const statements=await request.json(); const results=await env.DB.batch(statements.map(sql=>env.DB.prepare(sql))); return Response.json(results); } catch(error) {return new Response(String(error),{status:500});} } }; ",
    )?;
    let log = fs::File::create(dir.join("worker.log"))?;
    let mut server_command = Command::new("wrangler");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        server_command.process_group(0);
    }
    let server = server_command
        .current_dir(root)
        .args([
            "dev",
            "--local",
            "--ip",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--inspector-port",
            "0",
            "--config",
        ])
        .arg(dir.join("wrangler.toml"))
        .arg(dir.join("bench.mjs"))
        .arg("--persist-to")
        .arg(dir.join("state"))
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
    struct Server(std::process::Child);
    impl Drop for Server {
        fn drop(&mut self) {
            #[cfg(unix)]
            let _ = Command::new("kill")
                .args(["-TERM", "--", &format!("-{}", self.0.id())])
                .output();
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _server = Server(server);
    let client = reqwest::blocking::Client::new();
    let start = std::time::Instant::now();
    while client
        .get(format!("http://127.0.0.1:{port}/"))
        .send()
        .is_err()
    {
        if start.elapsed().as_secs() > 30 {
            return Err("local perf Worker did not start".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let mut before = String::new();
    // Home initial + room open; Board/List/detail each mount + room reopen;
    // return Home + room reopen; five Home minute ticks and three live updates.
    // Collections: 3 mounts + 3 room reopens. Home: 4 arrivals + 5 ticks + 3 events.
    for _ in 0..12 {
        read(&old_analytics, &mut before, false);
        read(&old_projects, &mut before, false);
    }
    for _ in 0..6 {
        collection(&old_ticket, &old_projects, &mut before, false);
    }
    read(
        &format!("{old_ticket} WHERE tickets.id=1"),
        &mut before,
        false,
    );
    let before_result = execute(root, &dir, &before, "before")?;
    execute(root, &dir, &fs::read_to_string(efficiency)?, "optimize")?;
    execute(
        root,
        &dir,
        &fs::read_to_string(activity_index)?,
        "activity-index",
    )?;
    let mut after = String::new();
    // One Home snapshot + shared projects; Board collection once; List reuses it.
    // Initial snapshot-to-socket handshake: session auth plus one version row.
    read(
        "SELECT version FROM workspace_version WHERE id=1",
        &mut after,
        false,
    );
    read(&analytics, &mut after, true);
    collection(&new_ticket, &new_projects, &mut after, true);
    read(
        &format!("{new_ticket} WHERE tickets.id=1"),
        &mut after,
        false,
    );
    for id in [2, 3, 4] {
        // Each mutation invalidates the response cache. Counts, one record and
        // visible Home analytics refresh; no collection reload. Include cold SQL
        // for analytics to make this estimate conservative across POPs.
        after.push_str(&format!(
            "UPDATE tickets SET title=title||' updated' WHERE id={id};\n"
        ));
        read(&new_projects, &mut after, true);
        read(
            &format!("{new_ticket} WHERE tickets.id={id}"),
            &mut after,
            false,
        );
        read(&analytics, &mut after, true);
    }
    let after_result = execute(root, &dir, &after, "after")?;
    let mut plans = String::new();
    for sql in [
        &new_projects,
        &format!("{new_ticket} WHERE tickets.id>0 ORDER BY tickets.id LIMIT 101"),
    ] {
        plans.push_str(&format!("EXPLAIN QUERY PLAN {sql};\n"));
    }
    let plans = execute(root, &dir, &plans, "plans")?;
    let summarize = |v: &Value| {
        let entries = v.as_array().unwrap();
        json!({"rows_read":entries.iter().map(|x|x["meta"]["rows_read"].as_u64().expect("wrangler must expose rows_read")).sum::<u64>(),"statements":entries.len(),"d1_duration_ms":entries.iter().filter_map(|x|x["meta"]["duration"].as_f64()).sum::<f64>()})
    };
    let mut report = json!({"baseline":"b42be78","fixture_tickets":240,"before":summarize(&before_result),"after":summarize(&after_result),"note":"Deterministic request-schedule replay; not browser elapsed time or Worker CPU. Three live updates; five idle minute ticks elided after. Includes cold analytics after each update.","plans":plans});
    let before_rows = report["before"]["rows_read"].as_f64().unwrap();
    let after_rows = report["after"]["rows_read"].as_f64().unwrap();
    let projected_rows = 256_552. + 2_414_178. * after_rows / before_rows;
    report["projected_rows_per_day"] = json!(projected_rows);
    fs::write(
        dir.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    println!(
        "{}\nEvidence: {}",
        serde_json::to_string_pretty(&report)?,
        dir.display()
    );
    // Hold every query family outside the baseline top three fixed.
    if projected_rows >= 1_000_000. {
        return Err("session replay exceeds projected 1M rows/day budget".into());
    }
    Ok(())
}
