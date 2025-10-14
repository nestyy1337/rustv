-- Add migration script here
ALTER TABLE movies
ADD COLUMN available BOOLEAN NOT NULL DEFAULT FALSE;
