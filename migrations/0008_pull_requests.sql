-- Retain the latest event even when an edit removes every ticket key.
CREATE TABLE github_pull_request_events (
 repo TEXT NOT NULL, number INTEGER NOT NULL, url TEXT NOT NULL,
 title TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('open','draft','merged','closed')),
 head_sha TEXT NOT NULL, updated_at TEXT NOT NULL, links TEXT NOT NULL,
 PRIMARY KEY(repo,number)
);
CREATE TABLE ticket_pull_requests (
 ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
 repo TEXT NOT NULL, number INTEGER NOT NULL, url TEXT NOT NULL,
 title TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('open','draft','merged','closed')),
 closes INTEGER NOT NULL CHECK(closes IN (0,1)), head_sha TEXT NOT NULL,
 updated_at TEXT NOT NULL, PRIMARY KEY(ticket_id,repo,number)
);
CREATE INDEX ticket_pull_requests_repo ON ticket_pull_requests(repo,number);
CREATE TRIGGER github_pr_insert AFTER INSERT ON github_pull_request_events BEGIN
 INSERT INTO ticket_pull_requests
 SELECT t.id,NEW.repo,NEW.number,NEW.url,NEW.title,NEW.state,json_extract(j.value,'$.closes'),NEW.head_sha,NEW.updated_at
 FROM json_each(NEW.links) j JOIN projects p ON p.key_prefix=json_extract(j.value,'$.prefix')
 JOIN tickets t ON t.project_id=p.id AND t.number=json_extract(j.value,'$.number');
END;
CREATE TRIGGER github_pr_update AFTER UPDATE ON github_pull_request_events BEGIN
 DELETE FROM ticket_pull_requests WHERE repo=NEW.repo AND number=NEW.number
 AND ticket_id NOT IN (SELECT t.id FROM json_each(NEW.links) j JOIN projects p ON p.key_prefix=json_extract(j.value,'$.prefix') JOIN tickets t ON t.project_id=p.id AND t.number=json_extract(j.value,'$.number'));
 INSERT INTO ticket_pull_requests
 SELECT t.id,NEW.repo,NEW.number,NEW.url,NEW.title,NEW.state,json_extract(j.value,'$.closes'),NEW.head_sha,NEW.updated_at
 FROM json_each(NEW.links) j JOIN projects p ON p.key_prefix=json_extract(j.value,'$.prefix')
 JOIN tickets t ON t.project_id=p.id AND t.number=json_extract(j.value,'$.number') WHERE true
 ON CONFLICT(ticket_id,repo,number) DO UPDATE SET url=excluded.url,title=excluded.title,state=excluded.state,closes=excluded.closes,head_sha=excluded.head_sha,updated_at=excluded.updated_at;
END;
CREATE TRIGGER ticket_pr_insert AFTER INSERT ON ticket_pull_requests BEGIN
 UPDATE tickets SET status=CASE
 WHEN NEW.state='merged' AND NEW.closes=1 AND status<>'canceled' THEN 'done'
 WHEN NEW.state='open' AND status IN ('backlog','todo','in_progress') THEN 'in_review'
 WHEN NEW.state='draft' AND status IN ('backlog','todo') THEN 'in_progress'
 WHEN NEW.state='closed' AND status IN ('in_review','in_progress') AND NOT EXISTS(SELECT 1 FROM ticket_pull_requests WHERE ticket_id=NEW.ticket_id AND state IN ('open','draft')) THEN 'todo'
 ELSE status END,
 updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=NEW.ticket_id;
END;
CREATE TRIGGER ticket_pr_update AFTER UPDATE ON ticket_pull_requests BEGIN
 UPDATE tickets SET status=CASE
 WHEN NEW.state='merged' AND NEW.closes=1 AND status<>'canceled' THEN 'done'
 WHEN NEW.state='open' AND status IN ('backlog','todo','in_progress') THEN 'in_review'
 WHEN NEW.state='draft' AND status IN ('backlog','todo') THEN 'in_progress'
 WHEN NEW.state='closed' AND status IN ('in_review','in_progress') AND NOT EXISTS(SELECT 1 FROM ticket_pull_requests WHERE ticket_id=NEW.ticket_id AND state IN ('open','draft')) THEN 'todo'
 ELSE status END,
 updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=NEW.ticket_id;
END;
CREATE TRIGGER ticket_pr_link_activity AFTER INSERT ON ticket_pull_requests BEGIN
 INSERT INTO comments(ticket_id,author,body) VALUES(NEW.ticket_id,'GitHub','Linked PR [' || NEW.repo || '#' || NEW.number || '](' || NEW.url || ') (' || NEW.state || ').');
END;
CREATE TRIGGER ticket_pr_change_activity AFTER UPDATE ON ticket_pull_requests
WHEN OLD.state<>NEW.state OR OLD.closes<>NEW.closes BEGIN
 INSERT INTO comments(ticket_id,author,body) VALUES(NEW.ticket_id,'GitHub','PR [' || NEW.repo || '#' || NEW.number || '](' || NEW.url || ') is ' || NEW.state || '.');
END;
CREATE TRIGGER ticket_pr_remove_activity AFTER DELETE ON ticket_pull_requests BEGIN
 INSERT INTO comments(ticket_id,author,body) SELECT OLD.ticket_id,'GitHub','Unlinked PR ' || OLD.repo || '#' || OLD.number || '.' WHERE EXISTS(SELECT 1 FROM tickets WHERE id=OLD.ticket_id);
END;
CREATE TRIGGER ticket_pr_status_activity AFTER UPDATE OF status ON tickets
WHEN NEW.status<>OLD.status AND EXISTS(SELECT 1 FROM ticket_pull_requests WHERE ticket_id=NEW.id) BEGIN
 INSERT INTO comments(ticket_id,author,body) VALUES(NEW.id,'system','Status changed from ' || upper(substr(replace(OLD.status,'_',' '),1,1)) || substr(replace(OLD.status,'_',' '),2) || ' to ' || upper(substr(replace(NEW.status,'_',' '),1,1)) || substr(replace(NEW.status,'_',' '),2) || '.');
END;
