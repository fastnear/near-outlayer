'use client';

import { useState } from 'react';
import { AddVersionFormData } from './types';
import { calculateWasmHashFromUrl } from '@/lib/wasm-hash';

interface AddVersionFormProps {
  projectName: string;
  onSubmit: (data: AddVersionFormData) => Promise<void>;
  onCancel: () => void;
  isSubmitting: boolean;
}

export function AddVersionForm({ projectName, onSubmit, onCancel, isSubmitting }: AddVersionFormProps) {
  const [formData, setFormData] = useState<AddVersionFormData>({
    projectName,
    sourceType: 'github',
    repo: '',
    commit: 'main',
    buildTarget: 'wasm32-wasip2',
    wasmUrl: '',
    wasmHash: '',
    setActive: true,
  });

  const [hashLoading, setHashLoading] = useState(false);
  const [hashError, setHashError] = useState<string | null>(null);

  const handleCalculateHash = async () => {
    if (!formData.wasmUrl) {
      setHashError('Please enter a WASM URL first');
      return;
    }

    setHashLoading(true);
    setHashError(null);

    try {
      const hash = await calculateWasmHashFromUrl(formData.wasmUrl);
      setFormData({ ...formData, wasmHash: hash });
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to calculate hash';
      setHashError(message);
    } finally {
      setHashLoading(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    await onSubmit(formData);
  };

  return (
    <div className="bg-white shadow rounded-lg p-6">
      <h3 className="text-lg font-medium text-gray-900 mb-4">
        Add Version to <span className="text-[#cc6600]">{projectName}</span>
      </h3>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* Source Type Toggle */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Code Source
          </label>
          <div className="flex space-x-4">
            <label className="inline-flex items-center">
              <input
                type="radio"
                name="sourceType"
                value="github"
                checked={formData.sourceType === 'github'}
                onChange={() => setFormData({ ...formData, sourceType: 'github' })}
                className="form-radio text-[#cc6600] focus:ring-[#cc6600]"
              />
              <span className="ml-2 text-sm text-gray-700">GitHub Repository</span>
            </label>
            <label className="inline-flex items-center">
              <input
                type="radio"
                name="sourceType"
                value="wasm_url"
                checked={formData.sourceType === 'wasm_url'}
                onChange={() => setFormData({ ...formData, sourceType: 'wasm_url' })}
                className="form-radio text-[#cc6600] focus:ring-[#cc6600]"
              />
              <span className="ml-2 text-sm text-gray-700">WASM URL</span>
            </label>
          </div>
        </div>

        {/* GitHub Source Fields */}
        {formData.sourceType === 'github' && (
          <>
            <div>
              <label htmlFor="repo" className="block text-sm font-medium text-gray-700">
                Repository <span className="text-red-500">*</span>
              </label>
              <input
                type="text"
                id="repo"
                value={formData.repo}
                onChange={(e) => setFormData({ ...formData, repo: e.target.value })}
                placeholder="https://github.com/owner/repo"
                className="mt-1 block w-full border-gray-300 rounded-md shadow-sm focus:ring-[#cc6600] focus:border-[#cc6600] sm:text-sm"
                required={formData.sourceType === 'github'}
              />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label htmlFor="commit" className="block text-sm font-medium text-gray-700">
                  Commit/Branch <span className="text-red-500">*</span>
                </label>
                <input
                  type="text"
                  id="commit"
                  value={formData.commit}
                  onChange={(e) => setFormData({ ...formData, commit: e.target.value })}
                  placeholder="main"
                  className="mt-1 block w-full border-gray-300 rounded-md shadow-sm focus:ring-[#cc6600] focus:border-[#cc6600] sm:text-sm"
                  required={formData.sourceType === 'github'}
                />
              </div>
              <div>
                <label htmlFor="buildTarget" className="block text-sm font-medium text-gray-700">
                  Build Target
                </label>
                <select
                  id="buildTarget"
                  value={formData.buildTarget}
                  onChange={(e) => setFormData({ ...formData, buildTarget: e.target.value })}
                  className="mt-1 block w-full border-gray-300 rounded-md shadow-sm focus:ring-[#cc6600] focus:border-[#cc6600] sm:text-sm"
                >
                  <option value="wasm32-wasip2">wasm32-wasip2</option>
                  {/* <option value="wasm32-wasip1">wasm32-wasip1 (not supported for projects)</option> */}
                </select>
              </div>
            </div>
          </>
        )}

        {/* WASM URL Source Fields */}
        {formData.sourceType === 'wasm_url' && (
          <>
            <div>
              <label htmlFor="wasmUrl" className="block text-sm font-medium text-gray-700">
                WASM URL <span className="text-red-500">*</span>
              </label>
              <input
                type="text"
                id="wasmUrl"
                value={formData.wasmUrl}
                onChange={(e) => setFormData({ ...formData, wasmUrl: e.target.value })}
                placeholder="https://example.com/my-app.wasm"
                className="mt-1 block w-full border-gray-300 rounded-md shadow-sm focus:ring-[#cc6600] focus:border-[#cc6600] sm:text-sm"
                required={formData.sourceType === 'wasm_url'}
              />
            </div>
            <div>
              <label htmlFor="wasmHash" className="block text-sm font-medium text-gray-700">
                SHA256 Hash <span className="text-red-500">*</span>
              </label>
              <div className="mt-1 flex gap-2">
                <input
                  type="text"
                  id="wasmHash"
                  value={formData.wasmHash}
                  onChange={(e) => setFormData({ ...formData, wasmHash: e.target.value })}
                  placeholder="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                  className="block w-full font-mono text-xs border-gray-300 rounded-md shadow-sm focus:ring-[#cc6600] focus:border-[#cc6600]"
                  required={formData.sourceType === 'wasm_url'}
                />
                <button
                  type="button"
                  onClick={handleCalculateHash}
                  disabled={hashLoading || !formData.wasmUrl}
                  className="inline-flex items-center px-3 py-2 border border-gray-300 shadow-sm text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-[#cc6600] disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap"
                >
                  {hashLoading ? (
                    <>
                      <svg className="animate-spin -ml-1 mr-2 h-4 w-4 text-gray-500" fill="none" viewBox="0 0 24 24">
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                        <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                      </svg>
                      Calculating...
                    </>
                  ) : (
                    'Calculate'
                  )}
                </button>
              </div>
              {hashError && (
                <p className="mt-1 text-xs text-red-600">{hashError}</p>
              )}
              <p className="mt-1 text-xs text-gray-500">
                Click &quot;Calculate&quot; to auto-fill from URL
              </p>
            </div>
            <div>
              <label htmlFor="wasmBuildTarget" className="block text-sm font-medium text-gray-700">
                Build Target
              </label>
              <select
                id="wasmBuildTarget"
                value={formData.buildTarget}
                onChange={(e) => setFormData({ ...formData, buildTarget: e.target.value })}
                className="mt-1 block w-full border-gray-300 rounded-md shadow-sm focus:ring-[#cc6600] focus:border-[#cc6600] sm:text-sm"
              >
                <option value="wasm32-wasip2">wasm32-wasip2</option>
                {/* <option value="wasm32-wasip1">wasm32-wasip1 (not supported for projects)</option> */}
              </select>
            </div>
          </>
        )}

        {/* Set Active Checkbox */}
        <div className="flex items-center">
          <input
            type="checkbox"
            id="setActive"
            checked={formData.setActive}
            onChange={(e) => setFormData({ ...formData, setActive: e.target.checked })}
            className="h-4 w-4 text-[#cc6600] focus:ring-[#cc6600] border-gray-300 rounded"
          />
          <label htmlFor="setActive" className="ml-2 block text-sm text-gray-700">
            Set as active version after adding
          </label>
        </div>

        {/* Buttons */}
        <div className="flex justify-end space-x-3 pt-4">
          <button
            type="button"
            onClick={onCancel}
            className="px-4 py-2 border border-gray-300 rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50"
            disabled={isSubmitting}
          >
            Cancel
          </button>
          <button
            type="submit"
            className="px-4 py-2 border border-transparent rounded-md text-sm font-medium text-white bg-[#cc6600] hover:bg-[#b35900] disabled:opacity-50"
            disabled={isSubmitting}
          >
            {isSubmitting ? 'Adding...' : 'Add Version'}
          </button>
        </div>
      </form>
    </div>
  );
}
