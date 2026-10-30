-- Additive: old Workers remain compatible. One indexed pass for project counts.
CREATE INDEX tickets_project_status_activity ON tickets(project_id,status,updated_at);
CREATE INDEX comments_visible_ticket ON comments(ticket_id) WHERE deleted_at IS NULL;
CREATE INDEX ticket_labels_label_ticket ON ticket_labels(label_id,ticket_id);

-- Invalidation covers all data embedded in lists/analytics, including relationship
-- changes that do not touch tickets.updated_at. Triggers also cover old Workers.
CREATE TABLE workspace_version (id INTEGER PRIMARY KEY CHECK(id=1), version INTEGER NOT NULL);
INSERT INTO workspace_version VALUES(1,1);
CREATE TRIGGER read_version_tickets_insert AFTER INSERT ON tickets BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_tickets_update AFTER UPDATE ON tickets BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_tickets_delete AFTER DELETE ON tickets BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_projects_insert AFTER INSERT ON projects BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_projects_update AFTER UPDATE ON projects BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_projects_delete AFTER DELETE ON projects BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_labels_insert AFTER INSERT ON labels BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_labels_update AFTER UPDATE ON labels BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_labels_delete AFTER DELETE ON labels BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_labels_insert AFTER INSERT ON ticket_labels BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_labels_update AFTER UPDATE ON ticket_labels BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_labels_delete AFTER DELETE ON ticket_labels BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_dependencies_insert AFTER INSERT ON ticket_dependencies BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_dependencies_update AFTER UPDATE ON ticket_dependencies BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_dependencies_delete AFTER DELETE ON ticket_dependencies BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_comments_insert AFTER INSERT ON comments BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_comments_update AFTER UPDATE ON comments BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_comments_delete AFTER DELETE ON comments BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_pull_requests_insert AFTER INSERT ON ticket_pull_requests BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_pull_requests_update AFTER UPDATE ON ticket_pull_requests BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_pull_requests_delete AFTER DELETE ON ticket_pull_requests BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_github_pull_request_checks_insert AFTER INSERT ON github_pull_request_checks BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_github_pull_request_checks_update AFTER UPDATE ON github_pull_request_checks BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_github_pull_request_checks_delete AFTER DELETE ON github_pull_request_checks BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_events_insert AFTER INSERT ON ticket_events BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_events_update AFTER UPDATE ON ticket_events BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_ticket_events_delete AFTER DELETE ON ticket_events BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_analytics_coverage_insert AFTER INSERT ON analytics_coverage BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_analytics_coverage_update AFTER UPDATE ON analytics_coverage BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
CREATE TRIGGER read_version_analytics_coverage_delete AFTER DELETE ON analytics_coverage BEGIN
 UPDATE workspace_version SET version=version+1 WHERE id=1;
END;
