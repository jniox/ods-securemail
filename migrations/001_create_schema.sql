-- Create the securemail schema
CREATE SCHEMA IF NOT EXISTS securemail;

-- Ensure pgcrypto is available
CREATE EXTENSION IF NOT EXISTS pgcrypto;
