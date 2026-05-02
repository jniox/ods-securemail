SET search_path = securemail, public;

CREATE TABLE IF NOT EXISTS mail_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    smtp_host VARCHAR(255) NOT NULL,
    smtp_port INTEGER NOT NULL,
    smtp_username VARCHAR(255) NOT NULL,
    smtp_password_encrypted BYTEA NOT NULL,
    smtp_encryption VARCHAR(10) NOT NULL CHECK (smtp_encryption IN ('tls', 'starttls', 'none')),
    imap_host VARCHAR(255),
    imap_port INTEGER,
    imap_username VARCHAR(255),
    imap_password_encrypted BYTEA,
    imap_encryption VARCHAR(10) CHECK (imap_encryption IN ('tls', 'starttls', 'none')),
    from_address VARCHAR(255) NOT NULL,
    from_name VARCHAR(255) NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT false,
    verified BOOLEAN NOT NULL DEFAULT false,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Unique constraint: one name per tenant (excluding soft-deleted)
CREATE UNIQUE INDEX mail_configs_tenant_id_name_key
    ON mail_configs (tenant_id, name) WHERE deleted_at IS NULL;

-- Index for finding default config per tenant
CREATE INDEX idx_mail_configs_tenant_default
    ON mail_configs (tenant_id, is_default) WHERE deleted_at IS NULL;

-- RLS
ALTER TABLE mail_configs ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON mail_configs
    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE POLICY tenant_isolation_insert ON mail_configs
    FOR INSERT WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
