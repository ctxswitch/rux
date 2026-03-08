# S3 API Interface

This document specifies the S3-compatible HTTP API that Rux implements. All endpoints follow the [Amazon S3 REST API](https://docs.aws.amazon.com/AmazonS3/latest/API/) conventions for request signing, headers, error responses, and XML formatting.

## Authentication

All requests are authenticated using **S3 Signature Version 4 (SigV4)**. The gateway validates the `Authorization` header (or presigned URL query parameters) against access keys stored in Kubernetes Secrets.

### Signature Methods

| Method | Description |
|---|---|
| `Authorization` header | Standard SigV4 header-based signing |
| Presigned URL | Query parameter signing (`X-Amz-Algorithm`, `X-Amz-Credential`, `X-Amz-Signature`, etc.) |
| `x-amz-content-sha256` | Required for chunked uploads, `UNSIGNED-PAYLOAD` accepted for non-chunked |

### Request Headers (Common)

| Header | Required | Description |
|---|---|---|
| `Authorization` | Yes (unless presigned) | SigV4 signature |
| `x-amz-date` | Yes | ISO 8601 timestamp |
| `x-amz-content-sha256` | Yes | SHA-256 of payload or `UNSIGNED-PAYLOAD` |
| `Host` | Yes | Bucket endpoint (virtual-hosted or path-style) |

## Addressing Style

Rux supports both addressing styles. Path-style is the default.

- **Path-style** (default): `https://s3.example.com/bucket/key`
- **Virtual-hosted-style**: `https://bucket.s3.example.com/key` — requires wildcard DNS (`*.s3.example.com`) and is enabled via `ingress.virtualHosted: true` in the Helm chart.

The gateway determines the addressing style by inspecting the `Host` header. If the host matches a subdomain of the configured S3 domain, the first label is extracted as the bucket name. Otherwise, the bucket is parsed from the URL path.

## Rux Custom Headers

| Header | Direction | Description |
|---|---|---|
| `X-Rux-Consistency` | Request | `eventual`, `quorum`, `strong`, or `all` |
| `X-Rux-Source-Region` | Response | ScyllaDB datacenter that served the read |
| `X-Rux-Version-Id` | Response | `timeuuid` version of the object |

---

## Bucket Operations

### CreateBucket

Creates a new bucket.

```
PUT /{bucket} HTTP/1.1
```

**Request Body** (optional):
```xml
<CreateBucketConfiguration>
  <LocationConstraint>us-east-1</LocationConstraint>
</CreateBucketConfiguration>
```

**Response**: `200 OK`

| Header | Description |
|---|---|
| `Location` | `/{bucket}` |

**Errors**:
- `409 BucketAlreadyExists` — bucket name is taken
- `409 BucketAlreadyOwnedByYou` — caller already owns this bucket

**Implementation**: `INSERT INTO rux.buckets ... IF NOT EXISTS` (LWT).

---

### DeleteBucket

Deletes a bucket. The bucket must be empty.

```
DELETE /{bucket} HTTP/1.1
```

**Response**: `204 No Content`

**Errors**:
- `404 NoSuchBucket`
- `409 BucketNotEmpty`

**Implementation**: Check `rux.listings` for any keys in the bucket. If empty, `DELETE FROM rux.buckets WHERE name = ?`.

---

### HeadBucket

Checks if a bucket exists and is accessible.

```
HEAD /{bucket} HTTP/1.1
```

**Response**: `200 OK` (no body)

**Errors**:
- `404 NoSuchBucket`
- `403 AccessDenied`

---

### ListBuckets

Lists all buckets owned by the authenticated user.

```
GET / HTTP/1.1
```

**Response**: `200 OK`
```xml
<ListAllMyBucketsResult>
  <Owner>
    <ID>owner-id</ID>
    <DisplayName>owner</DisplayName>
  </Owner>
  <Buckets>
    <Bucket>
      <Name>my-bucket</Name>
      <CreationDate>2026-01-01T00:00:00.000Z</CreationDate>
    </Bucket>
  </Buckets>
</ListAllMyBucketsResult>
```

**Implementation**: `SELECT * FROM rux.buckets_by_owner WHERE owner = ?`.

---

### GetBucketLocation

Returns the region (datacenter) of a bucket.

```
GET /{bucket}?location HTTP/1.1
```

**Response**: `200 OK`
```xml
<LocationConstraint>us-east-1</LocationConstraint>
```

---

### GetBucketVersioning

Returns the versioning state of a bucket.

```
GET /{bucket}?versioning HTTP/1.1
```

**Response**: `200 OK`
```xml
<VersioningConfiguration>
  <Status>Enabled</Status>
</VersioningConfiguration>
```

---

### PutBucketVersioning

Sets the versioning state of a bucket.

```
PUT /{bucket}?versioning HTTP/1.1
```

**Request Body**:
```xml
<VersioningConfiguration>
  <Status>Enabled</Status>
</VersioningConfiguration>
```

Valid `Status` values: `Enabled`, `Suspended`.

**Response**: `200 OK`

**Implementation**: `UPDATE rux.buckets SET versioning = ? WHERE name = ?`.

---

### GetBucketPolicy

Returns the bucket policy (JSON).

```
GET /{bucket}?policy HTTP/1.1
```

**Response**: `200 OK` with JSON body.

**Errors**:
- `404 NoSuchBucketPolicy`

---

### PutBucketPolicy

Sets the bucket policy.

```
PUT /{bucket}?policy HTTP/1.1
```

**Request Body**: JSON policy document.

**Response**: `200 OK`

---

### GetBucketLifecycleConfiguration

Returns lifecycle rules for a bucket.

```
GET /{bucket}?lifecycle HTTP/1.1
```

**Response**: `200 OK`
```xml
<LifecycleConfiguration>
  <Rule>
    <ID>expire-logs</ID>
    <Filter>
      <Prefix>logs/</Prefix>
    </Filter>
    <Status>Enabled</Status>
    <Expiration>
      <Days>30</Days>
    </Expiration>
  </Rule>
</LifecycleConfiguration>
```

---

### PutBucketLifecycleConfiguration

Sets lifecycle rules for a bucket.

```
PUT /{bucket}?lifecycle HTTP/1.1
```

**Request Body**: XML `LifecycleConfiguration`.

**Response**: `200 OK`

**Implementation**: Stores rules in `rux.buckets.lifecycle`. A background process in the gateway applies TTLs to matching objects.

---

## Object Operations

### PutObject

Uploads an object.

```
PUT /{bucket}/{key} HTTP/1.1
Content-Length: 10485760
Content-Type: application/octet-stream
```

**Request Headers**:

| Header | Required | Description |
|---|---|---|
| `Content-Length` | Yes | Object size in bytes |
| `Content-Type` | No | MIME type (default: `application/octet-stream`) |
| `Content-MD5` | No | Base64-encoded MD5 for integrity check |
| `x-amz-meta-*` | No | User-defined metadata |
| `x-amz-storage-class` | No | Ignored (single storage class) |
| `If-None-Match` | No | `*` to prevent overwrite (uses LWT) |
| `X-Rux-Consistency` | No | Consistency level for this write |

**Response**: `200 OK`

| Header | Description |
|---|---|
| `ETag` | MD5 hex digest of the object content, quoted |
| `x-amz-version-id` | Version ID (if versioning enabled) |
| `X-Rux-Source-Region` | Datacenter that handled the write |

**Implementation**:
1. Stream request body, chunking into 1 MB segments.
2. Write each chunk: `INSERT INTO rux.chunks (bucket, key, version_id, chunk_index, data, checksum) VALUES (?, ?, ?, ?, ?, ?)`.
3. Write metadata: `INSERT INTO rux.objects (...)`.
4. Write listing entry: `INSERT INTO rux.listings (...)`.

For non-versioned buckets without `If-None-Match`, metadata is an unconditional write (LWW). With `If-None-Match: *`, uses `INSERT ... IF NOT EXISTS`.

---

### GetObject

Retrieves an object.

```
GET /{bucket}/{key} HTTP/1.1
```

**Request Headers**:

| Header | Required | Description |
|---|---|---|
| `Range` | No | Byte range (`bytes=0-1023`) |
| `If-Match` | No | Return only if ETag matches |
| `If-None-Match` | No | Return only if ETag does not match |
| `If-Modified-Since` | No | Return only if modified after date |
| `If-Unmodified-Since` | No | Return only if not modified after date |
| `versionId` | No | Query param — specific version |
| `X-Rux-Consistency` | No | Consistency level for this read |

**Response**: `200 OK` (or `206 Partial Content` for range requests)

| Header | Description |
|---|---|
| `Content-Length` | Object size (or range size) |
| `Content-Type` | MIME type |
| `ETag` | Object ETag |
| `Last-Modified` | RFC 7231 date |
| `x-amz-version-id` | Version ID (if versioning enabled) |
| `x-amz-meta-*` | User-defined metadata |
| `Content-Range` | Byte range (if range request) |
| `X-Rux-Source-Region` | Datacenter that served the read |

**Implementation**:
1. Read metadata: `SELECT * FROM rux.objects WHERE bucket = ? AND key = ? LIMIT 1` (latest version).
2. For specific version: `SELECT * FROM rux.objects WHERE bucket = ? AND key = ? AND version_id = ?`.
3. Read chunks in parallel: `SELECT data FROM rux.chunks WHERE bucket = ? AND key = ? AND version_id = ? AND chunk_index = ?` for each chunk.
4. Stream chunks to client in order.
5. For range requests, compute the start/end chunk indices and byte offsets within those chunks.

**Errors**:
- `404 NoSuchKey`
- `304 Not Modified` (conditional)
- `412 Precondition Failed` (conditional)
- `416 InvalidRange`

---

### HeadObject

Returns object metadata without the body. Same as GetObject but returns headers only.

```
HEAD /{bucket}/{key} HTTP/1.1
```

**Response**: `200 OK` (no body, same headers as GetObject)

---

### DeleteObject

Deletes an object (or creates a delete marker if versioning is enabled).

```
DELETE /{bucket}/{key} HTTP/1.1
```

**Query Parameters**:

| Param | Description |
|---|---|
| `versionId` | Delete a specific version |

**Response**: `204 No Content`

| Header | Description |
|---|---|
| `x-amz-version-id` | Version ID of the deleted version or delete marker |
| `x-amz-delete-marker` | `true` if a delete marker was created |

**Implementation**:
- Non-versioned: Delete rows from `rux.objects`, `rux.chunks`, and `rux.listings`.
- Versioned (no versionId): Insert a delete marker row in `rux.objects`.
- Versioned (with versionId): Delete the specific version from `rux.objects` and its chunks.

---

### CopyObject

Copies an object.

```
PUT /{dest-bucket}/{dest-key} HTTP/1.1
x-amz-copy-source: /{source-bucket}/{source-key}
```

**Request Headers**:

| Header | Required | Description |
|---|---|---|
| `x-amz-copy-source` | Yes | Source bucket/key (optionally with `?versionId=`) |
| `x-amz-metadata-directive` | No | `COPY` (default) or `REPLACE` |
| `x-amz-copy-source-if-match` | No | Conditional copy |
| `x-amz-copy-source-if-none-match` | No | Conditional copy |

**Response**: `200 OK`
```xml
<CopyObjectResult>
  <ETag>"etag"</ETag>
  <LastModified>2026-01-01T00:00:00.000Z</LastModified>
</CopyObjectResult>
```

**Implementation**: Read all chunks from source, write new chunk rows with new version_id for destination, write new metadata and listing rows. No server-side copy shortcut — data is read and rewritten.

---

### ListObjectsV2

Lists objects in a bucket with prefix filtering.

```
GET /{bucket}?list-type=2 HTTP/1.1
```

**Query Parameters**:

| Param | Required | Description |
|---|---|---|
| `list-type` | Yes | Must be `2` |
| `prefix` | No | Filter to keys starting with this prefix |
| `delimiter` | No | Group keys by delimiter (typically `/`) |
| `max-keys` | No | Maximum keys to return (default 1000, max 1000) |
| `start-after` | No | Start listing after this key |
| `continuation-token` | No | Token from previous truncated response |

**Response**: `200 OK`
```xml
<ListBucketResult>
  <Name>my-bucket</Name>
  <Prefix>photos/</Prefix>
  <Delimiter>/</Delimiter>
  <MaxKeys>1000</MaxKeys>
  <IsTruncated>false</IsTruncated>
  <Contents>
    <Key>photos/cat.jpg</Key>
    <LastModified>2026-01-01T00:00:00.000Z</LastModified>
    <ETag>"etag"</ETag>
    <Size>1048576</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
  <CommonPrefixes>
    <Prefix>photos/vacation/</Prefix>
  </CommonPrefixes>
</ListBucketResult>
```

**Implementation**: `SELECT * FROM rux.listings WHERE bucket = ? AND prefix = ? AND name > ? ORDER BY name ASC LIMIT ?`. The gateway computes `CommonPrefixes` by grouping keys at the delimiter boundary. Continuation tokens encode the last key returned.

---

### ListObjectVersions

Lists all versions of objects in a bucket.

```
GET /{bucket}?versions HTTP/1.1
```

**Query Parameters**:

| Param | Description |
|---|---|
| `prefix` | Filter by prefix |
| `delimiter` | Group by delimiter |
| `max-keys` | Max versions to return (default 1000) |
| `key-marker` | Start after this key |
| `version-id-marker` | Start after this version |

**Response**: `200 OK`
```xml
<ListVersionsResult>
  <Name>my-bucket</Name>
  <Prefix/>
  <MaxKeys>1000</MaxKeys>
  <IsTruncated>false</IsTruncated>
  <Version>
    <Key>photos/cat.jpg</Key>
    <VersionId>version-uuid</VersionId>
    <IsLatest>true</IsLatest>
    <LastModified>2026-01-01T00:00:00.000Z</LastModified>
    <ETag>"etag"</ETag>
    <Size>1048576</Size>
  </Version>
  <DeleteMarker>
    <Key>photos/old.jpg</Key>
    <VersionId>version-uuid</VersionId>
    <IsLatest>true</IsLatest>
    <LastModified>2026-01-01T00:00:00.000Z</LastModified>
  </DeleteMarker>
</ListVersionsResult>
```

**Implementation**: `SELECT * FROM rux.objects WHERE bucket = ? AND key = ?` returns all versions (clustered DESC by version_id).

---

### GetObjectAttributes

Returns selected attributes of an object.

```
GET /{bucket}/{key}?attributes HTTP/1.1
x-amz-object-attributes: ETag,ObjectSize,StorageClass
```

**Response**: `200 OK`
```xml
<GetObjectAttributesResponse>
  <ETag>etag</ETag>
  <ObjectSize>1048576</ObjectSize>
  <StorageClass>STANDARD</StorageClass>
</GetObjectAttributesResponse>
```

---

## Multipart Upload Operations

### CreateMultipartUpload

Initiates a multipart upload.

```
POST /{bucket}/{key}?uploads HTTP/1.1
```

**Request Headers**:

| Header | Description |
|---|---|
| `Content-Type` | MIME type for the completed object |
| `x-amz-meta-*` | User-defined metadata for the completed object |

**Response**: `200 OK`
```xml
<InitiateMultipartUploadResult>
  <Bucket>my-bucket</Bucket>
  <Key>large-file.zip</Key>
  <UploadId>upload-uuid</UploadId>
</InitiateMultipartUploadResult>
```

**Implementation**: `INSERT INTO rux.multipart_uploads (bucket, key, upload_id, status, ...) VALUES (?, ?, ?, 'in_progress', ...)`.

---

### UploadPart

Uploads a part for a multipart upload.

```
PUT /{bucket}/{key}?partNumber={n}&uploadId={id} HTTP/1.1
Content-Length: 5242880
```

**Constraints**:
- Part numbers: 1 to 10000.
- Minimum part size: 5 MB (except the last part).
- Maximum part size: 5 GB.

**Response**: `200 OK`

| Header | Description |
|---|---|
| `ETag` | MD5 of the part data, quoted |

**Implementation**: `INSERT INTO rux.multipart_parts (bucket, upload_id, part_number, etag, size, data) VALUES (?, ?, ?, ?, ?, ?)`. Parts larger than 1 MB are chunked into multiple rows internally.

---

### CompleteMultipartUpload

Completes a multipart upload by assembling parts into a single object.

```
POST /{bucket}/{key}?uploadId={id} HTTP/1.1
```

**Request Body**:
```xml
<CompleteMultipartUpload>
  <Part>
    <PartNumber>1</PartNumber>
    <ETag>"part-etag-1"</ETag>
  </Part>
  <Part>
    <PartNumber>2</PartNumber>
    <ETag>"part-etag-2"</ETag>
  </Part>
</CompleteMultipartUpload>
```

**Response**: `200 OK`
```xml
<CompleteMultipartUploadResult>
  <Location>https://s3.example.com/my-bucket/large-file.zip</Location>
  <Bucket>my-bucket</Bucket>
  <Key>large-file.zip</Key>
  <ETag>"composite-etag"</ETag>
</CompleteMultipartUploadResult>
```

**ETag format**: For multipart uploads, the ETag is `MD5(concat(part_md5s))-{part_count}` (e.g., `"a1b2c3d4-3"`).

**Implementation**:
1. Validate all parts exist and ETags match.
2. Read parts from `rux.multipart_parts` in order.
3. Re-chunk part data into 1 MB chunks.
4. Write chunks to `rux.chunks`.
5. Write metadata to `rux.objects`.
6. Write listing to `rux.listings`.
7. Delete parts from `rux.multipart_parts`.
8. Update `rux.multipart_uploads` status to `completed`.

---

### AbortMultipartUpload

Cancels a multipart upload and deletes uploaded parts.

```
DELETE /{bucket}/{key}?uploadId={id} HTTP/1.1
```

**Response**: `204 No Content`

**Implementation**: Delete all rows from `rux.multipart_parts` for the upload_id, update status to `aborted` in `rux.multipart_uploads`.

---

### ListMultipartUploads

Lists in-progress multipart uploads for a bucket.

```
GET /{bucket}?uploads HTTP/1.1
```

**Query Parameters**:

| Param | Description |
|---|---|
| `prefix` | Filter by key prefix |
| `delimiter` | Group by delimiter |
| `max-uploads` | Max uploads to return (default 1000) |
| `key-marker` | Start after this key |
| `upload-id-marker` | Start after this upload ID |

**Response**: `200 OK`
```xml
<ListMultipartUploadsResult>
  <Bucket>my-bucket</Bucket>
  <Upload>
    <Key>large-file.zip</Key>
    <UploadId>upload-uuid</UploadId>
    <Initiated>2026-01-01T00:00:00.000Z</Initiated>
  </Upload>
</ListMultipartUploadsResult>
```

---

### ListParts

Lists uploaded parts for a multipart upload.

```
GET /{bucket}/{key}?uploadId={id} HTTP/1.1
```

**Query Parameters**:

| Param | Description |
|---|---|
| `max-parts` | Max parts to return (default 1000) |
| `part-number-marker` | Start after this part number |

**Response**: `200 OK`
```xml
<ListPartsResult>
  <Bucket>my-bucket</Bucket>
  <Key>large-file.zip</Key>
  <UploadId>upload-uuid</UploadId>
  <Part>
    <PartNumber>1</PartNumber>
    <ETag>"part-etag"</ETag>
    <Size>5242880</Size>
    <LastModified>2026-01-01T00:00:00.000Z</LastModified>
  </Part>
</ListPartsResult>
```

---

## Error Responses

All errors follow the S3 XML error format:

```xml
<Error>
  <Code>NoSuchKey</Code>
  <Message>The specified key does not exist.</Message>
  <Key>photos/missing.jpg</Key>
  <RequestId>request-uuid</RequestId>
</Error>
```

### Error Codes

| Code | HTTP Status | Description |
|---|---|---|
| `AccessDenied` | 403 | Authentication failed or not authorized |
| `BucketAlreadyExists` | 409 | Bucket name is taken |
| `BucketAlreadyOwnedByYou` | 409 | Caller already owns this bucket |
| `BucketNotEmpty` | 409 | Bucket has objects, cannot delete |
| `EntityTooLarge` | 400 | Object exceeds maximum size |
| `EntityTooSmall` | 400 | Multipart part below minimum size |
| `IncompleteBody` | 400 | Content-Length doesn't match body |
| `InternalError` | 500 | Server error |
| `InvalidArgument` | 400 | Invalid request parameter |
| `InvalidBucketName` | 400 | Bucket name violates naming rules |
| `InvalidDigest` | 400 | Content-MD5 doesn't match |
| `InvalidPart` | 400 | Part not found during CompleteMultipartUpload |
| `InvalidPartOrder` | 400 | Parts not in ascending order |
| `InvalidRange` | 416 | Range header cannot be satisfied |
| `InvalidRequest` | 400 | Generic bad request |
| `MalformedXML` | 400 | Request body is not valid XML |
| `MethodNotAllowed` | 405 | HTTP method not supported for this resource |
| `NoSuchBucket` | 404 | Bucket does not exist |
| `NoSuchKey` | 404 | Object does not exist |
| `NoSuchUpload` | 404 | Multipart upload does not exist |
| `NoSuchVersion` | 404 | Version does not exist |
| `NotImplemented` | 501 | Operation not supported |
| `PreconditionFailed` | 412 | Conditional header not met |
| `ServiceUnavailable` | 503 | Server overloaded or unavailable |
| `SignatureDoesNotMatch` | 403 | Signature validation failed |
| `TooManyBuckets` | 400 | Account bucket limit exceeded |

## Limits

| Limit | Value |
|---|---|
| Maximum object size (single PUT) | 5 GB |
| Maximum object size (multipart) | 5 TB |
| Maximum part size | 5 GB |
| Minimum part size (except last) | 5 MB |
| Maximum parts per upload | 10,000 |
| Maximum metadata size per object | 2 KB |
| Maximum key length | 1,024 bytes |
| Maximum bucket name length | 63 characters |
| Maximum buckets per account | 1,000 |
| Maximum keys per ListObjectsV2 | 1,000 |

## Operations Not Implemented

The following S3 operations are out of scope for the initial release:

- `SelectObjectContent` — SQL-based object querying
- `PutObjectLockConfiguration` / `GetObjectLockConfiguration` — object lock / WORM
- `PutObjectRetention` / `GetObjectRetention` — retention policies
- `PutObjectLegalHold` / `GetObjectLegalHold` — legal hold
- `GetBucketAccelerateConfiguration` — transfer acceleration
- `GetBucketAnalyticsConfiguration` — storage analytics
- `GetBucketCors` / `PutBucketCors` — CORS configuration
- `GetBucketEncryption` / `PutBucketEncryption` — server-side encryption config
- `GetBucketIntelligentTieringConfiguration` — intelligent tiering
- `GetBucketInventoryConfiguration` — inventory reports
- `GetBucketLogging` / `PutBucketLogging` — access logging
- `GetBucketMetricsConfiguration` — CloudWatch metrics
- `GetBucketNotificationConfiguration` — event notifications
- `GetBucketOwnershipControls` — ownership controls
- `GetBucketReplication` / `PutBucketReplication` — cross-region replication config
- `GetBucketRequestPayment` — requester pays
- `GetBucketTagging` / `PutBucketTagging` — bucket tags
- `GetBucketWebsite` / `PutBucketWebsite` — static website hosting
- `GetObjectTorrent` — BitTorrent
- `RestoreObject` — Glacier restore
- Batch operations (`DeleteObjects`, `S3 Batch Operations`)

These may be added in future releases based on demand.
