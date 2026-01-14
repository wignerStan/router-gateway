module.exports = {
  env: {
    browser: true,
    es2021: true,
    node: true,
  },
  extends: [
    "eslint:recommended",
    "plugin:@typescript-eslint/recommended",
    "plugin:@typescript-eslint/recommended-requiring-type-checking",
    "plugin:import/recommended",
    "plugin:import/typescript",
    "plugin:eslint-comments/recommended",
    "plugin:security/recommended",
    "plugin:sonarjs/recommended",
    "prettier",
  ],
  overrides: [
    {
      env: { node: true },
      files: [".eslintrc.{js,cjs}", "justfile"],
      parserOptions: { sourceType: "script" },
    },
    {
      files: ["*.test.ts", "*.spec.ts"],
      env: { jest: true },
      rules: {
        "@typescript-eslint/no-explicit-any": "off",
        "@typescript-eslint/no-unused-vars": "off",
      },
    },
  ],
  parser: "@typescript-eslint/parser",
  parserOptions: {
    ecmaVersion: 2021,
    sourceType: "module",
    project: "./tsconfig.json",
    tsconfigRootDir: __dirname,
  },
  plugins: [
    "@typescript-eslint",
    "security",
    "sonarjs",
  ],
  rules: {
    // TypeScript specific rules
    "@typescript-eslint/no-explicit-any": "warn",
    "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
    "@typescript-eslint/explicit-function-return-type": "off",
    "@typescript-eslint/explicit-module-boundary-types": "off",
    "@typescript-eslint/no-non-null-assertion": "warn",
    "@typescript-eslint/strict-boolean-expressions": "error",
    "@typescript-eslint/no-floating-promises": "error",
    "@typescript-eslint/no-misused-promises": "error",
    "@typescript-eslint/await-thenable": "error",
    "@typescript-eslint/no-unnecessary-type-assertion": "error",
    "@typescript-eslint/prefer-nullish-coalescing": "error",
    "@typescript-eslint/prefer-optional-chain": "error",

    // General code quality
    "no-console": ["warn", { allow: ["warn", "error"] }],
    "no-debugger": "error",
    "no-alert": "error",
    "no-var": "error",
    "prefer-const": "error",
    "prefer-arrow-callback": "error",
    "prefer-template": "error",
    "object-shorthand": "error",
    "no-duplicate-imports": "error",
    "no-useless-return": "error",
    "no-else-return": "error",
    "no-lonely-if": "error",

    // Import rules
    "import/order": [
      "error",
      {
        "newlines-between": "always",
        alphabetize: { order: "asc", caseInsensitive: true },
        groups: [
          "builtin",
          "external",
          "internal",
          "parent",
          "sibling",
          "index",
        ],
        pathGroups: [
          { pattern: "react", group: "external", position: "before" },
          { pattern: "@/**", group: "internal" },
        ],
        pathGroupsExcludedImportTypes: ["react"],
      },
    ],
    "import/no-unresolved": "off",
    "import/extensions": "off",
    "import/prefer-default-export": "off",

    // Security rules
    "security/detect-object-injection": "warn",
    "security/detect-non-literal-regexp": "warn",
    "security/detect-unsafe-regex": "warn",
    "security/detect-buffer-noassert": "warn",
    "security/detect-child-process": "warn",
    "security/detect-disable-mustache-escape": "warn",
    "security/detect-eval-with-expression": "warn",
    "security/detect-no-csrf-before-method-override": "warn",
    "security/detect-non-literal-fs-filename": "warn",
    "security/detect-non-literal-require": "warn",
    "security/detect-pseudoRandomBytes": "warn",

    // SonarJS rules
    "sonarjs/no-duplicate-string": "warn",
    "sonarjs/no-identical-functions": "warn",
    "sonarjs/no-one-iteration-loop": "warn",
    "sonarjs/no-unused-collection": "warn",
    "sonarjs/prefer-immediate-return": "warn",

    // Style rules
    quotes: ["error", "double", { avoidEscape: true }],
    semi: ["error", "always"],
    "comma-dangle": ["error", "always-multiline"],
    "arrow-parens": ["error", "always"],
    "arrow-body-style": ["error", "as-needed"],
    "no-trailing-spaces": "error",
    "eol-last": "error",
  },
  settings: {
    "import/resolver": {
      typescript: {
        alwaysTryTypes: true,
        project: "./tsconfig.json",
      },
    },
  },
};
