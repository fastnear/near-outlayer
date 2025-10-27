'use client';

import { useState, useEffect } from 'react';
import { AccessCondition, ComparisonOperator, LogicOperator } from './types';

interface AccessConditionBuilderProps {
  condition?: AccessCondition;
  onChange: (condition: AccessCondition) => void;
}

export function AccessConditionBuilder({ condition, onChange }: AccessConditionBuilderProps) {
  const [currentCondition, setCurrentCondition] = useState<AccessCondition>(
    condition || { type: 'AllowAll' }
  );

  useEffect(() => {
    if (condition) {
      setCurrentCondition(condition);
    }
  }, [condition]);

  const ruleTypes = [
    { value: 'AllowAll', label: '🌍 Everyone can access', description: 'No restrictions - open for all users' },
    { value: 'Logic', label: '🔗 Multiple rules (AND/OR)', description: 'Combine several rules together' },
    { value: 'Not', label: '🚫 Opposite rule (NOT)', description: 'Flip a rule to mean the opposite' },
    { value: 'NearBalance', label: '💰 NEAR Balance Check', description: 'Require minimum NEAR balance' },
    { value: 'FtBalance', label: '🪙 FT Balance Check', description: 'Require specific token balance' },
    { value: 'NftOwned', label: '🖼️ NFT Ownership Check', description: 'Require NFT ownership' },
    { value: 'Whitelist', label: '👥 Whitelist', description: 'Only specific accounts allowed' },
    { value: 'AccountPattern', label: '🔍 Account Pattern', description: 'Match account name with regex' },
  ];

  const operators = [
    { value: 'Gte', label: 'At least (≥)', description: 'Must have this amount or more' },
    { value: 'Lte', label: 'At most (≤)', description: 'Must have this amount or less' },
    { value: 'Gt', label: 'More than (>)', description: 'Must have more than this amount' },
    { value: 'Lt', label: 'Less than (<)', description: 'Must have less than this amount' },
    { value: 'Eq', label: 'Exactly (=)', description: 'Must have exactly this amount' },
    { value: 'Ne', label: 'Not exactly (≠)', description: 'Must not have exactly this amount' },
  ];

  const updateCondition = (updates: Partial<AccessCondition>) => {
    const newCondition = { ...currentCondition, ...updates } as AccessCondition;
    setCurrentCondition(newCondition);
    onChange(newCondition);
  };

  const handleTypeChange = (newType: string) => {
    let newCondition: AccessCondition;

    switch (newType) {
      case 'AllowAll':
        newCondition = { type: 'AllowAll' };
        break;
      case 'Logic':
        newCondition = { type: 'Logic', operator: 'And', conditions: [{ type: 'AllowAll' }] };
        break;
      case 'Not':
        newCondition = { type: 'Not', condition: { type: 'AllowAll' } };
        break;
      case 'NearBalance':
        newCondition = { type: 'NearBalance', operator: 'Gte', value: '1000000000000000000000000' };
        break;
      case 'FtBalance':
        newCondition = { type: 'FtBalance', contract: '', operator: 'Gte', value: '' };
        break;
      case 'NftOwned':
        newCondition = { type: 'NftOwned', contract: '', token_id: null };
        break;
      case 'Whitelist':
        newCondition = { type: 'Whitelist', accounts: [] };
        break;
      case 'AccountPattern':
        newCondition = { type: 'AccountPattern', pattern: '' };
        break;
      default:
        newCondition = { type: 'AllowAll' };
    }

    setCurrentCondition(newCondition);
    onChange(newCondition);
  };

  const addConditionToLogic = () => {
    if (currentCondition.type === 'Logic') {
      const newConditions = [...currentCondition.conditions, { type: 'AllowAll' as const }];
      updateCondition({ conditions: newConditions });
    }
  };

  const updateLogicCondition = (index: number, newCond: AccessCondition) => {
    if (currentCondition.type === 'Logic') {
      const newConditions = [...currentCondition.conditions];
      newConditions[index] = newCond;
      updateCondition({ conditions: newConditions });
    }
  };

  const removeLogicCondition = (index: number) => {
    if (currentCondition.type === 'Logic') {
      const newConditions = currentCondition.conditions.filter((_, i) => i !== index);
      if (newConditions.length === 0) {
        newConditions.push({ type: 'AllowAll' });
      }
      updateCondition({ conditions: newConditions });
    }
  };

  const convertToYoctoNEAR = (nearAmount: string): string => {
    if (!nearAmount || nearAmount === '') return '0';
    const nearAmountParts = nearAmount.split('.');
    const wholePart = nearAmountParts[0] || '0';
    const fractionalPart = (nearAmountParts[1] || '').padEnd(24, '0').substring(0, 24);
    return wholePart + fractionalPart;
  };

  const convertFromYoctoNEAR = (yoctoAmount: string): string => {
    if (!yoctoAmount || yoctoAmount === '0') return '0';
    try {
      const yoctoStr = yoctoAmount.toString();
      if (yoctoStr.length <= 24) {
        const paddedYocto = yoctoStr.padStart(24, '0');
        const near = parseFloat(`0.${paddedYocto}`);
        return near.toFixed(4);
      } else {
        const wholePart = yoctoStr.substring(0, yoctoStr.length - 24);
        const fractionalPart = yoctoStr.substring(yoctoStr.length - 24);
        const near = parseFloat(`${wholePart}.${fractionalPart}`);
        return near.toFixed(4);
      }
    } catch {
      return '0';
    }
  };

  return (
    <div className="access-rules-builder">
      {/* Rule Type Selector */}
      <div className="mb-4">
        <label className="block text-sm font-medium text-gray-700 mb-2">
          Rule Type
        </label>
        <select
          value={currentCondition.type}
          onChange={(e) => handleTypeChange(e.target.value)}
          className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
        >
          {ruleTypes.map(rule => (
            <option key={rule.value} value={rule.value} title={rule.description}>
              {rule.label}
            </option>
          ))}
        </select>
        {ruleTypes.find(r => r.value === currentCondition.type)?.description && (
          <p className="mt-1 text-xs text-gray-500">
            💡 {ruleTypes.find(r => r.value === currentCondition.type)!.description}
          </p>
        )}
      </div>

      {/* Logic (AND/OR) - Recursive */}
      {currentCondition.type === 'Logic' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md border border-gray-300">
          <div className="mb-3">
            <label className="block text-sm font-medium text-gray-700 mb-2">
              How should these rules work together?
            </label>
            <select
              value={currentCondition.operator}
              onChange={(e) => updateCondition({ operator: e.target.value as LogicOperator })}
              className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
            >
              <option value="And">🔒 ALL rules must pass (AND - stricter)</option>
              <option value="Or">🚪 ANY rule can pass (OR - easier)</option>
            </select>
            <p className="mt-1 text-xs text-gray-500">
              💡 {currentCondition.operator === 'And'
                ? 'User must meet ALL conditions below'
                : 'User only needs to meet ONE condition below'}
            </p>
          </div>

          <div className="space-y-3">
            <label className="block text-sm font-medium text-gray-700">
              Conditions
            </label>
            {currentCondition.conditions.map((cond, index) => (
              <div key={index} className="pl-4 border-l-2 border-blue-400 bg-white p-3 rounded">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-xs font-semibold text-blue-700">Condition #{index + 1}</span>
                  <button
                    type="button"
                    onClick={() => removeLogicCondition(index)}
                    className="text-xs text-red-600 hover:text-red-800 font-medium"
                  >
                    Remove
                  </button>
                </div>
                <AccessConditionBuilder
                  condition={cond}
                  onChange={(newCond) => updateLogicCondition(index, newCond)}
                />
              </div>
            ))}
            <button
              type="button"
              onClick={addConditionToLogic}
              className="mt-2 px-3 py-1 text-sm bg-blue-50 text-blue-700 border border-blue-300 rounded hover:bg-blue-100"
            >
              + Add Condition
            </button>
          </div>
        </div>
      )}

      {/* Not - Recursive */}
      {currentCondition.type === 'Not' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md border border-gray-300">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Which rule should be flipped?
          </label>
          <p className="text-xs text-gray-500 mb-3">
            💡 This will do the OPPOSITE of whatever rule you set below
          </p>
          <div className="pl-4 border-l-2 border-red-400 bg-white p-3 rounded">
            <AccessConditionBuilder
              condition={currentCondition.condition}
              onChange={(newCond) => updateCondition({ condition: newCond })}
            />
          </div>
        </div>
      )}

      {/* Whitelist */}
      {currentCondition.type === 'Whitelist' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Allowed Accounts (comma-separated)
          </label>
          <input
            type="text"
            value={currentCondition.accounts.join(', ')}
            onChange={(e) => {
              const accounts = e.target.value.split(',').map(a => a.trim()).filter(a => a.length > 0);
              updateCondition({ accounts });
            }}
            placeholder="alice.near, bob.near"
            className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
          />
        </div>
      )}

      {/* Account Pattern */}
      {currentCondition.type === 'AccountPattern' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Regex Pattern
          </label>
          <input
            type="text"
            value={currentCondition.pattern}
            onChange={(e) => updateCondition({ pattern: e.target.value })}
            placeholder=".*\.gov\.near"
            className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
          />
          <p className="mt-2 text-xs text-gray-500">
            Example: <code className="bg-white px-1 py-0.5 rounded">.*\.gov\.near</code> matches all .gov.near accounts
          </p>
        </div>
      )}

      {/* NEAR Balance */}
      {currentCondition.type === 'NearBalance' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md space-y-2">
          <label className="block text-sm font-medium text-gray-700">
            NEAR Balance Requirement
          </label>
          <div className="flex space-x-2">
            <select
              value={currentCondition.operator}
              onChange={(e) => updateCondition({ operator: e.target.value as ComparisonOperator })}
              className="block w-32 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
            >
              {operators.map(op => (
                <option key={op.value} value={op.value} title={op.description}>
                  {op.label}
                </option>
              ))}
            </select>
            <input
              type="number"
              step="0.01"
              value={convertFromYoctoNEAR(currentCondition.value)}
              onChange={(e) => {
                const yoctoValue = convertToYoctoNEAR(e.target.value);
                updateCondition({ value: yoctoValue });
              }}
              placeholder="1.0"
              className="block flex-1 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
            />
            <span className="inline-flex items-center px-3 text-sm text-gray-700 font-medium">
              NEAR
            </span>
          </div>
        </div>
      )}

      {/* FT Balance */}
      {currentCondition.type === 'FtBalance' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md space-y-3">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              FT Contract
            </label>
            <input
              type="text"
              value={currentCondition.contract}
              onChange={(e) => updateCondition({ contract: e.target.value })}
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
                value={currentCondition.operator}
                onChange={(e) => updateCondition({ operator: e.target.value as ComparisonOperator })}
                className="block w-32 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
              >
                {operators.map(op => (
                  <option key={op.value} value={op.value}>
                    {op.label}
                  </option>
                ))}
              </select>
              <input
                type="text"
                value={currentCondition.value}
                onChange={(e) => updateCondition({ value: e.target.value })}
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

      {/* NFT Owned */}
      {currentCondition.type === 'NftOwned' && (
        <div className="mb-4 p-4 bg-gray-50 rounded-md space-y-3">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              NFT Contract
            </label>
            <input
              type="text"
              value={currentCondition.contract}
              onChange={(e) => updateCondition({ contract: e.target.value })}
              placeholder="paras-token.near"
              className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Specific Token ID (optional)
            </label>
            <input
              type="text"
              value={currentCondition.token_id || ''}
              onChange={(e) => updateCondition({ token_id: e.target.value || null })}
              placeholder="Leave empty for any NFT from collection"
              className="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm"
            />
            <p className="mt-2 text-xs text-gray-500">
              {currentCondition.token_id
                ? '🎯 Requester must own this specific NFT'
                : '🖼️ Requester must own at least one NFT from this collection'}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
