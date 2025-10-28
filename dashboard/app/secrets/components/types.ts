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

export interface UserSecret {
  repo: string;
  branch: string | null;
  profile: string;
  created_at: number;
  updated_at: number;
  storage_deposit: string;
  access: unknown; // Contract format (PascalCase)
}

export interface FormData {
  repo: string;
  branch: string | null;
  profile: string;
  access: unknown; // Contract format
}
