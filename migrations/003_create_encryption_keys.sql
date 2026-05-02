SET search_path = securemail, public;

CREATE TABLE IF NOT EXISTS encryption_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    type VARCHAR(10) NOT NULL CHECK (type IN ('smime', 'pgp')),
    label VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL,
    fingerprint VARCHAR(64) NOT NULL,
    public_key TEXT NOT NULL,
    private_key_encrypted BYTEA,
    certificate TEXT,
    expires_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS encryption_keys_tenant_email_type_key
    ON encryption_keys (tenant_id, email, type) WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_encryption_keys_tenant_fingerprint
    ON encryption_keys (tenant_id, fingerprint);

CREATE INDEX IF NOT EXISTS idx_encryption_keys_tenant_expires
    ON encryption_keys (tenant_id, expires_at) WHERE deleted_at IS NULL;

-- RLS
ALTER TABLE encryption_keys ENABLE ROW LEVEL SECURITY;

DO $$ BEGIN
IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE tablename = 'encryption_keys' AND policyname = 'tenant_isolation') THEN
    CREATE POLICY tenant_isolation ON encryption_keys
        USING (tenant_id = current_setting('app.tenant_id', true)::uuid);
END IF;
END $$;

DO $$ BEGIN
IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE tablename = 'encryption_keys' AND policyname = 'tenant_isolation_insert') THEN
    CREATE POLICY tenant_isolation_insert ON encryption_keys
        FOR INSERT WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
END IF;
END $$;
