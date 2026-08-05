import eslint from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: ["dist/**", "node_modules/**", "vendor/**"],
  },
  {
    files: ["scripts/**/*.mjs", "test/**/*.mjs"],
    languageOptions: {
      globals: globals.node,
    },
    rules: eslint.configs.recommended.rules,
  },
  ...tseslint.configs.recommended.map((config) => ({
    ...config,
    files: ["src/**/*.ts"],
  })),
);
