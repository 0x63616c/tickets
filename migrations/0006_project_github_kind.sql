-- Preserve whether a linked GitHub Project belongs to a user or organization.
ALTER TABLE projects ADD COLUMN github_project_kind TEXT NOT NULL DEFAULT 'users'
  CHECK (github_project_kind IN ('users', 'orgs'));
