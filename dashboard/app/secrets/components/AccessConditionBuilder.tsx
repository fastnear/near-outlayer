'use client';

import { useState } from 'react';
import { AccessCondition, ComparisonOperator } from './types';

interface AccessConditionBuilderProps {
  onChange: (condition: AccessCondition) => void;
}

export function AccessConditionBuilder({ onChange }: AccessConditionBuilderProps) {
  const [conditionType, setConditionType] = useState<string>('AllowAll');
  const [whitelistAccounts, setWhitelistAccounts] = useState('');
  const [accountPattern, setAccountPattern] = useState('');
  const [nearBalanceValue, setNearBalanceValue] = useState('1');
  const [nearBalanceOperator, setNearBalanceOperator] = useState<ComparisonOperator>('Gte');
  const [ftContract, setFtContract] = useState('');
  const [ftBalanceValue, setFtBalanceValue] = useState('');
  const [ftBalanceOperator, setFtBalanceOperator] = useState<ComparisonOperator>('Gte');
  const [nftContract, setNftContract] = useState('');

  const buildCondition = (): AccessCondition => {
    switch (conditionType) {
      case 'AllowAll':
        return { type: 'AllowAll' };
      case 'Whitelist':
        return {
          type: 'Whitelist',
          accounts: whitelistAccounts.split(',').map(a => a.trim()).filter(a => a.length > 0),
        };
      case 'AccountPattern':
        return { type: 'AccountPattern', pattern: accountPattern };
      case 'NearBalance':
        return {
          type: 'NearBalance',
          operator: nearBalanceOperator,
          value: (BigInt(parseFloat(nearBalanceValue) * 1e24)).toString(),
        };
      case 'FtBalance':
        return {
          type: 'FtBalance',
          contract: ftContract,
          operator: ftBalanceOperator,
          value: ftBalanceValue,
        };
      case 'NftOwned':
        return { type: 'NftOwned', contract: nftContract };
      default:
        return { type: 'AllowAll' };
    }
  };

  const handleChange = () => {
    onChange(buildCondition());
  };

  const handleTypeChange = (newType: string) => {
    setConditionType(newType);
    // Build immediately with new type
    setTimeout(() => onChange(buildCondition()), 0);
  };

  return (
    <div>
      {/* Condition Type Selector */}
      <div className="mb-4">
        <label htmlFor="conditionType" className="block text-sm font-medium text-gray-700">
          Access Condition
        </label>
        <select
          id="conditionType"
          value={conditionType}
          onChange={(e) => handleTypeChange(e.target.value)}
          className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
        >
          <option value="AllowAll">🌍 Allow All (no restrictions)</option>
          <option value="Whitelist">👥 Whitelist (specific accounts)</option>
          <option value="AccountPattern">🔍 Account Pattern (regex)</option>
          <option value="NearBalance">💰 NEAR Balance Check</option>
          <option value="FtBalance">🪙 FT Balance Check</option>
          <option value="NftOwned">🖼️ NFT Ownership Check</option>
        </select>
      </div>

      {/* Condition-specific fields */}
      {conditionType === 'Whitelist' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Allowed Accounts (comma-separated)
          </label>
          <input
            type="text"
            value={whitelistAccounts}
            onChange={(e) => { setWhitelistAccounts(e.target.value); handleChange(); }}
            onBlur={handleChange}
            placeholder="alice.near, bob.near"
            className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
          />
        </div>
      )}

      {conditionType === 'AccountPattern' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Regex Pattern
          </label>
          <input
            type="text"
            value={accountPattern}
            onChange={(e) => { setAccountPattern(e.target.value); handleChange(); }}
            onBlur={handleChange}
            placeholder=".*\.gov\.near"
            className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
          />
          <p className="mt-2 text-xs text-gray-500">
            Example: <code className="bg-white px-1 py-0.5 rounded">.*\.gov\.near</code> matches all .gov.near accounts
          </p>
        </div>
      )}

      {conditionType === 'NearBalance' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md space-y-2">
          <label className="block text-sm font-medium text-gray-700">
            NEAR Balance Requirement
          </label>
          <div className="flex space-x-2">
            <select
              value={nearBalanceOperator}
              onChange={(e) => { setNearBalanceOperator(e.target.value as ComparisonOperator); handleChange(); }}
              className="block w-32 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
            >
              <option value="Gte">&gt;= (at least)</option>
              <option value="Lte">&lt;= (at most)</option>
              <option value="Gt">&gt; (more than)</option>
              <option value="Lt">&lt; (less than)</option>
              <option value="Eq">== (exactly)</option>
              <option value="Ne">!= (not equal)</option>
            </select>
            <input
              type="number"
              step="0.01"
              value={nearBalanceValue}
              onChange={(e) => { setNearBalanceValue(e.target.value); handleChange(); }}
              onBlur={handleChange}
              placeholder="1.0"
              className="block flex-1 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
            />
            <span className="inline-flex items-center px-3 text-sm text-gray-700 font-medium">
              NEAR
            </span>
          </div>
        </div>
      )}

      {conditionType === 'FtBalance' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md space-y-3">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              FT Contract
            </label>
            <input
              type="text"
              value={ftContract}
              onChange={(e) => { setFtContract(e.target.value); handleChange(); }}
              onBlur={handleChange}
              placeholder="usdt.near"
              className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Balance Requirement
            </label>
            <div className="flex space-x-2">
              <select
                value={ftBalanceOperator}
                onChange={(e) => { setFtBalanceOperator(e.target.value as ComparisonOperator); handleChange(); }}
                className="block w-32 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
              >
                <option value="Gte">&gt;=</option>
                <option value="Lte">&lt;=</option>
                <option value="Gt">&gt;</option>
                <option value="Lt">&lt;</option>
                <option value="Eq">==</option>
                <option value="Ne">!=</option>
              </select>
              <input
                type="text"
                value={ftBalanceValue}
                onChange={(e) => { setFtBalanceValue(e.target.value); handleChange(); }}
                onBlur={handleChange}
                placeholder="1000000"
                className="block flex-1 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
              />
            </div>
            <p className="mt-1 text-xs text-gray-500">
              Raw balance (considering token decimals). Example: 1000000 for 1 USDT (6 decimals)
            </p>
          </div>
        </div>
      )}

      {conditionType === 'NftOwned' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            NFT Contract
          </label>
          <input
            type="text"
            value={nftContract}
            onChange={(e) => { setNftContract(e.target.value); handleChange(); }}
            onBlur={handleChange}
            placeholder="paras-token.near"
            className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
          />
          <p className="mt-2 text-xs text-gray-500">
            Requester must own at least one NFT from this collection
          </p>
        </div>
      )}
    </div>
  );
}
