export default [
  {
    files: ["**/*.js","**/*.mjs","**/*.ts","**/*.tsx"],
    ignores: ["research/**","node_modules/**","dist/**","build/**"],
    languageOptions: { ecmaVersion: "latest", sourceType: "module" },
    rules: {
      "no-eval": "error",
      "no-implied-eval": "error",
      "no-new-func": "error",
      "no-restricted-globals": ["error", { name: "Function", message: "Use pure functions; no dynamic code" }],
      "no-console": ["error", { allow: [], }],
    },
    settings: {},
  },
  {
    files: ["scripts/**","**/*.test.*","**/__tests__/**"],
    rules: { "no-console": "off" }
  }
];
