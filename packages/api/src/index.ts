/**
 * API Layer for {{PROJECT_NAME}}
 *
 * This package provides HTTP handlers, API types, and request/response schemas.
 * Depends on @{{PROJECT_SCOPE}}/{{PROJECT_NAME}}-core.
 */

import type { Result, DomainEvent } from "@{{PROJECT_SCOPE}}/{{PROJECT_NAME}}-core";

// API Types
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export interface RequestConfig {
  timeout?: number;
  headers?: Record<string, string>;
}

// API Handler
export async function handleRequest<T>(
  handler: () => Promise<Result<T>>,
  config?: RequestConfig,
): Promise<ApiResponse<T>> {
  try {
    const result = await handler();

    if (result.ok) {
      return { success: true, data: result.value };
    }

    return {
      success: false,
      error: result.error?.message || "Unknown error",
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : "Unknown error",
    };
  }
}
