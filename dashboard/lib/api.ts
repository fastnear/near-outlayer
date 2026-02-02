/**
 * API Client for OffchainVM Coordinator
 */

import axios from 'axios';

export type NetworkType = 'testnet' | 'mainnet';

/**
 * Get Coordinator API base URL for the given network
 */
export function getCoordinatorApiUrl(network?: NetworkType): string {
  // Try to get network from localStorage if not provided
  let currentNetwork = network;
  if (!currentNetwork && typeof window !== 'undefined') {
    const stored = localStorage.getItem('near-wallet-selector:selectedNetworkId');
    if (stored === 'testnet' || stored === 'mainnet') {
      currentNetwork = stored;
    }
  }

  // Fallback to default network from env
  if (!currentNetwork) {
    currentNetwork = (process.env.NEXT_PUBLIC_DEFAULT_NETWORK || 'testnet') as NetworkType;
  }

  if (currentNetwork === 'mainnet') {
    return process.env.NEXT_PUBLIC_MAINNET_COORDINATOR_API_URL || 'https://api.outlayer.near.org';
  }

  return process.env.NEXT_PUBLIC_TESTNET_COORDINATOR_API_URL || 'http://localhost:8080';
}

const API_BASE_URL = getCoordinatorApiUrl();

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

export interface JobHistoryEntry {
  id: number;
  job_id: number | null;
  request_id: number;
  data_id: string | null;
  worker_id: string;
  success: boolean;
  status: string | null; // job status (completed, failed, access_denied, etc.)
  error_details: string | null; // detailed error message
  job_type: string | null;
  execution_time_ms: number | null;
  compile_time_ms: number | null;
  instructions_used: number | null;
  resolve_tx_id: string | null;
  user_account_id: string | null;
  near_payment_yocto: string | null;
  actual_cost_yocto: string | null;
  compile_cost_yocto: string | null;
  github_repo: string | null;
  github_commit: string | null;
  transaction_hash: string | null;
  created_at: string;
  // HTTPS call fields
  is_https_call: boolean;
  call_id: string | null;
  // Project info
  project_id: string | null;
  // HTTPS cost in USD (stablecoin minimal units, 6 decimals)
  compute_cost_usd: string | null;
}

export interface ExecutionStats {
  total_executions: number;
  successful_executions: number;
  failed_executions: number; // Infrastructure errors only
  access_denied_executions: number;
  compilation_failed_executions: number;
  execution_failed_executions: number;
  insufficient_payment_executions: number;
  custom_executions: number;
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

export interface PopularRepo {
  github_repo: string;
  total_executions: number;
  successful_executions: number;
  failed_executions: number; // Infrastructure errors only
  access_denied_executions: number;
  compilation_failed_executions: number;
  execution_failed_executions: number;
  insufficient_payment_executions: number;
  custom_executions: number;
  last_commit: string | null;
}

export interface PricingConfig {
  // NEAR pricing (for blockchain transactions)
  base_fee: string;
  per_instruction_fee: string;
  per_ms_fee: string;
  per_compile_ms_fee: string;
  // USD pricing (for Payment Keys / HTTPS API)
  base_fee_usd: string;
  per_instruction_fee_usd: string;
  per_sec_fee_usd: string;
  per_compile_ms_fee_usd: string;
  // Limits
  max_compilation_seconds: number;
  max_instructions: number;
  max_execution_seconds: number;
}

/**
 * Fetch list of workers
 */
export async function fetchWorkers(): Promise<WorkerInfo[]> {
  const response = await axios.get(`${API_BASE_URL}/public/workers`);
  return response.data;
}

/**
 * Fetch job history
 * @param source - Filter by source: "near", "https", or undefined for all
 */
export async function fetchJobs(
  limit: number = 50,
  offset: number = 0,
  userAccountId?: string,
  source?: 'near' | 'https'
): Promise<JobHistoryEntry[]> {
  const params: Record<string, string | number> = { limit, offset };
  if (userAccountId) {
    params.user_account_id = userAccountId;
  }
  if (source) {
    params.source = source;
  }
  const response = await axios.get(`${API_BASE_URL}/public/jobs`, { params });
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
 * Check if WASM exists by checksum (SHA256 hash)
 */
export async function checkWasmExistsByChecksum(
  checksum: string
): Promise<WasmInfo> {
  try {
    const response = await axios.get(`${API_BASE_URL}/public/wasm/exists/${checksum}`);
    return {
      exists: response.data.exists,
      checksum: checksum,
      file_size: response.data.file_size || null,
      created_at: response.data.created_at || null,
    };
  } catch (error) {
    // If 404 or other error, WASM doesn't exist
    return {
      exists: false,
      checksum: checksum,
      file_size: null,
      created_at: null,
    };
  }
}

/**
 * Fetch user earnings
 */
export async function fetchUserEarnings(userAccountId: string): Promise<UserEarnings> {
  const response = await axios.get(`${API_BASE_URL}/public/users/${userAccountId}/earnings`);
  return response.data;
}

/**
 * Fetch popular repositories
 */
export async function fetchPopularRepos(): Promise<PopularRepo[]> {
  const response = await axios.get(`${API_BASE_URL}/public/repos/popular`);
  return response.data;
}

/**
 * Fetch pricing configuration
 */
export async function fetchPricing(): Promise<PricingConfig> {
  const response = await axios.get(`${API_BASE_URL}/public/pricing`);
  return response.data;
}

/**
 * API Key Management
 */
export interface CreateApiKeyResponse {
  api_key: string;
  near_account_id: string;
  rate_limit_per_minute: number;
  created_at: number;
}

export interface CreateApiKeyRequest {
  near_account_id: string;
  key_name: string;
  rate_limit_per_minute?: number;
}

/**
 * Create API key (public endpoint - no auth required)
 */
export async function createApiKey(
  request: CreateApiKeyRequest
): Promise<CreateApiKeyResponse> {
  const baseUrl = getCoordinatorApiUrl();
  const response = await axios.post(
    `${baseUrl}/public/api-keys`,
    request
  );
  return response.data;
}

/**
 * Attestation data types
 */
export interface AttestationResponse {
  id: number;
  task_id: number;
  task_type: string;

  // TDX attestation data
  tdx_quote: string; // base64 encoded
  worker_measurement: string;

  // NEAR context (for blockchain calls)
  request_id?: number;
  caller_account_id?: string;
  transaction_hash?: string;
  block_height?: number;

  // HTTPS context (for HTTPS API calls)
  call_id?: string;
  payment_key_owner?: string;
  payment_key_nonce?: number;

  // Code source
  repo_url?: string;
  commit_hash?: string;
  build_target?: string;

  // Task data hashes
  wasm_hash?: string;
  input_hash?: string;
  output_hash: string;

  // V1 attestation fields (for jobs after OUTLAYER_ATTESTATION_V1)
  project_id?: string;
  secrets_ref?: string;
  attached_usd?: string;

  timestamp: number; // Unix timestamp
}

/**
 * Fetch attestation for a specific task by job ID (public endpoint)
 * Returns null if attestation doesn't exist
 */
export async function fetchAttestation(
  taskId: number
): Promise<AttestationResponse | null> {
  try {
    const response = await axios.get(
      `${API_BASE_URL}/attestations/${taskId}`
    );
    return response.data;
  } catch (error) {
    if (axios.isAxiosError(error) && error.response?.status === 404) {
      return null; // Attestation doesn't exist
    }
    throw error;
  }
}

