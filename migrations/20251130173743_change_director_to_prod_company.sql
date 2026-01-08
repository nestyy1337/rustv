-- Add migration script here
ALTER TABLE movies RENAME COLUMN director TO production_company;
