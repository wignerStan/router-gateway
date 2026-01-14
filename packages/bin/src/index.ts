#!/usr/bin/env bun
/**
 * Main Entry Point for {{PROJECT_NAME}}
 *
 * CLI entry point that orchestrates the application.
 * Depends on @{{PROJECT_SCOPE}}/{{PROJECT_NAME}}-core and @{{PROJECT_SCOPE}}/{{PROJECT_NAME}}-api.
 */

import { success, failure, type Result } from "@{{PROJECT_SCOPE}}/{{PROJECT_NAME}}-core";
import { handleRequest } from "@{{PROJECT_SCOPE}}/{{PROJECT_NAME}}-api";

interface Config {
  name: string;
  env: "development" | "production";
}

function loadConfig(): Result<Config> {
  const name = process.env.PROJECT_NAME || "{{PROJECT_NAME}}";
  const env = (process.env.NODE_ENV as Config["env"]) || "development";

  return success({ name, env });
}

async function main(): Promise<void> {
  console.log(`Starting {{PROJECT_NAME}}...`);

  const configResult = loadConfig();

  if (!configResult.ok) {
    console.error("Failed to load configuration");
    process.exit(1);
  }

  const config = configResult.value!;

  console.log(`Configuration loaded: ${config.name} (${config.env})`);

  const response = await handleRequest(async () => {
    return success({ status: "running", timestamp: new Date() });
  });

  if (response.success) {
    console.log(`Application running: ${JSON.stringify(response.data)}`);
  } else {
    console.error(`Error: ${response.error}`);
    process.exit(1);
  }
}

// Run main function
main().catch((error) => {
  console.error("Unhandled error:", error);
  process.exit(1);
});
