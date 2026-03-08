CREATE TABLE IF NOT EXISTS rux.buckets (
    name text,
    owner text,
    region text,
    versioning text,
    policy text,
    lifecycle text,
    created_at timestamp,
    PRIMARY KEY (name)
);

CREATE TABLE IF NOT EXISTS rux.objects (
    bucket text,
    key text,
    version_id timeuuid,
    etag text,
    size bigint,
    content_type text,
    user_metadata map<text, text>,
    chunk_count int,
    delete_marker boolean,
    created_at timestamp,
    PRIMARY KEY ((bucket, key), version_id)
) WITH CLUSTERING ORDER BY (version_id DESC)
  AND gc_grace_seconds = 3600;

CREATE TABLE IF NOT EXISTS rux.chunks (
    bucket text,
    key text,
    version_id timeuuid,
    chunk_index int,
    data blob,
    checksum text,
    PRIMARY KEY ((bucket, key, version_id, chunk_index))
);

CREATE TABLE IF NOT EXISTS rux.listings (
    bucket text,
    prefix text,
    name text,
    is_common_prefix boolean,
    version_id timeuuid,
    size bigint,
    etag text,
    last_modified timestamp,
    PRIMARY KEY ((bucket, prefix), name)
) WITH CLUSTERING ORDER BY (name ASC);

CREATE TABLE IF NOT EXISTS rux.multipart_uploads (
    bucket text,
    key text,
    upload_id timeuuid,
    content_type text,
    user_metadata map<text, text>,
    status text,
    initiated timestamp,
    PRIMARY KEY ((bucket, key), upload_id)
) WITH CLUSTERING ORDER BY (upload_id DESC);

-- Denormalized listing table for ListBuckets. Writes must be kept in sync
-- with rux.buckets at the application level. On partial failure the caller
-- should retry; reconciliation can detect drift by comparing against rux.buckets.
CREATE TABLE IF NOT EXISTS rux.buckets_by_owner (
    owner text,
    name text,
    region text,
    created_at timestamp,
    PRIMARY KEY (owner, name)
) WITH CLUSTERING ORDER BY (name ASC);

-- Denormalized table for ListMultipartUploads. Rows are inserted when an upload
-- is created and deleted on complete/abort. Must be kept in sync with
-- rux.multipart_uploads at the application level.
CREATE TABLE IF NOT EXISTS rux.active_multipart_uploads (
    bucket text,
    key text,
    upload_id timeuuid,
    initiated timestamp,
    PRIMARY KEY (bucket, key, upload_id)
);

CREATE TABLE IF NOT EXISTS rux.multipart_parts (
    bucket text,
    upload_id timeuuid,
    part_number int,
    data blob,
    size bigint,
    etag text,
    created_at timestamp,
    PRIMARY KEY ((bucket, upload_id), part_number)
) WITH CLUSTERING ORDER BY (part_number ASC);
