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
current AS (
 SELECT s.id AS subject_id,t.id AS ticket_id,t.project_id,p.key_prefix || '-' || t.number AS key,t.title,t.status,t.priority,
 (SELECT count(*) FROM ticket_dependencies d JOIN tickets blocker ON blocker.id=d.blocker_id
  WHERE d.blocked_id=t.id AND blocker.status NOT IN ('done','canceled')) AS blocker_count,
 (SELECT min(blocker.id) FROM ticket_dependencies d JOIN tickets blocker ON blocker.id=d.blocker_id
  WHERE d.blocked_id=t.id AND blocker.status NOT IN ('done','canceled')) AS blocker_id
 FROM tickets t JOIN active_projects p ON p.id=t.project_id JOIN ticket_subjects s ON s.ticket_id=t.id
 WHERE (?1=0 OR t.project_id=?1) AND t.status NOT IN ('done','canceled')
),
flow AS (
 SELECT DISTINCT b.n,e.subject_id,CASE WHEN e.kind='status' THEN 'completed' ELSE e.kind END AS metric
 FROM buckets b JOIN ticket_events e ON e.recorded_at>=b.start AND e.recorded_at<b.end
 JOIN active_projects p ON p.id=e.project_id CROSS JOIN clock
 WHERE e.recorded_at<=clock.as_of AND (?1=0 OR e.project_id=?1)
 AND (e.kind IN ('created','imported') OR (e.kind='status' AND e.new_value='done'))
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
 json_object('subject_id',s.id,'ticket_id',t.id,'key',p.key_prefix || '-' || t.number,'title',t.title,'status',t.status,
 'blocker_id',c.blocker_id,'blocker_count',coalesce(c.blocker_count,0),
 'blocker_key',(SELECT bp.key_prefix || '-' || bt.number FROM tickets bt JOIN projects bp ON bp.id=bt.project_id WHERE bt.id=c.blocker_id)) AS row
 FROM ticket_subjects s LEFT JOIN tickets t ON t.id=s.ticket_id LEFT JOIN projects p ON p.id=t.project_id
 LEFT JOIN current c ON c.subject_id=s.id
),
project_counts AS (
 SELECT p.id,p.name,p.color,p.icon,
 count(CASE WHEN t.status='backlog' THEN 1 END) AS backlog,
 count(CASE WHEN t.status='todo' THEN 1 END) AS todo,
 count(CASE WHEN t.status='in_progress' THEN 1 END) AS in_progress,
 count(CASE WHEN t.status='in_review' THEN 1 END) AS in_review
 FROM active_projects p LEFT JOIN tickets t ON t.project_id=p.id GROUP BY p.id ORDER BY p.name,p.id
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
