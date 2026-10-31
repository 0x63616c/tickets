WITH RECURSIVE
params AS (SELECT ?1 AS project,?2 AS metric,?3 AS week,?4 AS status,?5 AS cursor),
clock AS (SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now') AS as_of,
 strftime('%Y-%m-%dT00:00:00.000Z','now','-' || ((CAST(strftime('%w','now') AS INTEGER)+6)%7) || ' days','-49 days') AS first_week,
 (SELECT started_at FROM analytics_coverage WHERE id=1) AS history_from),
buckets(n,start,end) AS (
 SELECT 0,first_week,strftime('%Y-%m-%dT%H:%M:%fZ',first_week,'+7 days') FROM clock
 UNION ALL SELECT n+1,end,strftime('%Y-%m-%dT%H:%M:%fZ',end,'+7 days') FROM buckets WHERE n<7
),
active_projects AS (SELECT * FROM projects WHERE archived=0),
-- Match Board/List presentation without rewriting the stored status or history.
inventory AS NOT MATERIALIZED (
 SELECT t.id,t.project_id,t.number,t.title,t.status,t.priority,CASE WHEN t.status='todo' AND EXISTS(
  SELECT 1 FROM ticket_labels tl JOIN labels l ON l.id=tl.label_id
  WHERE tl.ticket_id=t.id AND l.name='later' COLLATE NOCASE
 ) THEN 'backlog' ELSE t.status END AS display_status
 FROM tickets t
 WHERE t.status NOT IN ('done','canceled')
),
-- Materialize only scoped open tickets before reusing blocker/label work in
-- counts, queues and cohorts. Aggregate each ticket's edges once for both count
-- and first blocker. Keep empty/resolved-only edges via LEFT JOIN. Inventory stays
-- inline so project counts can use covering indexes.
current AS MATERIALIZED (
 SELECT s.id AS subject_id,t.id AS ticket_id,t.project_id,p.key_prefix || '-' || t.number AS key,t.title,t.display_status AS status,t.priority,
 count(blocker.id) AS blocker_count,min(blocker.id) AS blocker_id
 FROM inventory t JOIN active_projects p ON p.id=t.project_id JOIN ticket_subjects s ON s.ticket_id=t.id
 LEFT JOIN ticket_dependencies d ON d.blocked_id=t.id
 LEFT JOIN tickets blocker ON blocker.id=d.blocker_id AND blocker.status NOT IN ('done','canceled')
 WHERE (?1=0 OR t.project_id=?1) AND t.status NOT IN ('done','canceled')
 GROUP BY t.id
),
-- Restrict history before expanding it into weekly buckets. Imported/deleted
-- subjects retain their historical counts, with active-project scope applied here.
filtered_events AS NOT MATERIALIZED (
 SELECT e.subject_id,e.recorded_at,CASE WHEN e.kind='status' THEN 'completed' ELSE e.kind END AS metric
 FROM ticket_events e JOIN active_projects p ON p.id=e.project_id
 WHERE e.recorded_at>=(SELECT first_week FROM clock) AND e.recorded_at<=(SELECT as_of FROM clock)
 AND (?1=0 OR e.project_id=?1)
 AND (e.kind IN ('created','imported') OR (e.kind='status' AND e.new_value='done'))
),
flow AS (
 SELECT DISTINCT b.n,e.subject_id,e.metric
 FROM filtered_events e JOIN buckets b ON e.recorded_at>=b.start AND e.recorded_at<b.end
),
cohort_ids AS (
 SELECT subject_id FROM current WHERE
 (?2='in_flight' AND status IN ('in_progress','in_review')) OR (?2='review' AND status='in_review') OR
 (?2='blocked' AND blocker_count>0) OR (?2='inventory' AND (?4='' OR status=?4))
 UNION SELECT subject_id FROM flow WHERE metric=?2 AND n=?3
),
cohort_page AS (SELECT subject_id FROM cohort_ids WHERE subject_id>?5 ORDER BY subject_id LIMIT 50),
-- Shared display rows preserve deleted historical subjects, without borrowing a reused ID.
display AS (
 SELECT s.id AS subject_id,t.id AS ticket_id,p.key_prefix || '-' || t.number AS key,t.title,t.status,
 c.blocker_id,coalesce(c.blocker_count,0) AS blocker_count,
 (SELECT bp.key_prefix || '-' || bt.number FROM tickets bt JOIN projects bp ON bp.id=bt.project_id WHERE bt.id=c.blocker_id) AS blocker_key,
 json_object('subject_id',s.id,'ticket_id',t.id,'key',p.key_prefix || '-' || t.number,'title',t.title,'type',t.type,'scope',t.scope,'status',t.status,
 'blocker_id',c.blocker_id,'blocker_count',coalesce(c.blocker_count,0),
 'blocker_key',(SELECT bp.key_prefix || '-' || bt.number FROM tickets bt JOIN projects bp ON bp.id=bt.project_id WHERE bt.id=c.blocker_id)) AS row
 FROM ticket_subjects s LEFT JOIN tickets t ON t.id=s.ticket_id LEFT JOIN projects p ON p.id=t.project_id
 LEFT JOIN current c ON c.subject_id=s.id
),
project_counts AS (
 SELECT p.id,p.name,p.color,p.icon,
 count(CASE WHEN t.display_status='backlog' THEN 1 END) AS backlog,
 count(CASE WHEN t.display_status='todo' THEN 1 END) AS todo,
 count(CASE WHEN t.display_status='in_progress' THEN 1 END) AS in_progress,
 count(CASE WHEN t.display_status='in_review' THEN 1 END) AS in_review
 FROM active_projects p LEFT JOIN inventory t ON t.project_id=p.id GROUP BY p.id ORDER BY p.name,p.id
)
SELECT (?1=0 OR EXISTS(SELECT 1 FROM active_projects WHERE id=?1)) AS scope_valid,
json_object(
 'as_of',as_of,'history_from',history_from,
 'projects',json((SELECT json_group_array(json_object('id',id,'name',name,'color',color,'icon',icon,'backlog',backlog,'todo',todo,'in_progress',in_progress,'in_review',in_review)) FROM project_counts)),
 'in_flight',(SELECT count(*) FROM current WHERE status IN ('in_progress','in_review')),
 'in_review',(SELECT count(*) FROM current WHERE status='in_review'),
 'blocked',(SELECT count(*) FROM current WHERE blocker_count>0),
 'weeks',json((SELECT json_group_array(json_object('start',b.start,'end',min(b.end,as_of),
 'coverage',CASE WHEN history_from>=b.end THEN 'none' WHEN history_from>b.start THEN 'partial' ELSE 'full' END,
 'created',CASE WHEN history_from<b.end THEN (SELECT count(*) FROM flow f WHERE f.n=b.n AND f.metric='created') END,
 'completed',CASE WHEN history_from<b.end THEN (SELECT count(*) FROM flow f WHERE f.n=b.n AND f.metric='completed') END,
 'imported',CASE WHEN history_from<b.end THEN (SELECT count(*) FROM flow f WHERE f.n=b.n AND f.metric='imported') END)) FROM buckets b)),
 'review_queue',json((SELECT json_group_array(json(row)) FROM (SELECT d.row FROM current c JOIN display d ON d.subject_id=c.subject_id WHERE c.status='in_review' ORDER BY CASE c.priority WHEN 0 THEN 5 ELSE c.priority END,c.subject_id LIMIT 5))),
 'blocked_queue',json((SELECT json_group_array(json(row)) FROM (SELECT d.row FROM current c JOIN display d ON d.subject_id=c.subject_id WHERE c.blocker_count>0 ORDER BY c.status='backlog',CASE c.priority WHEN 0 THEN 5 ELSE c.priority END,c.subject_id LIMIT 5))),
 'cohort',json((SELECT json_group_array(json(row)) FROM (SELECT d.row FROM cohort_page c JOIN display d ON d.subject_id=c.subject_id ORDER BY c.subject_id))),
 'cohort_total',(SELECT count(*) FROM cohort_ids),
 'next_cursor',CASE WHEN EXISTS(SELECT 1 FROM cohort_ids WHERE subject_id>(SELECT max(subject_id) FROM cohort_page)) THEN (SELECT max(subject_id) FROM cohort_page) END
) AS home FROM clock;
