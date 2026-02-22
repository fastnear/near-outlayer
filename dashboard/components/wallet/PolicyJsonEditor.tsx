interface PolicyJsonEditorProps {
  policyJsonText: string;
  onChangeText: (text: string) => void;
  jsonEdited: boolean;
  onReset: () => void;
}

export function PolicyJsonEditor({ policyJsonText, onChangeText, jsonEdited, onReset }: PolicyJsonEditorProps) {
  return (
    <div className="mt-4 pt-4 border-t">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold text-gray-800">Policy JSON</h3>
        {jsonEdited && (
          <button onClick={onReset} className="text-xs text-[#cc6600] hover:underline">
            Reset from form
          </button>
        )}
      </div>
      <textarea
        value={policyJsonText}
        onChange={(e) => onChangeText(e.target.value)}
        rows={Math.min(20, Math.max(6, policyJsonText.split('\n').length + 1))}
        className="w-full border border-gray-300 rounded px-3 py-2 text-xs font-mono bg-gray-50 focus:bg-white"
        spellCheck={false}
      />
      <p className="text-xs text-gray-400 mt-1">
        You can edit JSON directly &mdash; this is what will be submitted.
      </p>
    </div>
  );
}
