/**
 * API Client for OffchainVM Coordinator
 */

import axios from 'axios';

const API_BASE_URL = process.env.NEXT_PUBLIC_COORDINATOR_API_URL || 'http://localhost:8080';

export interface WorkerInfo {
  worker_id: string;
  worker_name: string;
  status: string;
  current_task_id: number | null;
  last_heartbeat_at: string;
  total_tasks_completed: number;
  total_tasks_failed: number;
  uptime_seconds: number | null;
}

export interface ExecutionHistoryEntry {
  id: number;
  request_id: number;
  data_id: string | null;
  worker_id: string;
  success: boolean;
  execution_time_ms: number;
  instructions_used: number | null;
  resolve_tx_id: string | null;
  user_account_id: string | null;
  near_payment_yocto: string | null;
  github_repo: string | null;
  github_commit: string | null;
  created_at: string;
}

export interface ExecutionStats {
  total_executions: number;
  successful_executions: number;
  failed_executions: number;
  total_instructions_used: number;
  average_execution_time_ms: number;
  total_near_paid_yocto: string;
  unique_users: number;
  active_workers: number;
}

export interface WasmInfo {
  exists: boolean;
  checksum: string | null;
  file_size: number | null;
  created_at: string | null;
}

export interface UserEarnings {
  user_account_id: string;
  total_executions: number;
  successful_executions: number;
  total_near_spent_yocto: string;
  total_instructions_used: number;
  average_execution_time_ms: number;
}

/**
 * Fetch list of workers
 */
export async function fetchWorkers(): Promise<WorkerInfo[]> {
  const response = await axios.get(`${API_BASE_URL}/public/workers`);
  return response.data;
}

/**
 * Fetch execution history
 */
export async function fetchExecutions(
  limit: number = 50,
  offset: number = 0,
  userAccountId?: string
): Promise<ExecutionHistoryEntry[]> {
  const params: any = { limit, offset };
  if (userAccountId) {
    params.user_account_id = userAccountId;
  }
  const response = await axios.get(`${API_BASE_URL}/public/executions`, { params });
  return response.data;
}

/**
 * Fetch system statistics
 */
export async function fetchStats(): Promise<ExecutionStats> {
  const response = await axios.get(`${API_BASE_URL}/public/stats`);
  return response.data;
}

/**
 * Check if WASM exists for repo/commit/target
 */
export async function checkWasmExists(
  repoUrl: string,
  commitHash: string,
  buildTarget: string = 'wasm32-wasip1'
): Promise<WasmInfo> {
  const response = await axios.get(`${API_BASE_URL}/public/wasm/info`, {
    params: {
      repo_url: repoUrl,
      commit_hash: commitHash,
      build_target: buildTarget,
    },
  });
  return response.data;
}

/**
 * Fetch user earnings
 */
export async function fetchUserEarnings(userAccountId: string): Promise<UserEarnings> {
  const response = await axios.get(`${API_BASE_URL}/public/users/${userAccountId}/earnings`);
  return response.data;
}
