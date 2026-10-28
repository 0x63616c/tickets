-- Project moves allocate a fresh destination number in the same UPDATE statement.
CREATE TRIGGER tickets_move_number AFTER UPDATE OF project_id ON tickets
WHEN NEW.project_id <> OLD.project_id
BEGIN
 UPDATE projects SET next_ticket_number=max(next_ticket_number,NEW.number+1) WHERE id=NEW.project_id;
END;
CREATE VIRTUAL TABLE tickets_search USING fts5(title,body,content='tickets',content_rowid='id');
INSERT INTO tickets_search(tickets_search) VALUES('rebuild');
CREATE TRIGGER tickets_search_insert AFTER INSERT ON tickets BEGIN
 INSERT INTO tickets_search(rowid,title,body) VALUES(NEW.id,NEW.title,NEW.body);
END;
CREATE TRIGGER tickets_search_delete AFTER DELETE ON tickets BEGIN
 INSERT INTO tickets_search(tickets_search,rowid,title,body) VALUES('delete',OLD.id,OLD.title,OLD.body);
END;
CREATE TRIGGER tickets_search_update AFTER UPDATE ON tickets BEGIN
 INSERT INTO tickets_search(tickets_search,rowid,title,body) VALUES('delete',OLD.id,OLD.title,OLD.body);
 INSERT INTO tickets_search(rowid,title,body) VALUES(NEW.id,NEW.title,NEW.body);
END;
