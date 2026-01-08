// TypeScript types for Projects page

export interface CodeSource {
  GitHub?: {
    repo: string;
    commit: string;
    build_target?: string | null;
  };
  WasmUrl?: {
    url: string;
    hash: string;
    build_target?: string | null;
  };
}

export interface ProjectView {
  uuid: string;
  owner: string;
  name: string;
  project_id: string;  // "owner.near/project-name"
  active_version: string;  // version_key: hash for WasmUrl, "repo@commit" for GitHub
  created_at: number;  // nanoseconds
  storage_deposit: string;  // U128 string
}

export interface VersionView {
  version_key: string;  // hash for WasmUrl, "repo@commit" for GitHub
  source: CodeSource;
  added_at: number;  // nanoseconds
  is_active: boolean;
}

// Type guards
export function isGitHubSource(source: CodeSource): source is { GitHub: NonNullable<CodeSource['GitHub']> } {
  return 'GitHub' in source && !!source.GitHub;
}

export function isWasmUrlSource(source: CodeSource): source is { WasmUrl: NonNullable<CodeSource['WasmUrl']> } {
  return 'WasmUrl' in source && !!source.WasmUrl;
}

// Form data for creating a new project
export interface CreateProjectFormData {
  name: string;
  sourceType: 'github' | 'wasm_url';
  // GitHub source
  repo: string;
  commit: string;
  buildTarget: string;
  // WasmUrl source
  wasmUrl: string;
  wasmHash: string;
}

// Form data for adding a version
export interface AddVersionFormData {
  projectName: string;
  sourceType: 'github' | 'wasm_url';
  repo: string;
  commit: string;
  buildTarget: string;
  wasmUrl: string;
  wasmHash: string;
  setActive: boolean;
}

// Utility functions
export function formatTimestamp(nanos: number): string {
  const date = new Date(nanos / 1_000_000);  // Convert nanoseconds to milliseconds
  return date.toLocaleString();
}

export function shortenHash(hash: string, length: number = 8): string {
  if (!hash || hash.length <= length * 2) return hash;
  return `${hash.substring(0, length)}...${hash.substring(hash.length - length)}`;
}

export function formatNearAmount(yoctoNear: string): string {
  const near = BigInt(yoctoNear) / BigInt(10 ** 24);
  const remainder = BigInt(yoctoNear) % BigInt(10 ** 24);
  const decimal = Number(remainder) / 10 ** 24;
  return `${near}.${decimal.toFixed(5).substring(2)} NEAR`;
}

// Normalize repo URL to consistent format (domain.com/owner/repo)
// Matches keystore-worker/src/utils.rs normalize_repo_url
export function normalizeRepoUrl(repo: string): string {
  let normalized = repo.trim();

  // Remove protocols
  if (normalized.startsWith('https://')) {
    normalized = normalized.slice(8);
  } else if (normalized.startsWith('http://')) {
    normalized = normalized.slice(7);
  } else if (normalized.startsWith('ssh://')) {
    normalized = normalized.slice(6);
  }

  // Handle git@ format (git@github.com:owner/repo or git@github.com/owner/repo)
  if (normalized.startsWith('git@')) {
    normalized = normalized.slice(4);
    // Replace : with / for git@github.com:owner/repo format
    normalized = normalized.replace(':', '/');
  }

  // Remove .git suffix if present
  if (normalized.endsWith('.git')) {
    normalized = normalized.slice(0, -4);
  }

  return normalized;
}

// Format version key for display
// - For WASM hash: returns as-is (e.g., "a1b2c3d4...")
// - For GitHub: normalizes and formats as "owner/repo@commit"
export function formatVersionKey(versionKey: string): string {
  if (!versionKey) return '';

  // Check if it's a GitHub version (contains @)
  const atIndex = versionKey.lastIndexOf('@');
  if (atIndex > 0) {
    const repo = versionKey.substring(0, atIndex);
    const commit = versionKey.substring(atIndex + 1);
    const normalized = normalizeRepoUrl(repo);
    // Extract owner/repo from github.com/owner/repo
    const parts = normalized.split('/');
    if (parts.length >= 3 && parts[0].includes('.')) {
      // Has domain prefix like github.com
      return `${parts.slice(1).join('/')}@${commit}`;
    }
    return `${normalized}@${commit}`;
  }

  // Not a GitHub version, return as-is (WASM hash)
  return versionKey;
}
