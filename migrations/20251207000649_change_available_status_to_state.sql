-- Add migration script here
ALTER TABLE movies DROP COLUMN available;
ALTER TABLE movies DROP COLUMN requested;

ALTER TABLE movies ADD COLUMN state INTEGR NOT NULL DEFAULT 0;
