/**
 * Core Library for {{PROJECT_NAME}}
 *
 * This package provides shared types, utilities, and domain models
 * used across the entire workspace.
 */

// Shared Types
export interface DomainEvent {
  id: string;
  timestamp: Date;
  type: string;
}

export interface Result<T, E = Error> {
  ok: boolean;
  value?: T;
  error?: E;
}

// Utilities
export function success<T>(value: T): Result<T> {
  return { ok: true, value };
}

export function failure<E>(error: E): Result<never, E> {
  return { ok: false, error };
}
