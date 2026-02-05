'use client';

import { ProjectView, VersionView, formatTimestamp, formatVersionKey } from './types';

interface ProjectCardProps {
  project: ProjectView;
  versions: VersionView[];
  versionCount: number;
  projectStorage?: { total_bytes: number; key_count: number };
  loadingVersions: boolean;
  expanded: boolean;
  onToggleExpand: () => void;
  onAddVersion: () => void;
  onSetActiveVersion: (wasmHash: string) => void;
  onRemoveVersion: (wasmHash: string) => void;
  onDeleteProject: () => void;
}

// Sort versions: active first, then by added_at desc
function sortVersions(versions: VersionView[]): VersionView[] {
  return [...versions].sort((a, b) => {
    if (a.is_active && !b.is_active) return -1;
    if (!a.is_active && b.is_active) return 1;
    return b.added_at - a.added_at; // newest first
  });
}

// Format bytes to human readable
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

export function ProjectCard({
  project,
  versions,
  versionCount,
  projectStorage,
  loadingVersions,
  expanded,
  onToggleExpand,
  onAddVersion,
  onSetActiveVersion,
  onRemoveVersion,
  onDeleteProject,
}: ProjectCardProps) {
  return (
    <div className="bg-white shadow rounded-lg overflow-hidden">
      {/* Project Header */}
      <div
        className="px-4 py-4 sm:px-6 cursor-pointer hover:bg-gray-50 transition-colors"
        onClick={onToggleExpand}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <div className="flex-shrink-0">
              <svg className="h-8 w-8 text-[#cc6600]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
              </svg>
            </div>
            <div>
              <h3 className="text-lg font-medium text-gray-900">{project.name}</h3>
              <p className="text-sm text-gray-500">{project.project_id}</p>
              <p className="text-xs text-gray-400 font-mono">{project.uuid}</p>
            </div>
          </div>
          <div className="flex items-center space-x-4">
            <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
              {versionCount} version{versionCount !== 1 ? 's' : ''}
            </span>
            <svg
              className={`h-5 w-5 text-gray-400 transition-transform ${expanded ? 'transform rotate-180' : ''}`}
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </div>
        </div>

        {/* Project Info Summary */}
        <div className="mt-2 flex flex-wrap gap-4 text-sm text-gray-500">
          <span>Created: {formatTimestamp(project.created_at)}</span>
          {project.active_version && (
            <span>Active: <code className="font-mono text-xs">{formatVersionKey(project.active_version)}</code></span>
          )}
        </div>

        {/* Storage Info */}
        {projectStorage && (
          <div className="mt-2 text-sm text-gray-500 flex items-center gap-1">
            <span>Storage: {formatBytes(projectStorage.total_bytes)} ({projectStorage.key_count} key{projectStorage.key_count !== 1 ? 's' : ''})</span>
            <div className="relative group">
              <svg className="h-4 w-4 text-gray-400 cursor-help" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-2 bg-gray-900 text-white text-xs rounded-lg opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-10">
                Persistent storage used by WASM executions
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Expanded Content */}
      {expanded && (
        <div className="border-t border-gray-200 px-4 py-4 sm:px-6 bg-gray-50">
          {/* Action Buttons */}
          <div className="flex flex-wrap gap-2 mb-4">
            <button
              onClick={(e) => { e.stopPropagation(); onAddVersion(); }}
              className="inline-flex items-center px-3 py-1.5 border border-transparent text-sm font-medium rounded-md text-white bg-[#cc6600] hover:bg-[#b35900] transition-colors"
            >
              <svg className="h-4 w-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
              </svg>
              Add Version
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); onDeleteProject(); }}
              className="inline-flex items-center px-3 py-1.5 border border-red-300 text-sm font-medium rounded-md text-red-700 bg-white hover:bg-red-50 transition-colors"
            >
              <svg className="h-4 w-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
              Delete Project
            </button>
          </div>

          {/* Versions List */}
          <h4 className="text-sm font-medium text-gray-700 mb-2">Versions</h4>
          {loadingVersions ? (
            <div className="flex items-center justify-center py-4">
              <svg className="animate-spin h-5 w-5 text-gray-400" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
              <span className="ml-2 text-sm text-gray-500">Loading versions...</span>
            </div>
          ) : versions.length === 0 ? (
            <p className="text-sm text-gray-500 py-2">No versions yet</p>
          ) : (
            <div className="space-y-2">
              {sortVersions(versions).map((version) => (
                <VersionItem
                  key={version.version_key}
                  version={version}
                  isActive={version.is_active}
                  onSetActive={() => onSetActiveVersion(version.version_key)}
                  onRemove={() => onRemoveVersion(version.version_key)}
                />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

interface VersionItemProps {
  version: VersionView;
  isActive: boolean;
  onSetActive: () => void;
  onRemove: () => void;
}

function VersionItem({ version, isActive, onSetActive, onRemove }: VersionItemProps) {
  // For WasmUrl, extract download URL
  const downloadUrl = version.source.WasmUrl?.url || '';
  const buildTarget = version.source.GitHub?.build_target || version.source.WasmUrl?.build_target;

  return (
    <div className={`px-3 py-2 rounded-md ${isActive ? 'bg-green-50 border border-green-200' : 'bg-white border border-gray-200'}`}>
      {/* Single row: version key left, actions right */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <code className="font-mono text-xs text-gray-700 truncate" title={version.version_key}>
            {formatVersionKey(version.version_key)}
          </code>
          {buildTarget && (
            <span className="text-xs text-gray-400 flex-shrink-0">({buildTarget})</span>
          )}
        </div>
        <div className="flex items-center gap-0.5 flex-shrink-0">
          {isActive && (
            <span className="inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800 whitespace-nowrap">
              Active
            </span>
          )}
          {downloadUrl && (
            <div className="relative group">
              <a
                href={downloadUrl}
                download={`${version.version_key}.wasm`}
                onClick={(e) => e.stopPropagation()}
                className="p-1 rounded hover:bg-blue-100 text-blue-600 hover:text-blue-800 transition-colors cursor-pointer inline-flex"
              >
                <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                </svg>
              </a>
              <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1 bg-gray-900 text-white text-xs rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-10">
                Download WASM
              </div>
            </div>
          )}
          {!isActive && (
            <>
              <div className="relative group">
                <button
                  onClick={(e) => { e.stopPropagation(); onSetActive(); }}
                  className="p-1 rounded hover:bg-orange-100 text-[#cc6600] hover:text-[#b35900] transition-colors cursor-pointer"
                >
                  <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                </button>
                <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1 bg-gray-900 text-white text-xs rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-10">
                  Set as active
                </div>
              </div>
              <div className="relative group">
                <button
                  onClick={(e) => { e.stopPropagation(); onRemove(); }}
                  className="p-1 rounded hover:bg-red-100 text-red-600 hover:text-red-800 transition-colors cursor-pointer"
                >
                  <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </button>
                <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1 bg-gray-900 text-white text-xs rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-10">
                  Remove
                </div>
              </div>
            </>
          )}
        </div>
      </div>
      <p className="text-xs text-gray-400 mt-1">Added {formatTimestamp(version.added_at)}</p>
    </div>
  );
}
