SET search_path = securemail, public;

CREATE TABLE IF NOT EXISTS templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    subject TEXT NOT NULL,
    body_html TEXT NOT NULL,
    body_text TEXT,
    variables JSONB NOT NULL DEFAULT '[]',
    mail_config_id UUID REFERENCES mail_configs(id),
    encryption_mode VARCHAR(20) NOT NULL DEFAULT 'none'
        CHECK (encryption_mode IN ('none', 'sign', 'encrypt', 'sign_and_encrypt')),
    key_id UUID REFERENCES encryption_keys(id),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX templates_tenant_name_key
    ON templates (tenant_id, name) WHERE deleted_at IS NULL;

CREATE INDEX idx_templates_tenant
    ON templates (tenant_id) WHERE deleted_at IS NULL;

-- RLS
ALTER TABLE templates ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON templates
    USING (tenant_id = current_setting('app.tenant_id', true)::uuid);

CREATE POLICY tenant_isolation_insert ON templates
    FOR INSERT WITH CHECK (tenant_id = current_setting('app.tenant_id', true)::uuid);
