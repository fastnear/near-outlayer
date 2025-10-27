import { AccessCondition } from './types';

// Format access condition for display
export function formatAccessCondition(access: unknown): string {
  if (typeof access === 'string' && access === 'AllowAll') {
    return '🌍 Allow all accounts';
  }

  if (typeof access === 'object' && access !== null) {
    const obj = access as Record<string, unknown>;

    if (obj.Whitelist && typeof obj.Whitelist === 'object' && obj.Whitelist !== null) {
      const whitelist = obj.Whitelist as { accounts: string[] };
      const accounts = whitelist.accounts.slice(0, 2).join(', ');
      const more = whitelist.accounts.length > 2 ? `... +${whitelist.accounts.length - 2}` : '';
      return `👥 Whitelist: ${accounts}${more}`;
    }
    if (obj.AccountPattern && typeof obj.AccountPattern === 'object' && obj.AccountPattern !== null) {
      const pattern = obj.AccountPattern as { pattern: string };
      return `🔍 Pattern: ${pattern.pattern}`;
    }
    if (obj.NearBalance && typeof obj.NearBalance === 'object' && obj.NearBalance !== null) {
      const nearBalance = obj.NearBalance as { value: string; operator: string };
      const nearAmount = (BigInt(nearBalance.value) / BigInt(1e24)).toString();
      return `💰 NEAR balance ${nearBalance.operator} ${nearAmount} NEAR`;
    }
    if (obj.FtBalance && typeof obj.FtBalance === 'object' && obj.FtBalance !== null) {
      const ftBalance = obj.FtBalance as { contract: string; operator: string; value: string };
      return `🪙 FT ${ftBalance.contract}: balance ${ftBalance.operator} ${ftBalance.value}`;
    }
    if (obj.NftOwned && typeof obj.NftOwned === 'object' && obj.NftOwned !== null) {
      const nftOwned = obj.NftOwned as { contract: string; token_id?: string | null };
      if (nftOwned.token_id) {
        return `🖼️ Owns NFT ${nftOwned.contract}#${nftOwned.token_id}`;
      }
      return `🖼️ Owns NFT from ${nftOwned.contract}`;
    }
    if (obj.Logic && typeof obj.Logic === 'object' && obj.Logic !== null) {
      const logic = obj.Logic as { operator: string; conditions: unknown[] };
      return `🔗 ${logic.operator}: ${logic.conditions.length} conditions`;
    }
    if (obj.Not) {
      return `🚫 NOT: (nested condition)`;
    }
  }

  return 'Unknown condition';
}

// Convert UI access condition to contract format (PascalCase enums)
export function convertAccessToContractFormat(access: AccessCondition): unknown {
  switch (access.type) {
    case 'AllowAll':
      return 'AllowAll';
    case 'Whitelist':
      return { Whitelist: { accounts: access.accounts } };
    case 'AccountPattern':
      return { AccountPattern: { pattern: access.pattern } };
    case 'NearBalance':
      return { NearBalance: { operator: access.operator, value: access.value } };
    case 'FtBalance':
      return { FtBalance: { contract: access.contract, operator: access.operator, value: access.value } };
    case 'NftOwned':
      return { NftOwned: { contract: access.contract, token_id: access.token_id } };
    case 'Logic':
      return {
        Logic: {
          operator: access.operator,
          conditions: access.conditions.map(cond => convertAccessToContractFormat(cond))
        }
      };
    case 'Not':
      return { Not: { condition: convertAccessToContractFormat(access.condition) } };
    default:
      return 'AllowAll';
  }
}
