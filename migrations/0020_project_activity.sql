-- Additive: status between these columns in the existing counts index prevents
-- a project activity range/order from using that index directly.
CREATE INDEX tickets_project_updated_at ON tickets(project_id,updated_at);
