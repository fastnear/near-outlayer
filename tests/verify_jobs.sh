#!/bin/bash
# Job Database Verification Script
#
# This script verifies the job-based workflow by directly querying
# the PostgreSQL database to check job states and timing.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

echo ""
echo "🔍 Job Database Verification"
echo "============================"
echo ""

# Configuration
DB_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost/offchainvm}"

# Check prerequisites
if ! command -v psql &> /dev/null; then
    echo -e "${RED}❌ psql not found${NC}"
    echo "   Install PostgreSQL client"
    exit 1
fi

echo "📊 Connecting to database..."
echo "  URL: $DB_URL"
echo ""

# Test database connection
if ! psql "$DB_URL" -c "SELECT 1;" > /dev/null 2>&1; then
    echo -e "${RED}❌ Database connection failed${NC}"
    echo "   Make sure PostgreSQL is running: docker-compose up -d"
    exit 1
fi
echo -e "${GREEN}✓ Database connected${NC}"
echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Jobs Overview${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

# Count jobs by type and status
echo "📊 Job Statistics:"
echo ""

psql "$DB_URL" -c "
SELECT
    job_type,
    status,
    COUNT(*) as count,
    COUNT(DISTINCT request_id) as unique_requests
FROM jobs
GROUP BY job_type, status
ORDER BY job_type, status;
" 2>/dev/null

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Recent Jobs (Last 10)${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

psql "$DB_URL" -c "
SELECT
    job_id,
    request_id,
    LEFT(data_id, 20) as data_id_preview,
    job_type,
    status,
    worker_id,
    LEFT(wasm_checksum, 16) as wasm_preview,
    EXTRACT(EPOCH FROM (completed_at - created_at))::integer as duration_sec,
    created_at
FROM jobs
ORDER BY job_id DESC
LIMIT 10;
" 2>/dev/null

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Compilation Jobs (with timing)${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

psql "$DB_URL" -c "
SELECT
    j.job_id,
    j.request_id,
    j.worker_id,
    j.status,
    LEFT(j.wasm_checksum, 16) as wasm_checksum,
    eh.time_ms as compile_time_ms,
    ROUND(eh.time_ms::numeric / 1000, 2) as compile_time_sec,
    j.created_at
FROM jobs j
LEFT JOIN execution_history eh ON j.job_id = eh.job_id
WHERE j.job_type = 'compile'
ORDER BY j.job_id DESC
LIMIT 10;
" 2>/dev/null

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Execution Jobs (with metrics)${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

psql "$DB_URL" -c "
SELECT
    j.job_id,
    j.request_id,
    j.worker_id,
    j.status,
    eh.time_ms as exec_time_ms,
    eh.instructions,
    ROUND(eh.instructions::numeric / 1000000, 2) as instructions_millions,
    j.created_at
FROM jobs j
LEFT JOIN execution_history eh ON j.job_id = eh.job_id
WHERE j.job_type = 'execute'
ORDER BY j.job_id DESC
LIMIT 10;
" 2>/dev/null

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Requests with Multiple Jobs${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

echo "Requests that triggered both compile and execute:"
echo ""

psql "$DB_URL" -c "
SELECT
    request_id,
    COUNT(*) as job_count,
    STRING_AGG(job_type || '(' || status || ')', ', ') as jobs,
    MIN(created_at) as first_job_created
FROM jobs
GROUP BY request_id
HAVING COUNT(*) > 1
ORDER BY request_id DESC
LIMIT 10;
" 2>/dev/null

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}WASM Cache Effectiveness${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

echo "WASM reuse (same checksum used multiple times):"
echo ""

psql "$DB_URL" -c "
SELECT
    wasm_checksum,
    LEFT(wasm_checksum, 16) as checksum_preview,
    COUNT(*) as times_used,
    MIN(created_at) as first_used,
    MAX(created_at) as last_used
FROM jobs
WHERE job_type = 'execute' AND wasm_checksum IS NOT NULL
GROUP BY wasm_checksum
HAVING COUNT(*) > 1
ORDER BY times_used DESC
LIMIT 10;
" 2>/dev/null

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Worker Performance${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

echo "Jobs completed by each worker:"
echo ""

psql "$DB_URL" -c "
SELECT
    worker_id,
    job_type,
    COUNT(*) as jobs_completed,
    ROUND(AVG(EXTRACT(EPOCH FROM (completed_at - created_at)))::numeric, 2) as avg_duration_sec,
    MIN(created_at) as first_job,
    MAX(completed_at) as last_job
FROM jobs
WHERE status = 'completed'
GROUP BY worker_id, job_type
ORDER BY worker_id, job_type;
" 2>/dev/null

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Failed Jobs${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

FAILED_COUNT=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM jobs WHERE status = 'failed';" 2>/dev/null | xargs)

if [ "$FAILED_COUNT" -gt 0 ]; then
    echo -e "${RED}⚠️  Found $FAILED_COUNT failed job(s)${NC}"
    echo ""

    psql "$DB_URL" -c "
    SELECT
        job_id,
        request_id,
        job_type,
        worker_id,
        created_at,
        completed_at
    FROM jobs
    WHERE status = 'failed'
    ORDER BY job_id DESC
    LIMIT 10;
    " 2>/dev/null
else
    echo -e "${GREEN}✓ No failed jobs${NC}"
fi

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Pending/In-Progress Jobs${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

PENDING_COUNT=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM jobs WHERE status IN ('pending', 'in_progress');" 2>/dev/null | xargs)

if [ "$PENDING_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}⏳ Found $PENDING_COUNT pending/in-progress job(s)${NC}"
    echo ""

    psql "$DB_URL" -c "
    SELECT
        job_id,
        request_id,
        job_type,
        status,
        worker_id,
        created_at,
        EXTRACT(EPOCH FROM (NOW() - created_at))::integer as age_seconds
    FROM jobs
    WHERE status IN ('pending', 'in_progress')
    ORDER BY created_at ASC;
    " 2>/dev/null
else
    echo -e "${GREEN}✓ No pending jobs${NC}"
fi

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Race Condition Detection${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

echo "Checking for duplicate jobs (should be NONE due to UNIQUE constraint):"
echo ""

DUPLICATE_COUNT=$(psql "$DB_URL" -t -c "
    SELECT COUNT(*)
    FROM (
        SELECT request_id, data_id, job_type, COUNT(*) as cnt
        FROM jobs
        GROUP BY request_id, data_id, job_type
        HAVING COUNT(*) > 1
    ) duplicates;
" 2>/dev/null | xargs)

if [ "$DUPLICATE_COUNT" -gt 0 ]; then
    echo -e "${RED}❌ Found $DUPLICATE_COUNT duplicate job(s) - UNIQUE constraint failed!${NC}"

    psql "$DB_URL" -c "
    SELECT request_id, data_id, job_type, COUNT(*) as duplicates
    FROM jobs
    GROUP BY request_id, data_id, job_type
    HAVING COUNT(*) > 1;
    " 2>/dev/null
else
    echo -e "${GREEN}✓ No duplicate jobs - UNIQUE constraint working correctly!${NC}"
fi

echo ""

echo "════════════════════════════════════════════════════════════════"
echo -e "${CYAN}Summary${NC}"
echo "════════════════════════════════════════════════════════════════"
echo ""

TOTAL_JOBS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM jobs;" 2>/dev/null | xargs)
COMPLETED_JOBS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM jobs WHERE status = 'completed';" 2>/dev/null | xargs)
TOTAL_REQUESTS=$(psql "$DB_URL" -t -c "SELECT COUNT(DISTINCT request_id) FROM jobs;" 2>/dev/null | xargs)

echo "📊 Overall Statistics:"
echo "  • Total jobs: $TOTAL_JOBS"
echo "  • Completed jobs: $COMPLETED_JOBS"
echo "  • Failed jobs: $FAILED_COUNT"
echo "  • Pending jobs: $PENDING_COUNT"
echo "  • Unique requests: $TOTAL_REQUESTS"
echo ""

# Calculate compile vs execute ratio
COMPILE_JOBS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM jobs WHERE job_type = 'compile';" 2>/dev/null | xargs)
EXECUTE_JOBS=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM jobs WHERE job_type = 'execute';" 2>/dev/null | xargs)

echo "🔨 Job Type Distribution:"
echo "  • Compile jobs: $COMPILE_JOBS"
echo "  • Execute jobs: $EXECUTE_JOBS"

if [ "$EXECUTE_JOBS" -gt 0 ] && [ "$COMPILE_JOBS" -gt 0 ]; then
    RATIO=$(echo "scale=2; $EXECUTE_JOBS / $COMPILE_JOBS" | bc)
    echo "  • Execute/Compile ratio: $RATIO"

    if (( $(echo "$RATIO > 1" | bc -l) )); then
        echo -e "  ${GREEN}✓ WASM cache is being utilized! (ratio > 1 means cache hits)${NC}"
    else
        echo -e "  ${YELLOW}⚠️  Every execution triggers compilation (no cache reuse)${NC}"
    fi
fi

echo ""

echo "📚 Next Steps:"
echo "  • Run job_workflow.sh to generate test data"
echo "  • Monitor worker logs for job processing"
echo "  • Check WASM cache: ls -lah /tmp/offchainvm/wasm/"
echo ""

echo "💡 Useful Queries:"
echo ""
echo "# View specific request:"
echo "psql $DB_URL -c \"SELECT * FROM jobs WHERE request_id = YOUR_REQUEST_ID;\""
echo ""
echo "# View execution history with job details:"
echo "psql $DB_URL -c \"SELECT j.*, eh.* FROM jobs j LEFT JOIN execution_history eh ON j.job_id = eh.job_id;\""
echo ""
echo "# Clear all jobs (for testing):"
echo "psql $DB_URL -c \"TRUNCATE jobs, execution_history CASCADE;\""
echo ""
