# Job-Based Workflow Implementation

## Overview

Refactored the NEAR Offshore system to use a **job-based workflow** that properly tracks compilation and execution as separate units of work, preventing race conditions when multiple workers process the same task.

## Problem Statement

**Before**: When multiple workers ran simultaneously, both would process the same request because:
- Each worker generated `request_id` locally in the event monitor
- No coordination between workers to claim exclusive ownership
- Compilation and execution were not tracked separately

**After**: Job-based architecture where:
- `request_id` and `data_id` come from the smart contract
- Coordinator assigns unique `job_id` for each unit of work
- Workers claim jobs atomically (first-come-first-served)
- Compilation time is tracked and charged separately

## Architecture Changes

### Database Schema

Created new `jobs` table with proper deduplication:

```sql
CREATE TABLE jobs (
    job_id BIGSERIAL PRIMARY KEY,
    request_id BIGINT NOT NULL,
    data_id TEXT NOT NULL,
    job_type TEXT NOT NULL CHECK (job_type IN ('compile', 'execute')),
    worker_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    wasm_checksum TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP,
    UNIQUE (request_id, data_id, job_type)  -- Prevents duplicate jobs
);
```

Key features:
- `UNIQUE` constraint on `(request_id, data_id, job_type)` prevents multiple workers from creating duplicate jobs
- `job_type` differentiates between compilation and execution
- `wasm_checksum` links execute job to compiled WASM

### API Changes

#### New Endpoints

**POST /jobs/claim** - Worker claims jobs for a task
```json
Request:
{
  "request_id": 50,
  "data_id": "base64...",
  "worker_id": "worker-123",
  "code_source": {...},
  "resource_limits": {...}
}

Response:
{
  "jobs": [
    {
      "job_id": 101,
      "job_type": "compile",
      "wasm_checksum": null,
      "allowed": true
    },
    {
      "job_id": 102,
      "job_type": "execute",
      "wasm_checksum": "abc123...",
      "allowed": true
    }
  ]
}
```

**Coordinator logic**:
1. Check if jobs already exist for this `(request_id, data_id)`
2. Calculate WASM checksum from code source
3. Check WASM cache:
   - If **cached**: return only `[execute job]`
   - If **not cached**: return `[compile job, execute job]`
4. Create jobs atomically with UNIQUE constraint

**POST /jobs/complete** - Worker completes a job
```json
{
  "job_id": 101,
  "success": true,
  "output": null,  // For execute jobs
  "error": null,
  "time_ms": 45000,  // Compilation took 45 seconds
  "instructions": 0,  // 0 for compile jobs
  "wasm_checksum": "abc123..."  // For compile jobs
}
```

#### Modified Endpoints

- **POST /tasks/create** - Still exists for event monitor (pushes to Redis queue)
- **GET /tasks/poll** - Still exists for long-polling (workers get tasks from Redis)
- **POST /tasks/complete** - DEPRECATED (kept for backward compatibility)
- **POST /tasks/fail** - DEPRECATED (kept for backward compatibility)

### Worker Workflow

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Worker polls Redis queue                                 │
│    GET /tasks/poll?timeout=60                               │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 2. Worker claims jobs for task                              │
│    POST /jobs/claim                                         │
│    Request: {request_id, data_id, worker_id, ...}          │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. Coordinator checks WASM cache and returns jobs           │
│    If WASM cached:     [execute]                            │
│    If WASM not cached: [compile, execute]                   │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 4. Worker processes compile job (if needed)                 │
│    - Clone GitHub repo                                       │
│    - Compile to WASM                                        │
│    - Upload to coordinator                                   │
│    - POST /jobs/complete {job_id, time_ms, wasm_checksum}  │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│ 5. Worker processes execute job                             │
│    - Download WASM from coordinator                          │
│    - Decrypt secrets (if provided)                          │
│    - Execute WASM with wasmi                                │
│    - Submit result to NEAR contract                         │
│    - POST /jobs/complete {job_id, time_ms, instructions}   │
└─────────────────────────────────────────────────────────────┘
```

### Key Implementation Details

#### Worker (main.rs)

**New handler functions**:
- `handle_compile_job()` - Compiles code and reports time
- `handle_execute_job()` - Executes WASM and submits to NEAR

**Worker iteration flow**:
```rust
// 1. Poll for task from Redis
let task = api_client.poll_task(timeout).await?;

// 2. Claim jobs for this task
let claim_response = api_client.claim_job(
    request_id, data_id, worker_id, &code_source, &resource_limits
).await?;

// 3. Process each job in order
for job in claim_response.jobs {
    match job.job_type {
        JobType::Compile => handle_compile_job(...).await?,
        JobType::Execute => handle_execute_job(...).await?,
    }
}
```

#### Coordinator (jobs.rs)

**claim_job handler**:
```rust
// Check if already claimed
let existing_jobs = sqlx::query!("SELECT * FROM jobs WHERE request_id = $1 AND data_id = $2")
    .fetch_all(&state.pool).await?;

if !existing_jobs.is_empty() {
    return Err(StatusCode::CONFLICT); // Another worker already claimed
}

// Calculate WASM checksum
let checksum = sha256(&format!("{:?}", code_source));

// Check if WASM exists in cache
let wasm_exists = check_wasm_cache(&checksum).await;

// Create jobs
let mut jobs = vec![];

if !wasm_exists {
    // Create compile job
    let compile_job = sqlx::query!(
        "INSERT INTO jobs (request_id, data_id, job_type, worker_id)
         VALUES ($1, $2, 'compile', $3) RETURNING job_id",
        request_id, data_id, worker_id
    ).fetch_one(&state.pool).await?;

    jobs.push(JobInfo {
        job_id: compile_job.job_id,
        job_type: JobType::Compile,
        wasm_checksum: None,
        allowed: true,
    });
}

// Always create execute job
let execute_job = sqlx::query!(
    "INSERT INTO jobs (request_id, data_id, job_type, worker_id, wasm_checksum)
     VALUES ($1, $2, 'execute', $3, $4) RETURNING job_id",
    request_id, data_id, worker_id, checksum
).fetch_one(&state.pool).await?;

jobs.push(JobInfo {
    job_id: execute_job.job_id,
    job_type: JobType::Execute,
    wasm_checksum: Some(checksum),
    allowed: true,
});

return Ok(Json(ClaimJobResponse { jobs }));
```

**complete_job handler**:
```rust
// Update job status
sqlx::query!(
    "UPDATE jobs SET status = $1, completed_at = NOW() WHERE job_id = $2",
    if success { "completed" } else { "failed" },
    job_id
).execute(&state.pool).await?;

// Save to execution_history
sqlx::query!(
    "INSERT INTO execution_history (job_id, worker_id, success, time_ms, instructions)
     VALUES ($1, $2, $3, $4, $5)",
    job_id, worker_id, success, time_ms, instructions
).execute(&state.pool).await?;

// Different handling for compile vs execute
if job_type == "compile" {
    // Store compilation time in separate table or add to jobs
    info!("Compilation completed: {}ms", time_ms);
} else {
    // Execution completed
    info!("Execution completed: {}ms, {} instructions", time_ms, instructions);
}
```

## Benefits

### 1. **No Race Conditions**
- UNIQUE constraint prevents duplicate jobs
- First worker to claim gets the work
- Other workers get CONFLICT status and move on

### 2. **Compilation Tracking**
- Separate `job_id` for compilation
- Track compile time in milliseconds
- Can charge for compilation separately

### 3. **WASM Cache Optimization**
- Coordinator checks cache before creating compile job
- If cached, skip compilation entirely
- Significant performance improvement for repeated executions

### 4. **Better Observability**
- Each job has unique ID
- Track which worker completed which job
- Separate metrics for compile vs execute

### 5. **Flexible Pricing**
- Charge for compilation time (ms-based, no instruction count)
- Charge for execution (instruction-based)
- Future: add memory-based pricing

## Testing

### Setup

1. **Start infrastructure**:
```bash
docker-compose up -d
```

2. **Apply migrations**:
```bash
sqlx migrate run --database-url postgres://postgres:postgres@localhost/offchainvm
```

3. **Build coordinator**:
```bash
cd coordinator
env SQLX_OFFLINE=true cargo build --release
./target/release/offchainvm-coordinator
```

4. **Run multiple workers**:
```bash
# Terminal 1
cd worker
WORKER_ID=worker-1 cargo run

# Terminal 2
cd worker
WORKER_ID=worker-2 cargo run
```

### Expected Behavior

1. Event monitor creates task in Redis queue
2. **Worker 1** polls and gets task
3. **Worker 1** calls `/jobs/claim` → gets `[compile, execute]`
4. **Worker 2** polls and gets same task (from event)
5. **Worker 2** calls `/jobs/claim` → gets `409 CONFLICT`
6. **Worker 2** moves on to next task
7. **Worker 1** completes compile job → uploads WASM
8. **Worker 1** completes execute job → submits to NEAR

### Logs to Verify

**Coordinator**:
```
INFO Creating compile job for request_id=50
INFO Creating execute job for request_id=50
INFO Worker worker-1 claimed 2 jobs
INFO Worker worker-2 attempted to claim already-claimed task
```

**Worker 1**:
```
🎯 Claiming jobs for request_id=50
✅ Claimed 2 job(s) for request_id=50
🔨 Starting compilation job_id=101
✅ Compilation successful: checksum=abc123 time=45000ms
⚙️ Starting execution job_id=102
✅ Execution successful: time=1234ms instructions=5000000
```

**Worker 2**:
```
🎯 Claiming jobs for request_id=50
⚠️ Failed to claim job (likely already claimed): Task already claimed by another worker
```

## Future Improvements

1. **Contract Updates**
   - Add `compile_time_ms` to `ResourceMetrics`
   - Add compilation pricing to `Pricing` struct
   - Charge for compilation in `resolve_execution`

2. **Job Timeouts**
   - Add `timeout_at` field to jobs table
   - Background task to reassign stuck jobs
   - Allow other workers to claim timed-out jobs

3. **Job Priorities**
   - Add `priority` field to jobs
   - Workers process high-priority jobs first
   - Premium users get faster processing

4. **Advanced Caching**
   - Cache entire execution results for deterministic code
   - Skip re-execution if input and code haven't changed
   - Instant results from cache

## Migration Path

### Phase 1: ✅ COMPLETE
- Implement job-based workflow in coordinator
- Update worker to use new endpoints
- Keep old endpoints for backward compatibility

### Phase 2: Testing (CURRENT)
- Run multiple workers simultaneously
- Verify no duplicate work
- Monitor logs for race conditions

### Phase 3: Contract Update
- Add compilation pricing to smart contract
- Update pricing calculations
- Deploy to testnet

### Phase 4: Cleanup
- Remove old handler functions (handle_compile_task, handle_execute_task)
- Remove deprecated endpoints (/tasks/complete, /tasks/fail)
- Remove old execution_requests table

## Files Changed

### Coordinator
- `migrations/20251017000004_job_based_workflow.sql` - New schema
- `src/models.rs` - Added JobType, JobInfo, ClaimJobRequest, ClaimJobResponse
- `src/handlers/jobs.rs` - New handlers (claim_job, complete_job)
- `src/handlers/tasks.rs` - Simplified (only poll and create)
- `src/main.rs` - Added job routes with auth

### Worker
- `src/api_client.rs` - Added claim_job() and complete_job() methods
- `src/main.rs` - New worker_iteration, handle_compile_job, handle_execute_job
- Old handlers kept for reference (will be removed after testing)

## Summary

This refactoring solves the critical race condition problem while adding valuable features like compilation tracking and WASM cache optimization. The job-based architecture is more scalable and provides better visibility into the system's operation.

**Status**: ✅ Implementation complete, ready for testing with multiple workers.
