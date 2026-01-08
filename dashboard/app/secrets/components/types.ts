// Access condition types matching contract
export type LogicOperator = 'And' | 'Or';
export type ComparisonOperator = 'Gte' | 'Lte' | 'Gt' | 'Lt' | 'Eq' | 'Ne';

export type AccessCondition =
  | { type: 'AllowAll' }
  | { type: 'Whitelist'; accounts: string[] }
  | { type: 'AccountPattern'; pattern: string }
  | { type: 'NearBalance'; operator: ComparisonOperator; value: string }
  | { type: 'FtBalance'; contract: string; operator: ComparisonOperator; value: string }
  | { type: 'NftOwned'; contract: string; token_id: string | null }
  | { type: 'DaoMember'; dao_contract: string; role: string }
  | { type: 'Logic'; operator: LogicOperator; conditions: AccessCondition[] }
  | { type: 'Not'; condition: AccessCondition };

// Secret accessor - defines what code can access/decrypt the secret
export type SecretAccessor =
  | { Repo: { repo: string; branch: string | null } }
  | { WasmHash: { hash: string } }
  | { Project: { project_id: string } };

export interface UserSecret {
  accessor: SecretAccessor;
  profile: string;
  created_at: number;
  updated_at: number;
  storage_deposit: string;
  access: unknown; // Contract format (PascalCase)
}

// Form data for creating secrets
export type SecretSourceType = 'repo' | 'wasm_hash' | 'project';

export interface FormData {
  sourceType: SecretSourceType;
  // Repo-based fields
  repo: string;
  branch: string | null;
  // WasmHash-based fields
  wasmHash: string;
  // Project-based fields
  projectId: string;
  // Common fields
  profile: string;
  access: unknown; // Contract format
}

// Helper functions for SecretAccessor
export function isRepoAccessor(accessor: SecretAccessor): accessor is { Repo: { repo: string; branch: string | null } } {
  return 'Repo' in accessor;
}

export function isWasmHashAccessor(accessor: SecretAccessor): accessor is { WasmHash: { hash: string } } {
  return 'WasmHash' in accessor;
}

export function isProjectAccessor(accessor: SecretAccessor): accessor is { Project: { project_id: string } } {
  return 'Project' in accessor;
}

export function getAccessorLabel(accessor: SecretAccessor): string {
  if (isRepoAccessor(accessor)) {
    const { repo, branch } = accessor.Repo;
    return branch ? `${repo}@${branch}` : repo;
  } else if (isWasmHashAccessor(accessor)) {
    const hash = accessor.WasmHash.hash;
    return `WASM: ${hash.substring(0, 8)}...${hash.substring(hash.length - 8)}`;
  } else if (isProjectAccessor(accessor)) {
    return `Project: ${accessor.Project.project_id}`;
  }
  return 'Unknown';
}
